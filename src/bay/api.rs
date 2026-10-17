use crate::{api, bay, config};
use reqwest::Client;
use serde_json::Value;

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

fn parse_torrent(row: &Value) -> Option<api::schema::BaseTorrent> {
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

    Some(api::schema::BaseTorrent {
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

/// Searches for torrents using JSON api torrents.
///
/// The API's `page` parameter is zero-based; page 0 is the first page.
/// Each page will return a maximum of 100 results.
pub async fn search(
    query: &str,
    pages: usize,
    use_tor: bool,
    strict_mode: bool,
    config: &config::env::Config,
    metadata: &config::constant::MetaData,
) -> api::schema::Result<Vec<api::schema::BaseTorrent>> {
    // Mark 1: Checks if the client has requested to use tor network
    let client: Client = if use_tor {
        // Mark 2: A new connection is created and verified everytime and this is a design choice
        match bay::tor::client(config, metadata).await {
            Ok(client) => {
                // Mark 3: If client creation is successful, verify if the client is routed through Tor network
                match bay::tor::verify(config, metadata, Some(client.clone())).await {
                    Ok(_) => {
                        log::info!("Connection is secure. Using Tor network.");
                    }
                    Err(e) => {
                        // Mark 4: If Tor verification has failed, and strict mode is enabled fail the request;
                        // Otherwise, log a warning and continue
                        if strict_mode {
                            log::info!("Strict mode was enabled, so failing due to: {}", e);
                            return Err(e);
                        } else {
                            log::warn!("Strict mode was disabled, so proceed with error: {}", e);
                        }
                    }
                }
                client
            }
            Err(e) => {
                return Err(e);
            }
        }
    } else {
        Client::builder().build()?
    };
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
