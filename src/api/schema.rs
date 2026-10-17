use std::error::Error;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// ### TorrentEntry
/// A single torrent's state as exposed through the WebUI/API — includes the
/// originally submitted torrent URL and transfer settings so the frontend can
/// prefill the retry modal and support re-downloading without extra round-trips.
#[derive(ToSchema, Clone, serde::Serialize)]
pub struct TorrentEntry {
    pub name: String,
    pub hash: String,
    pub url: String,
    pub save_path: String,
    pub status: String,
    pub remote_host: String,
    pub remote_username: String,
    pub remote_path: String,
    pub rsync_timeout: u8,
    pub delete_after_copy: bool,
    /// `true` when the locally downloaded files were deleted (e.g. via
    /// `delete_after_copy`). A plain rsync retry is impossible in that case —
    /// only a fresh re-download can recover this torrent.
    pub files_deleted: bool,
    pub qbit_state: String, // raw state string from qBittorrent, empty if not in qBit
    pub added_at: f64,
}

/// Simplified `Result` object for TOR setup.
pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Struct for the base torrent object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct BaseTorrent {
    pub title: Option<String>,
    pub category: Option<String>,
    pub size: Option<String>,
    pub seeders: Option<i64>,
    pub leechers: Option<i64>,
    pub magnet_link: Option<String>,
    pub uploader: Option<String>,
    #[serde(rename = "date_uploaded")]
    pub date: Option<String>,
}
