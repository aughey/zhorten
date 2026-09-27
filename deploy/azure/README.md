# Azure Container Apps Deployment

This deployment runs the normal `zhorten` container image on Azure Container
Apps and keeps the existing local database on an Azure Files volume mounted at
`/data`. It mirrors the Fly.io experiment: one HTTP container, one durable
volume, HTTPS at the platform edge, and scale-to-zero when idle.

Azure-specific references:

- [Azure Container Apps YAML create/update](https://learn.microsoft.com/en-us/cli/azure/containerapp)
- [Azure Files storage mounts](https://learn.microsoft.com/en-us/azure/container-apps/storage-mounts-azure-files)
- [HTTP scaling](https://learn.microsoft.com/en-us/azure/container-apps/scale-app)
- [Ingress configuration](https://learn.microsoft.com/en-us/azure/container-apps/ingress-how-to)

## Runtime Shape

The supplied [`container-app.yaml`](container-app.yaml) configures:

- External HTTP ingress on target port `3000`. Azure terminates HTTPS and
  forwards plain HTTP to the container.
- `allowInsecure = false`, so the public endpoint redirects HTTP to HTTPS.
- `minReplicas = 0`, so the app can scale all the way down when idle.
- `maxReplicas = 1`, because the local embedded database is mounted into one
  replica and is not a shared multi-writer database.
- An HTTP scale rule at `80` concurrent requests, matching the Cloud Run
  experiment's concurrency target while still capping replicas at one.
- A volume named `zhorten-data`, backed by an Azure Container Apps environment
  storage mount named `zhorten-data`, mounted at `/data`.
- `ZHORTEN_DB=/data/zhorten.db`, so sled stores durable state on the mounted
  volume.
- `ZHORTEN_SECURE_COOKIES=true`, because public traffic is served through
  HTTPS.
- `ZHORTEN_PASSWORD` as a Container Apps secret reference.

The root [`Dockerfile`](../../Dockerfile) is used unchanged. It builds the
Leptos browser bundle, builds the `zhorten` server binary, and runs the final
image as the non-root distroless user `10001:10001`.

## Storage Constraint

Keep this deployment to one running replica while it uses the local database.
Azure Files provides durable mounted storage, but it does not turn the embedded
database into a horizontally scalable storage service.

Safe shape:

```text
one Container App
one active revision
zero or one running replica
one Azure Files share mounted at /data
```

Do not raise `maxReplicas` above `1` unless storage is redesigned first. For a
true multi-replica serverless deployment, use a cloud database-backed adapter
instead of the local sled-backed image.

## One-Time Setup

Install and authenticate the Azure CLI, then choose globally unique names where
Azure requires them.

```bash
az login
az extension add --name containerapp --upgrade
az provider register --namespace Microsoft.App
az provider register --namespace Microsoft.OperationalInsights

RESOURCE_GROUP=zhorten-aca
LOCATION=eastus
CONTAINERAPPS_ENVIRONMENT=zhorten-env
STORAGE_ACCOUNT=<globally-unique-storage-account>
STORAGE_SHARE=zhorten-data
STORAGE_MOUNT=zhorten-data
CONTAINER_APP=zhorten

az group create \
  --name "$RESOURCE_GROUP" \
  --location "$LOCATION"

az containerapp env create \
  --name "$CONTAINERAPPS_ENVIRONMENT" \
  --resource-group "$RESOURCE_GROUP" \
  --location "$LOCATION"

az storage account create \
  --name "$STORAGE_ACCOUNT" \
  --resource-group "$RESOURCE_GROUP" \
  --location "$LOCATION" \
  --sku Standard_LRS

az storage share-rm create \
  --resource-group "$RESOURCE_GROUP" \
  --storage-account "$STORAGE_ACCOUNT" \
  --name "$STORAGE_SHARE" \
  --quota 1

STORAGE_KEY=$(
  az storage account keys list \
    --resource-group "$RESOURCE_GROUP" \
    --account-name "$STORAGE_ACCOUNT" \
    --query '[0].value' \
    --output tsv
)

az containerapp env storage set \
  --name "$CONTAINERAPPS_ENVIRONMENT" \
  --resource-group "$RESOURCE_GROUP" \
  --storage-name "$STORAGE_MOUNT" \
  --azure-file-account-name "$STORAGE_ACCOUNT" \
  --azure-file-account-key "$STORAGE_KEY" \
  --azure-file-share-name "$STORAGE_SHARE" \
  --access-mode ReadWrite
```

Update [`container-app.yaml`](container-app.yaml) before the first deploy:

- Set `location` to the same Azure region as `LOCATION`.
- Set `properties.configuration.secrets[0].value` to a long random password.
- Change `properties.template.containers[0].image` if you want to deploy a
  different tag, for example an Azure Container Registry image.
- Keep `properties.template.volumes[0].storageName` aligned with
  `STORAGE_MOUNT`.

If you publish from a private Azure Container Registry, add registry
configuration to the Container App or use a managed identity with pull access.

## Deploy

Deploy from the repository root:

```bash
az containerapp create \
  --name "$CONTAINER_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --environment "$CONTAINERAPPS_ENVIRONMENT" \
  --yaml deploy/azure/container-app.yaml \
  --query properties.configuration.ingress.fqdn
```

For later updates:

```bash
az containerapp update \
  --name "$CONTAINER_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --yaml deploy/azure/container-app.yaml
```

After the first deployment, remove the literal secret value from local copies of
the YAML or update secrets with the CLI so it is not accidentally committed.

```bash
az containerapp secret set \
  --name "$CONTAINER_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --secrets zhorten-password='choose-a-long-random-password'
```

## Verify

Check app state and the public hostname:

```bash
az containerapp show \
  --name "$CONTAINER_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --query '{fqdn:properties.configuration.ingress.fqdn, replicas:properties.template.scale}'

az containerapp revision list \
  --name "$CONTAINER_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --output table
```

Check logs while making a request:

```bash
az containerapp logs show \
  --name "$CONTAINER_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --follow

curl -I "https://$(az containerapp show \
  --name "$CONTAINER_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --query properties.configuration.ingress.fqdn \
  --output tsv)/admin"
```

To verify persistence, create a short link in the admin UI, let the app scale to
zero, then request the short link again after Azure starts a new replica.

## Optional MCP

Optional MCP support uses the same environment variables as the baseline server:

- `ZHORTEN_MCP`
- `ZHORTEN_MCP_HOST`

Add them as secrets or non-secret environment variables in
[`container-app.yaml`](container-app.yaml). For a public deployment,
`ZHORTEN_MCP_HOST` should include the Azure Container Apps hostname or your
custom domain.

## Operations

Useful commands:

```bash
az containerapp show --name "$CONTAINER_APP" --resource-group "$RESOURCE_GROUP"
az containerapp logs show --name "$CONTAINER_APP" --resource-group "$RESOURCE_GROUP"
az containerapp revision list --name "$CONTAINER_APP" --resource-group "$RESOURCE_GROUP" --output table
az containerapp secret list --name "$CONTAINER_APP" --resource-group "$RESOURCE_GROUP"
az containerapp env storage list --name "$CONTAINERAPPS_ENVIRONMENT" --resource-group "$RESOURCE_GROUP"
```

Azure Files backup and retention should be configured on the storage account if
this experiment becomes more than a disposable test. Keep logs sparse and watch
billing during public experiments. The checked-in shape is intentionally small:
one 0.25 vCPU / 0.5 GiB container, one tiny file share, and no minimum running
replicas.
