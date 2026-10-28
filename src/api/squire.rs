use crate::{api, config, database, search, squire, vpn};

use actix_web::{web, HttpRequest, HttpResponse};
use reqwest::Client;
use serde_json::Value;
use url::Url;
use uuid::Uuid;

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
pub fn authenticator(request: &HttpRequest, config: &config::env::Config) -> bool {
    request
        .headers()
        .get("apikey")
        .and_then(|apikey| apikey.to_str().ok())
        .map(|apikey| squire::misc::constant_time_eq(apikey, &config.apikey))
        .unwrap_or(false)
}

/// Builds a `GetTorrentResponse` for a torrent tracked in RuTorrent's own state —
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
pub fn to_entry(
    hash: &str,
    local: &config::settings::RsyncTrack,
    live_progress: Option<f64>,
    live_state: String,
) -> api::schema::GetTorrentResponse {
    api::schema::GetTorrentResponse {
        name: local.name.clone(),
        hash: hash.to_string(),
        url: local.put_item.url.clone(),
        save_path: local.put_item.save_path.clone(),
        status: resolve_status(local, live_progress),
        remote_host: local.put_item.remote_host.clone(),
        remote_username: local.put_item.remote_username.clone(),
        remote_path: local.put_item.remote_path.clone(),
        rsync_timeout: local.put_item.rsync_timeout,
        delete_after_copy: local.put_item.delete_after_copy,
        files_deleted: local.files_deleted,
        qbit_state: live_state,
        added_at: local.added_at,
    }
}

