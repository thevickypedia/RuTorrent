use actix_web::http::header::{HeaderName, HeaderValue};

use crate::{api, config, database, search, squire, vpn};

use actix_web::{HttpRequest, HttpResponse, Responder, web};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// API endpoint to get the current health status.
///
/// # Returns
///
/// Returns the HTTPResponse with a JSON message to indicate the API is up.
pub async fn status() -> impl Responder {
    HttpResponse::Ok().json(json!({ "status": "ok" }))
}

/// API endpoint to get the current version of the project.
///
/// # Returns
///
/// Returns the HTTPResponse with a JSON message resolved during compile time.
pub async fn version(metadata: web::Data<config::constant::MetaData>) -> impl Responder {
    HttpResponse::Ok().json(json!({ "version": metadata.pkg_version }))
}

/// Authenticates the `apikey` through incoming request headers.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// # Returns
///
/// Returns a boolean value to indicate the authentication status.
fn authenticator(request: &HttpRequest, config: &config::env::Config) -> bool {
    request
        .headers()
        .get("apikey")
        .and_then(|apikey| apikey.to_str().ok())
        .map(|apikey| squire::misc::constant_time_eq(apikey, &config.apikey))
        .unwrap_or(false)
}

/// API endpoint to GET download/copy status.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `db_connection` - Database connection received through app data.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl localhost:3000/torrent
/// ```
///
/// #### Sample Response
/// ```json
/// [
///   {
///     "name": "Sintel",
///     "hash": "08ada5a7a6183aae1e09d831df6748d566095a10",
///     "status": "Transferred",
///     "url": "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel",
///     "remote_host": "192.168.1.102",
///     "remote_username": "admin",
///     "remote_path": "/Users/admin/Sintel",
///     "rsync_timeout": 3,
///     "delete_after_copy": true
///   }
/// ]
/// ```
///
/// #### Status
/// * `200`: Successfully queued.
///
/// # Returns
///
/// Returns a JSON array of [`TorrentEntry`] objects.
#[utoipa::path(
    get,
    tag = "Torrent",
    path = "/torrent",
    responses(
        (status = 200, description = "Torrent list", body = Vec<api::schema::TorrentEntry>)
    )
)]
pub async fn get_torrents(
    request: HttpRequest,
    db_connection: web::Data<config::settings::DBConnection>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let mut out: Vec<api::schema::TorrentEntry> = Vec::new();
    let existing = match api::squire::get_existing(&client, &config).await {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mut existing_hashes: Vec<String> = Vec::new();
    let Ok(conn) = db_connection.lock() else {
        log::error!("Failed to lock database connection");
        return HttpResponse::InternalServerError().body("Database unavailable");
    };
    for (hash, local) in database::db::load_all(&conn) {
        let live = existing.iter().find(|t| t.hash.as_str() == hash.as_str());
        let live_progress = live.map(|t| t.progress);
        let live_state = live.map(|t| t.state.clone()).unwrap_or_default();
        out.push(api::squire::to_entry(
            &hash,
            &local,
            live_progress,
            live_state,
        ));
        existing_hashes.push(hash);
    }

    // Also surface torrents currently in qBittorrent that were never tracked
    // by this app at all (e.g. added directly through qBittorrent).
    for tracker in existing.iter() {
        if existing_hashes.contains(&tracker.hash) {
            continue;
        }
        out.push(api::squire::untracked_entry(
            tracker.name.clone(),
            tracker.hash.clone(),
            tracker.magnet_uri.clone(),
            tracker.save_path.clone(),
            tracker.progress,
            tracker.state.clone(),
        ));
    }

    HttpResponse::Ok().json(out)
}

/// API endpoint to ADD torrents to the download queue.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `pending` - Reference to the `PendingMap` object.
/// * `config` - Reference to the `Config` object.
/// * `db_connection` - Database connection received through app data.
/// * `body` - Request body that takes `PutItem` object.
///
/// #### Sample Request
/// ```shell
/// curl -X PUT localhost:3000/torrent \
///   -H "Content-Type: application/json" \
///   -d '[
///     # Download (at custom local path) and transfer content to ssh://admin@192.168.1.102:/Users/admin/Sintel and delete after transfer
///     {
///       "url": "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel",
///       "save_path": "/home/admin/Downloads"  # overrides the local `save_path`
///       "remote_host": "192.168.1.102",
///       "remote_username": "admin",
///       "remote_path": "/Users/admin/Sintel",
///       "delete_after_copy": true
///     },
///     # Download (at default local path) and transfer content to ssh://admin@192.168.1.100:/home/admin/Big_Buck retaining local content
///     {
///       "url": "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny",
///       "remote_host": "192.168.1.100",
///       "remote_username": "admin",
///       "remote_path": "/home/admin/Big_Buck"
///     },
///     # Download (at default local path) without any subsequent transfer (delete_after_copy does not apply without remote transfer)
///     {
///       "url": "magnet:?xt=urn:btih:2C6B6858D61DA9543D4231A71DB4B1C9264B0685&dn=Ubuntu%2022.04%20LTS"
///     }
///   ]'
/// ```
///
/// #### Sample Response
/// ```json
/// "Queued"
/// ```
///
/// # Returns
///
/// Returns a JSON object to indicate the status.
#[utoipa::path(
    put,
    tag = "Torrent",
    path = "/torrent",
    request_body = Vec<config::settings::PutItem>,
    responses(
        (status = 200, description = "A vector of PutResponse object with each torrent's name and state", body = Vec<config::settings::PutResponse>)
    )
)]
pub async fn put_torrent(
    request: HttpRequest,
    pending: web::Data<config::settings::PendingMap>,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    body: web::Json<Vec<config::settings::PutItem>>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let mut pending_lock = pending.write().await;

    let existing = match api::squire::get_existing(&client, &config).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut hashes: Vec<String> = existing
        .into_iter()
        .map(|i| i.hash.to_uppercase())
        .collect();
    match db_connection.lock() {
        Ok(conn) => {
            for (hash, _) in database::db::load_all(&conn) {
                let hash = hash.to_uppercase();
                if !hashes.contains(&hash) {
                    hashes.push(hash);
                }
            }
        }
        Err(_) => {
            log::error!("Failed to lock database connection");
            return HttpResponse::InternalServerError().body("Database unavailable");
        }
    }

    let mut response: Vec<config::settings::PutResponse> = Vec::new();
    for resolved in api::squire::resolve_payload(&body.into_inner(), &config).await {
        let mut item = match resolved {
            Ok(i) => i,
            Err(r) => {
                response.push(r);
                continue;
            }
        };
        let tag = Uuid::new_v4().to_string();
        let url = item.url.to_string();
        let name = item.name.as_ref().unwrap().to_string();
        let hash = item.hash.as_ref().unwrap().to_uppercase().to_string();
        let trackers = item.trackers.as_ref().unwrap().to_vec();

        if hashes.contains(&hash) {
            response.push(config::settings::PutResponse {
                name,
                state: "Conflict - already tracked; delete it or use retry/re-download".to_string(),
            });
            continue;
        }

        log::info!(
            "Adding torrent [{}]: {}, trackers: {}",
            tag,
            name,
            trackers.len()
        );

        let mut params = vec![("urls", &url), ("tags", &tag)];
        if item.save_path.is_empty() {
            item.save_path = squire::savepath::get_default_save_path(&client, &config, &name).await;
        }
        item.save_path = item.save_path.trim().to_string();
        log::info!("Destination for '{}': {}", name, item.save_path);
        params.push(("savepath", &item.save_path));

        let resp = client
            .post(format!("{}/api/v2/torrents/add", config.qbit_url))
            .form(&params)
            .send()
            .await;

        if let Err(e) =
            squire::qb::handle_response(resp, squire::qb::ResponseContext::AddTorrent).await
        {
            let state = squire::qb::error_text(e).await;
            log::error!("Failed to add '{}': {}", name, state);
            response.push(config::settings::PutResponse { name, state });
            continue;
        }
        hashes.push(hash.clone());

        // Only keep rsync info if ALL fields are present
        let has_rsync = !item.remote_host.is_empty()
            && !item.remote_username.is_empty()
            && !item.remote_path.is_empty();
        if has_rsync {
            item.remote_path = item.remote_path.trim().to_string();
            log::info!("Rsync location: {}:{}", item.remote_host, item.remote_path);
        } else {
            log::info!("No rsync location set, download-only");
        }
        pending_lock.insert(tag.clone(), item.clone());
        let Ok(conn) = db_connection.lock() else {
            log::error!("Failed to lock database connection");
            response.push(config::settings::PutResponse {
                name,
                state: format!(
                    "OK! Saving to: {} (warning: pending state not persisted)",
                    item.save_path
                ),
            });
            continue;
        };
        log::debug!("Updated database for pending");
        database::db::upsert_pending(&conn, &tag, &item);
        response.push(config::settings::PutResponse {
            name,
            state: format!("OK! Saving to: {}", item.save_path),
        });
    }

    HttpResponse::Ok().json(response)
}

