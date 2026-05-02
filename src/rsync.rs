use crate::settings;
use ssh2::Session as Ssh2Session;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub async fn run(
    state: settings::SharedState,
    hash: String,
    name: String,
    target: settings::RsyncTarget,
) {
    log::info!("Starting native SCP for {}", name);

    let local_path = format!("/downloads/{}", name);

    let mut file = match File::open(&local_path).await {
        Ok(f) => f,
        Err(e) => {
            log::error!("file open error: {}", e);
            return;
        }
    };

    let size = match file.metadata().await {
        Ok(m) => m.len(),
        Err(e) => {
            log::error!("metadata error: {}", e);
            return;
        }
    };

    let addr = format!("{}:22", target.host);

    // ⚠️ ssh2 is blocking → run in blocking thread
    let result = tokio::task::spawn_blocking(move || {
        // Connect TCP
        let tcp = TcpStream::connect(addr).expect("tcp connect error");

        // Create session
        let mut sess = Ssh2Session::new().expect("Failed to open session");
        sess.set_tcp_stream(tcp);
        sess.handshake().expect("Handshake error");

        // Authenticate (password example — adjust if using keys)
        sess.userauth_password(&target.username, &target.password)
            .expect("authentication failed");

        if !sess.authenticated() {
            return Err("authentication failed");
        }

        // Start SCP send
        let remote = sess
            .scp_send(Path::new(&target.path), 0o644, size, None)
            .expect("Failed to create remote session");

        Ok((sess, remote))
    })
    .await;

    let (_session, mut remote) = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            log::error!("ssh setup failed: {}", e);
            return;
        }
        Err(e) => {
            log::error!("join error: {}", e);
            return;
        }
    };

    let mut sent: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];

    loop {
        let n = match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                log::error!("read error: {}", e);
                return;
            }
        };

        if let Err(e) = remote.write_all(&buf[..n]) {
            log::error!("write error: {}", e);
            return;
        }

        sent += n as u64;

        let pct = sent as f64 / size as f64;

        let mut db = state.write().await;
        if let Some(entry) = db.get_mut(&hash) {
            entry.status = settings::Status::Copying(pct);
        }
    }

    // Finalize transfer
    if let Err(e) = remote.send_eof() {
        log::error!("send_eof error: {}", e);
    }
    if let Err(e) = remote.wait_eof() {
        log::error!("wait_eof error: {}", e);
    }
    if let Err(e) = remote.close() {
        log::error!("close error: {}", e);
    }
    if let Err(e) = remote.wait_close() {
        log::error!("wait_close error: {}", e);
    }

    log::info!("SCP complete: {}", name);

    let mut db = state.write().await;
    if let Some(entry) = db.get_mut(&hash) {
        entry.status = settings::Status::Completed;
    }
}
