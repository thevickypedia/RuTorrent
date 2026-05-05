use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use crate::settings;

/// Parses a progress percentage from an `rsync` output line.
///
/// # Arguments
///
/// * `line` - A single line of stdout emitted by the `rsync` process
///
/// # Returns
///
/// * `Some(f64)` - Progress as a value between `0.0` and `1.0` if parsing succeeds
/// * `None` - If the line does not contain a valid percentage
///
/// # Notes
///
/// - Expects a percentage value followed by `%` in the line.
/// - Converts the parsed percentage into a normalized fraction.
fn parse_progress(line: &str) -> Option<f64> {
    if let Some(idx) = line.find('%') {
        let start = line[..idx].rfind(' ')?;
        let pct = line[start..idx].trim();
        return pct.parse::<f64>().ok().map(|p| p / 100.0);
    }
    None
}

/// Executes an `rsync` process to transfer a file or directory to a remote target.
///
/// # Arguments
///
/// * `state` - Shared application state used to track transfer progress and status
/// * `hash` - Unique identifier for the transfer entry in the state
/// * `name` - Human-readable name of the item being transferred (used for logging)
/// * `source` - Local source path to be copied
/// * `target` - Remote rsync target configuration (username, host, and destination path)
///
/// # Notes
///
/// - Spawns an `rsync` process with progress reporting enabled.
/// - Parses stdout to extract progress updates and writes them into shared state.
/// - Logs all output lines for visibility.
/// - Marks the transfer as `Completed` after the process exits.
/// - Assumes an existing entry for `hash` is present in the shared state.
pub async fn run(
    state: settings::SharedState,
    hash: String,
    name: String,
    source: String,
    target: settings::RsyncTarget,
) {
    log::info!("Starting rsync for {}", name);

    let remote = format!("{}@{}:{}", target.username, target.host, target.path);

    let mut child = Command::new("rsync")
        .args([
            "-az",
            "--progress",
            "--partial",
            "--inplace",
            "-e",
            "ssh -o BatchMode=yes -o ConnectTimeout=5",
            &source,
            &remote,
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("rsync failed");

    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        log::info!("rsync: {}", line);

        if let Some(p) = parse_progress(&line) {
            let mut db = state.write().await;
            if let Some(e) = db.get_mut(&hash) {
                e.status = settings::Status::Copying(p);
            }
        }
    }

    let status = child.wait().await.expect("failed to wait on rsync");

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
        e.status = settings::Status::Completed;
    }
}
