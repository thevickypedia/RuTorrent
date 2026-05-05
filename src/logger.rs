use std::io::Write;

use chrono::{DateTime, Local};

/// Initializes the application logger with optional UTC or local timestamp formatting.
///
/// # Arguments
///
/// * `utc` - If `true`, initializes the logger using the default UTC-based format.
///   If `false`, uses a custom formatter with local time timestamps.
///
/// # Notes
///
/// - This function should only be called once during application startup.
pub fn init_logger(utc: bool, log_level: log::LevelFilter) {
    // Safe when executed in single threading
    unsafe {
        std::env::set_var(
            "RUST_LOG",
            format!("actix_web={0},actix_server={0},rutorrent={0}", log_level),
        );
    }
    if utc {
        env_logger::init();
    } else {
        env_logger::Builder::from_default_env()
            .format(|buf, record| {
                let local_time: DateTime<Local> = Local::now();
                writeln!(
                    buf,
                    "[{} {} {}] - {}",
                    local_time.format("%Y-%m-%dT%H:%M:%SZ"),
                    record.level(),
                    record.target(),
                    record.args()
                )
            })
            .init();
    }
}
