use serde;
use serde::Deserialize;

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
