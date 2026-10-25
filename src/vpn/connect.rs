use crate::{config, notifier, vpn};
use std::io;
use tokio::process::Command;
use tokio::time::{Duration, sleep};

use crate::vpn::models::VpnError;

const GETTER: &[&str] = &["status", "--json"];
const SETTER: &[&str] = &["relay", "set", "location"];
const CONNECTOR: &[&str] = &["connect"];
const DISCONNECTOR: &[&str] = &["disconnect"];
const LOCKDOWN_OFF: &[&str] = &["lockdown-mode", "set", "off"];
const LOCKDOWN_ON: &[&str] = &["lockdown-mode", "set", "on"];
const RESET: &[&str] = &["relay", "set", "location", "any"];
const RECONNECT: &[&str] = &["reconnect"];
const CMD_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RELAY_ATTEMPTS: usize = 3;

/// Runs CLI commands.
///
/// # Arguments
///
/// * `args` - An array of arbitrary commands.
///
/// # Returns
///
/// Returns the `Result<String, VpnError>` object.
async fn run_command(args: &[&str]) -> Result<String, VpnError> {
    let output = tokio::time::timeout(
        CMD_TIMEOUT,
        Command::new("mullvad")
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| {
        VpnError::Timeout(format!(
            "'mullvad {}' did not respond within {}s",
            args.join(" "),
            CMD_TIMEOUT.as_secs()
        ))
    })?
    .map_err(|e| {
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
/// # Arguments
///
/// * `lockdown` - Boolean flag to enable/disable lockdown mode before disconnecting vpn.
///
/// # Returns
///
/// Returns the `Result<vpn::models::MullvadResponse, VpnError>` object.
pub async fn disconnect(lockdown: bool) -> Result<vpn::models::MullvadResponse, VpnError> {
    if lockdown {
        log::info!("Enabling lockdown mode");
        run_command(LOCKDOWN_ON).await?;
    } else {
        log::info!("Disabling lockdown mode");
        run_command(LOCKDOWN_OFF).await?;
    }
    run_command(DISCONNECTOR).await?;
    log::info!("Disconnected vpn");
    await_connection("disconnected").await
}

/// Get current VPN status.
///
/// # Returns
///
/// Returns the `Result<vpn::models::MullvadResponse, VpnError>` object.
pub async fn get_current() -> Result<vpn::models::MullvadResponse, VpnError> {
    let out = run_command(GETTER).await?;
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
        let current = run_command(GETTER).await?;
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
            return Ok(response);
        }
    }
    Err(VpnError::Timeout(format!(
        "VPN did not reach '{state}' within 15s (last state: '{last_state}')"
    )))
}

/// Connect to a VPN relay by hostname.
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
    set_and_connect(&[hostname]).await?;
    await_connection("connected").await
}

/// Sets a relay selection and attempts to connect to it.
///
/// # Arguments
///
/// * `selection` - User selection which can be a hostname or 'country city' combination.
///
/// # Returns
///
/// Returns `Result<(), VpnError>` to indicate connection state.
async fn set_and_connect(selection: &[&str]) -> Result<(), VpnError> {
    let args: Vec<&str> = SETTER.iter().chain(selection).copied().collect();
    // CLI rejecting the relay => bad input (400); CLI missing stays 503
    run_command(&args).await.map_err(|e| match e {
        VpnError::Failed(m) => VpnError::Invalid(format!(
            "Relay '{}' rejected by mullvad: {m}",
            selection.join(" ")
        )),
        other => other,
    })?;
    log::debug!("Relay set to {}", selection.join(" "));
    run_command(CONNECTOR).await?;
    log::debug!("Connected to {}", selection.join(" "));
    Ok(())
}

/// Chooses targeted connection if whitelist and blacklist are provided.
/// Otherwise, resets the relay preference and lets mullvad's `RelaySelector` pick one.
///
/// `RelaySelector` uses a weighted roulette algorithm to pick the relay,
/// meaning more popular servers have a higher probability of selection
///
/// # Warnings
///
/// Worst case scenario this can run up to 45 seconds.
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
        let candidates = vpn::relay::eligible_relays(relay_map, &whitelisted, &blacklisted);
        if candidates.is_empty() {
            return Err(VpnError::Invalid(
                "No relay matches the whitelist/blacklist combination".into(),
            ));
        }
        let mut last_err = None;
        for relay in candidates.iter().take(MAX_RELAY_ATTEMPTS) {
            log::info!(
                "Selecting a relay from '{} {}' - {}, {}",
                relay.country_code,
                relay.city_code,
                relay.city,
                relay.country
            );
            if let Err(e) = set_and_connect(&[&relay.country_code, &relay.city_code]).await {
                if matches!(e, VpnError::Unavailable(_)) {
                    return Err(e);
                }
                log::warn!(
                    "Relay {}-{} rejected: {e}",
                    relay.country_code,
                    relay.city_code
                );
                last_err = Some(e);
                continue;
            }
            match await_connection("connected").await {
                Ok(connected) => return Ok(connected),
                Err(e @ VpnError::Unavailable(_)) => return Err(e), // retrying is pointless
                Err(e) => {
                    log::warn!(
                        "Connecting to {}-{} failed: {e}",
                        relay.country_code,
                        relay.city_code
                    );
                    last_err = Some(e);
                }
            }
        }
        // Don't leave the daemon pinned to a relay that failed
        let _ = run_command(RESET).await;
        notifier::notify(
            "RuTorrent: VPN ERROR".to_string(),
            format!(
                "Failed to connect to a relay matching the whitelist/blacklist after {MAX_RELAY_ATTEMPTS} attempts."
            ),
            config.to_owned(),
        );
        let err = match last_err {
            Some(VpnError::Invalid(m)) => VpnError::Failed(m),
            Some(e) => e,
            None => VpnError::Failed("No connection attempt was made".into()),
        };
        return Err(err);
    }
    if let Err(e) = run_command(RESET).await {
        log::error!("Failed to reset mullvad relay preferences - {}", e);
    }
    let connected = get_current().await.is_ok_and(|c| c.state == "connected");
    let (cmd, log_st) = if connected {
        log::info!("Existing connection found; resetting.");
        (RECONNECT, "reconnect")
    } else {
        log::info!("No existing connection found; establishing.");
        (CONNECTOR, "connect")
    };
    let result = match run_command(cmd).await {
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