/// API endpoint to DELETE a torrent.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
/// * `query` - JSON query parameters.
///
/// #### Sample Request (delete any downloaded files)
/// ```shell
/// curl -X DELETE "http://localhost:3000/torrent?hash=2C..85"
/// ```
///
/// #### Sample Request (retain any downloaded files)
/// ```shell
/// curl -X DELETE "http://localhost:3000/torrent?hash=2C..85&delete-files=false"
/// ```
///
/// #### Sample Response
/// ```json
/// Deleted
/// ```
///
/// # Returns
///
/// Returns a JSON object to indicate the status.
#[utoipa::path(
    delete,
    tag = "Torrent",
    path = "/torrent",
    params(
        ("hash" = String, Query, description = "Torrent hash"),
        ("delete-files" = Option<bool>, Query, description = "Delete files")
    ),
    responses(
        (status = 200, description = "Deleted <name> [<hash>]", body = String)
    )
)]
pub async fn delete_torrent(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let hash = match query.get("hash") {
        Some(i) => i.trim().to_lowercase(),
        None => return HttpResponse::BadRequest().body("Missing hash"),
    };

    let delete_files = match query.get("delete-files") {
        Some(v) => v == "true",
        None => true,
    };

    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };
    let (name, in_qbit) =
        match api::squire::resolve_name(&hash, &client, &config, Some(db_connection.get_ref())).await {
            Ok(Some(found)) => found,
            Ok(None) => return HttpResponse::NotFound().body("Torrent not found"),
            Err(e) => return e,
        };

    log::info!(
        "Deleting torrent, name: {}, hash: {}, deleteFiles: {}",
        name,
        hash,
        delete_files
    );

    // Best-effort qBit delete — if it's already gone from qBit this is a no-op.
    let resp = client
        .post(format!("{}/api/v2/torrents/delete", config.qbit_url))
        .form(&[
            ("hashes", hash.as_str()),
            ("deleteFiles", delete_files.to_string().as_str()),
        ])
        .send()
        .await;

    if let Err(e) =
        squire::qb::handle_response(resp, squire::qb::ResponseContext::DeleteTorrent).await
    {
        if in_qbit {
            log::error!("qBit delete failed for {} [{}]: {}", name, hash, e.status());
            return e;
        }
        log::warn!("qBit delete returned non-OK (already gone): {}", e.status());
    }

    // Always drop from RuTorrent state and DB regardless of qBit outcome
    if let Ok(conn) = db_connection.lock() {
        database::db::remove(&conn, &hash);
    } else {
        log::error!("Failed to lock database connection");
        return HttpResponse::InternalServerError()
            .body("Torrent deleted from qBittorrent, but state database is unavailable");
    }

    let fin = format!("Deleted {} [{}]", name, hash);
    log::info!("{}", fin);
    HttpResponse::Ok().body(fin)
}

