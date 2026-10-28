use std::collections::HashMap;
use actix_web::{web, HttpRequest, Responder, HttpResponse};
use uuid::Uuid;
use crate::{api, config, database, squire};

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
/// Returns a JSON array of `GetTorrentResponse` objects.
#[utoipa::path(
    get,
    tag = "Torrent",
    path = "/torrent",
    responses(
        (status = 200, description = "Torrent list", body = Vec<api::schema::GetTorrentResponse>),
        (status = 502, description = "mullvad CLI missing or relay list unavailable", body = String),
        (status = 503, description = "VPN did not reach the requested state", body = String),
    )
)]
pub async fn get(
    request: HttpRequest,
    db_connection: web::Data<config::settings::DBConnection>,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let mut out: Vec<api::schema::GetTorrentResponse> = Vec::new();
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
/// * `body` - Request body that takes `PutTorrentRequest` object.
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
    request_body = Vec<api::schema::PutTorrentRequest>,
    responses(
        (status = 200, description = "A vector of PutTorrentResponse object with each torrent's name and state", body = Vec<api::schema::PutTorrentResponse>)
    )
)]
pub async fn put(
    request: HttpRequest,
    pending: web::Data<config::settings::PendingMap>,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    body: web::Json<Vec<api::schema::PutTorrentRequest>>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let client = match squire::qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

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

    let mut response: Vec<api::schema::PutTorrentResponse> = Vec::new();
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
            response.push(api::schema::PutTorrentResponse {
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
            response.push(api::schema::PutTorrentResponse { name, state });
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
        pending.write().await.insert(tag.clone(), item.clone());
        let Ok(conn) = db_connection.lock() else {
            log::error!("Failed to lock database connection");
            response.push(api::schema::PutTorrentResponse {
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
        response.push(api::schema::PutTorrentResponse {
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
/// Deleted <name> [<hash>]
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
pub async fn delete(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
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
        match api::squire::resolve_name(&hash, &client, &config, Some(db_connection.get_ref()))
            .await
        {
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

/// API endpoint to RETRY a failed download OR transfer.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `state` - Reference to the `SharedState` object.
/// * `pending` - Reference to the `PendingMap` object (used for `redownload`).
/// * `config` - Reference to the `Config` object.
/// * `db_connection` - Database connection received through app data (used for `redownload`).
/// * `body` - Request body that takes `RetryTorrentRequest` object.
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
    request_body = api::schema::RetryTorrentRequest,
    responses(
        (status = 200, description = "Retry queued", body = String),
        (status = 400, description = "Not in a retriable state", body = String),
        (status = 404, description = "Not found", body = String),
    )
)]
pub async fn retry(
    request: HttpRequest,
    pending: web::Data<config::settings::PendingMap>,
    config: web::Data<config::env::Config>,
    db_connection: web::Data<config::settings::DBConnection>,
    mut body: web::Json<api::schema::RetryTorrentRequest>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }

    if body.hash.is_empty() {
        return HttpResponse::BadRequest().body("Missing hash");
    }
    body.hash = body.hash.trim().to_lowercase();

    if body.redownload {
        return api::squire::redownload_torrent(pending, config, db_connection, body.into_inner()).await;
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
            name,
            body.hash
        );
        return api::squire::redownload_torrent(pending, config, db_connection, body.into_inner()).await;
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

/// API endpoint to PAUSE OR RESUME a torrent in qBittorrent.
///
/// #### Sample Request
/// ```shell
/// curl -X PATCH localhost:3000/torrent?hash=2C..85&pause=true
/// ```
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
pub async fn pause(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
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

    let (name, in_qbit) = match api::squire::resolve_name(&hash, &client, &config, None).await {
        Ok(Some(found)) => found,
        Ok(None) => return HttpResponse::NotFound().body("Torrent not found"),
        Err(e) => return e,
    };
    if !in_qbit {
        return HttpResponse::NotFound().body("Torrent is not in qBittorrent");
    }

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
