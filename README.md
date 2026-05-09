# RuTorrent

[![made-with-rust][rust-logo]][rust-src-page]

[![build][gh-logo]][build]

### Summary
`RuTorrent` is a lightweight API that downloads magnet links and asynchronously transfers files to a remote server over SSH via `rsync`.

### Installation

```shell
cargo add rutorrent
```

### Usage
```rust,no_run
use rutorrent;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
   rutorrent::start().await
}
```

### Environment Variables
- **HOST**: Hostname to run `rutorrent`. Defaults to `127.0.0.1`
- **PORT**: Port to run `rutorrent`. Defaults to `3000`
- **APIKEY**: A random string to authenticate requests through the API interface.
- **WORKERS**: Number of Actix workers to run the API. Defaults to number of CPUs.
- **QBIT_URL**: Base URL for qbittorrent API. Defaults to `http://localhost:8080`
- **QBIT_USERNAME**: Username for qbittorrent API
- **QBIT_PASSWORD**: Password for qbittorrent API
- **UTC_LOGGER**: Boolean flag to set UTC timestamps in logs. Defaults to `true`
- **LOG**: Enum value to log to a `file` or `stdout`. Defaults to `stdout`
- **LOG_LEVEL**: Log level to set across `actix-web`, `actix-server` and `rurorrent`
- **REMOTE_HOST**: Remote hostname to copy via `rsync` through ssh. Can be overridden in `PUT /torrent`
- **REMOTE_USER**: Username for the remote host. Can be overridden in `PUT /torrent`
- **REMOTE_PATH**: Default path to copy in remote host. Can be overridden in `PUT /torrent`
- **SAVE_PATH**: Default path to save downloaded torrents locally. Can be overridden in `PUT /torrent`
- **NTFY_URL**: Ntfy server URL for notifications.
- **NTFY_TOPIC**: Subscribed ntfy topic for notifications.
- **NTFY_USERNAME**: Username to authentication ntfy notifications.
- **NTFY_PASSWORD**: Password to authentication ntfy notifications.
- **TELEGRAM_BOT_TOKEN**: Telegram bot token.
- **TELEGRAM_CHAT_ID**: Chat ID where telegram notifications has to be sent.

> [!TIP]
> To get the chat ID, message the bot on Telegram and run the command:
> ```shell
> curl -s "https://api.telegram.org/bot${BOT_TOKEN}/getUpdates" \
> | jq '.result[].message.chat.id'
> ```

### Setup Instructions
**qbittorrent**
```shell
sudo apt update
sudo apt install -y qbittorrent-nox
qbittorrent-nox -d --webui-port=${QBIT_PORT}
```

> [!WARNING]
> Running QBitAPI through CLI will have the default username `admin` and the default password `adminadmin`<br>
> Go to `Tools` → `Options` → `Web UI` → `Authentication` to change the default password

**rsync**
```shell
sudo apt update
sudo apt install rsync
```

**rsync + ssh**
```shell
ssh-keygen -t ed25519 -N "" -f ~/.ssh/id_ed25519
ssh-copy-id user@receiver_ip
ssh user@receiver_ip
```
> For `rsync` functionality to work, run the above on the machine where `rutorrent` runs

### API methods

1. `GET /torrent` - Returns the download/copy status.
    ```shell
    curl -H "apikey: ${APIKEY}" localhost:3000/torrent
    ```
2. `PUT /torrent` - Adds new torrent URLs to the queue.
    ```shell
    curl -X PUT localhost:3000/torrent \
      -H "Content-Type: application/json" \
      -H "apikey: ${APIKEY}" \
      -d '[
	    # Download (at custom local path) and transfer content to ssh://admin@192.168.1.102:/Users/admin/Sintel and delete after transfer
        {
          "url": "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel",
          "save_path": "/home/admin/Downloads",  # overrides the local `save_path`
          "remote_host": "192.168.1.102",
          "remote_username": "admin",
          "remote_path": "/Users/admin/Sintel",
          "delete_after_copy": true
        },
        # Download (at default local path) and transfer content to ssh://admin@192.168.1.100:/home/admin/Big_Buck retaining local content
        {
          "url": "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny",
          "remote_host": "192.168.1.100",
          "remote_username": "admin",
          "remote_path": "/home/admin/Big_Buck"
        },
	    # Download (at default local path) without any subsequent transfer (delete_after_copy does not apply without remote transfer)
        {
          "url": "magnet:?xt=urn:btih:2C6B6858D61DA9543D4231A71DB4B1C9264B0685&dn=Ubuntu%2022.04%20LTS"
        }
      ]'
    ```
3. `DELETE /torrent` - Deletes a torrent.
    ```shell
   curl -X DELETE -H "apikey: ${APIKEY}" "http://localhost:3000/torrent?name=Big+Buck+Bunny"
    ```

## License & copyright

&copy; Vignesh Rao

Licensed under the [MIT License][license]

[rust-logo]: https://img.shields.io/badge/Made%20with-Rust-black?style=for-the-badge&logo=Rust
[rust-src-page]: https://www.rust-lang.org/
[gh-logo]: https://github.com/thevickypedia/RuTorrent/actions/workflows/rust.yml/badge.svg
[build]: https://github.com/thevickypedia/RuTorrent/actions/workflows/rust.yml
[license]: https://github.com/thevickypedia/RuTorrent/blob/main/LICENSE
