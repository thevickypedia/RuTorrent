use serde;
use utoipa::ToSchema;
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayConnector {
    pub hostname: String,
    pub city_name: String,
    pub country_name: String,
}

pub type RelayMap = HashMap<String, HashMap<String, Vec<RelayConnector>>>;

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
