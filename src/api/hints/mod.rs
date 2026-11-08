use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use actix_web::{web, HttpRequest, Responder, HttpResponse};
use crate::{api, config};

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
        (status = 200, description = "Get hints object", body = api::schema::HintsResponse),
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
    // TODO: Get hints.json filename from env vars
    //  Add new endpoints to PUT and DELETE hints
    let path = PathBuf::from("hints.json");
    if !path.is_file() {
        return HttpResponse::NotFound().json("hints.json not found");
    }
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(e) => {
            log::error!("Failed to open '{}': {}", path.display(), e);
            return HttpResponse::InternalServerError().json("Failed to open hints.json");
        }
    };
    let reader = BufReader::new(file);
    let hints_response: api::schema::HintsResponse = match serde_json::from_reader(reader) {
        Ok(json) => json,
        Err(e) => {
            log::error!("Failed to parse '{}': {}", path.display(), e);
            return HttpResponse::InternalServerError().json("Failed to parse hints.json");
        }
    };
    log::info!("Hints: {:?}", hints_response);
    HttpResponse::Ok().json(hints_response)
}
