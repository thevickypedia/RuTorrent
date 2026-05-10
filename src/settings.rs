use crate::squire;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;

/// ### SharedState
/// Shared application state for tracking active rsync operations.
pub type SharedState = Arc<RwLock<HashMap<String, RsyncTrack>>>;
/// ### PendingMap
/// Shared map for storing pending torrent metadata before resolution.
pub type PendingMap = Arc<RwLock<HashMap<String, PutItem>>>;

/// ### DBConnection
/// Shared `ruslite` connection object.
pub type DBConnection = Arc<std::sync::Mutex<rusqlite::Connection>>;

/// ### LogOptions
/// Options for logging output.
#[derive(Debug, Clone, PartialEq)]
pub enum LogOptions {
    Stdout,
    File,
}

impl FromStr for LogOptions {
    type Err = String;

    /// Parses a string into a `LogOptions` value.
    ///
    /// Accepted values are:
    /// - `"stdout"` → logs to standard output
    /// - `"file"` → logs to a file
    ///
    /// Returns an error if the input does not match a supported option.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stdout" => Ok(LogOptions::Stdout),
            "file" => Ok(LogOptions::File),
            _ => Err(format!("Invalid log option: {}", s)),
        }
    }
}

/// ### Config
/// Application configuration loaded from environment variables.
#[derive(Clone)]
pub struct Config {
    // RuTorrent API config
    pub host: String,
    pub port: u16,
    pub apikey: String,
    pub workers: usize,

    // QBitTorrent WebUI config
    pub qbit_url: String,
    pub qbit_username: String,
    pub qbit_password: String,

    // RuTorrent logger config
    pub utc_logger: bool,
    pub log: LogOptions,
    pub log_level: log::LevelFilter,

    // Ntfy notification config
    pub ntfy_url: String,
    pub ntfy_topic: String,
    pub ntfy_username: String,
    pub ntfy_password: String,

    // Telegram notification config
    pub telegram_chat_id: String,
    pub telegram_bot_token: String,
}

/// Formats and prints the startup error message.
///
/// # Arguments
///
/// * `msg` - Message to be printed.
fn startup_error(msg: &str) {
    eprintln!("\nStartupError:\n\t{}\n", msg);
}

impl Config {
    /// Creates a new application configuration by reading environment variables.
    ///
    /// Required values are validated at startup, and the process will terminate
    /// with an error message if any critical configuration is missing or invalid.
    ///
    /// This includes:
    /// - API key validation
    /// - Worker count validation
    /// - Logging configuration parsing
    /// - External service configuration normalization (e.g. URL cleanup)
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

        let mut qbit_url = squire::get_env_var("qbit_url", Some("http://localhost:8080/"));
        let qbit_username = squire::get_env_var("qbit_username", None);
        let qbit_password = squire::get_env_var("qbit_password", None);
        qbit_url = qbit_url.strip_suffix("/").unwrap_or(&qbit_url).to_string();

        let utc_logger = squire::get_env_var("utc_logger", Some("true")) == "true";
        let default_log = squire::get_env_var("log", Some("stdout"));
        let log = match default_log.parse::<LogOptions>() {
            Ok(log) => log,
            Err(err) => {
                startup_error(&err.to_string());
                std::process::exit(1);
            }
        };
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
        ntfy_topic = ntfy_topic
            .strip_prefix("/")
            .unwrap_or(&ntfy_topic)
            .to_string();

        let telegram_bot_token = squire::get_env_var("telegram_bot_token", None);
        let telegram_chat_id = squire::get_env_var("telegram_chat_id", None);
        if !telegram_chat_id.is_empty() {
            match telegram_chat_id.parse::<usize>() {
                Ok(_) => (),
                Err(_) => {
                    startup_error(
                        format!("Invalid 'telegram_chat_id' value '{telegram_chat_id}'").as_str(),
                    );
                    std::process::exit(1)
                }
            };
        }

        Self {
            host,
            port,
            apikey,
            workers,
            qbit_url,
            qbit_username,
            qbit_password,
            utc_logger,
            log,
            log_level,
            ntfy_url,
            ntfy_topic,
            ntfy_username,
            ntfy_password,
            telegram_bot_token,
            telegram_chat_id,
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
    Failed,
}

/// ### RsyncTrack
/// Tracks a torrent and its associated rsync transfer state.
#[derive(Clone)]
pub struct RsyncTrack {
    pub name: String,
    pub status: Status,
    pub put_item: PutItem,
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
    #[serde(default = "default_delete_after_copy")]
    pub delete_after_copy: bool,
}

/// Gets the default host from the `remote_host` environment variable.
fn default_host() -> String {
    squire::get_env_var("remote_host", None)
}

/// Gets the default username from the `remote_user` environment variable.
fn default_username() -> String {
    squire::get_env_var("remote_user", None)
}

/// Gets the default remote path from the `remote_path` environment variable.
fn default_path() -> String {
    squire::get_env_var("remote_path", None)
}

/// Gets the default save path from the `save_path` environment variable.
fn default_save_path() -> String {
    squire::get_env_var("save_path", None)
}

/// Determines whether files should be deleted after copying.
///
/// This value is read from the `delete_after_copy` environment variable.
/// If the variable is missing or cannot be parsed as a boolean,
/// it defaults to `false`, since this is called during run-time.
fn default_delete_after_copy() -> bool {
    squire::get_env_var("delete_after_copy", Some("false"))
        .parse::<bool>()
        .unwrap_or(false)
}
