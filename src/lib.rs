#![allow(rustdoc::bare_urls)]
#![doc = include_str!("../README.md")]

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use reqwest::Client;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    sync::RwLock,
    time::{sleep, Duration},
};

mod logger;
mod qb;
mod rsync;
mod settings;

async fn qb_get(client: &Client, url: String) -> Option<Value> {
    match client.get(&url).send().await {
        Ok(r) => r.json().await.ok(),
        Err(e) => {
            log::info!("qB GET error: {}", e);
            None
        }
    }
}

fn spawn_worker(client: Client, state: settings::SharedState, config: settings::Config) {
    tokio::spawn(async move {
        log::info!("Worker started");
        loop {
            // take snapshot of hashes only (not status)
            let hashes: Vec<String> = {
                let db = state.read().await;
                db.keys().cloned().collect()
            };

            let url = format!(
                "{}/api/v2/torrents/info?hashes={}",
                config.base_url,
                hashes.join("|")
            );

            if let Some(resp) = qb_get(&client, url).await
                && let Some(arr) = resp.as_array()
            {
                let mut db = state.write().await;

                for t in arr {
                    let hash = t["hash"].as_str().unwrap_or("").to_string();
                    let progress = t["progress"].as_f64().unwrap_or(0.0);
                    let content_path = t["content_path"].as_str().unwrap_or("").to_string();

                    if let Some(entry) = db.get_mut(&hash)
                        && let settings::Status::Downloading(_) = entry.status
                    {
                        // Avoid rsync re-trigger when looped again
                        if matches!(entry.status, settings::Status::Downloading(_))
                            && progress >= 1.0
                        {
                            if let Some(target) = entry.rsync.clone() {
                                log::info!("Download complete → starting rsync: {}", entry.name);
                                entry.status = settings::Status::Copying(0.0);

                                let state_clone = state.clone();
                                let hash_clone = hash.clone();
                                let name_clone = entry.name.clone();

                                tokio::spawn(async move {
                                    rsync::run(
                                        state_clone,
                                        hash_clone,
                                        name_clone,
                                        content_path,
                                        target,
                                    )
                                    .await;
                                });
                            } else {
                                entry.status = settings::Status::Completed;
                            }
                        } else {
                            entry.status = settings::Status::Downloading(progress);
                        }
                    }
                }
            }

            sleep(Duration::from_secs(2)).await;
        }
    });
}

