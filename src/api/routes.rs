use crate::{api, config, database, squire};

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// API endpoint to get the current health status.
///
/// # Returns
///
/// Returns the HTTPResponse with a JSON message to indicate the API is up.
#[utoipa::path(
    get,
    path = "/status",
    security(()),
    responses(
        (status = 200, description = "List of users", body = serde_json::Value),
    ),
)]
pub async fn status() -> impl Responder {
    HttpResponse::Ok().json(json!({ "status": "ok" }))
}

/// API endpoint to get the current version of the project.
///
/// # Returns
///
/// Returns the HTTPResponse with a JSON message resolved during compile time.
#[utoipa::path(
    get,
    path = "/version",
    security(()),
    responses(
        (status = 200, description = "API version", body = serde_json::Value)
    )
)]
pub async fn version(metadata: web::Data<config::constant::MetaData>) -> impl Responder {
    HttpResponse::Ok().json(json!({ "version": metadata.pkg_version }))
}

/// Authenticates the `apikey` through incoming request headers.
///
/// # Arguments
///
/// - `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// # Returns
///
/// Returns a boolean value to indicate the authentication status.
fn authenticator(request: HttpRequest, config: &config::env::Config) -> bool {
    if let Some(apikey) = request.headers().get("apikey")
        && apikey.to_str().unwrap() == config.apikey
    {
        return true;
    }
    false
}

