use crate::config::defaults;
use crate::search;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// A shared result type for operations that may fail with any error that is
/// thread-safe and can be transferred across threads, allowing callers to
/// propagate errors without requiring a specific error type.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// `HttpError` object with status as `u16` and message as a `String` object.
#[derive(Debug)]
pub struct HttpError {
    pub status: u16,
    pub message: String,
}
impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for HttpError {}
impl HttpError {
    pub fn boxed(status: u16, msg: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(Self {
            status,
            message: msg.into(),
        })
    }
}

// Torrent
/// ### PutTorrentRequest
/// Represents an incoming request to add a new torrent with optional rsync target details.
#[derive(ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct PutTorrentRequest {
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

/// ### RetryTorrentRequest
/// Represents an incoming request to retry a failed copy, or trigger a fresh
/// re-download followed by a fresh rsync transfer.
#[derive(ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryTorrentRequest {
    pub hash: String,
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

// VPN
/// ### ConnectVPN
/// Represents query params on an incoming request to rotate vpn connection with optional specs.
#[derive(ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectVPNRequest {
    #[serde(default = "defaults::default_vpn_whitelist")]
    pub whitelist: String,
    #[serde(default = "defaults::default_vpn_blacklist")]
    pub blacklist: String,
    #[serde(default = "defaults::default_vpn_hostname")]
    pub hostname: String,
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ToggleFeatureRequest {
    pub feature: Features,
    pub switch: bool,
}

// Search

/// ### SearchTorrentRequest
/// Represents query params on an incoming request to explore torrents with optional specs.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(style = Form, parameter_in = Query)]
pub struct SearchTorrentRequest {
    pub query: String,
    #[serde(default = "defaults::default_search_category")]
    #[param(required = false)]
    pub category: u16,
    #[serde(default = "defaults::default_hydrate_categories")]
    #[param(required = false)]
    pub hydrate_categories: bool,
    #[serde(default = "defaults::default_use_tor")]
    #[param(required = false)]
    pub use_tor: bool,
}

/// ### SearchTorrentResponse
/// A normalized representation of a torrent listing returned by the search provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SearchTorrentResponse {
    pub title: Option<String>,
    pub category: Option<String>,
    pub size: Option<String>,
    pub seeders: Option<i64>,
    pub leechers: Option<i64>,
    pub magnet_link: Option<String>,
    pub uploader: Option<String>,
    /// The date the torrent was uploaded, serialized and deserialized using
    /// the `date_uploaded` field name for compatibility with provider responses.
    #[serde(rename = "date_uploaded")]
    pub date: Option<String>,
}

impl SearchTorrentResponse {
    /// Hydrate the category with a $parent > $child style.
    ///
    /// # Arguments
    ///
    /// * `category_map` - Flattened category hashmap.
    pub fn hydrate_category(&mut self, category_map: &std::collections::HashMap<u16, String>) {
        let category = self.category.as_deref().unwrap_or_default();
        if let Ok(category_id) = category.parse::<u16>() {
            let base = search::category::base_map(&category_id);
            let mapped = category_map.get(&category_id);
            if let (Some(base), Some(mapped)) = (base, mapped) {
                self.category = Some(format!("{} > {}", base, mapped));
            } else {
                self.category = mapped.cloned();
            }
        }
    }
}

/// ### PutTorrentResponse
/// Represents the outgoing response for adding a new torrent.
#[derive(ToSchema, Clone, serde::Serialize, serde::Deserialize)]
pub struct PutTorrentResponse {
    pub name: String,
    pub state: String,
}

/// ### GetTorrentResponse
/// A single torrent's state as exposed through the WebUI/API — includes the
/// originally submitted torrent URL and transfer settings so the frontend can
/// prefill the retry modal and support re-downloading without extra round-trips.
#[derive(ToSchema, Clone, serde::Serialize)]
pub struct GetTorrentResponse {
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

// VPN
#[derive(serde::Serialize, serde::Deserialize, ToSchema)]
pub struct MullvadResponse {
    pub state: String,
    pub details: Option<Details>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, ToSchema)]
pub struct Details {
    pub location: Option<Location>,
    pub locked_down: Option<bool>,
    pub feature_indicators: Option<Vec<Features>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, ToSchema)]
pub struct Location {
    pub city: String,
    pub country: String,
    pub ipv4: Option<String>,
    pub hostname: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, ToSchema, Eq, PartialEq)]
pub enum Features {
    LanSharing,
    QuantumResistance,
    Daita,
    Multihop,
    #[serde(other)]
    Unknown,
}

/// Mixed objects for `GET /vpn` response based on the `?features=<bool>` flag.
#[derive(utoipa::ToResponse, utoipa::ToSchema)]
pub enum GetVpnResponse {
    #[allow(unused)]
    Status(MullvadResponse),
    #[allow(unused)]
    Features(Vec<Features>),
}

/// Mixed objects for `GET /categories` response based on the `?nested=<bool>` flag.
#[derive(utoipa::ToResponse, utoipa::ToSchema)]
pub enum GetCategoriesResponse {
    #[allow(unused)]
    Flattened(std::collections::HashMap<u16, String>),
    #[allow(unused)]
    Nested(std::collections::HashMap<String, std::collections::HashMap<u16, String>>),
}
