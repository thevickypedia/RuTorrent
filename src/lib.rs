#![allow(rustdoc::bare_urls)]
#![doc = include_str!("../README.md")]

use actix_web::{web, App, HttpServer};
use std::sync::Arc;
use tokio::sync::RwLock;

mod api;
mod constant;
mod database;
mod db;
mod logger;
mod ntfy;
mod parser;
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
    let metadata = constant::build_info();
    let cli_args = parser::arguments(&metadata);
    if cli_args.read_db {
        let _ = db::print_content();
        return Ok(());
    }

    squire::load_env_file(cli_args.env_file);
    let config = settings::Config::new();
    logger::init_logger(config.utc_logger, config.log_level, &metadata);

    let db_conn = database::open();
    let initial_state = database::load_all(&db_conn);
    let initial_pending = database::load_pending(&db_conn);
    log::info!(
        "Loaded {} state and {} pending entries from database",
        initial_state.len(),
        initial_pending.len()
    );
    let state: settings::SharedState = Arc::new(RwLock::new(initial_state));
    let pending: settings::PendingMap = Arc::new(RwLock::new(initial_pending));

    let client = qb::client(&config)
        .await
        .expect("Failed to authenticate qBittorrent");
    let db_conn = Arc::new(std::sync::Mutex::new(db_conn));
    squire::spawn_worker(
        client,
        state.clone(),
        pending.clone(),
        config.clone(),
        db_conn.clone(),
    );

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
            .app_data(web::Data::new(db_conn.clone()))
            .app_data(web::Data::new(metadata.clone()))
            .route("/status", web::get().to(api::status))
            .route("/health", web::get().to(api::status))
            .route("/version", web::get().to(api::version))
            .route("/torrent", web::get().to(api::get_torrents))
            .route("/torrent", web::put().to(api::put_torrent))
            .route("/torrent", web::delete().to(api::delete_torrent))
            .route("/swagger", web::get().to(swagger::redirector))
            .route("/ui", web::get().to(swagger::redirector))
            .route("/", web::get().to(swagger::redirector))
            .service(swagger::service())
    })
    .bind((host, port))?
    .workers(workers)
    .run()
    .await
}
