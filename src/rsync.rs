use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use crate::settings;
fn parse_progress(line: &str) -> Option<f64> {
    if let Some(idx) = line.find('%') {
        let start = line[..idx].rfind(' ')?;
        let pct = line[start..idx].trim();
        return pct.parse::<f64>().ok().map(|p| p / 100.0);
    }
    None
}

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

    let _ = child.wait().await;

    log::info!("rsync complete: {}", name);

    let mut db = state.write().await;
    if let Some(e) = db.get_mut(&hash) {
        e.status = settings::Status::Completed;
    }
}
