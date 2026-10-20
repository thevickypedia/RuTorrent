use crate::vpn;
use rand::prelude::*;
use reqwest;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;

/// Get the list of active relays.
///
/// # Returns
///
/// Returns the `Vec<Value>` object.
pub async fn list_active_relays() -> Result<Vec<Value>, Box<dyn Error>> {
    let url = "https://api.mullvad.net/www/relays/all/";

    let data: Vec<Value> = reqwest::get(url).await?.error_for_status()?.json().await?;

    Ok(data
        .into_iter()
        .filter(|relay| relay.get("active").and_then(Value::as_bool) == Some(true))
        .collect())
}

/// Generate a relay map of type as a nested `HashMap` object.
///
/// # Returns
///
/// A nested map of country codes to city codes to relay lists: `{country_code: {city_code: [relays]}}`
pub async fn generate_relay_map(relays: &Vec<Value>) -> Result<vpn::models::RelayMap, Box<dyn Error>> {
    let mut relay_map: vpn::models::RelayMap = HashMap::new();

    for relay in relays.iter() {
        let Some(hostname) = relay["hostname"].as_str() else {
            log::warn!("Relay found without a hostname, skipping...");
            log::error!("{}", relay);
            continue;
        };

        let country_name = relay["country_name"].as_str().unwrap_or("");
        let country_code = relay["country_code"].as_str().unwrap_or("");
        let city_name = relay["city_name"].as_str().unwrap_or("");
        let city_code = relay["city_code"].as_str().unwrap_or("");

        let relay_connector = vpn::models::RelayConnector {
            hostname: hostname.to_string(),
            city_name: city_name.to_string(),
            country_name: country_name.to_string(),
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

/// Get the currently connected relay.
pub async fn current_relay() -> String {
    let Some(current) = vpn::connect::get_current() else {
        return "".to_string();
    };
    let Some(details) = current.details else {
        return "".to_string();
    };
    let Some(hostname) = details.location.hostname else {
        return "".to_string();
    };
    log::info!("Current relay: {}", hostname);
    hostname
}

/// Filters the rule map from based on specific filtering rules.
///
/// # Filtering Rules
///
/// * If `blacklist` is provided, exclude relays whose country or city code is blacklisted.
/// * If `whitelist` is provided, consider only relays whose country or city code is whitelisted.
/// * Exclude the currently connected relay, if any.
///
/// # Arguments
///
/// * `relay_map` - A nested map of country codes to city codes to relay lists:
///   `{country_code: {city_code: [relays]}}`.
/// * `whitelist` - An optional list of country or city codes to include.
/// * `blacklist` - An optional list of country or city codes to exclude.
///
/// # Returns
///
/// Returns the vector for `RelayConnector` objects.
pub async fn filter_relays(
    relay_map: vpn::models::RelayMap,
    whitelist: Option<Vec<String>>,
    blacklist: Option<Vec<String>>,
) -> Vec<vpn::models::RelayConnector> {
    log::debug!("whitelist: {:?}", whitelist);
    log::debug!("blacklist: {:?}", blacklist);
    let whitelisted = whitelist.unwrap_or_default();
    let blacklisted = blacklist.unwrap_or_default();
    let connected_relay = current_relay().await;
    let mut relays = Vec::new();
    for (country_code, country_data) in relay_map {
        // Skip all relays within a country if the 'country_code' is blacklisted
        if blacklisted.contains(&country_code) {
            continue;
        }
        for (city_code, city_data) in country_data {
            // Skip all relays within a city if the 'city_code' is blacklisted
            if blacklisted.contains(&city_code) {
                continue;
            }
            // Skip if whitelisted array exists, but does not contain the 'city_code' or 'country_code'
            if !whitelisted.is_empty()
                && !whitelisted.contains(&city_code)
                && !whitelisted.contains(&country_code)
            {
                continue;
            }
            for relay in city_data {
                // DO NOT connect to the same relay again
                if relay.hostname == connected_relay {
                    log::info!("Skipping the relay: {}", relay.hostname);
                } else {
                    relays.push(relay);
                }
            }
        }
    }
    relays
}

/// Selects a random relay from the relay map after applying the specified filters.
///
/// # Arguments
///
/// * `relays` - A vector of `RelayConnector` objects.
///
/// # Returns
///
/// * `Some(RelayConnector)` - A randomly selected relay that satisfies all filters.
/// * `None` - If no eligible relays are available.
pub async fn select_relay(relays: &Vec<vpn::models::RelayConnector>) -> Option<vpn::models::RelayConnector> {
    let mut rng = rand::rng();
    relays.choose(&mut rng).cloned()
}
