use crate::config::defaults;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;

/// ### PendingMap
/// Shared map for storing pending torrent metadata before resolution.
pub type PendingMap = Arc<RwLock<HashMap<String, PutItem>>>;

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
    pub put_item: PutItem,

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
}

/// ### PutItem
/// Represents an incoming request to add a new torrent with optional rsync target details.
#[derive(ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct PutItem {
    pub url: String,

    pub name: Option<String>,
    pub hash: Option<String>,
    pub trackers: Option<Vec<String>>,

    #[serde(default = "defaults::default_save_path")]
    pub save_path: String,

    #[serde(default = "defaults::default_host")]
    pub remote_host: String,
    #[serde(default = "defaults::default_username")]
    pub remote_username: String,
    #[serde(default = "defaults::default_path")]
    pub remote_path: String,
    #[serde(default = "defaults::default_timeout")]
    pub rsync_timeout: u8,
    #[serde(default = "defaults::default_delete_after_copy")]
    pub delete_after_copy: bool,
}

/// ### RetryOptions
/// Represents an incoming request to retry a failed copy, or trigger a fresh
/// re-download followed by a fresh rsync transfer.
#[derive(ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryOptions {
    pub name: String,
    /// When `true`, deletes any existing local files (if present) and starts
    /// a brand-new download from the original torrent URL, followed by a
    /// fresh rsync transfer once it completes. When `false` (default),
    /// resumes the rsync transfer using the existing local files.
    #[serde(default)]
    pub redownload: bool,
    #[serde(default = "defaults::default_host")]
    pub remote_host: String,
    #[serde(default = "defaults::default_username")]
    pub remote_username: String,
    #[serde(default = "defaults::default_path")]
    pub remote_path: String,
    #[serde(default = "defaults::default_timeout")]
    pub rsync_timeout: u8,
    #[serde(default = "defaults::default_delete_after_copy")]
    pub delete_after_copy: bool,
}
