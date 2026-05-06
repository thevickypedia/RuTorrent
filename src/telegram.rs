use crate::settings;
use reqwest::Client;
use std::time::Duration;

/// Sends a message to the user via Telegram.
///
/// # Arguments
/// * `message` - Message to be sent to the user.
/// * `parse_mode` - Parse mode. Defaults to `"markdown"` if `None`.
///
/// # Returns
/// * `bool` - Returns `true` if the message was sent successfully, `false` otherwise.
pub async fn send(config: &settings::Config, message: &str) -> bool {
    let client = match Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to build HTTP client: {}", e);
            return false;
        }
    };

    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        config.telegram_bot_token
    );

    let chat_id = config.telegram_chat_id.to_string();
    let params = [
        ("chat_id", chat_id.as_str()),
        ("text", message),
        ("parse_mode", "markdown"),
    ];

    match client.post(&url).form(&params).send().await {
        Ok(resp) => {
            if let Err(e) = resp.error_for_status_ref() {
                let masked = e
                    .to_string()
                    .replace(config.telegram_bot_token.as_str(), "**********");
                log::error!("{}", masked);
                return false;
            }

            match resp.text().await {
                Ok(body) => {
                    log::info!("Response: {}", body);
                    true
                }
                Err(e) => {
                    let masked = e
                        .to_string()
                        .replace(config.telegram_bot_token.as_str(), "**********");
                    log::error!("{}", masked);
                    false
                }
            }
        }
        Err(e) => {
            let masked = e
                .to_string()
                .replace(config.telegram_bot_token.as_str(), "**********");
            log::error!("{}", masked);
            false
        }
    }
}
