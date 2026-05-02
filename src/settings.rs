use std::env;

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