async fn get_torrents(
    state: web::Data<settings::SharedState>,
    config: web::Data<settings::Config>,
) -> impl Responder {
    log::info!("GET /torrent");

    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let url = format!("{}/api/v2/torrents/info", config.base_url);

    let resp: Value = match client.get(url).send().await {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                log::error!("JSON error: {}", e);
                return HttpResponse::InternalServerError().finish();
            }
        },
        Err(e) => {
            log::error!("request error: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let db = state.read().await;
    let mut map = HashMap::new();

    if let Some(arr) = resp.as_array() {
        for t in arr {
            let name = t["name"].as_str().unwrap_or("?").to_string();
            let hash = t["hash"].as_str().unwrap_or("").to_string();
            let progress = t["progress"].as_f64().unwrap_or(0.0);

            // check rsync state first
            if let Some(local) = db.get(&hash) {
                let status = match local.status {
                    settings::Status::Copying(p) => {
                        format!("Copying: {:.0}%", p * 100.0)
                    }
                    settings::Status::Completed => "Completed (download + copy)".to_string(),
                    settings::Status::Downloading(_) => {
                        format!("Downloading: {:.0}%", progress * 100.0)
                    }
                };

                map.insert(name, status);
            } else {
                // fallback to pure qBittorrent
                let status = if progress >= 1.0 {
                    "Completed".to_string()
                } else {
                    format!("Downloading: {:.0}%", progress * 100.0)
                };
                map.insert(name, status);
            }
        }
    }

    log::info!("GET complete");
    HttpResponse::Ok().json(map)
}

async fn put_torrent(
    state: web::Data<settings::SharedState>,
    config: web::Data<settings::Config>,
    req: web::Json<Vec<settings::PutItem>>,
) -> impl Responder {
    log::info!("PUT /torrent");

    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    for item in req.iter() {
        log::info!("Adding torrent: {}", item.url);

        // 1️⃣ Add torrent
        let resp = client
            .post(format!("{}/api/v2/torrents/add", config.base_url))
            .form(&[("urls", item.url.as_str())])
            .send()
            .await;

        if let Err(e) = qb::handle_response(resp, "ADD torrent").await {
            return e;
        }

        // 2️⃣ wait for qB to register it
        sleep(Duration::from_secs(2)).await;

        // 3️⃣ fetch all torrents and find this one
        let resp: Value = match client
            .get(format!("{}/api/v2/torrents/info", config.base_url))
            .send()
            .await
        {
            Ok(r) => match r.json().await {
                Ok(j) => j,
                Err(_) => return HttpResponse::InternalServerError().body("Invalid JSON"),
            },
            Err(_) => return HttpResponse::InternalServerError().body("Request failed"),
        };

        let mut found = None;

        if let Some(arr) = resp.as_array() {
            for t in arr {
                let name = t["name"].as_str().unwrap_or("");
                let hash = t["hash"].as_str().unwrap_or("");

                // crude but works: match by name or magnet substring
                if item.url.contains(name) || name.contains("magnet") {
                    found = Some((hash.to_string(), name.to_string()));
                    break;
                }
            }
        }

        let (hash, name) = match found {
            Some(v) => v,
            None => {
                log::error!("Could not resolve hash for {}", item.url);
                continue;
            }
        };

        log::info!("Resolved {} → {}", name, hash);

        // 4️⃣ store rsync intent
        let mut db = state.write().await;

        db.insert(
            hash.clone(),
            settings::RsyncTrack {
                name,
                status: settings::Status::Downloading(0.0),
                rsync: Some(settings::RsyncTarget {
                    host: item.host.clone(),
                    username: item.username.clone(),
                    path: item.path.clone(),
                }),
            },
        );
    }

    HttpResponse::Ok().body("Added")
}

async fn delete_torrent(
    config: web::Data<settings::Config>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    log::info!("DELETE /torrent");

    let identifier = match query.get("name") {
        Some(h) => h,
        None => return HttpResponse::BadRequest().body("Missing name"),
    };

    log::info!("Deleting torrent: {}", identifier);

    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    // fetch all torrents
    let resp: Value = match client
        .get(format!("{}/api/v2/torrents/info", config.base_url))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(_) => return HttpResponse::InternalServerError().body("Invalid JSON"),
        },
        Err(_) => return HttpResponse::InternalServerError().body("Request failed"),
    };

    // find matching torrent
    let mut found_hash = None;

    if let Some(arr) = resp.as_array() {
        for t in arr {
            let name = t["name"].as_str().unwrap_or("");
            let hash = t["hash"].as_str().unwrap_or("");

            if name == identifier {
                found_hash = Some(hash.to_string());
                break;
            }
        }
    }

    let hash = match found_hash {
        Some(h) => h,
        None => {
            log::info!("Torrent not found: {}", identifier);
            return HttpResponse::NotFound().body("Torrent not found");
        }
    };

    log::info!("Resolved {} → {}", identifier, hash);

    let force = query.get("force").map(|v| v == "true").unwrap_or(false);

    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    log::info!("Deleting torrent: {}", hash);

    let resp = client
        .post(format!("{}/api/v2/torrents/delete", config.base_url))
        .form(&[
            ("hashes", hash.as_str()),
            ("deleteFiles", if force { "true" } else { "false" }),
        ])
        .send()
        .await;

    if let Err(e) = qb::handle_response(resp, "DELETE torrent").await {
        return e;
    }

    log::info!("Successfully deleted {}", identifier);
    HttpResponse::Ok().body("Deleted")
}

pub async fn start() -> std::io::Result<()> {
    let config = settings::Config::new();
    logger::init_logger(config.utc_logger);
    let state: settings::SharedState = Arc::new(RwLock::new(HashMap::new()));

    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(_) => {
            panic!("Worker failed to authenticate.");
        }
    };

    spawn_worker(client, state.clone(), config.clone());

    let host = config.host.clone();
    let port = config.port;

    log::info!("Starting server on: http://{}:{}", host, port);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .app_data(web::Data::new(config.clone()))
            .route("/torrent", web::get().to(get_torrents))
            .route("/torrent", web::put().to(put_torrent))
            .route("/torrent", web::delete().to(delete_torrent))
    })
    .bind((host, port))?
    .run()
    .await
}
