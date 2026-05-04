use actix_web::{web, HttpResponse, Responder};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;
use crate::settings;
use crate::qb;


/// API endpoint to get download/copy status.
///
/// # Arguments
///
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
/// {
///   "Ubuntu 22.04 LTS": "Downloading: 39%",
///   "Sintel": "Copying 69%"
/// }
/// ```
///
/// # Returns
///
/// Returns a JSON object to indicate the status.
pub async fn get_torrents(
    state: web::Data<settings::SharedState>,
    config: web::Data<settings::Config>,
) -> impl Responder {
    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let resp: Value = match client
        .get(format!("{}/api/v2/torrents/info", config.base_url))
        .send()
        .await
    {
        Ok(r) => r.json().await.unwrap_or(Value::Null),
        Err(_) => Value::Null,
    };

    let db = state.read().await;
    let mut map = HashMap::new();

    if let Some(arr) = resp.as_array() {
        for t in arr {
            let name = t["name"].as_str().unwrap_or("?").to_string();
            let hash = t["hash"].as_str().unwrap_or("").to_string();
            let progress = t["progress"].as_f64().unwrap_or(0.0);

            let status = match db.get(&hash) {
                Some(local) => match local.status {
                    settings::Status::Copying(p) => format!("Copying: {:.0}%", p * 100.0),
                    settings::Status::Completed => "Completed".to_string(),
                    settings::Status::Downloading(_) => {
                        format!("Downloading: {:.0}%", progress * 100.0)
                    }
                },
                None => format!("Downloading: {:.0}%", progress * 100.0),
            };

            map.insert(name, status);
        }
    }

    HttpResponse::Ok().json(map)
}

/// API endpoint to add torrents to the download queue.
///
/// # Arguments
///
/// * `pending` - Reference to the `PendingMap` object.
/// * `config` - Reference to the `Config` object.
/// * `body` - Request body that takes `PutItem` object.
///
/// #### Sample Request
/// ```shell
/// curl -X PUT localhost:3000/torrent \
///   -H "Content-Type: application/json" \
///   -d '[
///     # Download and transfer content to ssh://admin@192.168.1.102:/Users/admin/Downloads
///     {
///       "url": "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel",
///       "host": "192.168.1.102",
///       "username": "admin",
///       "path": "/Users/admin/Downloads"
///     },
///     # Download and transfer content to ssh://admin@192.168.1.100:/home/admin/Documents
///     {
///       "url": "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny",
///       "host": "192.168.1.100",
///       "username": "admin",
///       "path": "/home/admin/Documents"
///     },
///     # Download without any subsequent transfer
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
pub async fn put_torrent(
    pending: web::Data<settings::PendingMap>,
    config: web::Data<settings::Config>,
    body: web::Json<Vec<settings::PutItem>>,
) -> impl Responder {
    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let mut pending_lock = pending.write().await;

    for item in body.iter() {
        let tag = Uuid::new_v4().to_string();

        log::info!("Adding torrent [{}]: {}", tag, item.url);

        let resp = client
            .post(format!("{}/api/v2/torrents/add", config.base_url))
            .form(&[("urls", item.url.as_str()), ("tags", tag.as_str())])
            .send()
            .await;

        if let Err(e) = qb::handle_response(resp, "ADD torrent").await {
            return e;
        }

        // Only keep rsync info if ALL fields are present
        if !item.host.is_empty()
            && !item.username.is_empty()
            && !item.path.is_empty()
        {
            pending_lock.insert(tag, item.clone());
        } else {
            log::info!("Adding torrent [{}]: {}", tag, item.url);
        };
    }

    HttpResponse::Ok().json("Queued")
}

/// API endpoint to delete a torrent.
///
/// # Arguments
///
/// * `state` - Reference to the `SharedState` object.
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
pub async fn delete_torrent(
    config: web::Data<settings::Config>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
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
        .get(format!("{}/api/v2/torrents/info", config.base_url))
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
        &delete_files
    );

    let resp = client
        .post(format!("{}/api/v2/torrents/delete", config.base_url))
        .form(&[
            ("hashes", hash.as_str()),
            ("deleteFiles", delete_files.to_string().as_str()),
        ])
        .send()
        .await;

    if let Err(e) = qb::handle_response(resp, "DELETE torrent").await {
        return e;
    }

    log::info!("Successfully deleted {}", identifier);
    HttpResponse::Ok().json("Deleted")
}
