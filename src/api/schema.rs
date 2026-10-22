use crate::search;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error};
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

/// A shared result type for operations that may fail with any error that is
/// thread-safe and can be transferred across threads, allowing callers to
/// propagate errors without requiring a specific error type.
pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// ### BaseTorrent
/// A normalized representation of a torrent listing returned by a search
/// provider — includes available metadata such as the title, category, size,
/// seed and leech counts, magnet link, uploader, and upload date, with optional
/// fields to accommodate incomplete or provider-specific results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct BaseTorrent {
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

impl BaseTorrent {
    /// Hydrate the category with a $parent > $child style.
    ///
    /// # Arguments
    ///
    /// * `category_map` - Flattened category hashmap.
    pub fn hydrate_category(&mut self, category_map: &HashMap<u16, String>) {
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
    pub fn boxed(status: u16, msg: impl Into<String>) -> Box<dyn Error + Send + Sync> {
        Box::new(Self {
            status,
            message: msg.into(),
        })
    }
}