/// API endpoint to RETRY a failed download [OR] transfer.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `state` - Reference to the `SharedState` object.
/// * `pending` - Reference to the `PendingMap` object (used for `redownload`).
/// * `config` - Reference to the `Config` object.
/// * `db_connection` - Database connection received through app data (used for `redownload`).
/// * `body` - Request body that takes `RetryOptions` object.
///
/// #### Sample Request (retry the transfer using existing local files)
/// ```shell
/// curl -X POST localhost:3000/torrent \
///   -H "Content-Type: application/json" \
///   -d '{
///     "hash": "2C..85",
///     "remote_host": "192.168.1.102",
///     "remote_username": "admin",
///     "remote_path": "/Users/admin/Sintel",
///     "delete_after_copy": true
///   }'
/// ```
///
/// #### Sample Request (delete old files, re-download from scratch, then transfer)
/// ```shell
/// curl -X POST localhost:3000/torrent \
///   -H "Content-Type: application/json" \
///   -d '{
///     "hash": "2C..85",
///     "redownload": true,
///     "remote_host": "192.168.1.102",
///     "remote_username": "admin",
///     "remote_path": "/Users/admin/Sintel",
///     "delete_after_copy": true
///   }'
/// ```
///
/// #### Status
/// * `200`: Retry (or re-download) queued.
/// * `400`: Torrent is not in a retriable state.
/// * `404`: Torrent not found in state.
///
/// # Returns
///
/// Returns a JSON string indicating the result.
#[utoipa::path(
    post,
    tag = "Torrent",
    path = "/torrent",
    request_body = config::settings::RetryOptions,
    responses(
        (status = 200, description = "Retry queued", body = String),
        (status = 400, description = "Not in a retriable state", body = String),
        (status = 404, description = "Not found", body = String),
    )
)]
pub async fn retry_torrent(
    request: HttpRequest,
    pending: web::Data<config::settings::PendingMap>,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    mut body: web::Json<config::settings::RetryOptions>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }

    if body.hash.is_empty() {
        return HttpResponse::BadRequest().body("Missing hash");
    }
    body.hash = body.hash.trim().to_lowercase();

    if body.redownload {
        return redownload_torrent(pending, config, db_connection, body.into_inner()).await;
    }

    // Find the hash for the given name in state
    let (name, mut put_item, files_deleted) = {
        let Ok(conn) = db_connection.lock() else {
            return HttpResponse::InternalServerError().body("Database unavailable");
        };
        match database::db::load_one(&conn, &body.hash) {
            None => return HttpResponse::NotFound().body("Torrent not found in state"),
            Some(entry) => match entry.status {
                config::settings::Status::CopyError
                | config::settings::Status::DownloadComplete
                | config::settings::Status::Failed
                | config::settings::Status::Transferred => {
                    (entry.name, entry.put_item, entry.files_deleted)
                }
                _ => return HttpResponse::BadRequest().body("Torrent is not in a retriable state"),
            },
        }
    };

    // Local files might be gone even though we didn't do the deleting
    // ourselves — e.g. `delete_after_copy` was off and the user removed
    // them manually. A plain rsync retry can't work with nothing to copy,
    // so transparently fall back to a fresh re-download + transfer instead
    // of surfacing an error; the user just asked to "retry" this torrent.
    let files_present =
        !put_item.save_path.is_empty() && std::path::Path::new(&put_item.save_path).exists();
    if files_deleted || !files_present {
        log::info!(
            "Local files missing for '{}' [{}], falling back to redownload",
            name, body.hash
        );
        return redownload_torrent(pending, config, db_connection, body.into_inner()).await;
    }

    if !body.remote_host.is_empty() {
        put_item.remote_host = body.remote_host.clone();
    }
    if !body.remote_username.is_empty() {
        put_item.remote_username = body.remote_username.clone();
    }
    if !body.remote_path.is_empty() {
        put_item.remote_path = body.remote_path.clone();
    }
    if body.rsync_timeout != 0 {
        put_item.rsync_timeout = body.rsync_timeout;
    }
    put_item.delete_after_copy = body.delete_after_copy;

    // Transition back to Copying, persist the (possibly overridden) transfer
    // settings so subsequent `GET /torrent` calls and modal prefills reflect
    // what was actually just submitted, and re-spawn rsync.
    {
        let Ok(conn) = db_connection.lock() else {
            return HttpResponse::InternalServerError().body("Database unavailable");
        };
        if let Some(mut entry) = database::db::load_one(&conn, &body.hash) {
            entry.status = config::settings::Status::Copying;
            entry.put_item = put_item.clone();
            database::db::upsert(&conn, &body.hash, &entry);
        }
    }

    let db_connection_clone = db_connection.as_ref().clone();
    let hash_clone = body.hash.clone();
    let name_clone = name.clone();
    tokio::spawn(async move {
        squire::rsync::run(db_connection_clone, hash_clone, name_clone, put_item).await;
    });

    let fin = format!("Retry queued: {} [{}]", name, body.hash);
    log::info!("{}", fin);
    HttpResponse::Ok().json(fin)
}

