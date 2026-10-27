use serde;
use serde::Deserialize;
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

#[derive(serde::Serialize, serde::Deserialize, ToSchema)]
pub struct MullvadResponse {
    pub state: String,
    pub details: Option<Details>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, ToSchema)]
pub struct Details {
    pub location: Option<Location>,
    pub locked_down: Option<bool>,
    pub feature_indicators: Option<Vec<Features>>
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
}
