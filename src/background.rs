use crate::{database, ntfy, qb, rsync, settings, squire, telegram};
use reqwest::Client;
use serde_json::Value;
use tokio::time::{sleep, Duration};

/// Resolves newly added torrents by matching them with pending entries and inserting them into shared state.
///
/// # Arguments
///
/// * `array` - Array of existing torrents in QBitAPI.
/// * `pending` - Shared map of pending torrent metadata keyed by tags.
/// * `state` - Shared state where active torrent tracking entries are stored.
/// * `db_connection` - Database connection received through app data.
async fn resolve_new_torrents(
    array: &Vec<Value>,
    pending: &settings::PendingMap,
    state: &settings::SharedState,
    db_connection: &settings::DBConnection,
) {
    let mut pending_lock = pending.write().await;
    let mut db = state.write().await;

    for t in array {
        let hash = t["hash"].as_str().unwrap_or("").to_string();
        let name = t["name"].as_str().unwrap_or("").to_string();
        let tags = t["tags"].as_str().unwrap_or("");

        if db.contains_key(&hash) {
            continue;
        }

        let matched_tag = tags
            .split(',')
            .map(str::trim)
            .find(|tag| pending_lock.contains_key(*tag));

        if let Some(tag) = matched_tag {
            let item = pending_lock.remove(tag).unwrap();
            log::info!("Resolved {} → {}", name, hash);

            db.insert(
                hash.clone(),
                settings::RsyncTrack {
                    name,
                    status: settings::Status::Downloading(0.0),
                    put_item: item,
                },
            );
            if let Ok(conn) = db_connection.lock() {
                database::remove_pending(&conn, tag);
                database::upsert(&conn, &hash, db.get(&hash).unwrap());
            }
        }
    }
}

/// Function to notify about an event.
///
/// # Arguments
///
/// * `title` - Subject of the notification.
/// * `body` - Body of the notification.
/// * `config` - Reference to the `Config` object.
///
/// # Notes
///
/// Sends notifications through `NTFY` and `Telegram` based on the availability of env vars.
fn notifier(title: String, body: String, config: settings::Config) {
    let title_clone = title.clone();
    let body_clone = body.clone();
    let config_clone = config.clone();
    if !config.ntfy_url.is_empty() && !config.ntfy_topic.is_empty() {
        log::info!("Sending NTFY notification to {}: {}", title_clone, body);
        tokio::spawn(async move {
            let _ = ntfy::send(&config, &title, &body).await;
        });
    }
    if !config_clone.telegram_bot_token.is_empty() && !config_clone.telegram_chat_id.is_empty() {
        log::info!(
            "Sending Telegram notification to {}: {}",
            title_clone,
            body_clone
        );
        tokio::spawn(async move {
            let message = format!("*{}*\n\n{}", &title_clone, &body_clone);
            let _ = telegram::send(&config_clone, &message).await;
        });
    }
}

