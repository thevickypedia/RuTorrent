use crate::{config, notifier, vpn};
use std::io;
use std::process::Command;
use tokio::time::{Duration, sleep};

use crate::vpn::models::VpnError;

const GETTER: &[&str] = &["status", "--json"];
const SETTER: &[&str] = &["relay", "set", "location"];
const CONNECTOR: &[&str] = &["connect"];
const DISCONNECTOR: &[&str] = &["disconnect"];
const RESET: &[&str] = &["relay", "set", "location", "any"];
const RECONNECT: &[&str] = &["reconnect"];

/// Runs CLI commands.
///
/// # Arguments
///
/// * `args` - An array of arbitrary commands.
///
/// # Returns
///
/// Returns the `io::Result<String> ` object.
fn run_command(args: &[&str]) -> Result<String, VpnError> {
    let output = Command::new("mullvad").args(args).output().map_err(|e| {
        VpnError::Unavailable(if e.kind() == io::ErrorKind::NotFound {
            "'mullvad' CLI was not found in PATH".to_string()
        } else {
            format!("Failed to run 'mullvad': {e}")
        })
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(VpnError::Failed(if stderr.is_empty() {
            format!("'mullvad {}' exited with {}", args.join(" "), output.status)
        } else {
            stderr
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Disconnect VPN.
///
/// # Returns
///
/// Returns the `Result<vpn::models::MullvadResponse, VpnError>` object.
pub async fn disconnect() -> Result<vpn::models::MullvadResponse, VpnError> {
    // TODO: Disable kill switch or else the server may get into an un-reachable state
    run_command(DISCONNECTOR)?;
    log::info!("Disconnected vpn");
    await_connection("disconnected").await
}

/// Get current VPN status.
///
/// # Returns
///
/// Returns the `Result<vpn::models::MullvadResponse, VpnError>` object.
pub fn get_current() -> Result<vpn::models::MullvadResponse, VpnError> {
    // TODO: During connecting or disconnecting, details isn’t a location object
    //  GET /vpn will therefore return a 502 “Unexpected output” in those windows
    //  It’s accurate, but may prefer a lenient details object
    let out = run_command(GETTER)?;
    serde_json::from_str(&out)
        .map_err(|e| VpnError::Failed(format!("Unexpected output from 'mullvad status': {e}")))
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
/// Returns the `Result<vpn::models::MullvadResponse, VpnError>` object.
pub async fn await_connection(state: &str) -> Result<vpn::models::MullvadResponse, VpnError> {
    let mut last_state = String::from("unknown");
    for attempt in 0..5 {
        sleep(Duration::from_secs(3)).await;
        log::debug!("Checking connection (attempt {})", attempt + 1);
        let current = run_command(GETTER)?;
        // MARK: Avoid parsing as struct for transient state as the "details" node is "nothing"
        let Ok(response) = serde_json::from_str::<serde_json::Value>(&current) else {
            log::error!("Failed to parse JSON from GETTER");
            continue;
        };
        last_state = response["state"].as_str().unwrap_or_default().to_string();
        log::debug!("VPN state: {}", last_state);
        if last_state == "error" {
            return Err(VpnError::Failed(
                "Mullvad daemon reported an error state; run 'mullvad status' on the host".into(),
            ));
        }
        if last_state == state {
            let response = serde_json::from_value::<vpn::models::MullvadResponse>(response)
                .map_err(|e| VpnError::Failed(format!("Unexpected mullvad status payload: {e}")))?;
            if state == "connected" && response.details.is_none() {
                return Err(VpnError::Failed(
                    "Connected, but mullvad returned no location details".into(),
                ));
            }
            if let Some(d) = response.details.as_ref() {
                log::info!("Connection validated to {:?}", d.location.hostname);
            }
            return Ok(response);
        }
    }
    Err(VpnError::Timeout(format!(
        "VPN did not reach '{state}' within 15s (last state: '{last_state}')"
    )))
}

/// Connect to a VPN relay by node.
///
/// # Returns
///
/// Returns the `Result<vpn::models::MullvadResponse, VpnError>` object.
pub async fn connect_by_hostname(hostname: &str) -> Result<vpn::models::MullvadResponse, VpnError> {
    if hostname.starts_with('-')
        || !hostname
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(VpnError::Invalid(format!("Invalid hostname '{hostname}'")));
    }
    set_and_connect(&[hostname])?;
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
/// Returns `Result<(), VpnError>` to indicate connection state.
fn set_and_connect(selection: &[&str]) -> Result<(), VpnError> {
    let args: Vec<&str> = SETTER.iter().chain(selection).copied().collect();
    // CLI rejecting the relay => bad input (400); CLI missing stays 503
    run_command(&args).map_err(|e| match e {
        VpnError::Failed(m) => VpnError::Invalid(format!(
            "Relay '{}' rejected by mullvad: {m}",
            selection.join(" ")
        )),
        other => other,
    })?;
    log::debug!("Relay set to {}", selection.join(" "));
    run_command(CONNECTOR)?;
    log::debug!("Connected to {}", selection.join(" "));
    Ok(())
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
/// Returns the `Result<vpn::models::MullvadResponse, VpnError>` object.
async fn target_connector(
    relay: &vpn::models::RelayLocation,
) -> Result<vpn::models::MullvadResponse, VpnError> {
    log::info!(
        "Selecting a relay from '{} {}' - {}, {}",
        relay.country_code,
        relay.city_code,
        relay.city,
        relay.country
    );
    set_and_connect(&[&relay.country_code, &relay.city_code])?;
    await_connection("connected").await
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
/// Returns the `Result<vpn::models::MullvadResponse, VpnError>` object.
pub async fn shuffler(
    whitelist: Option<Vec<String>>,
    blacklist: Option<Vec<String>>,
    config: &config::env::Config,
) -> Result<vpn::models::MullvadResponse, VpnError> {
    let whitelisted = whitelist.unwrap_or_default();
    let blacklisted = blacklist.unwrap_or_default();
    if !whitelisted.is_empty() || !blacklisted.is_empty() {
        log::debug!("whitelist: {:?}", whitelisted);
        log::debug!("blacklist: {:?}", blacklisted);
        let relay_map = vpn::relay::generate_relay_map(config.mullvad_relay_cache.to_owned())
            .await
            .map_err(|e| VpnError::Unavailable(format!("Failed to load relay list: {e}")))?;
        vpn::relay::validate_codes(&relay_map, &whitelisted, &blacklisted)?;
        let relay = vpn::relay::select_relay(relay_map, &whitelisted, &blacklisted)
            .await
            .ok_or_else(|| {
                VpnError::Invalid(
                    "No relay matches the whitelist/blacklist combination (are they conflicting?)"
                        .into(),
                )
            })?;
        match target_connector(&relay).await {
            Ok(connected) => return Ok(connected),
            Err(VpnError::Unavailable(m)) => return Err(VpnError::Unavailable(m)), // fallbacks are pointless
            Err(e) => {
                log::warn!("Targeted connection failed ({e}). Trying whitelisted regions.");
                for selection in &whitelisted {
                    if set_and_connect(&[selection]).is_ok()
                        && let Ok(connected) = await_connection("connected").await
                    {
                        return Ok(connected);
                    }
                }
                log::warn!("Failed to connect to preferred location. Resetting preferences..");
            }
        }
    }
    if let Err(e) = run_command(RESET) {
        log::error!("Failed to reset mullvad relay preferences - {}", e);
    }
    let connected = get_current().is_ok_and(|c| c.state == "connected");
    let (cmd, log_st) = if connected {
        log::info!("Existing connection found; resetting.");
        (RECONNECT, "reconnect")
    } else {
        log::info!("No existing connection found; establishing.");
        (CONNECTOR, "connect")
    };
    let result = match run_command(cmd) {
        Ok(_) => await_connection("connected").await,
        Err(e) => Err(e),
    };
    if let Err(e) = &result {
        log::error!("Failed to {} - {}", log_st, e);
        notifier::notify(
            "RuTorrent: VPN ERROR".to_string(),
            "Failed to connect to mullvad VPN even after a relay reset.".to_string(),
            config.to_owned(),
        );
    }
    result
}
