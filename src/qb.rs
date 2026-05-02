use crate::settings;
use actix_web::HttpResponse;
use reqwest::Client;

pub async fn client(config: &settings::Config) -> Result<Client, HttpResponse> {
    let client = Client::builder().cookie_store(true).build().unwrap();

    let request = client.post(format!("{}/api/v2/auth/login", config.base_url));

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