/// Builds a `GetTorrentResponse` for a torrent currently in qBittorrent that this
/// app never tracked (e.g. added directly through qBittorrent)
pub fn untracked_entry(
    name: String,
    hash: String,
    url: String,
    save_path: String,
    progress: f64,
    qbit_state: String,
) -> api::schema::GetTorrentResponse {
    api::schema::GetTorrentResponse {
        name,
        hash,
        url,
        save_path,
        status: format!("Downloading: {:.0}%", progress * 100.0),
        remote_host: String::new(),
        remote_username: String::new(),
        remote_path: String::new(),
        rsync_timeout: 0,
        delete_after_copy: false,
        files_deleted: false,
        qbit_state,
        added_at: 0.0,
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
fn resolve_status(local: &config::settings::RsyncTrack, live_progress: Option<f64>) -> String {
    match local.status {
        config::settings::Status::Copying => "Copying".to_string(),
        config::settings::Status::Transferred => "Transferred".to_string(),
        config::settings::Status::Completed => "Completed".to_string(),
        config::settings::Status::DownloadComplete => "Downloaded".to_string(),
        config::settings::Status::Failed => "Failed".to_string(),
        config::settings::Status::CopyError => "CopyError".to_string(),
        config::settings::Status::Downloading(last_known) => {
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
/// * `success` - Returns a vector of HashMap with `name`, `hash` and `progress` in key-value format.
/// * `failure` - Returns a HttpResponse object with the appropriate response code.
pub async fn get_existing(
    client: &Client,
    config: &config::env::Config,
) -> Result<Vec<squire::qb::Tracker>, HttpResponse> {
    let url = format!("{}/api/v2/torrents/info", config.qbit_url);
    let resp = client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| {
            log::error!("qBit torrents/info failed: {}", e);
            HttpResponse::ServiceUnavailable().body(format!("Failed to reach qBittorrent: {e}"))
        })?;
    let value: Value = resp.json().await.map_err(|e| {
        HttpResponse::BadGateway().body(format!("Invalid response from qBittorrent: {e}"))
    })?;
    let arr = value
        .as_array()
        .ok_or_else(|| HttpResponse::BadGateway().body("Unexpected response from qBittorrent"))?;
    Ok(arr.iter().map(squire::qb::parse_tracker).collect())
}

/// Extends the payload for `PutTorrentRequest` with resolved `name`, `hash` and `trackers`
///
/// # Arguments
///
/// * `body` - Request body that takes `PutTorrentRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// # Returns
///
/// Returns the extended `PutTorrentRequest` with attached `name`, `hash` and `trackers`
pub async fn resolve_payload(
    body: &[api::schema::PutTorrentRequest],
    config: &config::env::Config,
) -> Vec<Result<api::schema::PutTorrentRequest, api::schema::PutTorrentResponse>> {
    let mut ret: Vec<Result<api::schema::PutTorrentRequest, api::schema::PutTorrentResponse>> = Vec::new();
    for item in body.iter() {
        let mut url = match Url::parse(&item.url) {
            Ok(url) => url,
            Err(e) => {
                log::error!("Invalid URL '{}': {}", item.url, e);
                // skip this entry only, keep processing the rest of the batch
                ret.push(Err(api::schema::PutTorrentResponse {
                    name: item.url.clone(),
                    state: format!("Invalid URL: {e}"),
                }));
                continue;
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
        if url.scheme() != "magnet" || hash.is_empty() {
            ret.push(Err(api::schema::PutTorrentResponse {
                name: item.url.clone(),
                state: "Invalid magnet link: expected magnet:?xt=urn:btih:<hash>".into(),
            }));
            continue;
        }
        if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            ret.push(Err(api::schema::PutTorrentResponse {
                name: item.url.clone(),
                state: "Unsupported hash format: expected 40-character hex btih".into(),
            }));
            continue;
        }
        if name.is_empty() {
            name = hash.clone();
        }
        if trackers.is_empty() {
            log::warn!("No trackers found for {}, using search trackers.", name);
            // MARK 1: If trackers are empty; use search trackers to backfill
            let trackers_bak = search::explore::ensure_trackers(config).await;
            trackers.extend(trackers_bak.to_vec());
            // MARK 2: Since the Url is passed to QBit, update its query params with new trackers' list
            log::info!("Adding {} trackers to the source Url", trackers.len());
            let mut pairs = url.query_pairs_mut();
            for tracker in &trackers {
                pairs.append_pair("tr", tracker);
            }
        }
        ret.push(Ok(api::schema::PutTorrentRequest {
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
        }));
    }
    ret
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
/// * `opts` - The parsed `RetryTorrentRequest` (with `redownload == true`).
///
/// # Returns
///
/// Returns an `HttpResponse` indicating the result.
pub async fn redownload_torrent(
    pending: web::Data<config::settings::PendingMap>,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    opts: api::schema::RetryTorrentRequest,
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
        put_item.save_path = squire::savepath::get_default_save_path(&client, &config, &name).await;
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

/// Construct an `HttpResponse` object.
///
/// # Arguments
///
/// * `result` - Result as `Result<api::schema::MullvadResponse, vpn::models::VpnError>` object.
///
/// # Returns
///
/// Returns the final `HttpResponse` object.
pub fn vpn_response(
    result: Result<api::schema::MullvadResponse, vpn::models::VpnError>,
) -> HttpResponse {
    use vpn::models::VpnError::*;
    match result {
        Ok(r) => HttpResponse::Ok().json(r),
        Err(e) => {
            log::error!("VPN request failed: {}", e);
            match e {
                Invalid(m) => HttpResponse::BadRequest().body(m),
                Unavailable(m) => HttpResponse::ServiceUnavailable().body(m),
                Failed(m) => HttpResponse::BadGateway().body(m),
                Timeout(m) => HttpResponse::GatewayTimeout().body(m),
            }
        }
    }
}

/// Lowercased, trimmed, empty entries (e.g. trailing comma) dropped.
pub fn split_codes(raw: &str) -> Option<Vec<String>> {
    let v: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    (!v.is_empty()).then_some(v)
}

/// Get name with hash from the local database.
pub fn get_name_from_state(
    hash: &str,
    db_connection: &config::settings::DBConnection,
) -> Option<String> {
    if let Ok(conn) = db_connection.lock()
        && let Some(rsync_track) = database::db::load_one(&conn, hash)
    {
        return Some(rsync_track.name);
    }
    None
}

/// Get name with hash from the qBit API.
pub async fn get_name_from_qbit(
    hash: &str,
    client: &Client,
    config: &config::env::Config,
) -> Result<Option<String>, HttpResponse> {
    Ok(get_existing(client, config)
        .await?
        .into_iter()
        .find(|t| t.hash.eq_ignore_ascii_case(hash))
        .map(|t| t.name))
}

/// Returns `(name, in_qbit)`; qBit is authoritative, state is the fallback.
pub async fn resolve_name(
    hash: &str,
    client: &Client,
    config: &config::env::Config,
    db_connection: Option<&config::settings::DBConnection>,
) -> Result<Option<(String, bool)>, HttpResponse> {
    if let Some(name) = get_name_from_qbit(hash, client, config).await? {
        return Ok(Some((name, true)));
    }
    Ok(db_connection
        .and_then(|db| get_name_from_state(hash, db))
        .map(|name| (name, false)))
}
