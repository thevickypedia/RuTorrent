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
use uuid::Uuid;

mod logger;
mod qb;
mod rsync;
mod settings;

/* -----------------------------
   TEMP pending tracker
------------------------------*/
type PendingMap = Arc<RwLock<HashMap<String, settings::PutItem>>>;

/* -----------------------------
   qB GET helper
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
   Helper: resolve new torrents
------------------------------*/
async fn resolve_new_torrents(
    client: &Client,
    config: &settings::Config,
    pending: &PendingMap,
    state: &settings::SharedState,
) {
    let resp = qb_get(
        client,
        format!("{}/api/v2/torrents/info", config.base_url),
    )
        .await;

    let Some(arr) = resp.and_then(|v| v.as_array().cloned()) else {
        return;
    };

    let mut pending_lock = pending.write().await;
    let mut db = state.write().await;

    // naive but robust: assign first unmatched torrents
    for t in arr {
        let hash = t["hash"].as_str().unwrap_or("").to_string();
        let name = t["name"].as_str().unwrap_or("").to_string();

        if db.contains_key(&hash) {
            continue;
        }

        if let Some((id, item)) = pending_lock.iter().next().map(|(k, v)| (k.clone(), v.clone())) {
            log::info!("Resolved {} → {}", name, hash);

            db.insert(
                hash.clone(),
                settings::RsyncTrack {
                    name,
                    status: settings::Status::Downloading(0.0),
                    rsync: Some(settings::RsyncTarget {
                        host: item.host,
                        username: item.username,
                        path: item.path,
                    }),
                },
            );

            pending_lock.remove(&id);
        }
    }
}

/* -----------------------------
   WORKER
------------------------------*/
fn spawn_worker(
    client: Client,
    state: settings::SharedState,
    pending: PendingMap,
    config: settings::Config,
) {
    tokio::spawn(async move {
        log::info!("Worker started");

        loop {
            /* -----------------------------
               1. Resolve pending torrents
            ------------------------------*/
            resolve_new_torrents(&client, &config, &pending, &state).await;

            /* -----------------------------
               2. Poll tracked torrents
            ------------------------------*/
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
                    settings::Status::Copying(_) | settings::Status::Completed => continue,

                    settings::Status::Downloading(_) => {
                        entry.status = settings::Status::Downloading(progress);

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
------------------------------*/
async fn get_torrents(
    state: web::Data<settings::SharedState>,
    config: web::Data<settings::Config>,
) -> impl Responder {
    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let resp: Value = match client
        .get(format!("{}/api/v2/torrents/info", config.base_url))
        .send()
        .await
    {
        Ok(r) => r.json().await.unwrap_or(Value::Null),
        Err(_) => Value::Null,
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
                    settings::Status::Completed => "Completed".to_string(),
                    settings::Status::Downloading(_) => {
                        format!("Downloading: {:.0}%", progress * 100.0)
                    }
                },
                None => format!("Downloading: {:.0}%", progress * 100.0),
            };

            map.insert(name, status);
        }
    }

    HttpResponse::Ok().json(map)
}

/* -----------------------------
   PUT /torrent (FIXED)
------------------------------*/
async fn put_torrent(
    pending: web::Data<PendingMap>,
    config: web::Data<settings::Config>,
    req: web::Json<Vec<settings::PutItem>>,
) -> impl Responder {
    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let mut pending_lock = pending.write().await;

    for item in req.iter() {
        let id = Uuid::new_v4().to_string();

        log::info!("Queueing torrent [{}]: {}", id, item.url);

        let resp = client
            .post(format!("{}/api/v2/torrents/add", config.base_url))
            .form(&[("urls", item.url.as_str())])
            .send()
            .await;

        if let Err(e) = qb::handle_response(resp, "ADD torrent").await {
            return e;
        }

        pending_lock.insert(id, item.clone());
    }

    HttpResponse::Ok().body("Queued")
}

/* -----------------------------
   START SERVER
------------------------------*/
pub async fn start() -> std::io::Result<()> {
    let config = settings::Config::new();
    logger::init_logger(config.utc_logger);
    let state: settings::SharedState = Arc::new(RwLock::new(HashMap::new()));
    let pending: PendingMap = Arc::new(RwLock::new(HashMap::new()));

    let client = qb::client(&config)
        .await
        .expect("Failed to authenticate qBittorrent");

    spawn_worker(client, state.clone(), pending.clone(), config.clone());

    let host = config.host.clone();
    let port = config.port;

    log::info!("Starting server on: http://{}:{}", host, port);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .app_data(web::Data::new(pending.clone()))
            .app_data(web::Data::new(config.clone()))
            .route("/torrent", web::get().to(get_torrents))
            .route("/torrent", web::put().to(put_torrent))
    })
    .bind((host, port))?
    .run()
    .await
}
