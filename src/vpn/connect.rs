use std::process::Command;
use std::io;
use crate::vpn;
use tokio::time::{sleep, Duration};

const GETTER: &str = "mullvad status --json";
const SETTER: &str = "mullvad relay set location";
const CONNECTOR: &str = "mullvad connect";
const DISCONNECTOR: &str = "mullvad disconnect";

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
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()?;

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
    // TODO: When connecting to a bad node, the server can be left in an un-reachable state
    //       Check for a way to set with a timer in mullvad or do it manually in rutorrent
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

/// Picks a random relay and establishes VPN connection.
///
/// # Arguments
///
/// * `whitelist` - An array of city [OR] country codes to whitelist.
/// * `blacklist` - An array of city [OR] country codes to blacklist.
///
/// # Returns
///
/// Returns the `Option<vpn::models::MullvadResponse>` object.
pub async fn shuffler(
    whitelist: Option<Vec<String>>,
    blacklist: Option<Vec<String>>,
) -> Option<vpn::models::MullvadResponse> {
    let Ok(relay_map) = vpn::relay::generate_relay_map().await else {
        log::info!("Failed to generate relay map");
        return None;
    };
    log::debug!("Generated a relay map with {} countries", relay_map.len());
    let Some(relay) = vpn::relay::select_relay(relay_map, whitelist, blacklist).await else {
        log::info!("Failed to select a relay");
        return None;
    } ;
    log::info!("Selected the relay {} in {}, {}", relay.hostname, relay.city_name, relay.country_name);
    match run_command(&format!("{} {}", SETTER, relay.hostname)) {
        Ok(_) => {
            log::debug!("Relay set to {}", relay.hostname);
        },
        Err(e) => {
            log::error!("Failed to set to {} - {}", relay.hostname, e);
            return None;
        }
    }
    match run_command(CONNECTOR) {
        Ok(_) => {
            log::debug!("Connected to {}", relay.hostname);
        }
        Err(e) => {
            log::error!("Failed to connect to {} - {}", relay.hostname, e);
            return None;
        }
    }
    await_connection("connected").await
}
