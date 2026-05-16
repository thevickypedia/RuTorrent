use reqwest::Client;
use serde_json::Value;
use std::path::Path;

use crate::{settings, squire};

/// Constructs a default "Downloads" directory in the HOME folder.
///
/// # Arguments
///
/// * `child_dir` - Child directory that has to be appended to the default "Downloads" folder.
///
/// # Returns
///
/// Returns the constructed "Downloads" directory.
fn default_download_path(child_dir: &str) -> String {
    match dirs::home_dir() {
        Some(home) => {
            let path = home.join("Downloads").join(child_dir);
            match path.to_str() {
                Some(path_str) => path_str.to_string(),
                None => {
                    log::warn!("Downloads path contains invalid UTF-8, falling back to /tmp");
                    format!("/tmp/{}", child_dir)
                }
            }
        }
        None => {
            log::info!("Could not determine HOME directory, falling back to /tmp");
            format!("/tmp/{}", child_dir)
        }
    }
}

/// Fetches the default save path configured in qBittorrent.
///
/// # Arguments
///
/// * `client` - Authenticated HTTP client for qBittorrent API requests.
/// * `config` - Application configuration containing the qBittorrent URL.
///
/// # Returns
///
/// Returns the default save path as a `String`, or a fallback path if the
/// request fails or the field is absent.
pub async fn get_default_save_path(
    client: &Client,
    config: &settings::Config,
    child_dir: &String,
) -> String {
    // 1. Check environment variable override
    let default_save_env = squire::get_env_var("save_path", None);
    if !default_save_env.is_empty() {
        match std::fs::create_dir_all(&default_save_env) {
            Ok(_) => {
                log::debug!(
                    "Verified save_path environment directory exists: {}",
                    default_save_env
                );
            }
            Err(err) => {
                log::warn!(
                    "Failed to create save_path (from env) directory '{}': {}",
                    default_save_env,
                    err
                );
            }
        }
        let joined = Path::new(&default_save_env).join(child_dir);
        return joined.to_string_lossy().into_owned();
    }

    // 2. Fetch qBittorrent preferences
    let response = match client
        .get(format!("{}/api/v2/app/preferences", config.qbit_url))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            log::error!("Failed to fetch qBittorrent preferences: {}", err);
            return default_download_path(child_dir);
        }
    };

    // 3. Parse JSON response
    let resp_json: Value = match response.json().await {
        Ok(json) => json,
        Err(err) => {
            log::error!("Failed to parse qBittorrent preferences JSON: {}", err);
            return default_download_path(child_dir);
        }
    };

    // 4. Extract save_path
    match resp_json["save_path"].as_str() {
        Some(path) if !path.is_empty() => {
            log::info!("Using qBittorrent save path: {}", path);
            Path::new(path)
                .join(child_dir)
                .to_string_lossy()
                .into_owned()
        }
        Some(_) => {
            log::info!("qBittorrent save_path is empty, using fallback");
            default_download_path(child_dir)
        }
        None => {
            log::info!("qBittorrent preferences missing save_path, using fallback");
            default_download_path(child_dir)
        }
    }
}
