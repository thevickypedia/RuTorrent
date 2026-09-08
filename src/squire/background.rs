use crate::{config, database, notifier, squire};
use reqwest::Client;
use serde_json::Value;
use tokio::time::{sleep, Duration};

/// Recursively removes empty directories under (and including) `path`,
/// without ever touching a directory that still contains something.
///
/// qBittorrent's own `deleteFiles=true` only removes the torrent's actual
/// content (its own managed root), which can leave a now-empty custom
/// `save_path` wrapper directory behind (e.g. `save_path` was set to
/// `~/Downloads/Sintel` and only `~/Downloads/Sintel/Sintel` was removed).
/// This walks `path` bottom-up and deletes anything that is fully empty,
/// starting only from `path` itself — it never ascends above `path`, so a
/// shared parent (e.g. `~/Downloads`) that still holds other torrents' data
/// is never at risk.
///
/// # Arguments
///
/// * `path` - Directory to prune, typically a torrent's `save_path`.
///
/// # Returns
///
/// Returns `true` if `path` itself was empty (and thus removed).
fn prune_empty_dirs(path: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(path) else {
        // Doesn't exist (already gone) or isn't a directory — nothing to do.
        return false;
    };

    let mut is_empty = true;
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            if !prune_empty_dirs(&entry_path) {
                is_empty = false;
            }
        } else {
            is_empty = false;
        }
    }

    if is_empty {
        match std::fs::remove_dir(path) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("Failed to remove empty directory {:?}: {}", path, err);
                false
            }
        }
    } else {
        false
    }
}

