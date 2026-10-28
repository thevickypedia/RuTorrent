use crate::vpn;
use rand::prelude::*;
use reqwest;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::error::Error;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
pub struct RelayLocation {
    pub city: String,
    pub country: String,
    pub city_code: String,
    pub country_code: String,
}

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
) -> Result<Vec<RelayLocation>, Box<dyn Error>> {
    if let Some(relays_cached) = vpn::cache::load_relay_locations(cache) {
        log::debug!("Relays generated from cache: {}", relays_cached.len());
        return Ok(relays_cached);
    }
    log::info!("Failed to get relays from cache, requesting mullvad API.");
    let mut relays: Vec<RelayLocation> = Vec::new();
    for relay in list_active_relays().await?.iter() {
        let country_name = relay["country_name"].as_str().unwrap_or("");
        let country_code = relay["country_code"].as_str().unwrap_or("");
        let city_name = relay["city_name"].as_str().unwrap_or("");
        let city_code = relay["city_code"].as_str().unwrap_or("");
        let relay_connector = RelayLocation {
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
/// Returns a `Vec<RelayLocation>` object of all eligible objects.
pub fn eligible_relays(
    relay_map: Vec<RelayLocation>,
    whitelist: &[String],
    blacklist: &[String],
) -> Vec<RelayLocation> {
    let mut seen = HashSet::new();
    let mut relays: Vec<_> = relay_map
        .into_iter()
        .filter(|r| {
            !blacklist.contains(&r.country_code.to_lowercase())
                && !blacklist.contains(&r.city_code.to_lowercase())
                && (whitelist.is_empty()
                    || whitelist.contains(&r.city_code.to_lowercase())
                    || whitelist.contains(&r.country_code.to_lowercase()))
                && seen.insert((r.country_code.to_lowercase(), r.city_code.to_lowercase()))
        })
        .collect();
    relays.shuffle(&mut rand::rng());
    relays
}

/// Validates relays and country codes.
///
/// # Arguments
///
/// * `relays` - Reference to an array of `RelayLocation` objects.
/// * `whitelist` - Reference to an array of whitelisted city codes or country codes.
/// * `blacklist` - Reference to an array of blacklisted city codes or country codes.
///
/// # Returns
///
/// * `Ok(())` - All validations were done with no errors.
/// * `Err(vpn::connect::VpnError)` - VpnError object with the appropriate error message.
pub fn validate_codes(
    relays: &[RelayLocation],
    whitelist: &[String],
    blacklist: &[String],
) -> Result<(), vpn::connect::VpnError> {
    let whitelist_set: HashSet<&String> = whitelist.iter().collect();
    let conflicts: Vec<String> = blacklist
        .iter()
        .filter(|entry| whitelist_set.contains(entry))
        .cloned()
        .collect();
    if !conflicts.is_empty() {
        return Err(vpn::connect::VpnError::Invalid(format!(
            "Conflicting whitelist and blacklist entries were found: {}",
            conflicts.join(", ")
        )));
    }
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
        Err(vpn::connect::VpnError::Invalid(format!(
            "Unknown country/city code(s): {}",
            invalid.join(", ")
        )))
    }
}
