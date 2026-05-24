## RuTorrent UI

The RuTorrent web interface can be accessed directly from the machine running both **QBitAPI** and **RuTorrent**. However, the UI is also designed to run independently as a standalone frontend.

This setup is useful when you want to:

* Host the UI separately from the backend services
* Access RuTorrent remotely through a dedicated web server
* Deploy the frontend in a lightweight containerized environment

### Configure the Backend Endpoint

Before starting the standalone UI, update the `backend_origin` value in [upstream.conf]

This should point to the service endpoint where RuTorrent backend is running.

### Start the Standalone UI

Navigate to the `rutorrent-ui` directory and start the NGINX container using Docker Compose:

```shell
cd rutorrent-ui
docker-compose up -d
```

This launches the RuTorrent frontend independently using NGINX.

### Template Reuse

The Docker Compose configuration is designed to download the latest `index.html` (from GitHub) located in [templates] directory.

This ensures consistency between the embedded UI and the standalone deployment while avoiding duplication of frontend assets.

### Architecture Overview

```text
┌───────────────┐
│   Browser     │
└──────┬────────┘
       │
       ▼
┌───────────────┐
│ Standalone UI │
└──────┬────────┘
       │ NGINX reverse proxy
       ▼
┌───────────────┐
│ RuTorrent API │
└───────────────┘
```

### Notes

* Ensure Docker and Docker Compose are installed before starting the UI container.
* Verify that the backend service is reachable from the NGINX container.
* If deploying remotely, configure appropriate firewall and reverse proxy settings for secure access.

[upstream.conf]: https://github.com/thevickypedia/RuTorrent/blob/main/rutorrent-ui/nginx/upstream.conf?utm_source=chatgpt.com#L9
[templates]: https://github.com/thevickypedia/RuTorrent/tree/main/src/templates?utm_source=chatgpt.com
