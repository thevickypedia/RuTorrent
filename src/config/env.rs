use std::fmt;

use crate::{squire, warning};
use std::path::PathBuf;
use std::str::FromStr;

/// Formats and prints the startup error message.
///
/// # Arguments
///
/// * `msg` - Message to be printed.
fn startup_error(msg: &str) {
    eprintln!("\nStartupError:\n\t{}\n", msg);
}

/// Get and parse timeout from environment variables.
///
/// # Arguments
///
/// * `key` - Key for the environment variable.
/// * `default` - Default value if env var is unresolved.
///
/// # Returns
///
/// Returns a `u64` element.
fn get_and_parse_timeout(key: &str, default: &str) -> u64 {
    let value = squire::misc::get_env_var(key, Some(default));
    value.parse().unwrap_or(default.parse::<u64>().unwrap_or(1))
}

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
#[derive(Debug, Clone)]
pub enum TorScheme {
    Socks5,
    Socks5h,
}

impl fmt::Display for TorScheme {
    /// Formats the display trait for `TorScheme` value.
    ///
    /// Returns a lowercased value for the enum.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TorScheme::Socks5 => write!(f, "socks5"),
            TorScheme::Socks5h => write!(f, "socks5h"),
        }
    }
}
impl FromStr for TorScheme {
    type Err = String;

    /// Parses a string into a `TorScheme` value.
    ///
    /// Accepted values are:
    /// - `"socks5"` → uses local DNS resolution
    /// - `"socks5h"` → uses remote DNS resolution
    ///
    /// Returns an error if the input does not match a supported option.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "socks5" => Ok(TorScheme::Socks5),
            "socks5h" => Ok(TorScheme::Socks5h),
            _ => Err(format!("Invalid tor scheme: {}", s)),
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
    pub username: String,
    pub password: String,

    pub apikey: String,
    pub workers: usize,

    // QBitTorrent WebUI config
    pub qbit_url: String,
    pub qbit_username: String,
    pub qbit_password: String,
    pub qbit_timeout: u64,

    // RuTorrent logger config
    pub utc_logger: bool,
    pub log: LogOptions,
    pub log_level: log::LevelFilter,

    // Ntfy notification config
    pub ntfy_url: String,
    pub ntfy_topic: String,
    pub ntfy_username: String,
    pub ntfy_password: String,
    pub ntfy_timeout: u64,

    // Telegram notification config
    pub telegram_chat_id: String,
    pub telegram_bot_token: String,
    pub telegram_timeout: u64,

    // Tor configuration
    pub tor_scheme: TorScheme,
    pub tor_host: String,
    pub tor_port: u16,
    pub tor_timeout: u64,

    // Trackers
    pub trackers_file: Option<PathBuf>,
    pub max_trackers: usize,

    // Mullvad relay cache
    pub mullvad_relay_cache: Option<PathBuf>,
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
        let host = squire::misc::get_env_var("host", Some("127.0.0.1"));
        let port_raw = squire::misc::get_env_var("port", Some("3000"));
        let port = match port_raw.parse::<u16>() {
            Ok(p) => p,
            Err(e) => {
                startup_error(format!("Invalid 'port' value '{port_raw}': {e}").as_str());
                std::process::exit(1)
            }
        };
        let username = squire::misc::get_env_var("username", None);
        let password = squire::misc::get_env_var("password", None);

        let apikey = squire::misc::get_env_var("apikey", None);
        if apikey.is_empty() {
            startup_error("'apikey' is empty");
            std::process::exit(1)
        }
        match squire::misc::complexity_checker(&apikey, 32) {
            Ok(()) => (),
            Err(err) => {
                startup_error(format!("Invalid 'apikey': {}", err).as_str());
                std::process::exit(1)
            }
        }

        let default_workers = squire::misc::get_env_var("workers", Some("1"));
        let workers = match default_workers.parse::<usize>() {
            Ok(n) if n > 0 => n,
            Ok(_) => {
                startup_error(
                    format!(
                        "'workers' must be > 0, got {}",
                        default_workers
                    )
                    .as_str(),
                );
                std::process::exit(1)
            }
            Err(e) => {
                startup_error(
                    format!("Invalid 'workers' value '{default_workers}': {e}\n").as_str(),
                );
                std::process::exit(1)
            }
        };

        let mut qbit_url = squire::misc::get_env_var("qbit_url", Some("http://localhost:8080/"));
        let qbit_username = squire::misc::get_env_var("qbit_username", None);
        let qbit_password = squire::misc::get_env_var("qbit_password", None);
        qbit_url = qbit_url.strip_suffix("/").unwrap_or(&qbit_url).to_string();

