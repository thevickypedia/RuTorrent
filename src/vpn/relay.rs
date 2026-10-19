use reqwest;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use rand::prelude::*;
use crate::vpn;

async fn list_active_relays() -> Result<Vec<Value>, Box<dyn Error>> {
    let url = "https://api.mullvad.net/www/relays/all/";

    let data: Vec<Value> = reqwest::get(url)
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(data
        .into_iter()
        .filter(|relay| relay.get("active").and_then(Value::as_bool) == Some(true))
        .collect())
}

pub async fn generate_relay_map() -> Result<vpn::models::RelayMap, Box<dyn Error>> {
    let relays = list_active_relays().await?;
    let mut relay_map: vpn::models::RelayMap = HashMap::new();

    for relay in relays.iter() {

        let Some(hostname) = relay["hostname"].as_str() else {
            log::warn!("Relay found without a hostname, skipping...");
            log::error!("{}", relay);
            continue
        };

        let country_name = relay["country_name"].as_str().unwrap_or("");
        let country_code = relay["country_code"].as_str().unwrap_or("");
        let city_name = relay["city_name"].as_str().unwrap_or("");
        let city_code = relay["city_code"].as_str().unwrap_or("");

        let relay_connector = vpn::models::RelayConnector {
            hostname: hostname.to_string(),
            city_name: city_name.to_string(),
            country_name: country_name.to_string()
        };

        relay_map
            .entry(country_code.to_string())
            .or_default()
            .entry(city_code.to_string())
            .or_default()
            .push(relay_connector);
    }

    Ok(relay_map)
}

pub async fn select_relay(
    relay_map: vpn::models::RelayMap,
    whitelist: Option<Vec<String>>,
    blacklist: Option<Vec<String>>,
) -> Option<vpn::models::RelayConnector> {
    log::debug!("whitelist: {:?}", whitelist);
    log::debug!("blacklist: {:?}", blacklist);
    let whitelisted = whitelist.unwrap_or_default();
    let blacklisted = blacklist.unwrap_or_default();
    let mut relays = Vec::new();
    for (country_code, country_data) in relay_map {
        if blacklisted.contains(&country_code) {
            continue;
        }
        if !whitelisted.is_empty() && !whitelisted.contains(&country_code) {
            continue;
        }
        for (city_code, city_data) in country_data {
            if blacklisted.contains(&city_code) {
                continue;
            }
            relays.extend(city_data);
        }
    }
    let mut rng = rand::rng();
    relays.choose(&mut rng).cloned()
}
