# RuTorrent

[![made-with-rust][rust-logo]][rust-src-page]

[![build][gh-logo]][build]

### Summary

`RuTorrent` is a lightweight API that downloads magnet links and asynchronously transfers files to a remote server over
SSH via `rsync`.

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

| Key                  | Description                                                         | Default                    |
|----------------------|---------------------------------------------------------------------|----------------------------|
| `HOST`               | Hostname to run `rutorrent`                                         | `127.0.0.1`                |
| `PORT`               | Port to run `rutorrent`                                             | `3000`                     |
| `USERNAME`           | Username to host/authenticate the UI                                | -                          |
| `PASSWORD`           | Password to host/authenticate the UI                                | -                          |
| `APIKEY`             | A random string to authenticate requests through the API interface  | MANDATORY                  |
| `WORKERS`            | Number of Actix workers to run the API                              | `2`                        |
| `QBIT_URL`           | Base URL for qbittorrent API                                        | `http://localhost:8080`    |
| `QBIT_USERNAME`      | Username for qbittorrent API                                        | —                          |
| `QBIT_PASSWORD`      | Password for qbittorrent API                                        | —                          |
| `QBIT_TIMEOUT`       | Timeout for qbittorrent API                                         | `3`                        |
| `UTC_LOGGER`         | Boolean flag to set UTC timestamps in logs                          | `true`                     |
| `LOG`                | Enum value to log to a `file` or `stdout`                           | `stdout`                   |
| `LOG_LEVEL`          | Log level to set across `actix-web`, `actix-server` and `rurorrent` | `info`                     |
| `NTFY_URL`           | Ntfy server URL for notifications                                   | —                          |
| `NTFY_TOPIC`         | Subscribed ntfy topic for notifications                             | —                          |
| `NTFY_USERNAME`      | Username to authenticate ntfy notifications                         | —                          |
| `NTFY_PASSWORD`      | Password to authenticate ntfy notifications                         | —                          |
| `NTFY_TIMEOUT`       | Timeout for the Ntfy request                                        | —                          |
| `TELEGRAM_CHAT_ID`   | Chat ID where Telegram notifications have to be sent                | —                          |
| `TELEGRAM_BOT_TOKEN` | Telegram bot token                                                  | —                          |
| `TELEGRAM_TIMEOUT`   | Timeout for the Telegram request                                    | —                          |
| `TOR_URL`            | URL to connect to `tor` network. Allows `socks5` and `socks5h`      | `socks5h://127.0.0.1:9050` |
| `TOR_TIMEOUT`        | Maximum timeout through Tor network, in seconds                     | `10`                       |
| `TRACKERS_FILE`      | Filepath to custom trackers list with each tracker in a new line    | -                          |
| `MAX_TRACKERS`       | Number of trackers to fetch and/or use                              | `2`                        |

#### Environment variables that can be overridden with API calls

**Search `GET /search`**

| Key           | Description                                                 | Default | Endpoint      |
|---------------|-------------------------------------------------------------|---------|---------------|
| `USE_TOR`     | Boolean flag to use Tor network when searching for torrents | `true`  | `GET /search` |
| `STRICT_MODE` | Boolean flag to fail if Tor proxy fails to connect          | `true`  | `GET /search` |

**Database and post download**

| Key                 | Description                                       | Endpoint                        |
|---------------------|---------------------------------------------------|---------------------------------|
| `SAVE_PATH`         | Default path to save downloaded torrents locally. | `PUT /torrent`                  |
| `REMOTE_HOST`       | Remote hostname to copy via `rsync` through SSH.  | `PUT /torrent`, `POST /torrent` |
| `REMOTE_USER`       | Username for the remote host.                     | `PUT /torrent`, `POST /torrent` |
| `REMOTE_PATH`       | Default path to copy in remote host.              | `PUT /torrent`, `POST /torrent` |
| `RSYNC_TIMEOUT`     | Default SSH `ConnectTimeout` in rsync.            | `PUT /torrent`, `POST /torrent` |
| `DELETE_AFTER_COPY` | Boolean flag to be a leecher.                     | `PUT /torrent`, `POST /torrent` |

> [!TIP]
> To get the chat ID, message the bot on Telegram and run the command:
> ```shell
> curl -s "https://api.telegram.org/bot${BOT_TOKEN}/getUpdates" \
> | jq '.result[].message.chat.id'
> ```

### Setup Instructions

#### qBittorrent

```shell
sudo apt update
sudo apt install -y qbittorrent-nox
qbittorrent-nox -d --webui-port=${QBIT_PORT}
```

**Containerized**

