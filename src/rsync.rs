use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use crate::settings;

/// Executes an `rsync` process to transfer a file or directory to a remote target.
///
/// # Arguments
///
/// * `state` - Shared application state used to track transfer status.
/// * `hash` - Unique identifier for the transfer entry in the state.
/// * `name` - Human-readable name of the item being transferred (used for logging).
/// * `put_item` - Reference to the `PutItem` object.
///
/// # Notes
///
/// - Spawns an `rsync` process and streams stdout to logs.
/// - Marks the transfer as `Completed` or `Failed` after the process exits.
/// - Assumes an existing entry for `hash` is present in the shared state.
pub async fn run(
    state: settings::SharedState,
    hash: String,
    name: String,
    put_item: settings::PutItem,
) {
    log::info!("Starting rsync for {}", name);

    let remote = format!(
        "{}@{}:{}",
        put_item.remote_username, put_item.remote_host, put_item.remote_path
    );
    log::info!("{} -> {}", &put_item.save_path, &remote);

    let child_result = Command::new("rsync")
        .args(["-az", &put_item.save_path, &remote])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child = match child_result {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to start rsync for {}: {}", name, e);
            let mut db = state.write().await;
            if let Some(entry) = db.get_mut(&hash) {
                entry.status = settings::Status::Failed;
            }
            return;
        }
    };

    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        log::info!("rsync: {}", line);
    }

    let status = match child.wait().await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed waiting for rsync process for {}: {}", name, e);
            let mut db = state.write().await;
            if let Some(entry) = db.get_mut(&hash) {
                entry.status = settings::Status::Failed;
            }
            return;
        }
    };

    if status.success() {
        log::info!("rsync complete: {}", name);
    } else {
        let mut err_output = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let _ = stderr.read_to_string(&mut err_output).await;
        }
        log::error!(
            "rsync failed for {} with status {}. stderr: {}",
            name,
            status,
            err_output
        );
    }

    let mut db = state.write().await;
    if let Some(e) = db.get_mut(&hash) {
        e.status = if status.success() {
            settings::Status::Completed
        } else {
            settings::Status::Failed
        };
    }
}
