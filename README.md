# zhorten

zhorten is a tiny, simple replacement for [YOURLS](https://yourls.org/). It provides short links, click counts, QR codes, and a private administration screen without requiring a separate database or web server.

The application is written in Rust with a client-side rendered Leptos app and a small Axum server. The server exposes only redirect and JSON API routes, serves the static browser bundle, and stores links in an embedded local database. The goal is a fast, secure deployment with very little memory or operational overhead.

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

- [`crates/zhorten-app`](crates/zhorten-app/README.md) is the standalone Leptos CSR application compiled to WebAssembly.
- [`crates/zhorten-cli`](crates/zhorten-cli) is a command-line binding to the service layer for one-off list/create/follow/delete operations.
- [`crates/zhorten-core`](crates/zhorten-core/README.md) contains shared datatypes and API shapes used across the workspace.
- [`crates/zhorten-service`](crates/zhorten-service/README.md) contains the transport-agnostic functional API and its validation rules.
- [`crates/zhorten-database`](crates/zhorten-database) opens the configured local database backend behind the service trait.
- [`crates/zhorten-sled`](crates/zhorten-sled) is the sled-backed implementation of the service database trait.
- [`crates/zhorten-sqlite`](crates/zhorten-sqlite) is the SQLite-backed implementation of the service database trait.
- [`crates/zhorten-server`](crates/zhorten-server/README.md) is the Axum API, redirect, authentication, storage factory, and static-file server.
- [`crates/zhorten-google`](crates/zhorten-google) is a Cloud Run wrapper that stores links in Firestore.
- `public` contains the HTML shell and source assets copied into the browser bundle.

The browser and server are separate build artifacts. A normal Cargo build does not run `wasm-bindgen` or assemble the static site directory.

## Architecture

zhorten is organized as a small workspace with narrow boundaries:

- `zhorten-core` is the shared contract crate. It defines route-safe short-link codes plus the JSON request and response types used by both sides of the application.
- `zhorten-service` is the application service layer. It exposes plain Rust functions for listing, creating, deleting, and following links. It is transport-agnostic and talks to persistence only through its `Database` trait. Most of its work is delegation, with functional validation for short codes and destination URLs where needed.
- `zhorten-sled` and `zhorten-sqlite` are interchangeable local storage adapters used through `zhorten-database`.
- `zhorten-database` is the local storage factory. It accepts configuration, opens sled or SQLite, and returns a shared database trait object to executable entry points.
- `zhorten-server` is an Axum adapter around the service layer. It owns HTTP routing, request extraction, response/status-code mapping, sessions, security headers, static-file serving, and command-line configuration. It should not contain app logic.
- `zhorten-cli` is a command-line adapter around the service layer. It starts no web server; each process performs one requested service operation and exits.
- `zhorten-google` is a deployment-specific binary. It chooses Cloud Run defaults and the Firestore storage adapter, then starts the shared server.
- `zhorten-app` is the Leptos browser UI. It is client-side rendered, compiled to WebAssembly with the `csr` feature, and calls the server's same-origin JSON API using the shared shapes from `zhorten-core`.

The dependency direction is intentionally simple:

```text
zhorten-app    -> zhorten-core
zhorten-service -> zhorten-core
zhorten-sled / zhorten-sqlite -> zhorten-service -> zhorten-core
zhorten-database -> local storage adapters -> zhorten-service -> zhorten-core
zhorten-server -> zhorten-database -> zhorten-service -> zhorten-core
zhorten-cli -> zhorten-database -> zhorten-service -> zhorten-core
deploy binaries -> zhorten-server and storage-specific dependencies
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

### Local development

Build the debug browser bundle and assemble the static site:

```bash
mkdir -p target/site/assets/pkg

cargo build --package zhorten-app --lib \
  --target-dir target/front \
  --target wasm32-unknown-unknown \
  --no-default-features \
  --features csr

wasm-bindgen \
  --target web \
  --out-dir target/site/assets/pkg \
  --out-name zhorten_app \
  target/front/wasm32-unknown-unknown/debug/zhorten_app.wasm

cp public/index.html target/site/
cp -R public/assets/. target/site/assets/
```

Run the development server against that site bundle:

```bash
cargo run --package zhorten-server --bin zhorten -- \
  --password bar \
  --username foo \
  --database /tmp/dev.db \
  --site-root ./target/site \
  --address 127.0.0.1:3000
```

Open <http://127.0.0.1:3000/admin>. Rebuild the browser bundle after changing
`zhorten-app`; the development server is rebuilt automatically each time
`cargo run` starts.

### Release build

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
export ZHORTEN_ADDR='127.0.0.1:3000'
./target/release/zhorten
```

Open <http://127.0.0.1:3000/admin>. Rebuild the WASM bundle after changing `zhorten-app`, rebuild the executable after changing `zhorten-server`, and recopy `public` files after changing static assets.

The command-line options are also available through environment variables. The listener address and database path are intentionally explicit; cache size, site root, and cookie security have deployment-shaped defaults.

| Option | Environment variable | Default |
| --- | --- | --- |
| `--username` | `ZHORTEN_USERNAME` | Required |
| `--password` | `ZHORTEN_PASSWORD` | Required |
| `--database-backend sled|sqlite` | `ZHORTEN_DATABASE_BACKEND` | `sled` |
| `--database` | `ZHORTEN_DB` | Required |
| `--analytics disabled|enabled` | `ZHORTEN_ANALYTICS` | `disabled` |
| `--address` | `ZHORTEN_ADDR` | Required |
| `--cache-capacity` | `ZHORTEN_CACHE_CAPACITY` | `67108864` (64 MiB, sled only) |
| `--site-root` | `ZHORTEN_SITE_ROOT` | `./target/site` for `zhorten`, `/app/site` for the Cloud Run binary |
| `--secure-cookies true|false` | `ZHORTEN_SECURE_COOKIES` | `false` for `zhorten`, `true` for the Cloud Run binary |
| `--mcp` | `ZHORTEN_MCP` | Disabled |
| `--mcp-host` | `ZHORTEN_MCP_HOST` | Loopback hosts only |

The password is supplied at startup, converted to an Argon2 hash, and never written to the database. Authentication is managed by `axum-login` with a `tower-sessions` in-memory store. Sessions end when their one-day cookie expires or the service restarts.

## Run One-Off CLI Operations

`zhorten-cli` binds the same service layer to a command-line interface without
starting Axum or serving static files. It is useful for local inspection,
scripting, and demonstrating the transport-agnostic service boundary.

```bash
cargo run --package zhorten-cli -- --database ./data/zhorten.db create docs https://example.com/docs
cargo run --package zhorten-cli -- --database ./data/zhorten.db follow docs
cargo run --package zhorten-cli -- --database ./data/zhorten.db list
cargo run --package zhorten-cli -- --database ./data/zhorten.db delete docs
```

Commands:

- `list` or `dashboard` prints dashboard JSON.
- `create <code> <url>` validates and creates one short link, then prints the created record as JSON.
- `follow <code>` resolves the short code, records a click, and prints the destination URL.
- `delete <code>` or `remove <code>` deletes a short link and treats missing links as a successful no-op.

The CLI accepts the same local storage flags used by the server. The database path is explicit; sled treats it as a database directory, while SQLite treats it as a database file.

| Option | Environment variable | Default |
| --- | --- | --- |
| `--database-backend sled|sqlite` | `ZHORTEN_DATABASE_BACKEND` | `sled` |
| `--database` | `ZHORTEN_DB` | Required |
| `--analytics disabled|enabled` | `ZHORTEN_ANALYTICS` | `disabled` |
| `--cache-capacity` | `ZHORTEN_CACHE_CAPACITY` | `67108864` (64 MiB, sled only) |

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
  -e ZHORTEN_ADDR=0.0.0.0:3000 \
  -e ZHORTEN_DB=/data/zhorten.db \
  -e ZHORTEN_CACHE_CAPACITY=67108864 \
  -e ZHORTEN_SITE_ROOT=/app/site \
  -e ZHORTEN_SECURE_COOKIES=false \
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
  -e ZHORTEN_USERNAME=admin \
  -e ZHORTEN_PASSWORD='choose-a-long-random-password' \
  -e ZHORTEN_ADDR=0.0.0.0:3000 \
  -e ZHORTEN_DB=/data/zhorten.db \
  -e ZHORTEN_CACHE_CAPACITY=67108864 \
  -e ZHORTEN_SITE_ROOT=/app/site \
  -e ZHORTEN_SECURE_COOKIES=false \
  -v "$PWD/zhorten-data:/data" \
  ghcr.io/aughey/zhorten:latest
```

## Redirect Benchmark

The server crate includes a focused benchmark for public redirect throughput.
By default it runs sled and SQLite against both the real Axum router over local
HTTP/1.1 keep-alive connections and the raw `zhorten_service` redirect path.
Each scenario seeds one short link and repeatedly follows `/bench`.

```bash
cargo bench --package zhorten-server --bench redirect_throughput
```

The benchmark can be tuned with:

| Environment variable | Default | Meaning |
| --- | --- | --- |
| `ZHORTEN_REDIRECT_BENCH_SECONDS` | `10` | Measurement duration |
| `ZHORTEN_REDIRECT_BENCH_CONCURRENCY` | `64` | Concurrent client connections or service workers |
| `ZHORTEN_REDIRECT_BENCH_BACKEND` | `all` | `sled`, `sqlite`, or `all` |
| `ZHORTEN_REDIRECT_BENCH_LAYER` | `all` | `axum`, `service`, or `all` |
| `ZHORTEN_REDIRECT_BENCH_CACHE_CAPACITY` | `67108864` | sled cache capacity in bytes |

One local run on this branch using the defaults:

```text
$ cargo bench --package zhorten-server --bench redirect_throughput

redirect benchmark
  route: /bench
  concurrency: 64
  requested duration: 10s

  storage: sled
  layer: axum over TCP keep-alive
  duration: 10.003s
  requests: 438651
  throughput: 43850.50 req/s

  storage: sled
  layer: raw zhorten_service layer
  duration: 10.005s
  requests: 3113626
  throughput: 311210.81 req/s

  storage: sqlite
  layer: axum over TCP keep-alive
  duration: 10.002s
  requests: 71706
  throughput: 7168.89 req/s

  storage: sqlite
  layer: raw zhorten_service layer
  duration: 10.002s
  requests: 99908
  throughput: 9988.63 req/s
```

Treat these numbers as local-machine throughput, not a platform guarantee. They
are useful for comparing changes to the redirect path and for showing that the
deployed shape is one Axum process serving many redirects concurrently. Use
`ZHORTEN_REDIRECT_BENCH_LAYER=service` when you want to exclude the Axum web
stack and measure the transport-agnostic service layer directly.

## Experimental Container Hosting

Three scale-to-zero container deployment experiments live under `deploy`:

- [`deploy/fly`](deploy/fly/README.md) runs the existing sled-backed server on one Fly Machine with a persistent volume mounted at `/data`.
- [`deploy/google`](deploy/google/README.md) builds the `zhorten-google` binary for Cloud Run and uses Firestore for durable storage.
- [`deploy/azure`](deploy/azure/README.md) runs the existing sled-backed server on Azure Container Apps with an Azure Files volume mounted at `/data`.

These are side-by-side experiments, not replacements for the EC2 baseline. The Fly and Azure Container Apps deployments should stay single-instance while using sled. The Cloud Run deployment defaults to minimum instances `0`, maximum instances `1`, and concurrency `80` to demonstrate one tiny async server handling many requests.

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
ZHORTEN_ADDR=0.0.0.0:3000
ZHORTEN_DB=/data/zhorten.db
ZHORTEN_CACHE_CAPACITY=67108864
ZHORTEN_SITE_ROOT=/app/site
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