/// Deletes any existing local files for a tracked torrent (if present), then
/// re-adds it to qBittorrent from its originally stored URL to start a fresh
/// download. Once that download completes, the normal background worker
/// picks it up and kicks off a fresh rsync transfer exactly as it would for
/// a brand-new torrent.
///
/// # Arguments
///
/// * `state` - Reference to the `SharedState` object.
/// * `pending` - Reference to the `PendingMap` object.
/// * `config` - Reference to the `Config` object.
/// * `db_connection` - Database connection received through app data.
/// * `opts` - The parsed `RetryOptions` (with `redownload == true`).
///
/// # Returns
///
/// Returns an `HttpResponse` indicating the result.
async fn redownload_torrent(
    pending: web::Data<config::settings::PendingMap>,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    opts: config::settings::RetryOptions,
) -> HttpResponse {
    // Find the tracked entry and its originally stored URL/save path.
    let (name, mut put_item) = {
        let Ok(conn) = db_connection.lock() else {
            return HttpResponse::InternalServerError().body("Database unavailable");
        };
        match database::db::load_one(&conn, &opts.hash) {
            None => return HttpResponse::NotFound().body("Torrent not found in state"),
            Some(entry) => match entry.status {
                config::settings::Status::CopyError
                | config::settings::Status::DownloadComplete
                | config::settings::Status::Transferred
                | config::settings::Status::Failed => (entry.name, entry.put_item),
                _ => {
                    return HttpResponse::BadRequest().body(
                        "Torrent must be finished (or failed) before it can be re-downloaded",
                    );
                }
            },
        }
    };

    if put_item.url.is_empty() {
        return HttpResponse::BadRequest()
            .body("Original torrent URL is not available for re-download");
    }

    // Apply any transfer overrides supplied in the modal.
    if !opts.remote_host.is_empty() {
        put_item.remote_host = opts.remote_host.clone();
    }
    if !opts.remote_username.is_empty() {
        put_item.remote_username = opts.remote_username.clone();
    }
    if !opts.remote_path.is_empty() {
        put_item.remote_path = opts.remote_path.clone();
    }
    if opts.rsync_timeout != 0 {
        put_item.rsync_timeout = opts.rsync_timeout;
    }
    put_item.delete_after_copy = opts.delete_after_copy;

    // Delete any locally downloaded files left over from the previous attempt.
    if !put_item.save_path.is_empty() && std::path::Path::new(&put_item.save_path).exists() {
        if !squire::savepath::is_removable(&put_item.save_path, &name) {
            log::warn!(
                "Not removing '{}': not a per-torrent directory",
                put_item.save_path
            );
        } else if let Err(err) = std::fs::remove_dir_all(&put_item.save_path) {
            log::error!("Failed to remove old files for '{}': {}", name, err);
            return HttpResponse::InternalServerError()
                .body(format!("Failed to remove old files: {}", err));
        } else {
            log::info!(
                "Removed old local files for '{}' at {}",
                name,
                put_item.save_path
            );
        }
    }

    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    // Best-effort: remove the old torrent from qBittorrent if it's still
    // present, so re-adding the same magnet starts a genuinely fresh download.
    let resp = client
        .post(format!("{}/api/v2/torrents/delete", config.qbit_url))
        .form(&[("hashes", opts.hash.as_str()), ("deleteFiles", "true")])
        .send()
        .await;
    if let Err(e) =
        squire::qb::handle_response(resp, squire::qb::ResponseContext::DeleteTorrent).await
    {
        log::warn!(
            "Torrent '{}' delete-before-redownload returned: {}",
            name,
            e.status()
        );
    }

    // Re-add the torrent to kick off a brand-new download.
    let tag = Uuid::new_v4().to_string();
    if put_item.save_path.is_empty() {
        put_item.save_path =
            squire::savepath::get_default_save_path(&client, &config, &name).await;
    }
    put_item.save_path = put_item.save_path.trim().to_string();

    let resp = client
        .post(format!("{}/api/v2/torrents/add", config.qbit_url))
        .form(&[
            ("urls", put_item.url.as_str()),
            ("tags", tag.as_str()),
            ("savepath", put_item.save_path.as_str()),
        ])
        .send()
        .await;
    if let Err(e) = squire::qb::handle_response(resp, squire::qb::ResponseContext::AddTorrent).await
    {
        return e;
    }

    // Clear the stale record so the background worker's `resolve_new_torrents`
    // treats the re-added torrent (usually the same hash) as fresh.
    if let Ok(conn) = db_connection.lock() {
        database::db::remove(&conn, &opts.hash);
    }

    {
        let mut pending_lock = pending.write().await;
        pending_lock.insert(tag.clone(), put_item.clone());
    }
    if let Ok(conn) = db_connection.lock() {
        database::db::upsert_pending(&conn, &tag, &put_item);
    }

    log::info!(
        "Re-download queued for: {} (→ {})",
        name,
        put_item.save_path
    );
    let fin = format!("Re-download queued: {} [{}]", name, opts.hash);
    HttpResponse::Ok().json(fin)
}

