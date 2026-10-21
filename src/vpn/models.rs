use serde;
use utoipa::ToSchema;
use serde::Deserialize;

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