/// Spawns a background worker that monitors torrents and triggers rsync transfers upon completion.
///
/// # Arguments
///
/// * `client` - Authenticated HTTP client for qBittorrent API requests.
/// * `state` - Shared state used to track torrent and transfer progress.
/// * `pending` - Shared map of pending torrent metadata.
/// * `config` - Application configuration containing API settings.
/// * `db_connection` - Database connection received through app data.
///
/// # Notes
///
/// - Runs an infinite loop that periodically polls torrent status.
/// - Updates download progress and transitions completed torrents to rsync transfers.
/// - Spawns separate async tasks for rsync operations.
/// - Sleeps between polling cycles to avoid excessive API calls.
pub fn spawn_worker(
    mut client: Client,
    state: settings::SharedState,
    pending: settings::PendingMap,
    config: settings::Config,
    db_connection: settings::DBConnection,
) {
    tokio::spawn(async move {
        log::info!("Worker started");

        loop {
            sleep(Duration::from_secs(5)).await;

            // Skip all API calls when there is nothing to track.
            {
                let p = pending.read().await;
                let s = state.read().await;
                if p.is_empty() && s.is_empty() {
                    continue;
                }
            }

            // Check status of client and re-auth if request fails
            if let Some(response) =
                squire::qb_get(&client, format!("{}/api/v2/torrents/info", config.qbit_url)).await
            {
                /* -----------------------------
                   1. Resolve pending torrents
                ------------------------------*/
                let Some(array) = response.as_array() else {
                    log::warn!("No info received from QBitAPI");
                    continue;
                };

                log::trace!("Torrents active: {:?}", array);
                resolve_new_torrents(array, &pending, &state, &db_connection).await;
            } else {
                log::error!("Failed to get info from QBitAPI");

                // Re-create client when failed to authenticate
                client = match qb::client(&config).await {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("Failed to authenticate qBittorrent: {:?}", e);
                        return;
                    }
                };

                continue;
            }

            /* -----------------------------
               2. Poll tracked torrents
            ------------------------------*/
            let hashes: Vec<String> = {
                let db = state.read().await;
                db.keys().cloned().collect()
            };

            if hashes.is_empty() {
                continue;
            }

            let url = format!(
                "{}/api/v2/torrents/info?hashes={}",
                config.qbit_url,
                hashes.join("|")
            );

            let Some(resp) = squire::qb_get(&client, url).await else {
                continue;
            };

            let Some(arr) = resp.as_array() else {
                continue;
            };

            let mut db = state.write().await;

            // Remove entries that QBitAPI no longer knows about (deleted via WebUI).
            let returned: std::collections::HashSet<&str> =
                arr.iter().filter_map(|t| t["hash"].as_str()).collect();
            hashes.iter().for_each(|h| {
                if !returned.contains(h.as_str()) {
                    log::info!("Torrent removed from QBitAPI, dropping from state: {}", h);
                    db.remove(h);
                    if let Ok(conn) = db_connection.lock() {
                        database::remove(&conn, h);
                    }
                }
            });

            for t in arr {
                let hash = t["hash"].as_str().unwrap_or("").to_string();

                let Some(entry) = db.get_mut(&hash) else {
                    continue;
                };

                match entry.status {
                    settings::Status::Copying(_) => continue,

                    settings::Status::Failed => {
                        let config_cloned = config.clone();
                        let name_clone = entry.name.clone();
                        let put_item_clone = entry.put_item.clone();
                        notifier(
                            "RuTorrent: Transfer Failed".to_string(),
                            format!(
                                "Failed to transfer {} to {}",
                                name_clone, put_item_clone.remote_host
                            ),
                            config_cloned,
                        );
                        db.remove(&hash);
                        if let Ok(conn) = db_connection.lock() {
                            database::remove(&conn, &hash);
                        }
                    }

                    settings::Status::Completed => {
                        let config_cloned = config.clone();
                        let name_clone = entry.name.clone();
                        let put_item_clone = entry.put_item.clone();
                        notifier(
                            "RuTorrent: Transfer Complete".to_string(),
                            format!(
                                "{} has been transferred to {}",
                                name_clone, put_item_clone.remote_host
                            ),
                            config_cloned,
                        );
                        if put_item_clone.delete_after_copy {
                            let resp = client
                                .post(format!("{}/api/v2/torrents/delete", config.qbit_url))
                                .form(&[("hashes", hash.as_str()), ("deleteFiles", "true")])
                                .send()
                                .await;
                            if let Err(e) = qb::handle_response(resp, "DELETE torrent").await {
                                log::error!("Failed to delete torrent: {}", e.status());
                                if std::path::Path::new(&entry.put_item.save_path).exists()
                                    && let Err(err) =
                                        std::fs::remove_dir_all(&entry.put_item.save_path)
                                {
                                    log::error!("Failed to delete files: {}", err);
                                    notifier(
                                        "RuTorrent: Delete Failed".to_string(),
                                        format!("Failed to delete torrent: {}", name_clone),
                                        config.to_owned(),
                                    );
                                }
                            }
                        }
                        db.remove(&hash);
                        if let Ok(conn) = db_connection.lock() {
                            database::remove(&conn, &hash);
                        }
                    }

                    settings::Status::Downloading(_) => {
                        let progress = t["progress"].as_f64().unwrap_or(0.0);
                        entry.status = settings::Status::Downloading(progress);
                        if progress >= 1.0 {
                            log::info!("Download complete → rsync: {}", entry.name);
                            entry.status = settings::Status::Copying(0.0);
                            let state_clone = state.clone();
                            let hash_clone = hash.clone();
                            let name_clone = entry.name.clone();
                            let put_item_clone = entry.put_item.clone();
                            // Kick off transfer in the background
                            tokio::spawn(async move {
                                rsync::run(state_clone, hash_clone, name_clone, put_item_clone)
                                    .await;
                            });
                            // Kick off download complete notification in the background
                            let config_cloned = config.clone();
                            let name_clone = entry.name.clone();
                            notifier(
                                "RuTorrent: Download Complete".to_string(),
                                format!("{} has been downloaded", name_clone),
                                config_cloned,
                            );
                        }
                    }
                }
            }

            drop(db);
        }
    });
}