/// API endpoint to PAUSE [OR] RESUME a torrent in qBittorrent.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
/// * `query` - JSON query parameters.
///
/// # Returns
///
/// Returns an `HttpResponse` indicating the result.
#[utoipa::path(
    patch,
    tag = "Torrent",
    path = "/torrent",
    params(
        ("hash" = String, Query, description = "Torrent hash"),
        ("pause" = Option<bool>, Query, description = "true to pause, false to resume")
    ),
    responses(
        (status = 200, description = "Ok", body = String),
        (status = 404, description = "Not found", body = String),
    )
)]
pub async fn pause_torrent(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let hash = match query.get("hash") {
        Some(n) => n.trim().to_lowercase(),
        None => return HttpResponse::BadRequest().body("Missing hash"),
    };
    let pause = query.get("pause").map(|v| v == "true").unwrap_or(true);

    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let name = match api::squire::resolve_name(&hash, &client, &config, None).await {
        Ok(Some((name, _))) => name,
        Ok(None) => return HttpResponse::NotFound().body("Torrent not found"),
        Err(e) => return e,
    };

    let action = if pause { "stop" } else { "start" };
    let resp = client
        .post(format!("{}/api/v2/torrents/{}", config.qbit_url, action))
        .form(&[("hashes", hash.as_str())])
        .send()
        .await;

    if let Err(e) =
        squire::qb::handle_response(resp, squire::qb::ResponseContext::PauseResumeTorrent).await
    {
        return e;
    }

    let state_ = if pause { "Paused" } else { "Resumed" };
    let fin = format!("{} {} [{}]", state_, name, hash);
    log::info!("{}", fin);
    HttpResponse::Ok().body(fin)
}

