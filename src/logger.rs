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
pub fn init_logger(utc: bool) {
    // Safe when executed in single threading
    unsafe {
        std::env::set_var(
            "RUST_LOG",
            "actix_web=warn,actix_server=warn,rutorrent=info",
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
