use crate::settings;
use reqwest::Client;
use std::time::Duration;

pub async fn send(config: &settings::Config, title: &String, data: &String) -> bool {
    let client = match Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to build HTTP client: {}", e);
            return false;
        }
    };
    let url = format!("{}/{}", config.ntfy_url, config.ntfy_topic);

    let mut request = client
        .post(&url)
        .header("X-Title", title)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(data.to_string());

    let username = config.ntfy_username.to_string();
    let password = config.ntfy_password.to_string();
    if !username.is_empty() && !password.is_empty() {
        request = request.basic_auth(username, Some(password));
    }

    match request.send().await {
        Ok(resp) => match resp.error_for_status() {
            Ok(resp) => {
                match resp.text().await {
                    Ok(body) => log::info!("Ntfy response: {}", body.strip_suffix("\n").unwrap()),
                    Err(e) => log::error!("Failed to read response body: {}", e),
                }
                true
            }
            Err(e) => {
                log::error!("HTTP error: {}", e);
                false
            }
        },
        Err(e) => {
            log::error!("Request failed: {}", e);
            false
        }
    }
}
