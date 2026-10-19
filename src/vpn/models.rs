use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone)]
pub struct RelayConnector {
    pub hostname: String,
    pub city_name: String,
    pub country_name: String,
}

pub type RelayMap = HashMap<String, HashMap<String, Vec<RelayConnector>>>;

#[derive(Debug)]
pub struct MullvadConnection {
    pub hostname: String,
    pub city: String,
    pub country: String,
    pub ipv4: String,
}

#[derive(Deserialize)]
pub struct MullvadResponse {
    pub state: String,
    pub details: Option<Details>,
}

#[derive(Deserialize)]
pub struct Details {
    pub location: Location,
}

#[derive(Deserialize)]
pub struct Location {
    pub city: String,
    pub country: String,
    pub ipv4: Option<String>,
    pub hostname: String,
}
