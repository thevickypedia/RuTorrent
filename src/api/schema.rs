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
}
