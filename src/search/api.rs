use crate::{api, config, search};
use reqwest::Client;
use serde_json::Value;
use tokio::sync::OnceCell;

/// Stores and provides a thread-safe, asynchronously initialized cache of tracker URLs shared across callers.
static TRACKERS: OnceCell<Vec<String>> = OnceCell::const_new();

/// Extract a non-empty string or number field from a JSON object.
///
/// # Arguments
///
/// * `row` - JSON object containing the field.
/// * `key` - Name of the field to extract.
///
/// # Returns
///
/// Returns the trimmed field value as a `String`, or `None` if the field is missing, empty, or has an unsupported type.
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

/// Extract an integer field from a JSON object.
///
/// # Arguments
///
/// * `row` - JSON object containing the field.
/// * `key` - Name of the field to extract.
///
/// # Returns
///
/// Returns the field value as an `i64`, or `None` if it is missing or cannot be parsed.
fn integer_field(row: &Value, key: &str) -> Option<i64> {
    string_field(row, key)?.parse().ok()
}

/// Format a byte count as a human-readable size.
///
/// # Arguments
///
/// * `bytes` - Size in bytes.
///
/// # Returns
///
/// Returns the formatted size with an appropriate unit, from bytes to terabytes.
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

/// Parse a JSON object into a base torrent representation.
///
/// # Arguments
///
/// * `row` - JSON object containing torrent metadata.
///
/// # Returns
///
/// Returns a `BaseTorrent` if the object contains a valid title and is not an error or empty-result entry; otherwise, returns `None`.
fn parse_torrent(row: &Value) -> Option<api::schema::BaseTorrent> {
    let title = string_field(row, "name")?;

    if title.eq_ignore_ascii_case("no results returned") || title.eq_ignore_ascii_case("error") {
        return None;
    }

    let size = integer_field(row, "size")
        .map(format_size)
        .or_else(|| string_field(row, "size"));

    let magnet_link = string_field(row, "info_hash").map(|hash| {
        let mut magnet = format!(
            "magnet:?xt=urn:btih:{}&dn={}",
            hash,
            urlencoding::encode(&title)
        );
        let trackers: &Vec<String> = match TRACKERS.get() {
            Some(trackers) => {
                log::info!("Found [{}] trackers in memory", trackers.len());
                trackers
            }
            None => {
                log::warn!("No trackers in memory; using fallback");
                &search::trackers::TRACKERS
                    .iter()
                    .copied()
                    .map(String::from)
                    .collect()
            }
        };
        for tracker in trackers {
            magnet.push_str("&tr=");
            magnet.push_str(&urlencoding::encode(tracker));
        }
        magnet
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

/// Initialize the shared tracker list if it has not already been initialized.
///
/// # Returns
///
/// Returns a reference to the cached tracker list, initializing it asynchronously on the first call.
async fn ensure_trackers() -> &'static Vec<String> {
    TRACKERS
        .get_or_init(|| async { search::trackers::get().await })
        .await
}

/// Search for torrents using the JSON API, optionally routing requests through Tor.
///
/// # Arguments
///
/// * `query` - Search query string.
/// * `pages` - Number of result pages to retrieve, starting at page zero.
/// * `use_tor` - Whether to route requests through the Tor network.
/// * `strict` - Whether to fail if Tor verification fails.
/// * `config` - Application environment configuration.
/// * `metadata` - Application metadata required for Tor setup and verification.
///
/// # Returns
///
/// Returns a `Result` containing the collected torrent results, or an error if client creation, request execution, response validation, or strict Tor verification fails.
pub async fn search(
    query: &str,
    pages: usize,
    use_tor: bool,
    strict: bool,
    config: &config::env::Config,
    metadata: &config::constant::MetaData,
) -> api::schema::Result<Vec<api::schema::BaseTorrent>> {
    // Mark 1: Checks if the client has requested to use tor network
    let client: Client = if use_tor {
        log::info!("Connecting to Tor network...");
        // Mark 2: A new connection is created and verified everytime and this is a design choice
        match search::tor::client(config, metadata).await {
            Ok(client) => {
                // Mark 3: If client creation is successful, verify if client is routed through Tor network
                match search::tor::verify(config, metadata, Some(client.clone())).await {
                    Ok(_) => {}
                    Err(e) => {
                        // Mark 4: If Tor verification has failed, and strict mode is enabled fail the request;
                        // Otherwise, log a warning and continue
                        if strict {
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
    // TODO: May be I should apply trackers for all incoming torrents?
    ensure_trackers().await;
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
    log::info!("Found {} torrents.", torrents.len());
    Ok(torrents)
}
