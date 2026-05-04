use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;

/// ### SharedState
/// Shared application state for tracking active rsync operations.
pub type SharedState = Arc<RwLock<HashMap<String, RsyncTrack>>>;
/// ### PendingMap
/// Shared map for storing pending torrent metadata before resolution.
pub type PendingMap = Arc<RwLock<HashMap<String, PutItem>>>;

/// ### Config
/// Application configuration loaded from environment variables.
#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub utc_logger: bool,
}

/// ### Config::new
/// Creates a new [`Config`] instance from environment variables.
///
/// # Returns
///
/// A [`Config`] populated with environment values or sensible defaults.
impl Config {
    pub fn new() -> Self {
        let host = env::var("HOST").unwrap_or("127.0.0.1".to_string());
        let port = env::var("PORT")
            .unwrap_or("3000".to_string())
            .parse::<u16>()
            .unwrap();
        let base_url = env::var("BASE_URL").unwrap_or("http://localhost:8080".to_string());
        let username = env::var("USERNAME").unwrap_or_default();
        let password = env::var("PASSWORD").unwrap_or_default();
        let utc_logger = env::var("UTC_LOGGER").unwrap_or("true".to_string()) == "true";

        Self {
            host,
            port,
            base_url,
            username,
            password,
            utc_logger,
        }
    }
}

/// ### Status
/// Represents the current status of a torrent or transfer.
#[derive(Clone, Debug, Serialize)]
pub enum Status {
    Downloading(f64),
    Copying(f64),
    Completed,
}

/// ### RsyncTrack
/// Tracks a torrent and its associated rsync transfer state.
#[derive(Clone, Debug)]
pub struct RsyncTrack {
    pub name: String,
    pub status: Status,
    pub rsync: Option<RsyncTarget>,
}

/// ### RsyncTarget
/// Defines a remote rsync destination.
#[derive(Clone, Debug, Deserialize)]
pub struct RsyncTarget {
    pub host: String,
    pub username: String,
    pub path: String,
}

/// ### PutItem
/// Represents an incoming request to add a new torrent with optional rsync target details.
#[derive(Deserialize, Clone)]
pub struct PutItem {
    pub url: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub path: String,
}