/// API endpoint to get search category.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `nested` - Boolean flag to get the nested object.
///
/// #### Sample Request
/// ```shell
/// curl localhost:3000/search-categories?nested=false
/// ```
///
/// #### Sample Response
/// ```json
/// {
///   "100": "Audio",
///   "101": "Music",
///   "102": "Audio Books",
///   "103": "Sound Clips",
///   ...
/// }
/// ```
///
/// #### Status
/// * `200`: Successful.
///
/// # Returns
///
/// Returns a HashMap of categories with id and name as key-value pairs.
#[utoipa::path(
    get,
    tag = "Search",
    path = "/categories",
    params(
        ("nested" = Option<bool>, Query, description = "Boolean flag to get nested categories")
    ),
    responses(
        (status = 200, description = "Static category map", body = HashMap<u16, String>),
        (status = 200, description = "Nested category map", body = HashMap<String, HashMap<u16, String>>)
    )
)]
pub async fn search_category(
    request: HttpRequest,
    query: web::Query<HashMap<String, String>>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let nested = query.get("nested").map(|v| v == "true").unwrap_or(false);
    let categories = &search::category::categories();
    if nested {
        log::info!("Rendering nested categories");
        HttpResponse::Ok().json(categories)
    } else {
        log::info!("Rendering flattened categories");
        HttpResponse::Ok().json(search::category::flatten(categories))
    }
}

