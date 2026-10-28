use crate::config;

use actix_web::{HttpResponse, Responder, web};
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
