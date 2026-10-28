use crate::api;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// ### PendingMap
/// Shared map for storing pending torrent metadata before resolution.
pub type PendingMap = Arc<RwLock<HashMap<String, api::schema::PutTorrentRequest>>>;

/// ### DBConnection
/// Shared `ruslite` connection object.
pub type DBConnection = Arc<std::sync::Mutex<rusqlite::Connection>>;

/// ### Status
/// Represents the current status of a torrent or transfer.
#[derive(Clone, Debug, serde::Serialize)]
pub enum Status {
    Downloading(f64),
    DownloadComplete, // Torrent download-only completed
    Copying,          // Rsync/copy operation currently running
    Transferred,      // Rsync/copy completed successfully
    CopyError,        // Rsync/copy failed
    Failed,           // Generic terminal failure
    Completed,        // Temporary until copied, then Transferred [OR] removed entirely
}

/// ### RsyncTrack
/// Tracks a torrent and its associated rsync transfer state.
#[derive(Clone)]
pub struct RsyncTrack {
    pub name: String,
    pub status: Status,
    pub put_item: api::schema::PutTorrentRequest,

    /// Whether this torrent's hash is still known to qBittorrent.
    ///
    /// This is tracked independently of `status` so that `status` can keep
    /// representing the true last-known lifecycle outcome (Transferred,
    /// Failed, etc.) even after qBittorrent itself no longer has the torrent
    /// (deleted manually, via `delete_after_copy`, or via the WebUI).
    pub in_qbit: bool,

    /// Whether the locally downloaded files were deleted (e.g. via
    /// `delete_after_copy`). When `true`, a plain rsync retry is no longer
    /// possible since there's nothing left locally to copy — only a fresh
    /// re-download can recover this torrent.
    pub files_deleted: bool,
    pub added_at: f64,
}
