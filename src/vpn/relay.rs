use crate::vpn;
use rand::prelude::*;
use reqwest;
use serde_json::Value;
use std::error::Error;

/// Get the list of active relays.
///
/// # Returns
///
/// Returns the `Vec<Value>` object.
async fn list_active_relays() -> Result<Vec<Value>, Box<dyn Error>> {
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
pub async fn generate_relay_map() -> Result<Vec<vpn::models::RelayLocation>, Box<dyn Error>> {
    // TODO: Pass 'config.mullvad_relay_cache' instead of None
    if let Some(relays_cached) = vpn::cache::load_relay_locations(None) {
        log::debug!("Relays generated from cache: {}", relays_cached.len());
        return Ok(relays_cached)
    }
    log::info!("Failed to get relays from cache, requesting mullvad API.");
    let mut relays: Vec<vpn::models::RelayLocation> = Vec::new();
    for relay in list_active_relays().await?.iter() {
        let country_name = relay["country_name"].as_str().unwrap_or("");
        let country_code = relay["country_code"].as_str().unwrap_or("");
        let city_name = relay["city_name"].as_str().unwrap_or("");
        let city_code = relay["city_code"].as_str().unwrap_or("");
        let relay_connector = vpn::models::RelayLocation {
            city: city_name.to_string(),
            country: country_name.to_string(),
            city_code: city_code.to_string(),
            country_code: country_code.to_string(),
        };
        relays.push(relay_connector);
    }
    Ok(relays)
}

/// Selects a random relay from the relay map after applying the specified filters.
///
/// # Filtering Rules
///
/// * If `blacklist` is provided, exclude relays whose country or city code is blacklist.
/// * If `whitelist` is provided, consider only relays whose country or city code is whitelist.
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
/// * `Some(RelayConnector)` - A randomly selected relay that satisfies all filters.
/// * `None` - If no eligible relays are available.
pub async fn select_relay(
    relay_map: Vec<vpn::models::RelayLocation>,
    whitelist: &Vec<String>,
    blacklist: &Vec<String>,
) -> Option<vpn::models::RelayLocation> {
    log::debug!("whitelist: {:?}", whitelist);
    log::debug!("blacklist: {:?}", blacklist);
    let mut relays: Vec<vpn::models::RelayLocation> = Vec::new();
    for relay_location in relay_map {
        // Skip all relays within a country if the 'country_code' is blacklisted
        if blacklist.contains(&relay_location.country_code) {
            continue;
        }
        // Skip all relays within a city if the 'city_code' is blacklisted
        if blacklist.contains(&relay_location.city_code) {
            continue;
        }
        // Skip if whitelisted array exists, but does not contain the 'city_code' or 'country_code'
        if !whitelist.is_empty()
            && !whitelist.contains(&relay_location.city_code)
            && !whitelist.contains(&relay_location.country_code)
        {
            continue;
        }
        relays.push(relay_location);
    }
    let mut rng = rand::rng();
    relays.choose(&mut rng).cloned()
}
