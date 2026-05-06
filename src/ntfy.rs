use reqwest::Client;
use crate::settings;

pub async fn send(config: &settings::Config, title: &str, data: &str) -> bool {
    let client = Client::builder().build().unwrap();
    let url = format!("{}/{}", config.ntfy_url, config.ntfy_topic);
    log::info!("Client url: {}", &url);

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
                    Ok(body) => log::debug!("ntfy response: {}", body),
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
