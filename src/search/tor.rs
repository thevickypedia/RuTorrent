use crate::{api, config};
use reqwest::{Client, Proxy};
use std::time::Duration;

/// Create an HTTP client configured to use the Tor proxy.
///
/// # Arguments
///
/// * `config` - Application configuration containing Tor proxy settings and timeout.
/// * `metadata` - Application metadata containing the package version for the user agent.
///
/// # Returns
///
/// Returns a `Result` containing the configured HTTP client, or an error if the proxy configuration or client creation fails.
pub async fn client(
    config: &config::env::Config,
    metadata: &config::constant::MetaData,
) -> api::schema::Result<Client> {
    let url = format!("{}://{}:{}", config.tor_scheme, config.tor_host, config.tor_port);
    log::debug!("Proxy url: {}", url);
    let builder = Client::builder()
        .timeout(Duration::from_secs(config.tor_timeout))
        .user_agent(format!("apibay-rs/{}", metadata.pkg_version))
        .proxy(Proxy::all(url)?);
    Ok(builder.build()?)
}

/// Verify that an HTTP client routes requests through the Tor network.
///
/// # Arguments
///
/// * `config` - Application configuration containing Tor proxy settings.
/// * `metadata` - Application metadata used to configure a new client if needed.
/// * `existing_client` - Optional HTTP client to use; creates a new client if `None`.
///
/// # Returns
///
/// Returns a `Result` containing the verified HTTP client if the connection uses Tor, or an error if the request fails or Tor verification is unsuccessful.
pub async fn verify(
    config: &config::env::Config,
    metadata: &config::constant::MetaData,
    existing_client: Option<Client>
) -> api::schema::Result<Client> {
    let tor_client = existing_client.unwrap_or(client(config, metadata).await.unwrap_or_default());
    let response: serde_json::Value = tor_client
        .get("https://check.torproject.org/api/ip")
        .send()
        .await?
        .json()
        .await?;

    if response["IsTor"].as_bool().unwrap_or(false) {
        if let Some(ip) = response.get("IP").and_then(serde_json::Value::as_str) {
            log::info!("Tor IP: {}", ip);
        }
        Ok(tor_client.clone())
    } else {
        Err("Connection is not secure. Aborting...".into())
    }
}
