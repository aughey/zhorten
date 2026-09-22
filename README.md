# zhorten

zhorten is a tiny, simple replacement for [YOURLS](https://yourls.org/). It provides short links, click counts, QR codes, and a private administration screen without requiring a separate database or web server.

The application is written in Rust with Leptos and Axum. It serves its own UI and static files, and stores links in an embedded sled database. The goal is a fast, secure deployment with very little memory or operational overhead.

The production instance runs comfortably on AWS's smallest 64-bit Arm EC2 instance, a `t4g.nano` with 512 MB of memory, for a few dollars per month.

## Features

- One self-contained service with no external database
- Compatible `/code` and `/z/code` redirect paths
- Password-protected administration screen
- Click counts and last-click timestamps
- QR code generation
- Persistent embedded storage
- Non-root, health-checked Docker image
- Native AMD64 and ARM64 container builds

## Run with Cargo

zhorten uses the Rust nightly toolchain. Install the WebAssembly target and the `wasm-bindgen` CLI version locked by the project:

```bash
rustup toolchain install nightly
rustup override set nightly
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked --force
```

Build the browser bundle and server:

```bash
mkdir -p target/site/pkg

cargo build --locked --package zhorten --lib --release \
  --target-dir target/front \
  --target wasm32-unknown-unknown \
  --no-default-features \
  --features hydrate

wasm-bindgen \
  --target web \
  --out-dir target/site/pkg \
  --out-name zhorten \
  target/front/wasm32-unknown-unknown/release/zhorten.wasm

cp public/style.css public/favicon.svg target/site/

cargo build --locked --package zhorten --bin zhorten --release \
  --no-default-features \
  --features ssr
```

Start the server:

```bash
export ZHORTEN_USERNAME=admin
export ZHORTEN_PASSWORD='choose-a-long-random-password'
export ZHORTEN_DB='./data/zhorten.db'
export LEPTOS_SITE_ROOT='./target/site'
export LEPTOS_OUTPUT_NAME='zhorten'
./target/release/zhorten
```

Open <http://127.0.0.1:3000/admin>. Rerun the build commands after changing Rust code or frontend assets.

The command-line options are also available through environment variables:

| Option | Environment variable | Default |
| --- | --- | --- |
| `--username` | `ZHORTEN_USERNAME` | `admin` |
| `--password` | `ZHORTEN_PASSWORD` | Required |
| `--database` | `ZHORTEN_DB` | `./data/zhorten.db` |
| `--cache-capacity` | `ZHORTEN_CACHE_CAPACITY` | `67108864` (64 MiB) |
| `--address` | `ZHORTEN_ADDR` | `127.0.0.1:3000` |
| `--secure-cookies` | `ZHORTEN_SECURE_COOKIES` | `false` |

The password is supplied at startup and is never written to the database. Sessions are held in memory and end when their one-day cookie expires or the service restarts.

## Run with Docker

The public image is stored in the GitHub Container Registry at `ghcr.io/aughey/zhorten`. The `latest` tag is a multi-architecture image supporting both AMD64 and ARM64.

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