```shell
docker run -d \
  --name qbittorrent \
  -e WEBUI_PORT="${QBIT_PORT}" \
  -p "${QBIT_PORT}:${QBIT_PORT}" \
  -p 6881:6881 \
  -p 6881:6881/udp \
  -v "$HOME/qbittorrent/config:/config" \
  -v "$HOME/Downloads:/downloads" \
  --restart unless-stopped \
  lscr.io/linuxserver/qbittorrent:latest
```

> [!WARNING]
> Running QBitAPI through CLI will have the default username `admin` and the default password `adminadmin`<br>
> Go to `Tools` → `Options` → `Web UI` → `Authentication` to change the default password

#### RSync

```shell
# Install rsync
sudo apt update
sudo apt install rsync
# Setup SSH for remote transfer
ssh-keygen -t ed25519 -N "" -f ~/.ssh/id_ed25519
ssh-copy-id user@receiver_ip
ssh user@receiver_ip
```

> [!IMPORTANT]
> For `rsync` functionality to work, run the above commands on the machine where `rutorrent` runs

#### Tor

```shell
# Install tor
sudo apt update
sudo apt install tor
# Enable service
sudo systemctl enable --now tor
# Verify connection
nc -vz 127.0.0.1 9050
```

**Containerized**

```shell
docker run -d \
  --name tor \
  --restart unless-stopped \
  -p 127.0.0.1:9050:9050 \
  peterdavehello/tor-socks-proxy:latest
```

> [!WARNING]
> Tor service will be used as a proxy **ONLY** for RuTorrent's explore feature available through the `GET /search`
> endpoint<br>
> To set up proxy server in qBit, go to `Tools` → `Options` → `Proxy Server` and choose `SOCKS5` type

> [!CAUTION]
> Using Tor service as proxy in qBit, may delay or stall the download process

### API methods

#### Search

1. `GET /categories` - Returns the available categories for search.
   ```shell
   curl -H "apikey: ${APIKEY}" 'localhost:3000/categories?nested=false'
   ```
2. `GET /search` - Returns search results.
   ```shell
   curl -H "apikey: ${APIKEY}" 'localhost:3000/search?query=ubuntu&use_tor=true&strict=false'
   ```

#### Torrent

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
3. `POST /torrent` - Retry a failed download/transfer.
    ```shell
    curl -X POST localhost:3000/torrent \
      -H "Content-Type: application/json" \
      -H "apikey: ${APIKEY}" \
      -d '{
        "name": "Sintel",
        "remote_host": "192.168.1.99",
        "remote_username": "admin",
        "remote_path": "/Users/admin/Sintel",
        "delete_after_copy": false
      }'
    ```
4. `DELETE /torrent` - Deletes a torrent.
    ```shell
    curl -X DELETE -H "apikey: ${APIKEY}" "localhost:3000/torrent?name=Big+Buck+Bunny"
    ```
5. `PATCH /torrent` - Pause or resume a download.
    ```shell
    curl -X PATCH localhost:3000/torrent?name=Sintel&pause=true \
      -H "apikey: ${APIKEY}"
    ```

### VPN (Mullvad CLI)

1. `GET /vpn` - Get VPN state (cli: `mullvad status --json`)
   ```shell
   curl -H "apikey: ${APIKEY}" localhost:3000/vpn
   ```
2. `POST /vpn` - Connect/Reconnect/Change VPN relay
   ```shell
   curl -X POST --url "localhost:3000/vpn" \
     -H "Content-Type: application/json" \
     -H "apikey: ${APIKEY}" \
     -d '{"blacklist":"us,il,cl,cz","hostname":"","whitelist":"ch,se,ie,it,tyo"}'
   ```
3. `DELETE /vpn` - Disconnect vpn
   ```shell
   curl -X DELETE --url "localhost:3000/vpn?lockdown=false" \
     -H "apikey: ${APIKEY}"
   ```

### Standalone UI

> [!NOTE]
> Refer to the [rutorrent-ui] documentation for instructions on running the RuTorrent UI as a standalone service.

## License & copyright

&copy; Vignesh Rao

Licensed under the [MIT License][license]

[rutorrent-ui]: https://github.com/thevickypedia/RuTorrent/blob/main/rutorrent-ui/README.md
[rust-logo]: https://img.shields.io/badge/Made%20with-Rust-black?style=for-the-badge&logo=Rust
[rust-src-page]: https://www.rust-lang.org/
[gh-logo]: https://github.com/thevickypedia/RuTorrent/actions/workflows/rust.yml/badge.svg
[build]: https://github.com/thevickypedia/RuTorrent/actions/workflows/rust.yml
[license]: https://github.com/thevickypedia/RuTorrent/blob/main/LICENSE
