use crate::settings;
use reqwest::Client;
use std::time::Duration;

/// Sends a message to the user via Telegram.
///
/// # Arguments
/// * `config` - Reference to the `Config` object.
/// * `message` - Message to be sent to the user.
pub async fn send(config: &settings::Config, message: &str) {
    let client = match Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to build HTTP client: {}", e);
            return;
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
        Ok(body) => {
            log::debug!("Response: {:?}", body);
            log::info!(
                "Telegram notification has been sent to: {}",
                config.telegram_chat_id
            );
        }
        Err(e) => {
            let masked = e
                .to_string()
                .replace(config.telegram_bot_token.as_str(), "**********");
            log::error!("{}", masked);
        }
    }
}
