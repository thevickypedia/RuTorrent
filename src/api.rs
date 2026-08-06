use crate::{constant, savepath, settings};
use crate::{database, qb};

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use url::Url;
use utoipa::ToSchema;
use uuid::Uuid;

/// ### TorrentEntry
/// A single torrent's state as exposed through the WebUI/API — includes the
/// originally submitted torrent URL and transfer settings so the frontend can
/// prefill the retry modal and support re-downloading without extra round-trips.
#[derive(ToSchema, Clone, serde::Serialize)]
pub struct TorrentEntry {
    pub name: String,
    pub hash: String,
    pub status: String,
    pub url: String,
    pub remote_host: String,
    pub remote_username: String,
    pub remote_path: String,
    pub rsync_timeout: u8,
    pub delete_after_copy: bool,
    /// `true` when the locally downloaded files were deleted (e.g. via
    /// `delete_after_copy`). A plain rsync retry is impossible in that case —
    /// only a fresh re-download can recover this torrent.
    pub files_deleted: bool,
}

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
pub async fn version(metadata: web::Data<constant::MetaData>) -> impl Responder {
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
fn authenticator(request: HttpRequest, config: &settings::Config) -> bool {
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
        (status = 200, description = "Torrent list", body = Vec<TorrentEntry>)
    )
)]
pub async fn get_torrents(
    request: HttpRequest,
    state: web::Data<settings::SharedState>,
    config: web::Data<settings::Config>,
) -> impl Responder {
    if !authenticator(request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let db = state.read().await;
    let array = get_existing(&client, &config).await;
    let mut out: Vec<TorrentEntry> = Vec::new();

    if !config.data_storage {
        // Legacy behavior: entirely driven by qBittorrent's live torrent list.
        // Anything qBittorrent no longer knows about simply won't appear here.
        if array.is_empty() {
            return HttpResponse::Ok().json(out);
        }

        for t in array.iter() {
            let name = t["name"].to_string();
            let hash = t["hash"].to_string();
            let progress = t["progress"].parse::<f64>().unwrap();
            match db.get(&hash) {
                Some(local) => out.push(to_entry(&hash, local, Some(progress))),
                // Not in state — download-only torrent still in qBit, not tracked
                None => out.push(untracked_entry(name, hash, progress)),
            }
        }

        return HttpResponse::Ok().json(out);
    }

    // `data_storage` enabled: RuTorrent's own state/DB is the source of truth,
    // so every torrent it has ever tracked is always shown — even after it's
    // gone from qBittorrent (deleted manually, via `delete_after_copy`, or via
    // this app's own delete button).
    for (hash, local) in db.iter() {
        let live_progress = array
            .iter()
            .find(|t| t.get("hash").map(String::as_str) == Some(hash.as_str()))
            .and_then(|t| t.get("progress"))
            .and_then(|p| p.parse::<f64>().ok());
        out.push(to_entry(hash, local, live_progress));
    }

    // Also surface torrents currently in qBittorrent that were never tracked
    // by this app at all (e.g. added directly through qBittorrent).
    for t in array.iter() {
        let hash = t.get("hash").cloned().unwrap_or_default();
        if db.contains_key(&hash) {
            continue;
        }
        let name = t.get("name").cloned().unwrap_or_default();
        let progress = t
            .get("progress")
            .and_then(|p| p.parse::<f64>().ok())
            .unwrap_or(0.0);
        out.push(untracked_entry(name, hash, progress));
    }

    HttpResponse::Ok().json(out)
}

/// Builds a [`TorrentEntry`] for a torrent tracked in RuTorrent's own state —
/// carries the originally submitted URL and transfer settings alongside the
/// resolved status text.
///
/// # Arguments
///
/// * `hash` - Torrent hash (state key).
/// * `local` - The tracked `RsyncTrack` entry from RuTorrent's own state.
/// * `live_progress` - Freshly polled progress from qBittorrent, if the
///   torrent is still known to it. Falls back to the last known progress
///   captured on `local.status` when `None` (i.e. no longer in qBittorrent).
fn to_entry(hash: &str, local: &settings::RsyncTrack, live_progress: Option<f64>) -> TorrentEntry {
    TorrentEntry {
        name: local.name.clone(),
        hash: hash.to_string(),
        status: resolve_status(local, live_progress),
        url: local.put_item.url.clone(),
        remote_host: local.put_item.remote_host.clone(),
        remote_username: local.put_item.remote_username.clone(),
        remote_path: local.put_item.remote_path.clone(),
        rsync_timeout: local.put_item.rsync_timeout,
        delete_after_copy: local.put_item.delete_after_copy,
        files_deleted: local.files_deleted,
    }
}

/// Builds a [`TorrentEntry`] for a torrent currently in qBittorrent that this
/// app never tracked (e.g. added directly through qBittorrent). There's no
/// stored URL or transfer settings for these.
fn untracked_entry(name: String, hash: String, progress: f64) -> TorrentEntry {
    TorrentEntry {
        name,
        hash,
        status: format!("Downloading: {:.0}%", progress * 100.0),
        url: String::new(),
        remote_host: String::new(),
        remote_username: String::new(),
        remote_path: String::new(),
        rsync_timeout: 0,
        delete_after_copy: false,
        files_deleted: false,
    }
}

/// Resolves the human-readable status text for a tracked torrent.
///
/// # Arguments
///
/// * `local` - The tracked `RsyncTrack` entry from RuTorrent's own state.
/// * `live_progress` - Freshly polled progress from qBittorrent, if the
///   torrent is still known to it. Falls back to the last known progress
///   captured on `local.status` when `None` (i.e. no longer in qBittorrent).
///
/// # Returns
///
/// Returns the status string shown in the WebUI for this torrent.
fn resolve_status(local: &settings::RsyncTrack, live_progress: Option<f64>) -> String {
    match local.status {
        settings::Status::Copying => "Copying".to_string(),
        settings::Status::Transferred => "Transferred".to_string(),
        settings::Status::Completed => "Completed".to_string(),
        settings::Status::DownloadComplete => "Downloaded".to_string(),
        settings::Status::Failed => "Failed".to_string(),
        settings::Status::CopyError => "CopyError".to_string(),
        settings::Status::Downloading(last_known) => {
            let progress = live_progress.unwrap_or(last_known);
            let has_rsync = !local.put_item.remote_host.is_empty()
                && !local.put_item.remote_username.is_empty()
                && !local.put_item.remote_path.is_empty();
            if has_rsync {
                format!("Downloading: {:.0}% (→ copy queued)", progress * 100.0)
            } else {
                format!("Downloading: {:.0}%", progress * 100.0)
            }
        }
    }
}

/// Get existing torrents' information from QBitAPI.
///
/// # Arguments
///
/// * `client` - The HTTP client used to perform the request.
/// * `config` - Reference to the `Config` object.
///
/// # Returns
///
/// Returns a vector of HashMap with `name`, `hash` and `progress` in key-value format.
async fn get_existing(client: &Client, config: &settings::Config) -> Vec<HashMap<String, String>> {
    let resp: Value = match client
        .get(format!("{}/api/v2/torrents/info", config.qbit_url))
        .send()
        .await
    {
        Ok(r) => r.json().await.unwrap_or(Value::Null),
        Err(_) => Value::Null,
    };

    let mut vec = Vec::new();

    if let Some(arr) = resp.as_array() {
        for t in arr {
            let mut map = HashMap::new();
            map.insert(
                "name".to_string(),
                t["name"].as_str().unwrap_or("?").to_string(),
            );
            map.insert(
                "hash".to_string(),
                t["hash"].as_str().unwrap_or("").to_string(),
            );
            map.insert(
                "progress".to_string(),
                format!("{}", t["progress"].as_f64().unwrap_or(0.0)),
            );
            vec.push(map);
        }
    }
    vec
}

/// Extends the payload for `PutItem` with resolved `name`, `hash` and `trackers`
///
/// # Arguments
///
/// * `body` - Request body that takes `PutItem` object.
///
/// # Returns
///
/// Returns the extended `PutItem` with attached `name`, `hash` and `trackers`
fn resolve_payload(body: &[settings::PutItem]) -> Vec<settings::PutItem> {
    let mut ret: Vec<settings::PutItem> = Vec::new();
    for item in body.iter() {
        let url = match Url::parse(&item.url) {
            Ok(url) => url,
            Err(e) => {
                log::error!("Invalid URL '{}': {}", item.url, e);
                return Vec::new();
            }
        };
        let query_pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();

        let mut hash = String::new();
        let mut name = String::new();
        let mut trackers: Vec<String> = Vec::new();
        for (key, value) in query_pairs {
            if key == "xt" {
                hash = value.split(":").last().unwrap().to_string();
            } else if key == "dn" {
                name = value;
            } else {
                trackers.push(value);
            }
        }
        ret.push(settings::PutItem {
            url: url.to_string(),
            name: Some(name),
            hash: Some(hash),
            trackers: Some(trackers),
            save_path: item.save_path.to_owned(),
            remote_host: item.remote_host.to_string(),
            remote_username: item.remote_username.to_string(),
            remote_path: item.remote_path.to_string(),
            rsync_timeout: item.rsync_timeout.to_owned(),
            delete_after_copy: item.delete_after_copy,
        });
    }
    ret
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
    request_body = Vec<settings::PutItem>,
    responses(
        (status = 200, description = "Queued", body = String)
    )
)]
pub async fn put_torrent(
    request: HttpRequest,
    pending: web::Data<settings::PendingMap>,
    config: web::Data<settings::Config>,
    db_connection: web::Data<settings::DBConnection>,
    body: web::Json<Vec<settings::PutItem>>,
) -> impl Responder {
    if !authenticator(request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let mut pending_lock = pending.write().await;

    let existing = get_existing(&client, &config).await;
    let hashes: Vec<String> = existing
        .into_iter()
        .map(|i| i.get("hash").unwrap().to_uppercase().clone())
        .collect();

    let mut response: Vec<HashMap<String, String>> = Vec::new();
    for mut item in resolve_payload(&body.into_inner()) {
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
            item.save_path = savepath::get_default_save_path(&client, &config, &name).await;
        }
        item.save_path = item.save_path.trim().to_string();
        log::info!("Destination for '{}': {}", name, item.save_path);
        params.push(("savepath", &item.save_path));

        let resp = client
            .post(format!("{}/api/v2/torrents/add", config.qbit_url))
            .form(&params)
            .send()
            .await;

        if let Err(e) = qb::handle_response(resp, qb::ResponseContext::AddTorrent).await {
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
            database::upsert_pending(&conn, &tag, &item);
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
    config: web::Data<settings::Config>,
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

    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let resp: Value = match client
        .get(format!("{}/api/v2/torrents/info", config.qbit_url))
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

    log::info!(
        "Deleting torrent, name: {}, hash: {}, deleteFiles: {}",
        identifier,
        hash,
        delete_files
    );

    let resp = client
        .post(format!("{}/api/v2/torrents/delete", config.qbit_url))
        .form(&[
            ("hashes", hash.as_str()),
            ("deleteFiles", delete_files.to_string().as_str()),
        ])
        .send()
        .await;

    if let Err(e) = qb::handle_response(resp, qb::ResponseContext::DeleteTorrent).await {
        return e;
    }

    log::info!("Successfully deleted {}", identifier);
    HttpResponse::Ok().json("Deleted")
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
    state: web::Data<settings::SharedState>,
    pending: web::Data<settings::PendingMap>,
    config: web::Data<settings::Config>,
    db_connection: web::Data<settings::DBConnection>,
    body: web::Json<settings::RetryOptions>,
) -> impl Responder {
    if !authenticator(request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }

    if body.name.is_empty() {
        return HttpResponse::BadRequest().body("Missing name");
    }

    if body.redownload {
        return redownload_torrent(state, pending, config, db_connection, body.into_inner()).await;
    }

    // Find the hash for the given name in state
    let (hash, mut put_item, files_deleted) = {
        let db = state.read().await;
        let found = db.iter().find(|(_, entry)| entry.name == body.name);
        match found {
            None => return HttpResponse::NotFound().body("Torrent not found in state"),
            Some((hash, entry)) => match entry.status {
                settings::Status::CopyError
                | settings::Status::DownloadComplete
                | settings::Status::Transferred => {
                    (hash.clone(), entry.put_item.clone(), entry.files_deleted)
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
    let files_present = !put_item.save_path.is_empty()
        && std::path::Path::new(&put_item.save_path).exists();
    if files_deleted || !files_present {
        log::info!(
            "Local files missing for '{}', falling back to redownload",
            body.name
        );
        return redownload_torrent(state, pending, config, db_connection, body.into_inner()).await;
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
        let mut db = state.write().await;
        if let Some(entry) = db.get_mut(&hash) {
            entry.status = settings::Status::Copying;
            entry.put_item = put_item.clone();
        }
        if let Ok(conn) = db_connection.lock()
            && let Some(entry) = db.get(&hash)
        {
            database::upsert(&conn, &hash, entry);
        }
    }

    let state_clone = state.as_ref().clone();
    let db_connection_clone = db_connection.as_ref().clone();
    let hash_clone = hash.clone();
    let name_clone = body.name.clone();
    tokio::spawn(async move {
        crate::rsync::run(state_clone, db_connection_clone, hash_clone, name_clone, put_item).await;
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
    state: web::Data<settings::SharedState>,
    pending: web::Data<settings::PendingMap>,
    config: web::Data<settings::Config>,
    db_connection: web::Data<settings::DBConnection>,
    opts: settings::RetryOptions,
) -> HttpResponse {
    // Find the tracked entry and its originally stored URL/save path.
    let (hash, mut put_item) = {
        let db = state.read().await;
        let found = db.iter().find(|(_, entry)| entry.name == opts.name);
        match found {
            None => return HttpResponse::NotFound().body("Torrent not found in state"),
            Some((hash, entry)) => match entry.status {
                settings::Status::CopyError
                | settings::Status::DownloadComplete
                | settings::Status::Transferred
                | settings::Status::Failed => (hash.clone(), entry.put_item.clone()),
                _ => {
                    return HttpResponse::BadRequest()
                        .body("Torrent must be finished (or failed) before it can be re-downloaded");
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

    let client = match qb::client(&config).await {
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
    if let Err(e) = qb::handle_response(resp, qb::ResponseContext::DeleteTorrent).await {
        log::warn!(
            "Torrent '{}' delete-before-redownload returned: {}",
            opts.name,
            e.status()
        );
    }

    // Re-add the torrent to kick off a brand-new download.
    let tag = Uuid::new_v4().to_string();
    if put_item.save_path.is_empty() {
        put_item.save_path = savepath::get_default_save_path(&client, &config, &opts.name).await;
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
    if let Err(e) = qb::handle_response(resp, qb::ResponseContext::AddTorrent).await {
        return e;
    }

    // The re-added torrent will almost always resolve to the same hash it
    // had before (magnets are content-addressed). The background worker's
    // `resolve_new_torrents` skips any hash it already finds in `state`, so
    // the stale record from the previous attempt (still sitting there with
    // e.g. `Transferred`) would otherwise shadow the new download forever
    // and never get refreshed to `Downloading`. Clear it now that the fresh
    // add has actually succeeded, so the next poll tick claims it normally.
    {
        let mut db = state.write().await;
        db.remove(&hash);
    }
    if let Ok(conn) = db_connection.lock() {
        database::remove(&conn, &hash);
    }

    {
        let mut pending_lock = pending.write().await;
        pending_lock.insert(tag.clone(), put_item.clone());
    }
    if let Ok(conn) = db_connection.lock() {
        database::upsert_pending(&conn, &tag, &put_item);
    }

    log::info!(
        "Re-download queued for: {} (→ {})",
        opts.name,
        put_item.save_path
    );
    HttpResponse::Ok().json("Re-download queued")
}
