use crate::config;

mod ntfy;
mod telegram;

/// Function to notify about an event.
///
/// # Arguments
///
/// * `title` - Subject of the notification.
/// * `body` - Body of the notification.
/// * `config` - Reference to the `Config` object.
///
/// # Notes
///
/// Sends notifications through `NTFY` and `Telegram` based on the availability of env vars.
pub fn notify(title: String, body: String, config: config::env::Config) {
    let title_clone = title.clone();
    let body_clone = body.clone();
    let config_clone = config.clone();
    if !config.ntfy_url.is_empty() && !config.ntfy_topic.is_empty() {
        log::info!("Sending NTFY notification to {}: {}", title_clone, body);
        tokio::spawn(async move {
            let _ = ntfy::send(&config, &title, &body).await;
        });
    }
    if !config_clone.telegram_bot_token.is_empty() && !config_clone.telegram_chat_id.is_empty() {
        log::info!(
            "Sending Telegram notification to {}: {}",
            title_clone,
            body_clone
        );
        tokio::spawn(async move {
            let message = format!("*{}*\n\n{}", title_clone, body_clone);
            let _ = telegram::send(&config_clone, &message).await;
        });
    }
}
