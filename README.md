# zhorten

zhorten is a tiny, simple replacement for [YOURLS](https://yourls.org/). It provides short links, click counts, QR codes, and a private administration screen without requiring a separate database or web server.

The application is written in Rust with a client-side rendered Leptos app and a small Axum server. The server exposes only redirect and JSON API routes, serves the static browser bundle, and stores links in an embedded sled database. The goal is a fast, secure deployment with very little memory or operational overhead.

The production instance runs comfortably on AWS's smallest 64-bit Arm EC2 instance, a `t4g.nano` with 512 MB of memory, for a few dollars per month.

## Features

- Static client-side Leptos app served by a tight Rust API server
- One self-contained service with no external database
- Compatible `/code` and `/z/code` redirect paths
- Password-protected administration screen using `axum-login`
- Click counts and last-click timestamps
- QR code generation
- Persistent embedded storage
- Non-root, health-checked Docker image
- Native AMD64 and ARM64 container builds

## Project Layout

- `crates/zhorten-app` is the standalone Leptos CSR application compiled to WebAssembly.
- `crates/zhorten-core` contains shared datatypes and API shapes used across the workspace.
- `crates/zhorten-service` contains the transport-agnostic functional API and its validation rules.
- `crates/zhorten-server` is the Axum API, redirect, authentication, sled storage, and static-file server.
- `public` contains the HTML shell and source assets copied into the browser bundle.

The browser and server are separate build artifacts. A normal Cargo build does not run `wasm-bindgen` or assemble the static site directory.

## Architecture

zhorten is organized as four crates with narrow boundaries:

- `zhorten-core` is the shared contract crate. It defines route-safe short-link codes plus the JSON request and response types used by both sides of the application.
- `zhorten-service` is the application service layer. It exposes plain Rust functions for listing, creating, deleting, and following links. It is transport-agnostic and talks to persistence only through its `Database` trait. Most of its work is delegation, with functional validation for short codes and destination URLs where needed.
- `zhorten-server` is an Axum adapter around the service layer. It owns HTTP routing, request extraction, response/status-code mapping, sessions, security headers, static-file serving, command-line configuration, and the sled implementation of the service database trait. It should not contain app logic.
- `zhorten-app` is the Leptos browser UI. It is client-side rendered, compiled to WebAssembly with the `csr` feature, and calls the server's same-origin JSON API using the shared shapes from `zhorten-core`.

The dependency direction is intentionally simple:

```text
zhorten-app    -> zhorten-core
zhorten-service -> zhorten-core
zhorten-server -> zhorten-service -> zhorten-core
```

The server and app meet at the HTTP/API boundary. The server serves the static CSR bundle, but it does not render the UI, and the app does not know about sled, Axum, sessions, or deployment details.

## Build and Run from Source

zhorten uses the Rust nightly toolchain. Install the WebAssembly target and the `wasm-bindgen` CLI version locked by the project:

```bash
rustup toolchain install nightly
rustup override set nightly
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked --force
```

Build the browser bundle, assemble `target/site`, and build the server:

```bash
mkdir -p target/site/assets/pkg

cargo build --locked --package zhorten-app --lib --release \
  --target-dir target/front \
  --target wasm32-unknown-unknown \
  --no-default-features \
  --features csr

wasm-bindgen \
  --target web \
  --out-dir target/site/assets/pkg \
  --out-name zhorten_app \
  target/front/wasm32-unknown-unknown/release/zhorten_app.wasm

cp public/index.html target/site/
cp -R public/assets/. target/site/assets/

cargo build --locked --package zhorten-server --bin zhorten --release
```

The resulting artifacts are:

- `target/site/index.html` and `target/site/assets/`: the complete static browser application.
- `target/release/zhorten`: the server executable.

Start the server:

```bash
export ZHORTEN_USERNAME=admin
export ZHORTEN_PASSWORD='choose-a-long-random-password'
export ZHORTEN_DB='./data/zhorten.db'
export ZHORTEN_SITE_ROOT='./target/site'
./target/release/zhorten
```

Open <http://127.0.0.1:3000/admin>. Rebuild the WASM bundle after changing `zhorten-app`, rebuild the executable after changing `zhorten-server`, and recopy `public` files after changing static assets.

The command-line options are also available through environment variables:

| Option | Environment variable | Default |
| --- | --- | --- |
| `--username` | `ZHORTEN_USERNAME` | `admin` |
| `--password` | `ZHORTEN_PASSWORD` | Required |
| `--database` | `ZHORTEN_DB` | `./data/zhorten.db` |
| `--cache-capacity` | `ZHORTEN_CACHE_CAPACITY` | `67108864` (64 MiB) |
| `--address` | `ZHORTEN_ADDR` | `127.0.0.1:3000` |
| `--site-root` | `ZHORTEN_SITE_ROOT` | `./target/site` |
| `--mcp` | `ZHORTEN_MCP` | Disabled |
| `--mcp-host` | `ZHORTEN_MCP_HOST` | Loopback hosts only |
| `--secure-cookies` | `ZHORTEN_SECURE_COOKIES` | `false` |

The password is supplied at startup, converted to an Argon2 hash, and never written to the database. Authentication is managed by `axum-login` with a `tower-sessions` in-memory store. Sessions end when their one-day cookie expires or the service restarts.

## MCP

The MCP endpoint is disabled unless a bearer token is provided at startup:

```bash
./target/release/zhorten --mcp 'choose-a-long-random-token'
```

For a public deployment, allow each hostname that terminates or proxies MCP requests. The option may be repeated; `ZHORTEN_MCP_HOST` accepts a comma-separated list.

```bash
./target/release/zhorten \
  --mcp 'choose-a-long-random-token' \
  --mcp-host z.washucsc.org
```

When enabled, MCP is available at `/mcp` and every request must include:

```text
Authorization: Bearer choose-a-long-random-token
```

The MCP server exposes tools for listing links, suggesting route-safe short codes, creating one link, bulk creating links, deleting one link, and bulk deleting links. Bulk create reports per-link successes and validation failures; bulk delete treats missing links as successful no-ops, matching the JSON API.

## Run with Docker

The public image is stored in the GitHub Container Registry at `ghcr.io/aughey/zhorten`. The `latest` tag is a multi-architecture image supporting both AMD64 and ARM64.

To build the production image from the current checkout instead:

```bash
docker build -t zhorten:local .
```

Use `zhorten:local` in place of `ghcr.io/aughey/zhorten:latest` in the commands below.

```bash
docker volume create zhorten-data

docker run -d \
  --name zhorten \
  --restart unless-stopped \
  -p 80:3000 \
  -e ZHORTEN_USERNAME=admin \
  -e ZHORTEN_PASSWORD='choose-a-long-random-password' \
  -v zhorten-data:/data \
  ghcr.io/aughey/zhorten:latest
```

The administration screen is now available at <http://127.0.0.1/admin>.

Inside the container:

- `/app/zhorten` is the server executable.
- `/app/site` contains the browser bundle and static assets.
- `/data/zhorten.db` is the embedded database.
- Port `3000` serves the application.

The production image is a roughly 41 MB distroless image. It runs as UID and GID `10001`, contains no shell or package manager, and includes a built-in health check.

To use a host directory instead of a named Docker volume, make it writable by the container user:

```bash
mkdir -p ./zhorten-data
sudo chown 10001:10001 ./zhorten-data

docker run -d \
  --name zhorten \
  --restart unless-stopped \
  -p 80:3000 \
  -e ZHORTEN_PASSWORD='choose-a-long-random-password' \
  -v "$PWD/zhorten-data:/data" \
  ghcr.io/aughey/zhorten:latest
```

## AWS EC2 Setup

The reference deployment for `z.washucsc.org` uses:

- A `t4g.nano` ARM64 instance with 512 MB of memory
- Amazon Linux 2023
- An 8 GB `gp3` root volume
- Docker from the Amazon Linux repositories
- A bind-mounted sled database under `/home/ec2-user/zhorten/data`
- A systemd service running Docker as `ec2-user`
- Direct Docker port publishing from host port `80` to container port `3000`

Create an EC2 security group with inbound TCP port `80` open to the internet and port `22` restricted to trusted administration addresses. Open port `443` as well if an HTTPS reverse proxy will be added. Allocate and associate an Elastic IP before updating DNS so the address survives instance stops and starts.

Connect to the instance and install Docker:

```bash
sudo dnf install -y docker
sudo systemctl enable --now docker
sudo usermod -aG docker ec2-user
```

Log out and reconnect so the Docker group membership takes effect. Then create the application directories and configuration:

```bash
mkdir -p /home/ec2-user/zhorten/data
sudo chown 10001:10001 /home/ec2-user/zhorten/data
umask 077

cat > /home/ec2-user/zhorten/zhorten.env <<'EOF'
ZHORTEN_USERNAME=admin
ZHORTEN_PASSWORD=replace-with-a-long-random-password
ZHORTEN_CACHE_CAPACITY=67108864
ZHORTEN_SECURE_COOKIES=false
RUST_LOG=info
EOF

chmod 600 /home/ec2-user/zhorten/zhorten.env
docker pull ghcr.io/aughey/zhorten:latest
```

Install `/etc/systemd/system/zhorten.service`:

```ini
[Unit]
Description=zhorten URL shortener
Requires=docker.service
After=docker.service network-online.target
Wants=network-online.target

[Service]
Type=simple
User=ec2-user
Group=ec2-user
SupplementaryGroups=docker
WorkingDirectory=/home/ec2-user/zhorten
ExecStartPre=-/usr/bin/docker rm -f zhorten
ExecStart=/usr/bin/docker run --name zhorten --rm --pull=never -p 80:3000 --env-file /home/ec2-user/zhorten/zhorten.env -v /home/ec2-user/zhorten/data:/data ghcr.io/aughey/zhorten:latest
ExecStop=/usr/bin/docker stop -t 15 zhorten
Restart=always
RestartSec=5
TimeoutStopSec=30

[Install]
WantedBy=multi-user.target
```

Enable and verify the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now zhorten
sudo systemctl status zhorten
curl -I http://127.0.0.1/
```

The bind-mounted database survives container replacement and reboots. To deploy a new image:

```bash
docker pull ghcr.io/aughey/zhorten:latest
sudo systemctl restart zhorten
```

Check service and container logs with:

```bash
sudo journalctl -u zhorten -f
docker logs zhorten
```

## HTTPS

zhorten can serve HTTP directly and does not require another web server. For a public administration screen, HTTPS is strongly recommended. A small reverse proxy such as Caddy can terminate TLS on ports `80` and `443` and proxy to zhorten on a loopback-only port such as `127.0.0.1:3000`. Set `ZHORTEN_SECURE_COOKIES=true` when the public site uses HTTPS.

## Container Publishing

Pushes to `main` build and publish `ghcr.io/aughey/zhorten:latest`. Git version tags such as `v1.2.0` also publish semantic-version tags. GitHub Actions builds AMD64 and ARM64 images on native runners and combines them into one multi-architecture manifest. Pull requests build both architectures without publishing them.