/// API endpoint to search for torrents.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `payload` - Reference to the `SearchQuery` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl localhost:3000/search?query=ubuntu
/// ```
///
/// #### Sample Response
/// ```json
/// [
///     {
///         "title": "Ubuntu 22.04 LTS",
///         "category": "303",
///         "size": "3.40 GB",
///         "seeders": 40,
///         "leechers": 12,
///         "magnet_link": "magnet:?xt=urn:btih:2C6B6858D61DA9543D4231A71DB4B1C9264B0685&dn=Ubuntu%2022.04%20LTS",
///         "uploader": "rjaa",
///         "date_uploaded": "1652877231"
///     },
///     {
///         "title": "Clinton D., Negus C. Ubuntu Linux Bible 11ed 2025",
///         "category": "601",
///         "size": "8.16 MB",
///         "seeders": 27,
///         "leechers": 0,
///         "magnet_link": "magnet:?xt=urn:btih:232CD67EB3FFBD7C37BF9EC3EE887417E5AE1EE6&dn=Clinton%20D.%2C%20Negus%20C.%20Ubuntu%20Linux%20Bible%2011ed%202025",
///         "uploader": "andryold1",
///         "date_uploaded": "1765526528"
///     }
/// ]
/// ```
///
/// #### Status
/// * `200`: Successful.
/// * `404`: Not found.
///
/// # Returns
///
/// Returns a JSON array of [`BaseTorrent`] objects.
#[utoipa::path(
    get,
    tag = "Search",
    path = "/search",
    params(config::settings::SearchTorrents),
    responses(
        (status = 200, description = "List of results as structured object", body = Vec<api::schema::BaseTorrent>),
        (status = 404),
        (status = 502),
        (status = 503),
        (status = 504),
    )
)]
pub async fn search_torrents(
    request: HttpRequest,
    payload: web::Query<config::settings::SearchTorrents>,
    config: web::Data<config::env::Config>,
    metadata: web::Data<config::constant::MetaData>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    log::debug!("Query: {:?}", payload);
    let torrents = match search::api::search(
        &payload.query,
        payload.category,
        payload.use_tor,
        payload.strict,
        payload.hydrate_categories,
        &config,
        &metadata,
    )
    .await
    {
        Ok(torrents) => torrents,
        Err(e) => {
            log::error!("Failed to search for {} - {}", payload.query, e);
            let status = if let Some(h) = e.downcast_ref::<api::schema::HttpError>() {
                actix_web::http::StatusCode::from_u16(h.status)
                    .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR)
            } else if let Some(r) = e.downcast_ref::<reqwest::Error>() {
                if r.is_timeout() {
                    actix_web::http::StatusCode::GATEWAY_TIMEOUT
                } else if r.is_builder() {
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    actix_web::http::StatusCode::BAD_GATEWAY
                }
            } else {
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
            };
            return HttpResponse::build(status).json(e.to_string());
        }
    };
    let length = torrents.len();
    let mut response = HttpResponse::Ok().json(torrents);
    response.headers_mut().insert(
        HeaderName::from_static("results"),
        HeaderValue::from(length),
    );
    response
}

/// API endpoint to get the current vpn relay using mullvad cli.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl localhost:3000/vpn
/// ```
///
/// #### Sample Response
/// ```json
/// {
///   "state": "connected",
///   "details": {
///     "location": {
///       "city": "Madrid",
///       "country": "Spain",
///       "ipv4": null,
///       "hostname": "es-mad-wg-203"
///     }
///   }
/// }
/// ```
///
/// #### Status
/// * `200`: Response from mullvad.
/// * `502`: Mullvad CLI error.
///
/// # Returns
///
/// Returns the `vpn::models::MullvadResponse` object.
#[utoipa::path(
    get,
    tag = "VPN",
    path = "/vpn",
    responses(
        (status = 200, description = "Response message from mullvad cli", body = vpn::models::MullvadResponse),
        (status = 502, description = "mullvad CLI error", body = String)
    )
)]
pub async fn get_vpn(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    api::squire::vpn_response(vpn::connect::get_current().await)
}

