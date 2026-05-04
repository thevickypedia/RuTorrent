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

/* -----------------------------
   qB GET helper (safe fallback)
------------------------------*/
async fn qb_get(client: &Client, url: String) -> Option<Value> {
    match client.get(&url).send().await {
        Ok(r) => r.json().await.ok(),
        Err(e) => {
            log::warn!("qB GET error: {}", e);
            None
        }
    }
}

/* -----------------------------
   WORKER (single source of truth)
   - polls qBittorrent
   - updates local state
   - triggers rsync ONCE per torrent
------------------------------*/
fn spawn_worker(client: Client, state: settings::SharedState, config: settings::Config) {
    tokio::spawn(async move {
        log::info!("Worker started");
        loop {
            // snapshot hashes only
            let hashes: Vec<String> = {
                let db = state.read().await;
                db.keys().cloned().collect()
            };

            if hashes.is_empty() {
                sleep(Duration::from_secs(2)).await;
                continue;
            }

            let url = format!(
                "{}/api/v2/torrents/info?hashes={}",
                config.base_url,
                hashes.join("|")
            );

            let Some(resp) = qb_get(&client, url).await else {
                sleep(Duration::from_secs(2)).await;
                continue;
            };

            let Some(arr) = resp.as_array() else {
                sleep(Duration::from_secs(2)).await;
                continue;
            };

            let mut db = state.write().await;

            for t in arr {
                let hash = t["hash"].as_str().unwrap_or("").to_string();
                let progress = t["progress"].as_f64().unwrap_or(0.0);
                let content_path = t["content_path"].as_str().unwrap_or("").to_string();

                let Some(entry) = db.get_mut(&hash) else {
                    continue;
                };

                match entry.status {
                    /* -----------------------------
                       Already copying/completed → skip
                    ------------------------------*/
                    settings::Status::Copying(_) | settings::Status::Completed => {
                        continue;
                    }

                    /* -----------------------------
                       Still downloading
                    ------------------------------*/
                    settings::Status::Downloading(_) => {
                        entry.status = settings::Status::Downloading(progress);

                        // only trigger ONCE when crossing threshold
                        if progress >= 1.0 {
                            if let Some(target) = entry.rsync.clone() {
                                log::info!("Download complete → rsync: {}", entry.name);

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

            drop(db);
            sleep(Duration::from_secs(2)).await;
        }
    });
}

/* -----------------------------
   GET /torrent
   merges:
   - qBittorrent state
   - rsync state
------------------------------*/
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

            let status = match db.get(&hash) {
                Some(local) => match local.status {
                    settings::Status::Copying(p) => format!("Copying: {:.0}%", p * 100.0),

                    settings::Status::Completed => "Completed (download + copy)".to_string(),
                    settings::Status::Downloading(_) => {
                        format!("Downloading: {:.0}%", progress * 100.0)
                    }
                },

                None => {
                    if progress >= 1.0 {
                        "Completed".to_string()
                    } else {
                        format!("Downloading: {:.0}%", progress * 100.0)
                    }
                }
            };

            map.insert(name, status);
        }
    }

    HttpResponse::Ok().json(map)
}

/* -----------------------------
   PUT /torrent
   FIXED:
   - correct hash resolution
   - per-item rsync config
------------------------------*/
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

        // 1. add torrent
        let resp = client
            .post(format!("{}/api/v2/torrents/add", config.base_url))
            .form(&[("urls", item.url.as_str())])
            .send()
            .await;

        if let Err(e) = qb::handle_response(resp, "ADD torrent").await {
            return e;
        }

        sleep(Duration::from_secs(2)).await;

        // 2. fetch torrents
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

        // 3. correct hash resolution (FIXED)
        let mut found = None;

        if let Some(arr) = resp.as_array() {
            for t in arr {
                let hash = t["hash"].as_str().unwrap_or("").to_string();
                let name = t["name"].as_str().unwrap_or("").to_string();

                if item.url.contains(&name) || name.contains("magnet") {
                    found = Some((hash, name));
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

        // 4. store state
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

/* -----------------------------
   DELETE /torrent (unchanged but safe)
------------------------------*/
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
        None => return HttpResponse::NotFound().body("Torrent not found"),
    };

    let resp = client
        .post(format!("{}/api/v2/torrents/delete", config.base_url))
        .form(&[("hashes", hash.as_str())])
        .send()
        .await;

    if let Err(e) = qb::handle_response(resp, "DELETE torrent").await {
        return e;
    }

    log::info!("Successfully deleted {}", identifier);
    HttpResponse::Ok().body("Deleted")
}

/* -----------------------------
   START SERVER
------------------------------*/
pub async fn start() -> std::io::Result<()> {
    let config = settings::Config::new();
    logger::init_logger(config.utc_logger);
    let state: settings::SharedState = Arc::new(RwLock::new(HashMap::new()));

    let client = qb::client(&config)
        .await
        .expect("Failed to authenticate qBittorrent");

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
