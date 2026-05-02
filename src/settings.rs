use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Config {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

impl Config {
    pub fn new() -> Self {
        let base_url = env::var("BASE_URL").unwrap_or("http://localhost:8080".to_string());
        let username = env::var("USERNAME").unwrap_or_default();
        let password = env::var("PASSWORD").unwrap_or_default();

        Self {
            base_url,
            username,
            password,
        }
    }
}

pub type SharedState = Arc<RwLock<HashMap<String, RsyncTrack>>>;

#[derive(Clone, Debug, Serialize)]
pub enum Status {
    Downloading(f64),
    Copying(f64),
    Completed,
}

#[derive(Clone, Debug)]
pub struct RsyncTrack {
    pub name: String,
    pub status: Status,
    pub rsync: Option<RsyncTarget>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RsyncTarget {
    pub host: String,
    pub username: String,
    pub remote_path: String,
}

#[derive(Deserialize)]
pub struct PutRequest {
    pub urls: Vec<String>,
    pub rsync: Option<RsyncTarget>,
}