/// API endpoint to get download/copy status.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `state` - Reference to the `SharedState` object.
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
/// * `409`: Duplicate request.
/// * `400`: Invalid magnet link.
///
/// # Returns
///
/// Returns a JSON array of [`TorrentEntry`] objects.
#[utoipa::path(
    get,
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
    if !authenticator(request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let mut out: Vec<api::schema::TorrentEntry> = Vec::new();
    let array = api::squire::get_existing(&client, &config).await;

    let mut existing_hashes: Vec<String> = Vec::new();
    if let Ok(conn) = db_connection.lock() {
        for (hash, local) in database::db::load_all(&conn) {
            let live = array.iter().find(|t| t.hash.as_str() == hash.as_str());
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
    }

    // Also surface torrents currently in qBittorrent that were never tracked
    // by this app at all (e.g. added directly through qBittorrent).
    for tracker in array.iter() {
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

/// API endpoint to add torrents to the download queue.
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
    path = "/torrent",
    request_body = Vec<config::settings::PutItem>,
    responses(
        (status = 200, description = "Queued", body = String)
    )
)]
pub async fn put_torrent(
    request: HttpRequest,
    pending: web::Data<config::settings::PendingMap>,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    body: web::Json<Vec<config::settings::PutItem>>,
) -> impl Responder {
    if !authenticator(request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let mut pending_lock = pending.write().await;

    let existing = api::squire::get_existing(&client, &config).await;
    let hashes: Vec<String> = existing
        .into_iter()
        .map(|i| i.hash.to_uppercase().clone())
        .collect();

    let mut response: Vec<HashMap<String, String>> = Vec::new();
    for mut item in api::squire::resolve_payload(&body.into_inner()) {
        let tag = Uuid::new_v4().to_string();
        let url = item.url.to_string();
        let name = item.name.as_ref().unwrap().to_string();
        let hash = item.hash.as_ref().unwrap().to_uppercase().to_string();
        let trackers = item.trackers.as_ref().unwrap().to_vec();

        if hashes.contains(&hash) {
            response.push(HashMap::from([(
                name,
                "Conflict - Duplicate request".to_string(),
            )]));
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
            log::error!("{:?}", e.status().to_string());
            response.push(HashMap::from([(name, e.status().to_string())]));
            continue;
        }

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
        if let Ok(conn) = db_connection.lock() {
            log::debug!("Updated database for pending");
            database::db::upsert_pending(&conn, &tag, &item);
        } else {
            log::error!("Failed to update database for pending");
        }
        response.push(HashMap::from([(
            name,
            format!("OK! Saving to: {}", item.save_path),
        )]));
    }

    HttpResponse::Ok().json(response)
}

/// API endpoint to delete a torrent.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
/// * `query` - JSON query parameters.
///
/// #### Sample Request (delete any downloaded files)
/// ```shell
/// curl -X DELETE "http://localhost:3000/torrent?name=Ubuntu+22.04+LTS"
/// ```
///
/// #### Sample Request (retain any downloaded files)
/// ```shell
/// curl -X DELETE "http://localhost:3000/torrent?name=Ubuntu+22.04+LTS&delete-files=false"
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
    path = "/torrent",
    params(
        ("name" = String, Query, description = "Torrent name"),
        ("delete-files" = bool, Query, description = "Delete files")
    ),
    responses(
        (status = 200, description = "Deleted", body = String)
    )
)]
pub async fn delete_torrent(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    if !authenticator(request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let identifier = match query.get("name") {
        Some(i) => i,
        None => return HttpResponse::BadRequest().body("Missing name"),
    };

    let delete_files = match query.get("delete-files") {
        Some(v) => v == "true",
        None => true,
    };

    // Resolve hash from RuTorrent's own state first (works even if qBit already removed it).
    let hash_from_state = {
        if let Ok(conn) = db_connection.lock() {
            let db = database::db::load_all(&conn);
            db.iter()
                .find(|(_, v)| v.name == *identifier)
                .map(|(h, _)| h.clone())
        } else {
            None
        }
    };

    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    // Resolve hash from qBit (may differ from state if torrent was added externally)
    let hash_from_qbit = {
        let existing = api::squire::get_existing(&client, &config).await;
        existing
            .iter()
            .find(|torrent| torrent.name == *identifier)
            .map(|torrent| torrent.hash.to_owned())
    };

    // Prefer the qBit hash (authoritative); fall back to state hash for the qBit delete call
    let hash = match hash_from_qbit.as_ref().or(hash_from_state.as_ref()) {
        Some(h) => h.clone(),
        None => return HttpResponse::NotFound().body("Torrent not found"),
    };

    log::info!(
        "Deleting torrent, name: {}, hash: {}, deleteFiles: {}",
        identifier,
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
        log::warn!(
            "qBit delete returned non-OK (may already be gone): {}",
            e.status()
        );
    }

    // Always drop from RuTorrent state and DB regardless of qBit outcome
    {
        if let Ok(conn) = db_connection.lock() {
            database::db::remove(&conn, &hash);
        }
    }
    if let Ok(conn) = db_connection.lock() {
        database::db::remove(&conn, &hash);
    }

    log::info!("Successfully deleted {}", identifier);
    HttpResponse::Ok().body("Deleted")
}

/// API endpoint to retry a failed rsync transfer.
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
/// curl -X POST localhost:3000/retry \
///   -H "Content-Type: application/json" \
///   -d '{
///     "name": "Sintel",
///     "remote_host": "192.168.1.102",
///     "remote_username": "admin",
///     "remote_path": "/Users/admin/Sintel",
///     "delete_after_copy": true
///   }'
/// ```
///
/// #### Sample Request (delete old files, re-download from scratch, then transfer)
/// ```shell
/// curl -X POST localhost:3000/retry \
///   -H "Content-Type: application/json" \
///   -d '{
///     "name": "Sintel",
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
    path = "/retry",
    params(
        ("name" = String, Query, description = "Torrent name")
    ),
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
    body: web::Json<config::settings::RetryOptions>,
) -> impl Responder {
    if !authenticator(request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }

    if body.name.is_empty() {
        return HttpResponse::BadRequest().body("Missing name");
    }

    if body.redownload {
        return redownload_torrent(pending, config, db_connection, body.into_inner()).await;
    }

    // Find the hash for the given name in state
    let (hash, mut put_item, files_deleted) = {
        let Ok(conn) = db_connection.lock() else {
            return HttpResponse::InternalServerError().body("Database unavailable");
        };
        match database::db::find_by_name(&conn, &body.name) {
            None => return HttpResponse::NotFound().body("Torrent not found in state"),
            Some((hash, entry)) => match entry.status {
                config::settings::Status::CopyError
                | config::settings::Status::DownloadComplete
                | config::settings::Status::Failed
                | config::settings::Status::Transferred => {
                    (hash, entry.put_item, entry.files_deleted)
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
            "Local files missing for '{}', falling back to redownload",
            body.name
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
        if let Some(mut entry) = database::db::load_one(&conn, &hash) {
            entry.status = config::settings::Status::Copying;
            entry.put_item = put_item.clone();
            database::db::upsert(&conn, &hash, &entry);
        }
    }

    let db_connection_clone = db_connection.as_ref().clone();
    let hash_clone = hash.clone();
    let name_clone = body.name.clone();
    tokio::spawn(async move {
        squire::rsync::run(db_connection_clone, hash_clone, name_clone, put_item).await;
    });

    log::info!("Retry queued for: {}", body.name);
    HttpResponse::Ok().json("Retry queued")
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
    let (hash, mut put_item) = {
        let Ok(conn) = db_connection.lock() else {
            return HttpResponse::InternalServerError().body("Database unavailable");
        };
        match database::db::find_by_name(&conn, &opts.name) {
            None => return HttpResponse::NotFound().body("Torrent not found in state"),
            Some((hash, entry)) => match entry.status {
                config::settings::Status::CopyError
                | config::settings::Status::DownloadComplete
                | config::settings::Status::Transferred
                | config::settings::Status::Failed => (hash, entry.put_item),
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
        if let Err(err) = std::fs::remove_dir_all(&put_item.save_path) {
            log::error!("Failed to remove old files for '{}': {}", opts.name, err);
            return HttpResponse::InternalServerError()
                .body(format!("Failed to remove old files: {}", err));
        }
        log::info!(
            "Removed old local files for '{}' at {}",
            opts.name,
            put_item.save_path
        );
    }

    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    // Best-effort: remove the old torrent from qBittorrent if it's still
    // present, so re-adding the same magnet starts a genuinely fresh download.
    let resp = client
        .post(format!("{}/api/v2/torrents/delete", config.qbit_url))
        .form(&[("hashes", hash.as_str()), ("deleteFiles", "true")])
        .send()
        .await;
    if let Err(e) =
        squire::qb::handle_response(resp, squire::qb::ResponseContext::DeleteTorrent).await
    {
        log::warn!(
            "Torrent '{}' delete-before-redownload returned: {}",
            opts.name,
            e.status()
        );
    }

    // Re-add the torrent to kick off a brand-new download.
    let tag = Uuid::new_v4().to_string();
    if put_item.save_path.is_empty() {
        put_item.save_path =
            squire::savepath::get_default_save_path(&client, &config, &opts.name).await;
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
        database::db::remove(&conn, &hash);
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
        opts.name,
        put_item.save_path
    );
    HttpResponse::Ok().json("Re-download queued")
}

/// API endpoint to pause or resume a torrent in qBittorrent.
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
    post,
    path = "/torrent/pause",
    params(
        ("name" = String, Query, description = "Torrent name"),
        ("pause" = bool, Query, description = "true to pause, false to resume")
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
    if !authenticator(request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let name = match query.get("name") {
        Some(n) => n,
        None => return HttpResponse::BadRequest().body("Missing name"),
    };
    let pause = query.get("pause").map(|v| v == "true").unwrap_or(true);

    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let existing = api::squire::get_existing(&client, &config).await;
    let hash = existing.iter().find_map(|t| {
        if &t.name == name {
            Some(t.hash.to_owned())
        } else {
            None
        }
    });

    let hash = match hash {
        Some(h) => h,
        None => return HttpResponse::NotFound().body("Torrent not found"),
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

    log::info!(
        "{} torrent: {}",
        if pause { "Paused" } else { "Resumed" },
        name
    );
    HttpResponse::Ok().body(if pause { "Paused" } else { "Resumed" })
}
