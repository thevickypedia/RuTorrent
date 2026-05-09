use std::{fs::OpenOptions, io::Write};

use crate::{constant, settings};
use chrono::{DateTime, Local};
use env_logger::Target;

/// Initializes the application logger with optional UTC or local timestamp formatting.
///
/// # Arguments
///
/// * `config` - Application configuration.
/// * `metadata` - Project build metadata.
///
/// # Notes
///
/// - This function should only be called once during application startup.
pub fn init_logger(config: &settings::Config, metadata: &constant::MetaData) {
    // Safe when executed in single threading
    unsafe {
        std::env::set_var(
            "RUST_LOG",
            format!(
                "actix_web={0},actix_server={0},{1}={0}",
                config.log_level, metadata.crate_name
            ),
        );
    }

    let mut builder = env_logger::Builder::from_default_env();

    // Configure output target
    if config.log == settings::LogOptions::File {
        // Ensure logs directory exists
        std::fs::create_dir_all("logs").unwrap();

        // Generate timestamped filename
        let timestamp = Local::now().format("%Y-%m-%d_%H:%M:%S");

        let log_file_name = format!("logs/{}_{}.log", metadata.crate_name, timestamp);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file_name)
            .unwrap();

        builder.target(Target::Pipe(Box::new(file)));
    }

    // Configure formatting
    if !config.utc_logger {
        builder.format(|buf, record| {
            let local_time: DateTime<Local> = Local::now();
            writeln!(
                buf,
                "[{} {} {}] - {}",
                local_time.format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        });
    }

    builder.init();
}
