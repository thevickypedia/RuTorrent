use crate::{api, config};
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use std::collections::HashMap;

/// Load the hints JSON file into the `Hints` struct.
///
/// # Arguments
///
/// * `config` - Reference to the `Config` object.
///
/// # Returns
///
/// Returns the `Result<Hints, Box<dyn Error>>` object.
async fn getter(
    config: &config::env::Config,
) -> Result<api::schema::Hints, Box<dyn std::error::Error>> {
    let data = tokio::fs::read(&config.hints).await?;
    let hints_response: api::schema::Hints = serde_json::from_slice(&data)?;
    log::info!("Hints: {:?}", hints_response);
    Ok(hints_response)
}

/// Writes the hints JSON file with the `Hints` struct.
///
/// # Arguments
///
/// * `config` - Reference to the `Config` object.
/// * `data` - Reference to the `Hints` object.
///
/// # Returns
///
/// Returns the `Result<(), Box<dyn std::error::Error>>` object.
async fn setter(
    config: &config::env::Config,
    data: &api::schema::Hints,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_vec_pretty(data)?;
    tokio::fs::write(&config.hints, json).await?;
    Ok(())
}

/// Writes the hints JSON file with the `Hints` struct and returns an `HttpResponse` object.
///
/// # Arguments
///
/// * `config` - Reference to the `Config` object.
/// * `data` - Reference to the `Hints` object.
///
/// # Returns
///
/// Returns the `HttpResponse` object.
async fn set_and_return(
    config: &config::env::Config,
    data: &api::schema::Hints,
) -> HttpResponse {
    match setter(config, data).await {
        Ok(_) => HttpResponse::Ok().json(data),
        Err(err) => {
            log::error!("{:?}", err);
            HttpResponse::InternalServerError().json(err.to_string())
        }
    }
}

/// API endpoint to GET hints stored in a JSON file.
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
/// # Returns
///
/// Returns the `Hints` object.
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
pub async fn get(request: HttpRequest, config: web::Data<config::env::Config>) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    if !config.hints.is_file() {
        let warn = format!("{} not found", config.hints.display());
        log::warn!("{}", warn);
        return HttpResponse::NotFound().json(warn);
    }
    match getter(&config).await {
        Ok(res) => HttpResponse::Ok().json(res),
        Err(err) => {
            log::error!("{:?}", err);
            HttpResponse::InternalServerError().json(err.to_string())
        }
    }
}

/// API endpoint to PUT a hint. Updates existing JSON file or creates a new one.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `query` - JSON query parameters.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl -X 'PUT' \
///   'localhost:3000/hints?key=save_paths' \
///   -H 'Content-Type: application/json' \
///   -d '["/home/Movies", "/home/TV Shows", "/home/Photos"]'
/// ```
///
/// # Returns
///
/// Returns the updated `Hints` object.
#[utoipa::path(
    put,
    tag = "Hints",
    path = "/hints",
    params(
        ("key" = String, Query, description = "Hint key to add/update"),
    ),
    request_body = Vec<String>,
    responses(
        (status = 200, description = "Updated Hints object", body = Vec<String>),
        (status = 400, description = "Invalid key", body = String),
        (status = 500, description = "Failed to open or parse hints file", body = String),
    )
)]
pub async fn put(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
    query: web::Query<HashMap<String, String>>,
    body: web::Json<Vec<String>>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let key = match query.get("key") {
        Some(i) => i.trim().to_lowercase(),
        None => return HttpResponse::BadRequest().body("Missing key"),
    };
    if !api::schema::HINTS.has_field(&key) {
        return HttpResponse::BadRequest().body(format!(
            "Invalid hint key '{}'; use one of {:?}",
            key,
            api::schema::HINTS.keys()
        ));
    }
    // Update existing or create a new object
    let mut data = match getter(&config).await {
        Ok(mut current_hints) => {
            current_hints.clear_field(&key);
            current_hints
        },
        Err(err) => {
            log::warn!("{:?}", err);
            api::schema::Hints::new()
        }
    };
    data.add_field(&key, body.0);
    set_and_return(&config, &data).await
}

/// API endpoint to PATCH hints in (re-write) the JSON file.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `body` - Request body that takes `Hints` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Sample Request
/// ```shell
/// curl -X 'PATCH' \
///   'localhost:3000/hints' \
///   -H 'Content-Type: application/json' \
///   -d '{
///   "save_paths": ["/Users/admin/Downloads", "/Users/admin/Documents"],
///   "remote_hosts": ["192.168.1.102", "192.168.1.103"],
///   "remote_users": ["admin", "user"],
///   "remote_paths": ["/home/Movies", "/home/TV Shows", "/home/Photos"]
/// }'
/// ```
///
/// # Returns
///
/// Returns the updated `Hints` object.
#[utoipa::path(
    patch,
    tag = "Hints",
    path = "/hints",
    request_body = api::schema::Hints,
    responses(
        (status = 200, description = "Updated Hints object", body = Vec<String>),
        (status = 500, description = "Failed to open or parse hints file", body = String),
    )
)]
pub async fn patch(
    request: HttpRequest,
    config: web::Data<config::env::Config>,
    body: web::Json<api::schema::Hints>,
) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    set_and_return(&config, &body).await
}

/// API endpoint to DELETE a hint stored in the JSON file.
///
/// # Arguments
///
/// * `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
/// * `query` - JSON query parameters.
///
/// #### Sample Request (delete any downloaded files)
/// ```shell
/// curl -X DELETE "localhost:3000/hints?key=remote_users"
/// ```
///
/// # Returns
///
/// Returns the updated `Hints` object.
#[utoipa::path(
    delete,
    tag = "Hints",
    path = "/hints",
    params(
        ("key" = String, Query, description = "Hints key to delete"),
    ),
    responses(
        (status = 200, description = "Updated Hints object", body = Vec<String>),
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
        return HttpResponse::BadRequest().body(format!(
            "Invalid hint key '{}'; use one of {:?}",
            key,
            api::schema::HINTS.keys()
        ));
    }
    match getter(&config).await {
        Ok(mut res) => {
            res.clear_field(key.as_str());
            set_and_return(&config, &res).await
        }
        Err(err) => {
            log::error!("{:?}", err);
            HttpResponse::InternalServerError().json(err.to_string())
        }
    }
}
