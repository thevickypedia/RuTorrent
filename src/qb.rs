use crate::settings;
use actix_web::HttpResponse;
use reqwest::Client;

/// Creates an authenticated HTTP client for interacting with the qBittorrent Web API.
///
/// # Arguments
///
/// * `config` - A reference to the application configuration containing:
///   - `qbit_api`: The base URL of the qBittorrent Web API. Defaults to `http://localhost:8080`
///   - `username`: The username for authentication (optional)
///   - `password`: The password for authentication (optional)
///
/// # Returns
///
/// * `Ok(Client)` - A configured and authenticated [`reqwest::Client`] ready
///   for subsequent API requests.
/// * `Err(HttpResponse)` - Returned if the authentication request fails or
///   the server responds with an error.
///
/// # Notes
///
/// - Cookies are persisted in the client to maintain the authenticated session.
/// - The function assumes the qBittorrent Web API is reachable at the given `qbit_api`.
pub async fn client(config: &settings::Config) -> Result<Client, HttpResponse> {
    let client = Client::builder().cookie_store(true).build().unwrap();

    let request = client.post(format!("{}/api/v2/auth/login", config.qbit_api));

    let request = if config.username.is_empty() || config.password.is_empty() {
        request
    } else {
        request.form(&[
            ("username", config.username.as_str()),
            ("password", config.password.as_str()),
        ])
    };

    let resp = request.send().await;
    handle_response(resp, "LOGIN").await?;

    Ok(client)
}

/// Handles and validates an HTTP response from the qBittorrent Web API.
///
/// # Arguments
///
/// * `resp` - The result of an HTTP request, containing either a [`reqwest::Response`]
///   or a [`reqwest::Error`]
/// * `context` - A string describing the request context (used for logging)
///
/// # Returns
///
/// * `Ok(())` - If the response indicates success according to HTTP status
///   and qBittorrent's response contract
/// * `Err(HttpResponse)` - Returned if the request fails, the HTTP status is not successful,
///   or the response body does not match the expected success format
///
/// # Notes
///
/// - Treats any non-success HTTP status as an internal server error.
/// - qBittorrent considers a request successful if the body is empty or equals `"Ok."`.
pub async fn handle_response(
    resp: Result<reqwest::Response, reqwest::Error>,
    context: &str,
) -> Result<(), HttpResponse> {
    match resp {
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();

            log::info!("{} -> HTTP {} body: {:?}", context, status, body);

            if !status.is_success() {
                return Err(HttpResponse::InternalServerError().body(body));
            }

            // qBittorrent success contract
            if !body.trim().is_empty() && body.trim() != "Ok." {
                return Err(HttpResponse::BadRequest().body(body));
            }

            Ok(())
        }
        Err(e) => {
            log::info!("{} request failed: {}", context, e);
            Err(HttpResponse::InternalServerError().body("Request failed"))
        }
    }
}
