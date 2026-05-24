use base64::{engine::general_purpose, Engine as _};
use crate::settings;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_json::json;

/// API endpoint to render the UI.
///
/// # Arguments
///
/// * `config` - Reference to the `Config` object.
///
/// #### Status
/// * `200`: HTML content of the index page.
/// * `501`: Username or password not set on server side.
///
/// # Returns
///
/// Returns the HTTPResponse with `Content-Type` header set to `text/html` and `body` as content of the HTML file.
pub async fn index_page(config: web::Data<settings::Config>) -> impl Responder {
    if config.username.is_empty() || config.password.is_empty() {
        log::warn!("Username and password are required to access the UI");
        return HttpResponse::NotImplemented().finish();
    }
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("templates/index.html"))
}

/// Helper function to base64 decode the header string.
///
/// # Arguments
///
/// * `value` - Base64 encoded string to decode.
///
/// # Returns
///
/// Returns the decoded string if successful, otherwise an error.
fn base64_decode(value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let decoded = general_purpose::STANDARD.decode(value)?;
    let result = String::from_utf8(decoded)?;
    Ok(result)
}

/// API endpoint to authenticate the UI.
///
/// # Arguments
///
/// - `request` - Reference to the `HttpRequest` object.
/// * `config` - Reference to the `Config` object.
///
/// #### Status
/// * `200`: Successfully authenticated with JSON object as `{"apikey": "ApiKeyValue"}`
/// * `401`: Invalid credentials or missing authorization header.
/// * `501`: Username or password not set on server side.
///
/// # Returns
///
/// Returns an `HttpResponse` object with the apikey if successful.
pub async fn authenticator(
    request: HttpRequest,
    config: web::Data<settings::Config>,
) -> impl Responder {
    if config.username.is_empty() || config.password.is_empty() {
        log::warn!("Username and password are required to authenticate the UI");
        return HttpResponse::NotImplemented().finish();
    }
    let auth_header = match request.headers().get("Authorization") {
        Some(head) => head,
        None => {
            log::warn!("No Authorization Header received");
            return HttpResponse::Unauthorized().finish()
        },
    };
    let auth_header = match auth_header.to_str() {
        Ok(header) => header,
        Err(err) => {
            log::warn!("{}", err);
            return HttpResponse::Unauthorized().finish()
        },
    };
    let encoded = match auth_header.strip_prefix("Basic ") {
        Some(value) => value,
        None => {
            log::warn!("Authorization header missing Basic prefix");
            return HttpResponse::Unauthorized().finish()
        },
    };
    let decoded = match base64_decode(encoded) {
        Ok(decoded_) => decoded_,
        Err(err) => {
            log::warn!("{}", err);
            return HttpResponse::Unauthorized().finish()
        },
    };
    let auth_parts = decoded.splitn(2, ':').collect::<Vec<&str>>();
    if auth_parts.len() != 2 {
        log::warn!("Expected two Authorization headers, received {:?}", auth_header);
        return HttpResponse::Unauthorized().finish();
    }
    let username = auth_parts[0].to_string();
    let password = auth_parts[1].to_string();
    if username == config.username && password == config.password {
        return HttpResponse::Ok().json(json!({ "apikey": config.apikey }))
    }
    log::warn!("Username and password do not match");
    HttpResponse::Unauthorized().finish()
}
