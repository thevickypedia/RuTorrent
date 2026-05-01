#![allow(rustdoc::bare_urls)]
#![doc = include_str!("../README.md")]

use reqwest::blocking::Client;

pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    let magnet = "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel";
    let client = Client::new();
    client
        .post("http://localhost:8080/api/v2/torrents/add")
        .form(&[("urls", magnet)])
        .send()?;
    println!("Magnet sent to qBittorrent");
    Ok(())
}
