use crate::{rsync, settings};
use reqwest::Client;
use serde_json::Value;
use tokio::time::{sleep, Duration};

/// Sends a GET request to the qBittorrent API and parses the JSON response.
///
/// # Arguments
///
/// * `client` - The HTTP client used to perform the request
/// * `url` - The full API endpoint URL
///
/// # Returns
///
/// * `Some(Value)` - Parsed JSON response if the request succeeds
/// * `None` - If the request fails or the response cannot be parsed
async fn qb_get(client: &Client, url: String) -> Option<Value> {
    match client.get(&url).send().await {
        Ok(r) => r.json().await.ok(),
        Err(e) => {
            log::warn!("qB GET error: {}", e);
            None
        }
    }
}

/// Resolves newly added torrents by matching them with pending entries and inserting them into shared state.
///
/// # Arguments
///
/// * `client` - The HTTP client used to query the qBittorrent API
/// * `config` - Application configuration containing the API base URL
/// * `pending` - Shared map of pending torrent metadata keyed by tags
/// * `state` - Shared state where active torrent tracking entries are stored
async fn resolve_new_torrents(
    client: &Client,
    config: &settings::Config,
    pending: &settings::PendingMap,
    state: &settings::SharedState,
) {
    let resp = qb_get(client, format!("{}/api/v2/torrents/info", config.base_url)).await;

    let Some(arr) = resp.and_then(|v| v.as_array().cloned()) else {
        return;
    };

    let mut pending_lock = pending.write().await;
    let mut db = state.write().await;

    for t in arr {
        let hash = t["hash"].as_str().unwrap_or("").to_string();
        let name = t["name"].as_str().unwrap_or("").to_string();
        let tags = t["tags"].as_str().unwrap_or("");

        if db.contains_key(&hash) {
            continue;
        }

        if let Some(item) = pending_lock.remove(tags) {
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
        }
    }
}

/// Spawns a background worker that monitors torrents and triggers rsync transfers upon completion.
///
/// # Arguments
///
/// * `client` - Authenticated HTTP client for qBittorrent API requests
/// * `state` - Shared state used to track torrent and transfer progress
/// * `pending` - Shared map of pending torrent metadata
/// * `config` - Application configuration containing API settings
///
/// # Notes
///
/// - Runs an infinite loop that periodically polls torrent status.
/// - Updates download progress and transitions completed torrents to rsync transfers.
/// - Spawns separate async tasks for rsync operations.
/// - Sleeps between polling cycles to avoid excessive API calls.
pub fn spawn_worker(
    client: Client,
    state: settings::SharedState,
    pending: settings::PendingMap,
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
