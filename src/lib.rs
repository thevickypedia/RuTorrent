#![allow(rustdoc::bare_urls)]
#![doc = include_str!("../README.md")]

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use reqwest::Client;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::RwLock,
    time::{sleep, Duration},
};

mod settings;
mod logger;
mod qb;

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
            let snapshot: Vec<(String, String, Option<settings::RsyncTarget>)> = {
                let db = state.read().await;
                db.iter()
                    .map(|(h, v)| (h.clone(), v.name.clone(), v.rsync.clone()))
                    .collect()
            };

            for (hash, name, rsync) in snapshot {
                if rsync.is_none() {
                    continue;
                }

                let url = format!(
                    "{}/api/v2/torrents/properties?hash={}",
                    config.base_url, hash
                );

                if let Some(resp) = qb_get(&client, url).await {
                    let progress = resp["progress"].as_f64().unwrap_or(0.0);

                    if progress >= 1.0 {
                        log::info!("Download complete → starting rsync: {}", name);

                        let state_clone = state.clone();
                        let hash_clone = hash.clone();
                        let name_clone = name.clone();
                        let target = rsync.unwrap();

                        tokio::spawn(async move {
                            run_rsync(state_clone, hash_clone, name_clone, target).await;
                        });
                    }
                }
            }

            sleep(Duration::from_secs(2)).await;
        }
    });
}

async fn run_rsync(
    state: settings::SharedState,
    hash: String,
    name: String,
    target: settings::RsyncTarget,
) {
    log::info!("Starting rsync for {}", name);

    let local_path = format!("/downloads/{}", name);
    let remote = format!("{}@{}:{}", target.username, target.host, target.remote_path);

    let mut child = Command::new("rsync")
        .args(["-avz", "--progress", &local_path, &remote])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("rsync failed");

    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        log::info!("rsync: {}", line);

        if let Some(p) = parse_progress(&line) {
            let mut db = state.write().await;
            if let Some(e) = db.get_mut(&hash) {
                e.status = settings::Status::Copying(p);
            }
        }
    }

    let _ = child.wait().await;

    log::info!("rsync complete: {}", name);

    let mut db = state.write().await;
    if let Some(e) = db.get_mut(&hash) {
        e.status = settings::Status::Completed;
    }
}

fn parse_progress(line: &str) -> Option<f64> {
    if let Some(idx) = line.find('%') {
        let start = line[..idx].rfind(' ')?;
        let pct = line[start..idx].trim();
        return pct.parse::<f64>().ok().map(|p| p / 100.0);
    }
    None
}

async fn get_torrents(config: web::Data<settings::Config>) -> impl Responder {
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
                log::info!("JSON error: {}", e);
                return HttpResponse::InternalServerError().finish();
            }
        },
        Err(e) => {
            log::info!("request error: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut map = HashMap::new();

    if let Some(arr) = resp.as_array() {
        for t in arr {
            let name = t["name"].as_str().unwrap_or("?").to_string();
            let progress = t["progress"].as_f64().unwrap_or(0.0);

            let status = if progress >= 1.0 {
                "Completed"
            } else {
                "Downloading"
            };

            map.insert(name, format!("{}: {:.0}%", status, progress * 100.0));
        }
    }

    log::info!("GET complete");
    HttpResponse::Ok().json(map)
}

async fn put_torrent(
    state: web::Data<settings::SharedState>,
    config: web::Data<settings::Config>,
    req: web::Json<settings::PutRequest>,
) -> impl Responder {
    log::info!("PUT /torrent");

    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let joined = req.urls.join("\n");

    let resp = client
        .post(format!("{}/api/v2/torrents/add", config.base_url))
        .form(&[("urls", joined.as_str())])
        .send()
        .await;

    if let Err(e) = qb::handle_response(resp, "ADD torrent").await {
        return e;
    }

    log::info!("torrent submitted");

    // cache rsync intent ONLY (not truth)
    let mut db = state.write().await;

    for url in &req.urls {
        db.insert(
            url.clone(),
            settings::RsyncTrack {
                name: url.clone(),
                status: settings::Status::Downloading(0.0),
                rsync: req.rsync.clone(),
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
