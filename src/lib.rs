#![allow(rustdoc::bare_urls)]
#![doc = include_str!("../README.md")]

use actix_web::{web, App, HttpServer};
use std::sync::Arc;
use tokio::sync::RwLock;

mod api;
mod config;
mod database;
mod notifier;
mod output;
mod squire;
mod swagger;

use output::display;

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
    let metadata = config::constant::build_info();
    let cli_args = squire::parser::arguments(&metadata);
    if cli_args.read_db {
        let _ = database::squire::print_content();
        return Ok(());
    }

    squire::misc::load_env_file(cli_args.env_file);
    let config = config::env::Config::new();
    output::logger::init_logger(&config, &metadata);

    let db_conn = database::db::open();
    let initial_pending = database::db::load_pending(&db_conn);
    log::info!("Loaded {} pending entries from database", initial_pending.len());
    let pending: config::settings::PendingMap = Arc::new(RwLock::new(initial_pending));

    let client = match squire::qb::client(&config).await {
        Ok(client) => client,
        Err(_) => {
            error!("Failed to authenticate qBittorrent");
        }
    };
    let db_conn = Arc::new(std::sync::Mutex::new(db_conn));
    squire::background::spawn_worker(client, pending.clone(), config.clone(), db_conn.clone());

    let host = config.host.clone();
    let port = config.port;
    let workers = config.workers;

    log::info!("Starting server on: http://{}:{} with {} workers", host, port, workers);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pending.clone()))
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(db_conn.clone()))
            .app_data(web::Data::new(metadata.clone()))
            .route("/status", web::get().to(api::routes::status))
            .route("/health", web::get().to(api::routes::status))
            .route("/version", web::get().to(api::routes::version))
            .route("/torrent", web::get().to(api::routes::get_torrents))
            .route("/torrent", web::put().to(api::routes::put_torrent))
            .route("/torrent", web::delete().to(api::routes::delete_torrent))
            .route("/retry", web::post().to(api::routes::retry_torrent))
            .route("/torrent/pause", web::post().to(api::routes::pause_torrent))
            .route("/swagger", web::get().to(swagger::openapi::redirector))
            .route("/ui", web::get().to(swagger::openapi::redirector))
            .route("/authenticator", web::post().to(swagger::ui::authenticator))
            .route("/", web::get().to(swagger::ui::index_page))
            .service(swagger::openapi::service())
    })
    .bind((host, port))?
    .workers(workers)
    .run()
    .await
}