        if !qbit_url.contains("0.0.0.0")
            && !qbit_url.contains("localhost")
            && !qbit_url.contains("127.0.0.1")
        {
            // NOTE: Since QBitTorrentAPI does not have a built-in callback function,
            // the current process monitors ongoing downloads and then triggers rsync
            // This requires both QBitTorrent and RuTorrent to run on the same device
            warning!(
                "qbit_url appears to be a remote location. This will invalidate the rsync callback."
            );
        }

        let tor_scheme =
            match squire::misc::get_env_var("tor_scheme", Some("socks5h")).parse::<TorScheme>() {
                Ok(ts) => ts,
                Err(e) => {
                    startup_error(format!("Invalid 'tor_scheme' value: {e}").as_str());
                    std::process::exit(1)
                }
            };
        let tor_host = squire::misc::get_env_var("tor_host", Some("127.0.0.1"));
        let tor_port = match squire::misc::get_env_var("tor_port", Some("9050")).parse::<u16>() {
            Ok(p) => p,
            Err(e) => {
                startup_error(format!("Invalid 'tor_host' value: {e}").as_str());
                std::process::exit(1)
            }
        };

        let utc_logger = squire::misc::get_env_var("utc_logger", Some("true")) == "true";
        let default_log = squire::misc::get_env_var("log", Some("stdout"));
        let log = match default_log.parse::<LogOptions>() {
            Ok(log) => log,
            Err(err) => {
                startup_error(&err.to_string());
                std::process::exit(1);
            }
        };
        let default_log_level = squire::misc::get_env_var("log_level", Some("info"));
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

        let mut ntfy_url = squire::misc::get_env_var("ntfy_url", None);
        let mut ntfy_topic = squire::misc::get_env_var("ntfy_topic", None);
        let ntfy_username = squire::misc::get_env_var("ntfy_username", None);
        let ntfy_password = squire::misc::get_env_var("ntfy_password", None);

        ntfy_url = ntfy_url.strip_suffix("/").unwrap_or(&ntfy_url).to_string();
        ntfy_topic = ntfy_topic
            .strip_prefix("/")
            .unwrap_or(&ntfy_topic)
            .to_string();

        let telegram_bot_token = squire::misc::get_env_var("telegram_bot_token", None);
        let telegram_chat_id = squire::misc::get_env_var("telegram_chat_id", None);
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

        let qbit_timeout = get_and_parse_timeout("qbit_timeout", "3");
        let ntfy_timeout = get_and_parse_timeout("ntfy_timeout", "3");
        let telegram_timeout = get_and_parse_timeout("telegram_timeout", "3");
        let tor_timeout = get_and_parse_timeout("tor_timeout", "10");

        let raw_trackers_file = squire::misc::get_env_var("trackers_file", None);
        let trackers_file: Option<PathBuf> = if raw_trackers_file.is_empty() {
            None
        } else {
            Some(PathBuf::from(raw_trackers_file))
        };

        let max_trackers =
            match squire::misc::get_env_var("max_trackers", Some("10")).parse::<usize>() {
                Ok(mt) => {
                    if mt > 0 {
                        mt
                    } else {
                        startup_error(format!("Invalid 'max_trackers' value '{mt}'").as_str());
                        std::process::exit(1);
                    }
                }
                Err(err) => {
                    startup_error(&err.to_string());
                    std::process::exit(1)
                }
            };

        let mullvad_cache_env = squire::misc::get_env_var("mullvad_relay_cache", None);
        let mullvad_relay_cache = if !mullvad_cache_env.is_empty() {
            if !mullvad_cache_env.ends_with("relays.json") {
                startup_error("'mullvad_relay_cache' must be a path to 'relays.json'");
                std::process::exit(1)
            }
            Some(PathBuf::from(mullvad_cache_env))
        } else {
            std::env::var("MULLVAD_CACHE_DIR")
                .ok()
                .map(|dir| PathBuf::from(dir).join("relays.json"))
        };

        Self {
            host,
            port,
            username,
            password,
            apikey,
            workers,
            qbit_url,
            qbit_username,
            qbit_password,
            qbit_timeout,
            utc_logger,
            log,
            log_level,
            ntfy_url,
            ntfy_topic,
            ntfy_username,
            ntfy_password,
            ntfy_timeout,
            telegram_bot_token,
            telegram_chat_id,
            telegram_timeout,
            tor_scheme,
            tor_host,
            tor_port,
            tor_timeout,
            trackers_file,
            max_trackers,
            mullvad_relay_cache,
        }
    }
}
