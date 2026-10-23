use crate::vpn;
use rand::prelude::*;
use reqwest;
use serde_json::Value;
use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;

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
pub async fn generate_relay_map(
    cache: Option<PathBuf>,
) -> Result<Vec<vpn::models::RelayLocation>, Box<dyn Error>> {
    if let Some(relays_cached) = vpn::cache::load_relay_locations(cache) {
        log::debug!("Relays generated from cache: {}", relays_cached.len());
        return Ok(relays_cached);
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

/// Selects distinct (country, city) locations passing both lists, in random order.
///
/// # Filtering Rules
///
/// * If `blacklist` is provided, exclude relays whose country or city code is blacklist.
/// * If `whitelist` is provided, consider only relays whose country or city code is whitelist.
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
pub fn eligible_relays(
    relay_map: Vec<vpn::models::RelayLocation>,
    whitelist: &[String],
    blacklist: &[String],
) -> Vec<vpn::models::RelayLocation> {
    let mut seen = HashSet::new();
    let mut relays: Vec<_> = relay_map
        .into_iter()
        .filter(|r| {
            !blacklist.contains(&r.country_code)
                && !blacklist.contains(&r.city_code)
                && (whitelist.is_empty()
                || whitelist.contains(&r.city_code)
                || whitelist.contains(&r.country_code))
                && seen.insert((r.country_code.clone(), r.city_code.clone()))
        })
        .collect();
    relays.shuffle(&mut rand::rng());
    relays
}

pub fn validate_codes(
    relays: &[vpn::models::RelayLocation],
    whitelist: &[String],
    blacklist: &[String],
) -> Result<(), vpn::models::VpnError> {
    let known: HashSet<String> = relays
        .iter()
        .flat_map(|r| [r.country_code.to_lowercase(), r.city_code.to_lowercase()])
        .collect();
    let invalid: Vec<&str> = whitelist
        .iter()
        .chain(blacklist)
        .map(String::as_str)
        .filter(|c| !known.contains(&c.to_lowercase()))
        .collect();
    if invalid.is_empty() {
        Ok(())
    } else {
        Err(vpn::models::VpnError::Invalid(format!(
            "Unknown country/city code(s): {}",
            invalid.join(", ")
        )))
    }
}
