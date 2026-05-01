#![allow(rustdoc::bare_urls)]
#![doc = include_str!("../README.md")]

use reqwest::blocking::Client;
use serde_json::Value;
use std::{thread::sleep, time::Duration};

const BASE: &str = "http://localhost:8080";
const USERNAME: &str = "admin";
const PASSWORD: &str = "YOUR_PASSWORD";

fn login(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let resp = client
        .post(format!("{}/api/v2/auth/login", BASE))
        .form(&[
            ("username", USERNAME),
            ("password", PASSWORD),
        ])
        .send()?;

    let text = resp.text()?;
    if text != "Ok." {
        return Err(format!("Login failed: {}", text).into());
    }

    println!("Authenticated");
    Ok(())
}

fn add_torrents(client: &Client, urls: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let joined = urls.join("\n"); // qBittorrent accepts newline-separated URLs

    client
        .post(format!("{}/api/v2/torrents/add", BASE))
        .form(&[("urls", joined)])
        .send()?;

    println!("Sent {} torrent(s)", urls.len());
    Ok(())
}

fn get_hashes(client: &Client) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let resp: Value = client
        .get(format!("{}/api/v2/torrents/info", BASE))
        .send()?
        .json()?;

    let hashes = resp
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|t| t["hash"].as_str().map(|s| s.to_string()))
        .collect();

    Ok(hashes)
}

fn poll_completion(client: &Client, hashes: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let url = format!(
            "{}/api/v2/torrents/info?hashes={}",
            BASE,
            hashes.join("|")
        );

        let resp: Value = client.get(&url).send()?.json()?;

        let mut all_done = true;

        if let Some(arr) = resp.as_array() {
            for t in arr {
                let name = t["name"].as_str().unwrap_or("?");
                let progress = t["progress"].as_f64().unwrap_or(0.0);
                let state = t["state"].as_str().unwrap_or("");

                println!(
                    "{}: {:.2}% ({})",
                    name,
                    progress * 100.0,
                    state
                );

                if progress < 1.0 {
                    all_done = false;
                }
            }
        }

        if all_done {
            println!("DONE");
            break;
        }

        sleep(Duration::from_secs(2));
    }

    Ok(())
}

pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    let magnets = vec![
        "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel"
    ];

    // 👇 THIS is the important part: cookie store enabled
    let client = Client::builder()
        .cookie_store(true)
        .build()?;

    login(&client)?;
    add_torrents(&client, &magnets)?;

    // give qbittorrent a moment to register torrents
    sleep(Duration::from_secs(2));

    let hashes = get_hashes(&client)?;
    poll_completion(&client, &hashes)?;

    Ok(())
}
