use std::process::Command;
use std::io;
use crate::vpn;
use tokio::time::{sleep, Duration};

const GETTER: &str = "mullvad status --json";
const SETTER: &str = "mullvad relay set location";
const CONNECTOR: &str = "mullvad connect";

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

pub fn get_current() -> Option<vpn::models::MullvadResponse> {
    let current = run_command(GETTER).ok()?;
    let response: vpn::models::MullvadResponse = serde_json::from_str(&current).ok()?;
    Some(response)
}

pub async fn await_connection() -> Option<vpn::models::MullvadResponse> {
    // TODO: When connecting to a bad node, the server can be left in an un-reachable state
    //       Check for a way to set with a timer in mullvad or do it manually in rutorrent
    for attempt in 0..5 {
        log::info!("Checking connection (attempt {})", attempt + 1);
        match run_command(GETTER) {
            Ok(current) => {
                let response: vpn::models::MullvadResponse =
                    match serde_json::from_str(&current) {
                        Ok(response) => response,
                        Err(e) => {
                            log::error!("Failed to parse getter response: {}", e);
                            return None;
                        }
                    };
                log::info!("VPN state: {}", response.state);
                if response.state == "connected" {
                    let details = match response.details.as_ref() {
                        Some(details) => details,
                        None => {
                            log::error!("Response is connected but details is None");
                            return None;
                        }
                    };
                    let location = &details.location;
                    log::info!("Connection validated to {:?}", location.hostname);
                    return Some(response);
                }
            }
            Err(e) => {
                log::error!("Failed to run getter: {}", e);
                return None;
            }
        }
        log::info!("Sleeping for 3s");
        sleep(Duration::from_secs(3)).await;
    }
    None
}

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
    await_connection().await
}

pub async fn shuffler(
    whitelist: Option<Vec<String>>,
    blacklist: Option<Vec<String>>,
) -> Option<vpn::models::MullvadResponse> {
    if let Some(current) = get_current() {
        if current.state == "connected" {
            match current.details.as_ref() {
                Some(details) => {
                    let location = &details.location;
                    log::info!("Currently connected to {}, {} - {:?} [{:?}]", location.city, location.country, location.hostname, location.ipv4);
                },
                None => {
                    log::warn!("Response is connected but details is None");
                }
            };
        } else {
            log::info!("Currently not connected to a relay")
        }
    } else {
        log::error!("Failed to get current connection status");
    }
    let Ok(relays) = vpn::relay::generate_relay_map().await else {
        log::info!("Failed to generate relay map");
        return None;
    };
    log::info!("Generated a relay map with {} countries", relays.len());
    let Some(relay) = vpn::relay::select_relay(relays, whitelist, blacklist).await else {
        log::info!("Failed to select a relay");
        return None;
    } ;
    log::info!("Selected the relay {} in {}, {}", relay.hostname, relay.city_name, relay.country_name);
    match run_command(&format!("{} {}", SETTER, relay.hostname)) {
        Ok(_) => {
            log::info!("Relay set to {}", relay.hostname);
        },
        Err(e) => {
            log::error!("Failed to set to {} - {}", relay.hostname, e);
            return None;
        }
    }
    match run_command(CONNECTOR) {
        Ok(_) => {
            log::info!("Connected to {}", relay.hostname);
        }
        Err(e) => {
            log::error!("Failed to connect to {} - {}", relay.hostname, e);
            return None;
        }
    }
    await_connection().await
}
