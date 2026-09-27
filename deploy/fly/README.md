# Fly.io Deployment

This deployment runs the normal zhorten Axum server on one Fly Machine and
keeps the existing sled database on one persistent Fly Volume mounted at
`/data`. It is the lowest-friction scale-to-zero experiment because it preserves
the same self-contained storage model as the EC2/Docker baseline.

Fly-specific references:

- [`fly.toml` service configuration](https://fly.io/docs/reference/configuration/#the-http_service-section)
- [Proxy autostop/autostart](https://fly.io/docs/reference/fly-proxy-autostop-autostart/)
- [Fly Volumes](https://fly.io/docs/volumes/overview/)
- [`fly volumes` commands](https://fly.io/docs/flyctl/volumes/)

## Runtime Shape

The supplied [`fly.toml`](fly.toml) configures:

- `internal_port = 3000`, matching the container's Axum listener.
- `force_https = true`, so public traffic is HTTPS.
- `auto_start_machines = true`, so an idle app wakes when traffic arrives.
- `auto_stop_machines = "stop"`, so Fly can stop the Machine after it is idle.
- `min_machines_running = 0`, so the app can scale all the way down.
- One volume named `zhorten_data`, mounted at `/data`.
- `ZHORTEN_DB=/data/zhorten.db`, so sled stores durable state on the volume.
- `ZHORTEN_SECURE_COOKIES=true`, because the public app is served through HTTPS.

The root `Dockerfile` is used unchanged. It builds the Leptos browser bundle,
builds the `zhorten` server binary, and runs the final image as the non-root
distroless user `10001:10001`.

## Storage Constraint

Keep this deployment to one running Machine while it uses sled. The database is
an embedded local database on one mounted volume, not a shared network database.
That is exactly what makes this deployment simple and cheap, but it means the
app should not be scaled horizontally.

Safe shape:

```text
one Fly app
one primary region
one Machine
one volume mounted at /data
```

Do not run `fly scale count 2` for this app unless storage is redesigned first.
For multi-instance Cloud Run-style storage, use the Firestore-backed Google
experiment instead.

## One-Time Setup

Install and authenticate `flyctl`, then choose a globally unique Fly app name.
If the app name is not `zhorten`, update the `app = "zhorten"` line in
[`fly.toml`](fly.toml) before running the commands below.

Choose a region close to the expected users. The checked-in config uses `ord`.
Create the app and one small volume in the same region:

```bash
fly apps create zhorten
fly volumes create zhorten_data --size 1 --region ord --app zhorten
```

Set the admin credentials as Fly secrets. These are not committed into
`fly.toml`.

```bash
fly secrets set \
  ZHORTEN_USERNAME=admin \
  ZHORTEN_PASSWORD='choose-a-long-random-password' \
  --app zhorten
```

Optional MCP support uses the same environment variables as the baseline server:

```bash
fly secrets set \
  ZHORTEN_MCP='choose-a-long-random-token' \
  ZHORTEN_MCP_HOST='zhorten.fly.dev' \
  --app zhorten
```

If you use a custom domain, set `ZHORTEN_MCP_HOST` to that hostname instead of
the default `*.fly.dev` hostname.

## Deploy

Deploy from the repository root:

```bash
fly deploy --config deploy/fly/fly.toml --dockerfile Dockerfile
```

The deploy builds the production image from the root `Dockerfile`, pushes it to
Fly's registry, and updates the app's Machine. After deploy, Fly's proxy should
start the Machine on the first request and stop it again after it becomes idle.

## Verify

Check app and Machine state:

```bash
fly status --config deploy/fly/fly.toml
fly machine list --config deploy/fly/fly.toml
fly volumes list --config deploy/fly/fly.toml
```

Check logs while making a request:

```bash
fly logs --config deploy/fly/fly.toml
curl -I https://zhorten.fly.dev/admin
```

The first request after an idle stop can include a cold start. Subsequent
requests should hit the warm Axum process.

To verify persistence, create a short link in the admin UI, stop the Machine,
then request the short link again after Fly restarts it:

```bash
fly machine list --config deploy/fly/fly.toml
fly machine stop <machine-id> --config deploy/fly/fly.toml
curl -I https://zhorten.fly.dev/<code>
```

## Operations

Useful commands:

```bash
fly logs --config deploy/fly/fly.toml
fly releases --config deploy/fly/fly.toml
fly secrets list --config deploy/fly/fly.toml
fly ssh console --config deploy/fly/fly.toml
```

Volume snapshots are managed by Fly. For manual inspection and recovery:

```bash
fly volumes list --config deploy/fly/fly.toml
fly volumes show <volume-id> --config deploy/fly/fly.toml
fly volumes snapshots list <volume-id> --config deploy/fly/fly.toml
```

To resize the volume later:

```bash
fly volumes extend <volume-id> --size <new-size-gb> --config deploy/fly/fly.toml
```

Keep logs sparse and watch billing during public experiments. The checked-in
config is intentionally conservative: one small VM, one tiny volume, and no
minimum running Machines.

## Configuration Notes

The checked-in `fly.toml` keeps deploy-time configuration in three places:

- Non-secret runtime defaults in `[env]`.
- The durable volume mount in `[[mounts]]`.
- Proxy and scale-to-zero behavior in `[http_service]`.

Secrets belong in Fly secrets:

- `ZHORTEN_USERNAME`
- `ZHORTEN_PASSWORD`
- optional `ZHORTEN_MCP`
- optional `ZHORTEN_MCP_HOST`

The application listens on `0.0.0.0:3000` inside the container. Fly terminates
TLS at the edge and forwards HTTP to that internal port.
