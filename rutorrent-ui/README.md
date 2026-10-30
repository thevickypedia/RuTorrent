## RuTorrent UI

The RuTorrent web interface can be accessed directly from the machine running both **qBit API** and **RuTorrent**.
However, this guide is to set up the Nginx reverse proxy as UI in front of the service running on the same LAN.

This setup is useful when you want to:

* Host the UI separately from the backend services
* Access RuTorrent remotely through a dedicated web server
* Deploy the frontend in a lightweight containerized environment
* Access the API/UI through Nginx without exposing the host machine to public internet
* Contain VPN connections to one host machine, and still have access to it through internal network

### Configure the Backend Endpoint

Before starting the standalone UI, update the `backend_origin` value in [upstream.conf]

This should point to the service endpoint where RuTorrent backend is running.

### Start the nginx server

Navigate to the `rutorrent-ui` directory and start the NGINX container using Docker Compose:

```shell
cd rutorrent-ui
docker-compose up -d
```

This launches the RuTorrent frontend independently using NGINX.

### Architecture Overview

```text
 External
  ┌─────────────────┐
  │     Browser     │
  └────────┬────────┘
           │
┌──────────│──────────┐
│ LAN      ▼          │
│  ┌───────────────┐  │
│  │  Nginx Proxy  │  │
│  └───────┬───────┘  │
│          │          │
│          ▼          │
│  ┌───────────────┐  │
│  │ RuTorrent API │  │
│  └───────────────┘  │
└─────────────────────┘
```

### Notes

* Ensure Docker and Docker Compose are installed before starting the UI container.
* Verify that the backend service is reachable from the NGINX container.
* If deploying remotely, configure appropriate firewall and reverse proxy settings for secure access.
* If the host machine uses a vpn, make sure to enable `LanSharing` (built-in support available through mullvad cli)

[upstream.conf]: https://github.com/thevickypedia/RuTorrent/blob/main/rutorrent-ui/nginx/upstream.conf?utm_source=chatgpt.com#L9
[templates]: https://github.com/thevickypedia/RuTorrent/tree/main/src/templates?utm_source=chatgpt.com
