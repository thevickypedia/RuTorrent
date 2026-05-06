use crate::settings;
use reqwest::Client;
use std::time::Duration;

/// Sends a message to the user via Ntfy.
///
/// # Arguments
/// * `config` - Reference to the `Config` object.
/// * `title` - Subject of the notification.
/// * `body` - Body of the notification.
pub async fn send(config: &settings::Config, title: &String, body: &String) {
    let client = match Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to build HTTP client: {}", e);
            return;
        }
    };
    let url = format!("{}/{}", config.ntfy_url, config.ntfy_topic);

    let mut request = client
        .post(&url)
        .header("X-Title", title)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body.to_string());

    let username = config.ntfy_username.to_string();
    let password = config.ntfy_password.to_string();
    if !username.is_empty() && !password.is_empty() {
        request = request.basic_auth(username, Some(password));
    }

    match request.send().await {
        Ok(body) => {
            log::debug!("Response: {:?}", body);
            log::info!("Ntfy notification has been sent to: {}", config.ntfy_topic);
        }
        Err(err) => {
            log::error!("{}", err);
        }
    }
}
