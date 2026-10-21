use crate::{config, notifier, vpn};
use std::io;
use std::process::Command;
use tokio::time::{Duration, sleep};

const GETTER: &str = "mullvad status --json";
const SETTER: &str = "mullvad relay set location";
const CONNECTOR: &str = "mullvad connect";
const DISCONNECTOR: &str = "mullvad disconnect";
const RESET: &str = "mullvad relay set location any";
const RECONNECT: &str = "mullvad reconnect";

/// Runs CLI commands.
///
/// # Arguments
///
/// * `command` - Reference to the command string.
///
/// # Returns
///
/// Returns the `io::Result<String> ` object.
fn run_command(command: &str) -> io::Result<String> {
    let output = Command::new("sh").arg("-c").arg(command).output()?;

    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Disconnect VPN.
///
/// # Returns
///
/// Returns the `Option<vpn::models::MullvadResponse>` object.
pub async fn disconnect() -> Option<vpn::models::MullvadResponse> {
    match run_command(DISCONNECTOR) {
        Ok(_) => {
            log::info!("Disconnected vpn");
        }
        Err(e) => {
            log::error!("Failed to disconnect VPN - {}", e);
            return None;
        }
    }
    await_connection("disconnected").await
}

/// Get current VPN status.
///
/// # Returns
///
/// Returns the `Option<vpn::models::MullvadResponse>` object.
pub fn get_current() -> Option<vpn::models::MullvadResponse> {
    let current = run_command(GETTER).ok()?;
    let response: vpn::models::MullvadResponse = serde_json::from_str(&current).ok()?;
    Some(response)
}

/// Waits for the connection to get to the given state.
///
/// Makes 5 attempts with 3s intervals between each attempt.
///
/// # Arguments
///
/// * `state` - State to await.
///
/// # Returns
///
/// Returns the `Option<vpn::models::MullvadResponse>` object.
pub async fn await_connection(state: &str) -> Option<vpn::models::MullvadResponse> {
    for attempt in 0..5 {
        log::debug!("Sleeping for 3s");
        sleep(Duration::from_secs(3)).await;
        log::info!("Checking connection (attempt {})", attempt + 1);
        match run_command(GETTER) {
            Ok(current) => {
                // MARK: Avoid parsing as struct for transient state as the "details" node is "nothing"
                let Ok(response) = serde_json::from_str::<serde_json::Value>(&current) else {
                    log::error!("Failed to parse JSON from GETTER");
                    continue
                };
                let current_state = response["state"].as_str().unwrap_or_default();
                log::debug!("VPN state: {}", current_state);
                if current_state == state {
                    // MARK: Parse as MullvadResponse
                    let Ok(response) = serde_json::from_value::<vpn::models::MullvadResponse>(response) else {
                        log::error!("Failed to parse Value derived from getter response: {}", current);
                        return None;
                    };
                    let details = match response.details.as_ref() {
                        Some(details) => details,
                        None => {
                            log::error!("Response is connected but details is None");
                            return None;
                        }
                    };
                    let location = &details.location;
                    log::debug!("Connection validated to {:?}", location.hostname);
                    return Some(response);
                }
            }
            Err(e) => {
                log::error!("Failed to run getter: {}", e);
                return None;
            }
        }
    }
    None
}

/// Connect to a VPN relay by node.
///
/// # Returns
///
/// Returns the `Option<vpn::models::MullvadResponse>` object.
pub async fn connect_by_node(node: &String) -> Option<vpn::models::MullvadResponse> {
    match run_command(&format!("{} {}", SETTER, node)) {
        Ok(_) => {
            log::info!("Relay set to {}", node);
        },
        Err(e) => {
            log::error!("Failed to set to {} - {}", node, e);
            return None;
        }
    }
    match run_command(CONNECTOR) {
        Ok(_) => {
            log::info!("Connected to {}", node);
        }
        Err(e) => {
            log::error!("Failed to connect to {} - {}", node, e);
            return None;
        }
    }
    await_connection("connected").await
}

/// Sets a relay selection and attempst to connect to it.
///
/// # Arguments
///
/// * `selection` - User selection which can be a hostname or 'country city' combination.
///
/// # Returns
///
/// Returns `true` if connected, `false` otherwise.
async fn set_and_connect(selection: &String) -> bool {
    match run_command(&format!("{} {}", SETTER, selection)) {
        Ok(_) => {
            log::debug!("Relay set to {}", selection);
        }
        Err(e) => {
            log::error!("Failed to set to {} - {}", selection, e);
            return false;
        }
    }
    match run_command(CONNECTOR) {
        Ok(_) => {
            log::debug!("Connected to {}", selection);
            return true;
        }
        Err(e) => {
            log::error!("Failed to connect to {} - {}", selection, e);
        }
    }
    false
}

/// Picks a random relay location and establishes VPN connection.
///
/// # Arguments
///
/// * `whitelist` - An array of city [OR] country codes to whitelist.
/// * `blacklist` - An array of city [OR] country codes to blacklist.
///
/// # Returns
///
/// Returns the `Option<vpn::models::MullvadResponse>` object.
pub async fn target_connector(
    whitelist: &Vec<String>,
    blacklist: &Vec<String>,
) -> Option<vpn::models::MullvadResponse> {
    let Ok(relay_map) = vpn::relay::generate_relay_map().await else {
        log::info!("Failed to generate relay map");
        return None;
    };
    log::debug!("Generated a relay map with {} countries", relay_map.len());
    let Some(relay) = vpn::relay::select_relay(relay_map, whitelist, blacklist).await else {
        log::info!("Failed to select a relay");
        return None;
    };
    // MARK: Format is 'mullvad relay set location $countryCode $cityCode $hostName'
    let selection = &format!("{} {}", relay.country_code, relay.city_code);
    log::info!("Selecting a relay from '{}' - {}, {}", selection, relay.city, relay.country);
    if set_and_connect(selection).await {
        return await_connection("connected").await
    };
    None
}

/// Chooses targeted connection if whitelist and blacklist are provided.
/// Otherwise, resets the relay preference and lets mullvad's `RelaySelector` pick one.
///
/// `RelaySelector` uses a weighted roulette algorithm to pick the relay,
/// meaning more popular servers have a higher probability of selection
///
/// # Warnings
///
/// 1. Attempts to select and connect to a location using the provided whitelist and blacklist.
/// 2. If the connection fails and a whitelist is provided, attempts to connect to each whitelisted region until successful or all options are exhausted.
/// 3. If all attempts fail, resets the relay preferences and reconnects using the default relay selection.
///
/// # Arguments
///
/// * `whitelist` - Optional array of city [OR] country codes to whitelist.
/// * `blacklist` - Optional array of city [OR] country codes to blacklist.
///
/// # Returns
///
/// Returns the `Option<vpn::models::MullvadResponse>` object.
pub async fn shuffler(
    whitelist: Option<Vec<String>>,
    blacklist: Option<Vec<String>>,
    config: &config::env::Config,
) -> Option<vpn::models::MullvadResponse> {
    let whitelisted = whitelist.unwrap_or_default();
    let blacklisted = blacklist.unwrap_or_default();
    if !whitelisted.is_empty() || !blacklisted.is_empty() {
        if let Some(connected) = target_connector(&whitelisted, &blacklisted).await {
            return Some(connected);
        } else {
            if !whitelisted.is_empty() {
                log::warn!(
                    "Failed to connect to the chosen relay. Trying to connect to whitelisted regions."
                );
                for selection in whitelisted {
                    log::debug!("Selected region: {}", selection);
                    if set_and_connect(&selection).await
                        && let Some(connected_bak) = await_connection("connected").await {
                            return Some(connected_bak);
                        }
                }
            }
            log::warn!("Failed to connect to preferred location. Resetting preferences..")
        }
    }
    match run_command(RESET) {
        Ok(_) => {
            log::debug!("mullvad relay preferences have been reset");
        }
        Err(e) => {
            log::error!("Failed to reset mullvad relay preferences - {}", e);
        }
    }
    let connected = get_current().is_some_and(|connect| connect.state == "connected");
    let (cmd, log_st) = if connected {
        log::info!("Existing connection found; resetting.");
        (RECONNECT, "reconnect")
    } else {
        log::info!("No existing connection found; establishing.");
        (CONNECTOR, "connect")
    };
    match run_command(cmd) {
        Ok(_) => {
            log::debug!("vpn has been {}ed", log_st);
            if let Some(connected_bak) = await_connection("connected").await {
                return Some(connected_bak);
            }
        }
        Err(e) => {
            log::error!("Failed to {} - {}", log_st, e);
        }
    }
    notifier::notify(
        "RuTorrent: VPN ERROR".to_string(),
        "Failed to connect to mullvad VPN even after a relay reset.".to_string(),
        config.to_owned(),
    );
    None
}
