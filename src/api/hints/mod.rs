use std::fs::File;
use std::io::BufReader;
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
    // TODO: Add new endpoints to PUT and DELETE hints
    if !config.hints.is_file() {
        let warn = format!("{} not found", config.hints.display());
        log::warn!("{}", warn);
        return HttpResponse::NotFound().json(warn);
    }
    let file = match File::open(&config.hints) {
        Ok(file) => file,
        Err(e) => {
            let error = format!("Failed to open {} - {}", config.hints.display(), e);
            log::error!("{}", error);
            return HttpResponse::InternalServerError().json(error);
        }
    };
    let reader = BufReader::new(file);
    let hints_response: api::schema::HintsResponse = match serde_json::from_reader(reader) {
        Ok(json) => json,
        Err(e) => {
            let error = format!("Failed to parse {} - {}", config.hints.display(), e);
            log::error!("{}", error);
            return HttpResponse::InternalServerError().json(error);
        }
    };
    log::info!("Hints: {:?}", hints_response);
    HttpResponse::Ok().json(hints_response)
}
