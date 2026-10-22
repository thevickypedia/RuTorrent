use crate::{api, config, search, squire, vpn};

use actix_web::HttpResponse;
use reqwest::Client;
use serde_json::Value;
use url::Url;

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
pub fn to_entry(
    hash: &str,
    local: &config::settings::RsyncTrack,
    live_progress: Option<f64>,
    live_state: String,
) -> api::schema::TorrentEntry {
    api::schema::TorrentEntry {
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

/// Builds a [`TorrentEntry`] for a torrent currently in qBittorrent that this
/// app never tracked (e.g. added directly through qBittorrent)
pub fn untracked_entry(
    name: String,
    hash: String,
    url: String,
    save_path: String,
    progress: f64,
    qbit_state: String,
) -> api::schema::TorrentEntry {
    api::schema::TorrentEntry {
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

/// Extends the payload for `PutItem` with resolved `name`, `hash` and `trackers`
///
/// # Arguments
///
/// * `body` - Request body that takes `PutItem` object.
/// * `config` - Reference to the `Config` object.
///
/// # Returns
///
/// Returns the extended `PutItem` with attached `name`, `hash` and `trackers`
pub async fn resolve_payload(
    body: &[config::settings::PutItem],
    config: &config::env::Config,
) -> Vec<Result<config::settings::PutItem, config::settings::PutResponse>> {
    let mut ret: Vec<Result<config::settings::PutItem, config::settings::PutResponse>> = Vec::new();
    for item in body.iter() {
        let mut url = match Url::parse(&item.url) {
            Ok(url) => url,
            Err(e) => {
                log::error!("Invalid URL '{}': {}", item.url, e);
                // skip this entry only, keep processing the rest of the batch
                ret.push(Err(config::settings::PutResponse {
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
            ret.push(Err(config::settings::PutResponse {
                name: item.url.clone(),
                state: "Invalid magnet link: expected magnet:?xt=urn:btih:<hash>".into(),
            }));
            continue;
        }
        if trackers.is_empty() {
            let ident = if name.is_empty() {
                &item.url.to_string()
            } else {
                &name
            };
            log::warn!("No trackers found for {}, using search trackers.", ident);
            // MARK 1: If trackers are empty; use search trackers to backfill
            let trackers_bak = search::api::ensure_trackers(config).await;
            trackers.extend(trackers_bak.to_vec());
            // MARK 2: Since the Url is passed to QBit, update its query params with new trackers' list
            log::info!("Adding {} trackers to the source Url", trackers.len());
            let mut pairs = url.query_pairs_mut();
            for tracker in &trackers {
                pairs.append_pair("tr", tracker);
            }
        }
        ret.push(Ok(config::settings::PutItem {
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

pub fn vpn_response(
    result: Result<vpn::models::MullvadResponse, vpn::models::VpnError>,
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
