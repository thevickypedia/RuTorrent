use crate::{config, api};
use reqwest::{Client, Proxy};
use std::time::Duration;

pub async fn client(config: &config::env::Config, metadata: &config::constant::MetaData) -> api::schema::Result<Client> {
    let url = format!("socks5h://{}:{}", config.tor_host, config.tor_port);
    let builder = Client::builder()
        .timeout(Duration::from_secs(config.tor_timeout))
        .user_agent(format!("thebay-rs/{}", metadata.pkg_version))
        .proxy(Proxy::all(url)?);
    Ok(builder.build()?)
}

pub async fn verify(config: &config::env::Config, metadata: &config::constant::MetaData, existing_client: Option<Client>) -> bool {
    let tor_client = existing_client.unwrap_or(client(config, metadata).await.unwrap_or_default());

    let url = "https://httpbin.org/get";
    let tor_response = match tor_client.get(url).timeout(Duration::from_secs(3)).send().await {
        Ok(response) => response,
        Err(e) => {
            log::error!("Failed to connect through Tor: {}", e);
            return false;
        }
    };

    let tor_response = match tor_response.error_for_status() {
        Ok(response) => response,
        Err(e) => {
            log::error!("Tor request returned an error status: {}", e);
            return false;
        }
    };

    let tor_json = match tor_response.json::<serde_json::Value>().await {
        Ok(json) => json,
        Err(e) => {
            log::error!("Failed to parse Tor response: {}", e);
            return false;
        }
    };

    let tor_ip = match tor_json["origin"].as_str() {
        Some(ip) => ip.to_owned(),
        None => {
            log::error!("Missing origin in Tor response");
            return false;
        }
    };

    println!("Tor IP: {tor_ip}");

    let net_response = match reqwest::get(url).await {
        Ok(response) => response,
        Err(e) => {
            log::error!("Failed to connect without Tor: {}", e);
            return false;
        }
    };

    let net_response = match net_response.error_for_status() {
        Ok(response) => response,
        Err(e) => {
            log::error!("Regular request returned an error status: {}", e);
            return false;
        }
    };

    let net_json = match net_response.json::<serde_json::Value>().await {
        Ok(json) => json,
        Err(e) => {
            log::error!("Failed to parse regular response: {}", e);
            return false;
        }
    };

    let net_ip = match net_json["origin"].as_str() {
        Some(ip) => ip.to_owned(),
        None => {
            log::error!("Missing origin in regular response");
            return false;
        }
    };

    println!("Regular IP: {net_ip}");

    if tor_ip == net_ip {
        log::error!("Connection is not secure. Aborting...");
        return false;
    }

    log::info!("Tor connection verified successfully.");
    true
}
