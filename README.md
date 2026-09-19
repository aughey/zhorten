# zhorten

A deliberately small, self-hosted URL shortener built with Rust, Leptos, Axum, and sled.

## Run

```powershell
.\scripts\dev.ps1 -Password "choose-a-password"
```

Then open <http://127.0.0.1:3000/admin>. The default username is `admin`.

```powershell
.\scripts\dev.ps1 -Username jane -Password "choose-a-password" -Database ./data/links.db -Address 0.0.0.0:3000
```

The included scripts build the browser and server halves without depending on a particular `cargo-leptos` release. With a current `cargo-leptos`, `cargo leptos watch -- --password "choose-a-password"` also provides hot reload.

The same settings can be supplied as `ZHORTEN_USERNAME`, `ZHORTEN_PASSWORD`, `ZHORTEN_DB`, and `ZHORTEN_ADDR`.

Short URLs use `/z/:code`. Each redirect atomically updates its total and stores a compact timestamped click event in sled. Admin sessions live in memory and expire via their one-day cookie or when the server restarts.

For production, put the server behind an HTTPS reverse proxy. The admin password is intentionally supplied at startup and is never written to the database.

## Docker

```powershell
docker build -t zhorten .
docker run --rm -p 3000:3000 -v zhorten-data:/data -e ZHORTEN_PASSWORD="choose-a-password" zhorten
```

Pushes to `main` build and publish `ghcr.io/<owner>/<repository>:latest`. Pull requests build the image without publishing it; version tags such as `v1.2.0` also publish semantic-version tags.
