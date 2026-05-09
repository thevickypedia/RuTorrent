use crate::{constant, settings};
use crate::{database, qb};

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use url::Url;
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
/// [{"Sintel":"400"},{"Big Buck Bunny":"409"},{"Ubuntu 22.04 LTS":"200"}]
/// ```
///
/// #### Status
/// * `200`: Successfully queued.
/// * `409`: Duplicate request.
/// * `400`: Invalid magnet link.
///
/// # Returns
///
/// Returns a JSON object to indicate the status.
#[utoipa::path(
    get,
    path = "/torrent",
    responses(
        (status = 200, description = "Torrent status map", body = HashMap<String, String>)
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
    let mut map = HashMap::new();
    let array = get_existing(&client, &config).await;

    if array.is_empty() {
        return HttpResponse::Ok().json(map);
    }

    for t in array.iter() {
        let name = t["name"].to_string();
        let hash = t["hash"].to_string();
        let progress = t["progress"].parse::<f64>().unwrap();

        let status = match db.get(&hash) {
            Some(local) => match local.status {
                settings::Status::Copying(p) => format!("Copying: {:.0}%", p * 100.0),
                settings::Status::Completed => "Completed".to_string(),
                settings::Status::Failed => "Failed".to_string(),
                settings::Status::Downloading(_) => {
                    format!("Downloading: {:.0}%", progress * 100.0)
                }
            },
            None => format!("Downloading: {:.0}%", progress * 100.0),
        };

        map.insert(name, status);
    }

    HttpResponse::Ok().json(map)
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
        let url = Url::parse(&item.url).expect("Invalid URL");
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
            delete_after_copy: item.delete_after_copy,
        });
    }
    ret
}

/// API endpoint to add torrents to the download queue.
///
/// # Arguments
///
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
    db_conn: web::Data<settings::DbConn>,
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
    for item in resolve_payload(&body.into_inner()) {
        let tag = Uuid::new_v4().to_string();
        let url = item.url.to_string();
        let name = item.name.as_ref().unwrap().to_string();
        let hash = item.hash.as_ref().unwrap().to_uppercase().to_string();
        let trackers = item.trackers.as_ref().unwrap().to_vec();
        let save_path = item.save_path.to_string();

        if hashes.contains(&hash) {
            response.push(HashMap::from([(name, 409.to_string())]));
            continue;
        }

        log::info!(
            "Adding torrent [{}]: {}, trackers: {}",
            tag,
            &name,
            trackers.len()
        );

        let mut params = vec![("urls", &url), ("tags", &tag)];
        if !save_path.is_empty() {
            params.push(("savepath", &save_path));
        }

        let resp = client
            .post(format!("{}/api/v2/torrents/add", config.qbit_url))
            .form(&params)
            .send()
            .await;

        if let Err(e) = qb::handle_response(resp, "ADD torrent").await {
            log::error!("{:?}", e.status().to_string());
            response.push(HashMap::from([(name, 400.to_string())]));
            continue;
        }

        // Only keep rsync info if ALL fields are present
        if !item.remote_host.is_empty()
            && !item.remote_username.is_empty()
            && !item.remote_path.is_empty()
        {
            pending_lock.insert(tag.clone(), item.clone());
            if let Ok(conn) = db_conn.lock() {
                database::upsert_pending(&conn, &tag, &item);
            }
        } else {
            log::info!("Adding torrent [{}]: {}", tag, item.url);
        };
        response.push(HashMap::from([(name, 200.to_string())]));
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
        &delete_files
    );

    let resp = client
        .post(format!("{}/api/v2/torrents/delete", config.qbit_url))
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
