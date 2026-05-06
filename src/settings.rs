use crate::squire;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;

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
    pub apikey: String,
    pub workers: usize,

    pub qbit_api: String,

    pub username: String,
    pub password: String,

    pub utc_logger: bool,
    pub log_level: log::LevelFilter,

    pub ntfy_url: String,
    pub ntfy_topic: String,
    pub ntfy_username: String,
    pub ntfy_password: String,
}

fn startup_error(msg: &str) {
    eprintln!("\nStartupError:\n\t{}\n", msg);
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
        let apikey = squire::get_env_var("apikey", None);
        if apikey.is_empty() {
            startup_error("'apikey' is empty");
            std::process::exit(1)
        }
        match squire::complexity_checker(&apikey, 32) {
            Ok(()) => (),
            Err(err) => {
                startup_error(format!("Invalid 'apikey': {}", err).as_str());
                std::process::exit(1)
            }
        }

        let available_workers = std::thread::available_parallelism().map_or(2, NonZeroUsize::get);
        let default_workers =
            squire::get_env_var("workers", Some(available_workers.to_string().as_str()));
        let workers = match default_workers.parse::<usize>() {
            Ok(n) if n > 0 => n,
            Ok(_) => {
                startup_error(format!("'workers' must be > 0, got {}", default_workers).as_str());
                std::process::exit(1)
            }
            Err(e) => {
                startup_error(
                    format!("Invalid 'workers' value '{default_workers}': {e}\n").as_str(),
                );
                std::process::exit(1)
            }
        };

        let mut qbit_api = squire::get_env_var("qbit_api", Some("http://localhost:8080/"));
        let username = squire::get_env_var("username", None);
        let password = squire::get_env_var("password", None);
        qbit_api = qbit_api.strip_suffix("/").unwrap_or(&qbit_api).to_string();

        let utc_logger = squire::get_env_var("utc_logger", Some("true")) == "true";
        let default_log_level = squire::get_env_var("log_level", Some("info"));
        let log_level = match default_log_level.parse::<log::LevelFilter>() {
            Ok(level) => level,
            Err(_) => {
                startup_error(
                    format!(
                        "Invalid 'log_level' value '{default_log_level}'. Expected one of: off, error, warn, info, debug, trace"
                    ).as_str()
                );
                std::process::exit(1)
            }
        };

        let mut ntfy_url = squire::get_env_var("ntfy_url", None);
        let mut ntfy_topic = squire::get_env_var("ntfy_topic", None);
        let ntfy_username = squire::get_env_var("ntfy_username", None);
        let ntfy_password = squire::get_env_var("ntfy_password", None);

        ntfy_url = ntfy_url.strip_suffix("/").unwrap_or(&ntfy_url).to_string();
        ntfy_topic = ntfy_topic.strip_prefix("/").unwrap_or(&ntfy_topic).to_string();

        Self {
            host,
            port,
            apikey,
            workers,
            qbit_api,
            username,
            password,
            utc_logger,
            log_level,
            ntfy_url,
            ntfy_topic,
            ntfy_username,
            ntfy_password,
        }
    }
}

/// ### Status
/// Represents the current status of a torrent or transfer.
#[derive(Clone, Debug, serde::Serialize)]
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
#[derive(Clone, Debug, serde::Deserialize)]
pub struct RsyncTarget {
    pub host: String,
    pub username: String,
    pub path: String,
}

/// ### PutItem
/// Represents an incoming request to add a new torrent with optional rsync target details.
#[derive(ToSchema, Clone, serde::Serialize, serde::Deserialize)]
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
