use crate::{api, config};
use reqwest::{Client, Proxy};
use std::time::Duration;

pub async fn client(
    config: &config::env::Config,
    metadata: &config::constant::MetaData,
) -> api::schema::Result<Client> {
    let url = format!("socks5h://{}:{}", config.tor_host, config.tor_port);
    log::debug!("Proxy url: {}", url);
    let builder = Client::builder()
        .timeout(Duration::from_secs(config.tor_timeout))
        .user_agent(format!("thebay-rs/{}", metadata.pkg_version))
        .proxy(Proxy::all(url)?);
    Ok(builder.build()?)
}

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