/// Resolves newly added torrents by matching them with pending entries and inserting them into shared state.
///
/// Iterates over every torrent currently in qBittorrent and ensures each one
/// is present in the DB and in-memory state. Torrents that match a pending tag
/// are resolved with their full metadata; all others are auto-tracked as
/// download-only entries. This guarantees that **everything in qBit is in the DB**.
///
/// # Arguments
///
/// * `array` - Array of existing torrents in QBitAPI.
/// * `pending` - Shared map of pending torrent metadata keyed by tags.
/// * `state` - Shared state where active torrent tracking entries are stored.
/// * `db_connection` - Database connection received through app data.
async fn resolve_new_torrents(
    array: &Vec<Value>,
    pending: &config::settings::PendingMap,
    db_connection: &config::settings::DBConnection,
) {
    let mut pending_lock = pending.write().await;

    let Ok(conn) = db_connection.lock() else {
        log::error!("Failed to lock database connection");
        return;
    };
    let mut existing = database::db::load_all(&conn);

    for t in array {
        // MARK: To generate JSON for different stages, in-case we need more fields parsed
        // use std::fs::File;
        // use std::io::BufWriter;
        // let file = File::create("value.json").unwrap();
        // let writer = BufWriter::new(file);
        // serde_json::to_writer(writer, t).unwrap();
        let torrent = squire::qb::parse_tracker(t);
        // Already tracked — nothing to do
        if existing.contains_key(&torrent.hash) {
            continue;
        }

        let matched_tag = torrent.tags
            .split(',')
            .map(str::trim)
            .find(|tag| pending_lock.contains_key(*tag));

        let item = if let Some(tag) = matched_tag {
            log::info!("Resolved {} → {}", &torrent.name, &torrent.hash);
            pending_lock.remove(tag).unwrap()
        } else {
            // Torrent exists in qBit but has no pending entry — auto-track it
            // so that the DB is always a superset of what qBit knows about.
            log::info!("Auto-tracking torrent found in QBit (not in DB): {}", torrent.name);
            config::settings::PutItem {
                url: torrent.magnet_uri,
                name: Some(torrent.name.clone()),
                hash: Some(torrent.hash.clone()),
                trackers: None,
                save_path: torrent.save_path,
                remote_host: String::new(),
                remote_username: String::new(),
                remote_path: String::new(),
                rsync_timeout: 3,
                delete_after_copy: false,
            }
        };

        let entry = config::settings::RsyncTrack {
            name: torrent.name,
            status: config::settings::Status::Downloading(0.0),
            put_item: item,
            in_qbit: true,
            files_deleted: false,
        };

        if let Some(tag) = matched_tag {
            database::db::remove_pending(&conn, tag);
        }
        database::db::upsert(&conn, &torrent.hash, &entry);
        existing.insert(torrent.hash, entry);
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
fn notifier(title: String, body: String, config: config::env::Config) {
    let title_clone = title.clone();
    let body_clone = body.clone();
    let config_clone = config.clone();
    if !config.ntfy_url.is_empty() && !config.ntfy_topic.is_empty() {
        log::info!("Sending NTFY notification to {}: {}", title_clone, body);
        tokio::spawn(async move {
            let _ = notifier::ntfy::send(&config, &title, &body).await;
        });
    }
    if !config_clone.telegram_bot_token.is_empty() && !config_clone.telegram_chat_id.is_empty() {
        log::info!(
            "Sending Telegram notification to {}: {}",
            title_clone,
            body_clone
        );
        tokio::spawn(async move {
            let message = format!("*{}*\n\n{}", title_clone, body_clone);
            let _ = notifier::telegram::send(&config_clone, &message).await;
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
/// - On every tick, scans **all** torrents in qBittorrent and ensures each one
///   is present in the DB (auto-tracking any that are missing).
/// - Scans all DB entries whose `in_qbit` flag is set and syncs their status
///   from the live qBittorrent API; entries no longer found in qBit have their
///   `in_qbit` flag cleared but are otherwise left intact.
/// - Everything present in qBittorrent is guaranteed to exist in the DB.
/// - Spawns separate async tasks for rsync operations.
/// - Sleeps between polling cycles to avoid excessive API calls.
pub fn spawn_worker(
    mut client: Client,
    pending: config::settings::PendingMap,
    config: config::env::Config,
    db_connection: config::settings::DBConnection,
) {
    let mut n = 0;
    let max_auth_errors = 30;
    let interval = Duration::from_secs(5);

    tokio::spawn(async move {
        log::info!("Worker started");

        loop {
            sleep(interval).await;
            n += 1;

            /* 1. Fetch all torrents from qBit; auto-insert anything new. */
            if let Some(response) =
                squire::misc::qb_get(&client, format!("{}/api/v2/torrents/info", config.qbit_url))
                    .await
            {
                let Some(array) = response.as_array() else {
                    log::warn!("No info received from QBitAPI");
                    continue;
                };
                log::trace!("Torrents active: {:?}", array);
                resolve_new_torrents(array, &pending, &db_connection).await;
            } else {
                log::error!("Failed to get info from QBitAPI");
                client = match squire::qb::client(&config).await {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!(
                            "Failed to authenticate qBittorrent: {:?} on {}-th attempt",
                            e,
                            n
                        );
                        if n > max_auth_errors {
                            return;
                        } else {
                            continue;
                        }
                    }
                };
                continue;
            }

            /* 2. Sync status for all DB entries still marked in_qbit. */
            let db_hashes: Vec<String> = {
                let Ok(conn) = db_connection.lock() else {
                    continue;
                };
                database::db::load_all(&conn)
                    .into_iter()
                    .filter(|(_, v)| v.in_qbit)
                    .map(|(h, _)| h)
                    .collect()
            };

            if db_hashes.is_empty() {
                continue;
            }

            let url = format!(
                "{}/api/v2/torrents/info?hashes={}",
                config.qbit_url,
                db_hashes.join("|")
            );

            let Some(resp) = squire::misc::qb_get(&client, url).await else {
                continue;
            };
            let Some(arr) = resp.as_array() else { continue };

            // Entries qBit no longer knows about: flag in_qbit = false.
            let trackers: Vec<squire::qb::Tracker> = arr.iter().map(squire::qb::parse_tracker).collect();
            let returned: std::collections::HashSet<&str> =
                trackers.iter().map(|t| t.hash.as_str()).collect();
            if let Ok(conn) = db_connection.lock() {
                for h in db_hashes.iter().filter(|h| !returned.contains(h.as_str())) {
                    log::info!("Torrent removed from QBitAPI, keeping DB record: {}", h);
                    if let Some(mut entry) = database::db::load_one(&conn, h) {
                        entry.in_qbit = false;
                        database::db::upsert(&conn, h, &entry);
                    }
                }
            }

            for torrent in trackers {
                let mut entry = {
                    let Ok(conn) = db_connection.lock() else { continue };
                    match database::db::load_one(&conn, &torrent.hash) {
                        Some(e) => e,
                        None => continue,
                    }
                };

                if torrent.state.as_str() == "error" {
                    if !matches!(entry.status, config::settings::Status::Failed) {
                        log::error!("Download errored for {}: {}", entry.name, torrent.state);
                        entry.status = config::settings::Status::Failed;
                        if let Ok(conn) = db_connection.lock() {
                            database::db::upsert(&conn, &torrent.hash, &entry);
                        }
                        notifier(
                            "RuTorrent: Download Error".to_string(),
                            format!("Download errored for {}", entry.name),
                            config.clone(),
                        );
                    }
                } else if !matches!(
                    entry.status,
                    config::settings::Status::Transferred
                        | config::settings::Status::Completed
                        | config::settings::Status::Copying
                        | config::settings::Status::DownloadComplete
                ) {
                    let download_complete = matches!(
                        torrent.state.as_str(),
                        "uploading"
                            | "stalledUP"
                            | "pausedUP"
                            | "queuedUP"
                            | "forcedUP"
                            | "checkingUP"
                    );
                    if download_complete {
                        let has_rsync = !entry.put_item.remote_host.is_empty()
                            && !entry.put_item.remote_username.is_empty()
                            && !entry.put_item.remote_path.is_empty();
                        if has_rsync {
                            log::info!("Download complete → rsync: {}", entry.name);
                            entry.status = config::settings::Status::Copying;
                            if let Ok(conn) = db_connection.lock() {
                                database::db::upsert(&conn, &torrent.hash, &entry);
                            }
                            let db_connection_clone = db_connection.clone();
                            let hash_clone = torrent.hash.clone();
                            let name_clone = entry.name.clone();
                            let put_item_clone = entry.put_item.clone();
                            tokio::spawn(async move {
                                squire::rsync::run(
                                    db_connection_clone,
                                    hash_clone,
                                    name_clone,
                                    put_item_clone,
                                )
                                .await;
                            });
                            notifier(
                                "RuTorrent: Download Complete".to_string(),
                                format!("{} has been downloaded", entry.name),
                                config.clone(),
                            );
                        } else {
                            log::info!("Download complete (no rsync): {}", entry.name);
                            entry.status = config::settings::Status::DownloadComplete;
                            if let Ok(conn) = db_connection.lock() {
                                database::db::upsert(&conn, &torrent.hash, &entry);
                            }
                            notifier(
                                "RuTorrent: Download Complete".to_string(),
                                format!("{} has been downloaded", entry.name),
                                config.clone(),
                            );
                        }
                    } else {
                        entry.status = config::settings::Status::Downloading(torrent.progress);
                        if let Ok(conn) = db_connection.lock() {
                            database::db::upsert(&conn, &torrent.hash, &entry);
                        }
                    }
                }

                if matches!(entry.status, config::settings::Status::Completed) {
                    let name_clone = entry.name.clone();
                    let put_item_clone = entry.put_item.clone();
                    notifier(
                        "RuTorrent: Transfer Complete".to_string(),
                        format!(
                            "{} has been transferred to {}",
                            name_clone, put_item_clone.remote_host
                        ),
                        config.clone(),
                    );
                    if put_item_clone.delete_after_copy {
                        let resp = client
                            .post(format!("{}/api/v2/torrents/delete", config.qbit_url))
                            .form(&[("hashes", torrent.hash.as_str()), ("deleteFiles", "true")])
                            .send()
                            .await;
                        let mut files_deleted = true;
                        if let Err(e) = squire::qb::handle_response(
                            resp,
                            squire::qb::ResponseContext::DeleteTorrent,
                        )
                            .await
                        {
                            log::error!("Failed to delete torrent: {}", e.status());
                            if std::path::Path::new(&entry.put_item.save_path).exists()
                                && let Err(err) =
                                std::fs::remove_dir_all(&entry.put_item.save_path)
                            {
                                log::error!("Failed to delete files: {}", err);
                                files_deleted = false;
                                notifier(
                                    "RuTorrent: Delete Failed".to_string(),
                                    format!("Failed to delete torrent: {}", name_clone),
                                    config.clone(),
                                );
                            }
                        }
                        if files_deleted {
                            prune_empty_dirs(std::path::Path::new(&entry.put_item.save_path));
                        }
                        entry.status = config::settings::Status::Transferred;
                        entry.in_qbit = false;
                        entry.files_deleted = files_deleted;
                    } else {
                        entry.status = config::settings::Status::Transferred;
                    }
                    if let Ok(conn) = db_connection.lock() {
                        database::db::upsert(&conn, &torrent.hash, &entry);
                    }
                }
            }
        }
    });
}
