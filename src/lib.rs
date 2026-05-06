#![allow(rustdoc::bare_urls)]
#![doc = include_str!("../README.md")]

use actix_web::{web, App, HttpServer};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

mod api;
mod logger;
mod ntfy;
mod qb;
mod rsync;
mod settings;
mod squire;
mod swagger;
mod telegram;

/// Contains entrypoint and initializer settings to trigger the asynchronous `HTTPServer`
///
/// # Examples
///
/// ```no_run
/// #[actix_rt::main]
/// async fn main() -> std::io::Result<()> {
///    rutorrent::start().await
/// }
/// ```
pub async fn start() -> std::io::Result<()> {
    squire::load_env_file();
    let config = settings::Config::new();
    logger::init_logger(config.utc_logger, config.log_level);
    let state: settings::SharedState = Arc::new(RwLock::new(HashMap::new()));
    let pending: settings::PendingMap = Arc::new(RwLock::new(HashMap::new()));

    let client = qb::client(&config)
        .await
        .expect("Failed to authenticate qBittorrent");
    squire::spawn_worker(client, state.clone(), pending.clone(), config.clone());

    let host = config.host.clone();
    let port = config.port;
    let workers = config.workers;

    log::info!(
        "Starting server on: http://{}:{} with {} workers",
        host,
        port,
        workers
    );

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .app_data(web::Data::new(pending.clone()))
            .app_data(web::Data::new(config.clone()))
            .route("/status", web::get().to(api::status))
            .route("/health", web::get().to(api::status))
            .route("/version", web::get().to(api::version))
            .route("/torrent", web::get().to(api::get_torrents))
            .route("/torrent", web::put().to(api::put_torrent))
            .route("/torrent", web::delete().to(api::delete_torrent))
            .route("/swagger", web::get().to(swagger::redirector))
            .route("/ui", web::get().to(swagger::redirector))
            .service(swagger::service())
    })
    .bind((host, port))?
    .workers(workers)
    .run()
    .await
}
