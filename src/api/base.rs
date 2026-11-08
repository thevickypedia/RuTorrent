use crate::{api, config, squire};

use actix_web::{HttpRequest, HttpResponse, Responder, web};
use serde_json::json;

/// API endpoint to get the current health status.
///
/// # Returns
///
/// Returns the `HTTPResponse` with a JSON message to indicate the API is up.
pub async fn status() -> impl Responder {
    HttpResponse::Ok().json(json!({ "status": "ok" }))
}

/// API endpoint to get the current version of the project.
///
/// # Returns
///
/// Returns the `HTTPResponse` with a JSON message resolved during compile time.
pub async fn version(metadata: web::Data<config::constant::MetaData>) -> impl Responder {
    HttpResponse::Ok().json(json!({ "version": metadata.pkg_version }))
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
    tag = "Common",
    path = "/hints",
    responses(
        (status = 200, description = "Torrent list", body = Vec<String>)
    )
)]
pub async fn hints(request: HttpRequest, config: web::Data<config::env::Config>) -> impl Responder {
    if !api::squire::authenticator(&request, &config) {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let file_hints: Vec<String> = std::fs::read_to_string("hints")
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let env_hints = squire::misc::get_env_var("hints", None)
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect::<Vec<String>>();
    let final_hints: Vec<String> = file_hints
        .into_iter()
        .chain(env_hints)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    log::info!("Hints: {:?}", final_hints);
    HttpResponse::Ok().json(final_hints)
}
