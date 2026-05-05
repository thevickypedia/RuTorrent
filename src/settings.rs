use crate::squire;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub qbit_api: String,
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
        let host = squire::get_env_var("host", Some("127.0.0.1"));
        let port = squire::get_env_var("port", Some("3000"))
            .parse::<u16>()
            .unwrap();

        let qbit_api = squire::get_env_var("qbit_api", Some("http://localhost:8080"));
        let username = squire::get_env_var("username", None);
        let password = squire::get_env_var("password", None);

        let utc_logger = squire::get_env_var("utc_logger", Some("true")) == "true";

        Self {
            host,
            port,
            qbit_api,
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
#[derive(Deserialize, Clone, Debug)]
pub struct PutItem {
    pub url: String,

    pub name: Option<String>,
    pub hash: Option<String>,
    pub trackers: Option<Vec<String>>,

    #[serde(default = "default_save_path")]
    pub save_path: String,

    #[serde(default = "default_host")]
    pub remote_host: String,
    #[serde(default = "default_username")]
    pub remote_username: String,
    #[serde(default = "default_path")]
    pub remote_path: String,
}

fn default_host() -> String {
    squire::get_env_var("remote_host", None)
}

fn default_username() -> String {
    squire::get_env_var("remote_user", None)
}

fn default_path() -> String {
    squire::get_env_var("remote_path", None)
}

fn default_save_path() -> String {
    squire::get_env_var("save_path", None)
}