/// API endpoint to connect to a vpn relay using mullvad cli.
///
/// - `hostname` input takes the highest priority.
///
/// - Supports both city and country codes for whitelist and blacklist.
///
/// - Check [https://api.mullvad.net/www/relays/all/](https://api.mullvad.net/www/relays/all/) to get the list of all relays.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `payload` - Reference to the `ConnectVPN` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl -X POST localhost:3000/vpn \
///   -H "Content-Type: application/json" -d '{}'
/// ```
///
/// #### Sample Response
/// ```json
/// {
///   "state": "connected",
///   "details": {
///     "location": {
///       "city": "Madrid",
///       "country": "Spain",
///       "ipv4": null,
///       "hostname": "es-mad-wg-203"
///     }
///   }
/// }
/// ```
///
/// #### Status
/// * `200`: Response from mullvad.
/// * `400`: Invalid user input.
/// * `502`: Mullvad CLI error.
/// * `503`: Mullvad CLI missing relay.
/// * `504`: VPN did not reach requested state.
///
/// # Returns
///
/// Returns the `vpn::models::MullvadResponse` object.
#[utoipa::path(
    post,
    tag = "VPN",
    path = "/vpn",
    request_body = Option<config::settings::ConnectVPN>,
    responses(
        (status = 200, description = "Response message from mullvad cli", body = vpn::models::MullvadResponse),
        (status = 400, description = "Invalid hostname, or unknown/conflicting whitelist/blacklist codes", body = String),
        (status = 502, description = "mullvad CLI error", body = String),
        (status = 503, description = "mullvad CLI missing or relay list unavailable", body = String),
        (status = 504, description = "VPN did not reach the requested state", body = String),
    )
)]
pub async fn connect_vpn(
    request: HttpRequest,
    body: web::Json<config::settings::ConnectVPN>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    if !body.hostname.trim().is_empty() {
        return api::squire::vpn_response(
            vpn::connect::connect_by_hostname(body.hostname.trim()).await,
        );
    }
    api::squire::vpn_response(
        vpn::connect::shuffler(
            api::squire::split_codes(&body.whitelist),
            api::squire::split_codes(&body.blacklist),
            &config,
        )
        .await,
    )
}

/// API endpoint to disconnect mullvad vpn.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl -X DELETE localhost:3000/vpn?lockdown=false
/// ```
///
/// #### Sample Response
/// ```json
/// {
///   "state": "disconnected",
///   "details": {
///     "location": null,
///     "locked_down": true,
///   }
/// }
/// ```
///
/// #### Status
/// * `200`: Response from mullvad.
/// * `400`: Invalid user input.
/// * `502`: Mullvad CLI error.
/// * `503`: Mullvad CLI missing relay.
/// * `504`: VPN did not reach requested state.
///
/// # Returns
///
/// Returns the `vpn::models::MullvadResponse` object.
#[utoipa::path(
    delete,
    tag = "VPN",
    path = "/vpn",
    params(
        ("lockdown" = Option<bool>, Query, description = "Boolean flag to set lockdown mode.")
    ),
    responses(
        (status = 200, description = "Disconnected", body = vpn::models::MullvadResponse),
        (status = 502, description = "mullvad CLI error", body = String),
        (status = 504, description = "VPN did not reach the requested state", body = String),
    )
)]
pub async fn disconnect_vpn(
    request: HttpRequest,
    query: web::Query<HashMap<String, String>>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let lockdown = query.get("lockdown").map(|v| v == "true").unwrap_or(false);
    log::warn!(
        "Received disconnect request with lockdown state as {}",
        lockdown
    );
    api::squire::vpn_response(vpn::connect::disconnect(lockdown).await)
}
