use crate::{api, squire};
use reqwest::Client;
use std::collections::HashSet;
use std::path::PathBuf;

/// URL of the remote Git repository containing the preferred tracker list.
const TRACKER_LIST_URL: &str =
    "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_best.txt";

/// Contains the built-in tracker URLs used as a fallback when remote retrieval fails.
pub const TRACKERS: &[&str] = &[
    "udp://tracker.coppersurfer.tk:6969/announce",
    "udp://tracker.openbittorrent.com:6969/announce",
    "udp://tracker.bittor.pw:1337/announce",
    "udp://tracker.opentrackr.org:1337",
    "udp://bt.xxx-tracker.com:2710/announce",
    "udp://public.popcorn-tracker.org:6969/announce",
    "udp://eddie4.nl:6969/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://p4p.arenabg.com:1337/announce",
    "udp://tracker.tiny-vps.com:6969/announce",
    "udp://open.stealth.si:80/announce",
];

/// Fetch tracker URLs from the remote Git repository.
///
/// # Returns
///
/// Returns up to 50 unique tracker URLs using supported protocols (`udp://`, `http://`, and `https://`).
/// Returns an error if the request fails, the response has an unsuccessful status, or no valid trackers are found.
async fn fetch_git_trackers() -> api::schema::Result<Vec<String>> {
    log::info!("Fetching git trackers from {}", TRACKER_LIST_URL);
    let body = Client::new()
        .get(TRACKER_LIST_URL)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let mut seen = HashSet::new();

    // TODO: Remove hard coded upper bound
    let trackers = body
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.starts_with("udp://")
                || line.starts_with("https://")
                || line.starts_with("http://")
        })
        .filter(|line| seen.insert(line.to_string()))
        .take(50)
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if trackers.is_empty() {
        return Err("Tracker list contained no valid entries".into());
    }

    Ok(trackers)
}

/// Read tracker URLs from a local file.
///
/// # Arguments
///
/// * `filepath` - Path to the file containing tracker URLs.
///
/// # Returns
///
/// Returns up to 50 lines from the file as a vector of strings, or an I/O error if the file cannot be read.
async fn fetch_io_trackers(filepath: &PathBuf) -> std::io::Result<Vec<String>> {
    // TODO: Remove hard coded upper bound
    let contents = std::fs::read_to_string(filepath)?;
    Ok(contents.lines().take(50).map(String::from).collect())
}

/// Retrieve the configured local tracker file path from the environment.
///
/// # Returns
///
/// Returns the configured file path if the `trackers_file` environment variable is non-empty; otherwise, returns `None`.
fn get_trackers_file() -> Option<PathBuf> {
    // TODO: Move standalone env var to config and update readme
    let raw_trackers_file = squire::misc::get_env_var("trackers_file", None);
    let trackers_file: Option<PathBuf> = if raw_trackers_file.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw_trackers_file))
    };
    trackers_file
}

/// Retrieve tracker URLs from the configured sources in priority order.
///
/// Attempts to load trackers from the configured local file first, then fetches them from the remote Git repository.
/// Falls back to the built-in tracker list if the remote request fails.
///
/// # Returns
///
/// Returns a vector of tracker URLs from the first available source containing usable trackers.
pub async fn get() -> Vec<String> {
    let mut trackers: Vec<String> = Vec::new();
    if let Some(filepath) = &get_trackers_file() {
        let io_trackers = fetch_io_trackers(filepath).await.unwrap_or_else(|e| {
            log::warn!("Failed to read IO trackers: {e}");
            Vec::new()
        });
        if !io_trackers.is_empty() {
            log::info!("Trackers loaded from {:?}: {:?}", filepath.to_str(), trackers);
            trackers.extend(io_trackers);
        }
    }
    if !trackers.is_empty() {
        return trackers;
    }
    fetch_git_trackers().await.unwrap_or_else(|e| {
        log::warn!("Failed to fetch Git trackers, using hard coded list");
        log::error!("{e}");
        TRACKERS.iter().copied().map(String::from).collect()
    })
}
