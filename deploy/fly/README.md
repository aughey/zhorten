# Fly.io Deployment

This deployment keeps the baseline sled storage model and mounts a single Fly
volume at `/data`. Run only one machine while using this embedded database.

```bash
fly apps create zhorten
fly volumes create zhorten_data --size 1 --region ord
fly secrets set \
  ZHORTEN_USERNAME=admin \
  ZHORTEN_PASSWORD='choose-a-long-random-password'

fly deploy --config deploy/fly/fly.toml --dockerfile Dockerfile
```

Optional MCP support uses the same settings as the baseline server:

```bash
fly secrets set \
  ZHORTEN_MCP='choose-a-long-random-token' \
  ZHORTEN_MCP_HOST='your-app.fly.dev'
```

Useful checks:

```bash
fly status --config deploy/fly/fly.toml
fly logs --config deploy/fly/fly.toml
fly ssh console --config deploy/fly/fly.toml
```

The important constraint is storage ownership: sled is safe here because one
machine owns one mounted volume. Do not scale this Fly app horizontally until
storage is redesigned around a shared backend.
