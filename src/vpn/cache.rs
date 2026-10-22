use crate::vpn::models::RelayLocation;
use std::{fs::File, io::BufReader, path::PathBuf};

/// Get default cache location allocated based on the operating system.
///
/// # References
///
/// https://github.com/mullvad/mullvadvpn-app#cache
///
/// # Returns
///
/// Returns an `Option<PathBuf` object if cache location exists.
pub fn get_cache() -> Option<PathBuf> {
    let path = match std::env::consts::OS {
        "linux" => "/var/cache/mullvad-vpn/relays.json",
        "macos" => "/Library/Caches/mullvad-vpn/relays.json",
        "windows" => r"C:\ProgramData\Mullvad VPN\cache\relays.json",
        _ => return None,
    };
    let filepath = PathBuf::from(path);
    filepath.exists().then_some(filepath)
}

/// Resolve the filepath based on its presence.
///
/// # Preferences
///
/// * Checks if user provided filepath exists.
/// * Fall back to the default cache location.
/// * If neither exists returns None.
///
/// # Arguments
///
/// * `filepath` - Filepath provided by the user.
///
/// # Returns
///
/// Returns the cache filepath if exists.
fn resolve_path(filepath: Option<PathBuf>) -> Option<PathBuf> {
    match filepath {
        Some(path) => {
            if path.exists() {
                Some(path)
            } else {
                log::error!("Relay locations file does not exist: {}", path.display());
                None
            }
        }
        None => get_cache(),
    }
}

/// Loads all relay locations into a struct.
///
/// # Arguments
///
/// * `filepath` - Filepath provided by the user.
///
/// # Returns
///
/// Returns an `Option<Vec<RelayLocation>>` object.
pub fn load_relay_locations(filepath: Option<PathBuf>) -> Option<Vec<RelayLocation>> {
    let path = match resolve_path(filepath) {
        Some(path) => path,
        None => {
            log::error!("No valid relay locations file found");
            return None;
        }
    };

    let file = match File::open(&path) {
        Ok(file) => file,
        Err(e) => {
            log::error!(
                "Failed to open relay locations file '{}': {}",
                path.display(),
                e
            );
            return None;
        }
    };

    let reader = BufReader::new(file);

    let json: serde_json::Value = match serde_json::from_reader(reader) {
        Ok(json) => json,
        Err(e) => {
            log::error!(
                "Failed to parse relay locations JSON '{}': {}",
                path.display(),
                e
            );
            return None;
        }
    };

    let locations = match json.get("locations") {
        Some(locations) => locations,
        None => {
            log::error!("Missing 'locations' field in '{}'", path.display());
            return None;
        }
    };

    let locations = match locations.as_object() {
        Some(locations) => locations,
        None => {
            log::error!(
                "'locations' field is not a JSON object in '{}'",
                path.display()
            );
            return None;
        }
    };

    let mut relay_locations = Vec::new();

    for (code, location) in locations {
        let (country_code, city_code) = match code.split_once('-') {
            Some(codes) => codes,
            None => {
                log::warn!("Skipping relay location with invalid code: '{}'", code);
                continue;
            }
        };

        let city = match location.get("city").and_then(|v| v.as_str()) {
            Some(city) => city.to_string(),
            None => {
                log::warn!(
                    "Skipping relay location '{}': missing or invalid city",
                    code
                );
                continue;
            }
        };

        let country = match location.get("country").and_then(|v| v.as_str()) {
            Some(country) => country.to_string(),
            None => {
                log::warn!(
                    "Skipping relay location '{}': missing or invalid country",
                    code
                );
                continue;
            }
        };

        relay_locations.push(RelayLocation {
            city,
            country,
            city_code: city_code.to_string(),
            country_code: country_code.to_string(),
        });
    }

    if relay_locations.is_empty() {
        log::error!("No valid relay locations found in '{}'", path.display());
        return None;
    }

    Some(relay_locations)
}
