use serde;
use serde::{Deserialize, Deserializer};
use utoipa::ToSchema;

#[derive(Debug)]
pub enum VpnError {
    Invalid(String),     // 400 - bad user input
    Unavailable(String), // 503 - mullvad CLI missing / relay list unreachable
    Failed(String),      // 502 - mullvad CLI returned an error
    Timeout(String),     // 504 - state never reached
}

impl std::fmt::Display for VpnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (Self::Invalid(m) | Self::Unavailable(m) | Self::Failed(m) | Self::Timeout(m)) = self;
        write!(f, "{m}")
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RelayLocation {
    pub city: String,
    pub country: String,
    pub city_code: String,
    pub country_code: String,
}

fn lenient_details<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Details>, D::Error> {
    let raw = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value::<Details>(raw).ok()) // null / transient shapes -> None
}

#[derive(serde::Serialize, serde::Deserialize, ToSchema)]
pub struct MullvadResponse {
    pub state: String,
    #[serde(default, deserialize_with = "lenient_details")]
    pub details: Option<Details>,
}

#[derive(serde::Serialize, serde::Deserialize, ToSchema)]
pub struct Details {
    pub location: Location,
}

#[derive(serde::Serialize, serde::Deserialize, ToSchema)]
pub struct Location {
    pub city: String,
    pub country: String,
    pub ipv4: Option<String>,
    pub hostname: Option<String>,
}
