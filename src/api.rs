use actix_web::{web, HttpResponse, Responder};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;
use crate::settings;
use crate::qb;
/* -----------------------------
   GET /torrent
------------------------------*/

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

/* -----------------------------
   PUT /torrent (FIXED)
------------------------------*/
pub async fn put_torrent(
    pending: web::Data<settings::PendingMap>,
    config: web::Data<settings::Config>,
    req: web::Json<Vec<settings::PutItem>>,
) -> impl Responder {
    let client = match qb::client(&config).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let mut pending_lock = pending.write().await;

    for item in req.iter() {
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

    HttpResponse::Ok().body("Queued")
}

/* -----------------------------
   DELETE /torrent
------------------------------*/
pub async fn delete_torrent(
    config: web::Data<settings::Config>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    log::info!("DELETE /torrent");

    let identifier = match query.get("name") {
        Some(i) => i,
        None => return HttpResponse::BadRequest().body("Missing name"),
    };

    let delete_files = match query.get("files") {
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
    HttpResponse::Ok().body("Deleted")
}
