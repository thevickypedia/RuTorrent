use reqwest;
use utoipa::ToSchema;
use std::collections::HashMap;
use regex::Regex;

/// Function to retrieve the REGEX object for an IPv4 address format
///
/// # Returns
///
/// A `Regex` object that can be used to match an IPv4 address format
pub fn ip_regex() -> Regex {
    Regex::new(
        r"^(?:(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])$"
    ).unwrap()
}

/// Mapping of URLs to check for public IP addresses
///
/// # Returns
///
/// A `HashMap` containing the URL and a boolean value indicating whether the URL expects text or JSON response
pub fn public_ip_mapping() -> HashMap<String, bool> {
    let mapping: HashMap<String, bool> = vec![
        ("https://checkip.amazonaws.com/".to_string(), true), // expects text
        ("https://api.ipify.org/".to_string(), true),         // expects text
        ("https://ipinfo.io/ip/".to_string(), true),          // expects text
        ("https://v4.ident.me/".to_string(), true),           // expects text
        ("https://httpbin.org/ip".to_string(), false),        // expects JSON
        ("https://myip.dnsomatic.com/".to_string(), true),    // expects text
    ]
        .into_iter()
        .collect();
    mapping
}

/// Function to retrieve the public IP address
///
/// # Returns
///
/// An `Option` containing the public IP address as a `String` if found, otherwise `None`
pub async fn public_ip_address() -> Option<String> {
    let ip_regex = ip_regex();
    let mapping = public_ip_mapping();

    for (url, expects_text) in mapping {
        match reqwest::get(&url).await {
            Ok(response) => {
                let extracted_ip = if expects_text {
                    response.text().await.unwrap_or_default().trim().to_string()
                } else {
                    response.json::<serde_json::Value>().await.ok()
                        .and_then(|json| json["origin"].as_str().map(str::to_string))
                        .unwrap_or_default()
                };

                if ip_regex.is_match(&extracted_ip) {
                    return Some(extracted_ip);
                }
            }
            Err(err) => {
                log::error!("Failed to fetch from {}: {}", url, err);
                continue; // Move on to the next URL
            }
        }
    }

    None
}

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema)]
pub struct IpLocation {
    pub ip: String,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub loc: Option<String>,
    pub org: Option<String>,
    pub postal: Option<String>,
    pub timezone: Option<String>,
}

pub async fn ip_location(ip: &str) -> Option<IpLocation> {
    let url = format!("https://ipinfo.io/{}/json", ip);
    let response = reqwest::get(url).await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<IpLocation>().await.ok()
}
