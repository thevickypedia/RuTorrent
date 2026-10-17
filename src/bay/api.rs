use crate::{config, bay};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use utoipa::ToSchema;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

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

fn string_field(row: &Value, key: &str) -> Option<String> {
    let value = row.get(key)?;
    let result = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    let result = result.trim().to_owned();
    (!result.is_empty()).then_some(result)
}

fn integer_field(row: &Value, key: &str) -> Option<i64> {
    string_field(row, key)?.parse().ok()
}

fn format_size(bytes: i64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes.max(0) as f64;
    let mut unit = 0;

    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", size as u64, UNITS[unit])
    } else {
        format!("{:.2} {}", size, UNITS[unit])
    }
}

fn parse_torrent(row: &Value) -> Option<BaseTorrent> {
    let title = string_field(row, "name")?;

    if title.eq_ignore_ascii_case("no results returned") || title.eq_ignore_ascii_case("error") {
        return None;
    }

    let size = integer_field(row, "size")
        .map(format_size)
        .or_else(|| string_field(row, "size"));

    let magnet_link = string_field(row, "info_hash").map(|hash| {
        format!(
            "magnet:?xt=urn:btih:{}&dn={}",
            hash,
            urlencoding::encode(&title)
        )
    });

    Some(BaseTorrent {
        title: Some(title),
        category: string_field(row, "category"),
        size,
        seeders: integer_field(row, "seeders"),
        leechers: integer_field(row, "leechers"),
        magnet_link,
        uploader: string_field(row, "username"),
        date: string_field(row, "added"),
    })
}

/// Search `pages` pages and return all parsed torrents.
///
/// The API's `page` parameter is zero-based; page 0 is the first page.
pub async fn search(
    query: &str,
    pages: usize,
    config: &config::env::Config,
) -> Result<Vec<BaseTorrent>> {
    let client = match bay::tor::client(config).await {
        Ok(client) => client,
        Err(e) => {
            return Err(e);
        }
    };
    // TODO: Fail if tor verification fails and query param is missing bypass tor
    bay::tor::verify(config, Some(client.clone())).await;
    let mut torrents = Vec::new();
    for page in 0..pages {
        let response = client
            .get("https://apibay.org/q.php")
            .query(&[("q", query), ("cat", "0"), ("page", &page.to_string())])
            .send()
            .await?
            .error_for_status()?;
        let payload: Value = response.json().await?;
        let Some(rows) = payload.as_array() else {
            continue;
        };
        torrents.extend(rows.iter().filter_map(parse_torrent));
    }
    Ok(torrents)
}
