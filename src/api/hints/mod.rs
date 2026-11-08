use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use actix_web::{web, HttpRequest, Responder, HttpResponse};
use crate::{api, config};

fn getter(config: &config::env::Config) -> Result<api::schema::Hints, Box<dyn std::error::Error>> {
    let file = File::open(&config.hints)?;
    let reader = BufReader::new(file);
    let hints_response: api::schema::Hints = serde_json::from_reader(reader)?;
    log::info!("Hints: {:?}", hints_response);
    Ok(hints_response)
}

fn setter(config: &config::env::Config, data: &api::schema::Hints) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(&config.hints)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, data)?;
    Ok(())
}

/// API endpoint to GET hints stored in a file (with line breaks) or environment variables (comma separated list).
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl localhost:3000/hints
/// ```
///
/// #### Status
/// * `200`: Successfully queued.
///
/// # Returns
///
/// Returns a JSON object of `Vec<String>` object.
#[utoipa::path(
    get,
    tag = "Hints",
    path = "/hints",
    responses(
        (status = 200, description = "Get hints object", body = api::schema::Hints),
        (status = 404, description = "Hints file not found", body = String),
        (status = 500, description = "Failed to open or parse hints file", body = String),
    )
)]
pub async fn get(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    // TODO: Replace BufReader and BufWrtier with tokio for async
    if !config.hints.is_file() {
        let warn = format!("{} not found", config.hints.display());
        log::warn!("{}", warn);
        return HttpResponse::NotFound().json(warn);
    }
    match getter(&config) {
        Ok(res) => {
            HttpResponse::Ok().json(res)
        },
        Err(err) => {
            log::error!("{:?}", err);
            HttpResponse::InternalServerError().json(err.to_string())
        },
    }
}


/// API endpoint to ADD hints.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
/// * `body` - Request body that takes `Hints` object.
///
/// #### Sample Request
/// ```shell
/// curl -X PUT localhost:3000/torrent \
///   -H "Content-Type: application/json" \
///   -d '{
///     "save_paths": ["/home/admin/TV Shows", "/home/admin/Movies"]
///     "remote_hosts": ["192.168.1.102", "192.168.1.101"],
///     "remote_users": "admin",
///     "remote_paths": "/Users/admin/Sintel",
///   }'
/// ```
///
/// #### Sample Response
/// ```json
/// "Added"
/// ```
///
/// # Returns
///
/// Returns a JSON object to indicate the status.
#[utoipa::path(
    put,
    tag = "Hints",
    path = "/hints",
    request_body = api::schema::Hints,
    responses(
        (status = 200, description = "Hints object", body = Vec<api::schema::Hints>),
        (status = 400, description = "Invalid key", body = String),
        (status = 404, description = "Hints file not found", body = String),
        (status = 500, description = "Failed to open or parse hints file", body = String),
    )
)]
pub async fn put(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
    body: web::Json<api::schema::Hints>,
) -> impl Responder {
    // TODO: Replace this to patch and create a put endpoint to add/replace one particular key
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    match setter(&config, &body) {
        Ok(_) => {
            HttpResponse::Ok().json(format!("Updated {}", &config.hints.display()))
        },
        Err(err) => {
            log::error!("{:?}", err);
            HttpResponse::InternalServerError().json(err.to_string())
        },
    }
}

/// API endpoint to DELETE a hint.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
/// * `query` - JSON query parameters.
///
/// #### Sample Request (delete any downloaded files)
/// ```shell
/// curl -X DELETE "http://localhost:3000/hints?key=remote_users"
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
    tag = "Hints",
    path = "/hints",
    params(
        ("key" = String, Query, description = "Hints key to delete"),
    ),
    responses(
        (status = 200, description = "Get cleared hints object", body = api::schema::Hints),
        (status = 400, description = "Invalid key", body = String),
        (status = 404, description = "Hints file not found", body = String),
        (status = 500, description = "Failed to open or parse hints file", body = String),
    )
)]
pub async fn delete(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let key = match query.get("key") {
        Some(i) => i.trim().to_lowercase(),
        None => return HttpResponse::BadRequest().body("Missing key"),
    };
    if !api::schema::HINTS.has_field(&key) {
        return HttpResponse::BadRequest().body(format!("Invalid hint key '{}'; use one of {:?}", key, api::schema::HINTS.keys()));
    }
    match getter(&config) {
        Ok(mut res) => {
            res.clear_field(key.as_str());
            match setter(&config, &res) {
                Ok(_) => {
                    HttpResponse::Ok().json(res)
                },
                Err(err) => {
                    log::error!("{:?}", err);
                    HttpResponse::InternalServerError().json(err.to_string())
                },
            }
        },
        Err(err) => {
            log::error!("{:?}", err);
            HttpResponse::InternalServerError().json(err.to_string())
        },
    }
}
