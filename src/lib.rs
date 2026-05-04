#![allow(rustdoc::bare_urls)]
#![doc = include_str!("../README.md")]

use actix_web::{web, App, HttpServer};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

mod logger;
mod qb;
mod rsync;
mod settings;
mod api;
mod squire;
/* -----------------------------
   START SERVER
------------------------------*/
pub async fn start() -> std::io::Result<()> {
    let config = settings::Config::new();
    logger::init_logger(config.utc_logger);
    let state: settings::SharedState = Arc::new(RwLock::new(HashMap::new()));
    let pending: settings::PendingMap = Arc::new(RwLock::new(HashMap::new()));

    let client = qb::client(&config)
        .await
        .expect("Failed to authenticate qBittorrent");

    squire::spawn_worker(client, state.clone(), pending.clone(), config.clone());

    let host = config.host.clone();
    let port = config.port;

    log::info!("Starting server on: http://{}:{}", host, port);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .app_data(web::Data::new(pending.clone()))
            .app_data(web::Data::new(config.clone()))
            .route("/torrent", web::get().to(api::get_torrents))
            .route("/torrent", web::put().to(api::put_torrent))
            .route("/torrent", web::delete().to(api::delete_torrent))
    })
    .bind((host, port))?
    .run()
    .await
}
