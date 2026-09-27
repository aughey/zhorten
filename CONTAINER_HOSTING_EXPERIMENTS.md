# Container Hosting Experiments

This document is an implementation handoff for adding two new deployment shapes
to zhorten:

1. A Fly.io deployment that keeps the current "tiny self-contained web service"
   model and uses a persistent volume for embedded storage.
2. A Google Cloud Run deployment that demonstrates scale-to-zero container
   hosting with Google-managed durable storage.

The goal is not to replace the current EC2 deployment immediately. The current
single-container EC2 deployment is the baseline: it is simple, cheap, durable,
and already runs comfortably on a very small ARM64 instance. These experiments
exist to make the tradeoffs concrete for future documentation and educational
material.

## Motivation

zhorten is intentionally tiny. It serves short redirects, a private admin UI,
click counts, QR codes, and optional MCP tools. A normal installation is expected
to have fewer than 100 links, a handful of admin writes per day, and very low
redirect volume. The current deployment on a small ARM64 EC2 instance costs only
a few dollars per month, but it is always on even though the service is idle most
of the day.

The desired hosting shape is:

- Run the existing Rust/Axum service as a normal HTTP server.
- Start a tiny container on demand when traffic arrives.
- Let one running container multiplex many concurrent requests.
- Shut the container down after a short idle period.
- Accept cold starts in the hundreds of milliseconds.
- Avoid a full hosted relational database.
- Preserve the simplicity of the current deployment wherever possible.
- Keep worst-case cost exposure low during abusive traffic.

AWS Lambda was considered, but it is the wrong primary shape for this experiment.
Lambda concurrency is invocation concurrency: one HTTP request maps to one
invocation and one concurrent execution slot. That means Lambda scales by
duplicating handlers rather than by routing many simultaneous HTTP requests into
one warm async server process. That can still be cheap, but it does not
demonstrate the thing zhorten is especially good at: one tiny process handling
many lightweight requests.

Fly.io Machines and Google Cloud Run are better fits because both can run a
normal containerized HTTP server and scale idle capacity down to zero or near
zero.

## Existing Architecture

The workspace currently has these relevant crates:

- `zhorten-core`: shared API types and route-safe short codes.
- `zhorten-service`: transport-agnostic application operations and the
  `Database` trait.
- `zhorten-server`: Axum HTTP server, authentication, static file serving, MCP,
  and the current sled-backed database adapter.
- `zhorten-app`: client-side Leptos admin UI compiled to WebAssembly.

The important persistence seam is `zhorten_service::Database`:

- `dashboard`
- `create_link`
- `remove_link`
- `follow_link`

The current implementation lives in `crates/zhorten-server/src/sled_db.rs`.
Future deployment-specific storage should implement the same service trait
instead of changing business logic in `zhorten-service`.

## Target End State

The repository should support three side-by-side deployment stories:

1. **Standalone/EC2 baseline**
   - Existing Docker image.
   - Existing sled database under a Docker volume or host bind mount.
   - Best baseline for simplicity and predictable always-on operation.

2. **Fly.io**
   - One tiny Fly Machine.
   - `auto_start_machines = true`.
   - `auto_stop_machines` enabled, preferably `stop` or `suspend` after idle.
   - `min_machines_running = 0`.
   - Persistent Fly volume mounted at `/data`.
   - Existing sled database can remain the production storage backend.
   - Best demonstration of "wake a small container, serve many requests, sleep."

3. **Google Cloud Run**
   - One Cloud Run service using the zhorten container.
   - Minimum instances set to `0`.
   - Maximum instances set to `1` for the academic/single-process demonstration,
     unless the deployer intentionally wants horizontal scaling.
   - High per-instance concurrency so one container can handle many requests.
   - Google-managed storage backend instead of sled on local disk.
   - Best demonstration of scale-to-zero container hosting with managed storage.

## Proposed Repository Changes

Add deployment-specific crates while preserving the existing crate boundaries.

Suggested new crates:

- `crates/zhorten-fly`
- `crates/zhorten-google`

The exact naming can change, but the intent should stay clear:

- `zhorten-fly` owns Fly.io-specific startup/config/deployment wiring.
- `zhorten-google` owns Google Cloud Run-specific startup/config and Google
  storage integration.

There are two reasonable implementation styles:

1. **Thin deployment binaries**
   - Keep shared Axum route construction in `zhorten-server`.
   - Move reusable server assembly code out of `main.rs` if needed.
   - Each deployment crate provides a small binary that chooses configuration and
     database implementation, then starts the shared server.

2. **One binary with feature-selected storage**
   - Add feature flags to `zhorten-server`, such as `storage-sled` and
     `storage-google`.
   - Build different container images with different features.
   - This is less crate-heavy, but can make deployment behavior less obvious.

Prefer the first approach for the educational series. Separate crates make the
tradeoffs easier to explain and keep deployment-specific dependencies from
blurring the baseline server.

## Shared Server Refactor

Before adding new deployment binaries, inspect `crates/zhorten-server/src/main.rs`
and decide how much server construction should become reusable.

Likely refactor:

- Keep HTTP handlers in `handlers.rs`.
- Keep auth in `auth.rs`.
- Keep MCP wiring in `mcp.rs`.
- Extract a reusable app/server builder that accepts:
  - database implementation
  - username
  - password
  - static site root
  - secure cookie setting
  - optional MCP token/host allowlist
  - listener address/port
- Preserve the current `zhorten` binary behavior exactly.

Do not move business validation into deployment crates. The `zhorten-service`
crate should remain the policy boundary for link creation, deletion, listing,
and following.

## Fly.io Plan

Fly.io should be the lowest-friction experiment because it can keep the current
embedded storage story.

### Runtime Shape

- Build a Linux container using the existing multi-stage Docker process or a
  Fly-specific Dockerfile if necessary.
- Run the existing Axum server inside the container.
- Mount a Fly volume at `/data`.
- Set `ZHORTEN_DB=/data/zhorten.db`.
- Set `ZHORTEN_ADDR=0.0.0.0:3000`.
- Set `ZHORTEN_SECURE_COOKIES=true` for HTTPS deployments.
- Provide admin credentials through Fly secrets:
  - `ZHORTEN_USERNAME`
  - `ZHORTEN_PASSWORD`
  - optional `ZHORTEN_MCP`

### Fly Configuration

Add a Fly app configuration file, likely one of:

- `deploy/fly/fly.toml`
- `fly.toml` at repo root if the repository will contain only one Fly app

Prefer `deploy/fly/fly.toml` unless Fly tooling strongly prefers root-level
configuration.

Important settings to capture:

- HTTP service internal port: `3000`.
- Auto-start enabled.
- Auto-stop enabled.
- Minimum machines running: `0`.
- Single region for the persistent volume.
- One process group unless the Fly workflow requires more.

The important architectural constraint is that the sled database should be used
by exactly one running machine. Do not scale the Fly deployment to multiple
machines while using one local volume-backed embedded database. If multiple
machines are desired later, storage must be redesigned first.

### Fly Storage

Use a small persistent volume mounted at `/data`.

The current sled adapter is acceptable for the first Fly implementation. The
next agent should verify that:

- The distroless container user can write to the mounted volume.
- The volume mount path matches `ZHORTEN_DB`.
- Fly stop/suspend behavior does not corrupt the sled database.
- The service flushes writes after mutations, as the current adapter already
  does.

### Fly Deliverables

- Fly deployment crate or binary if needed.
- Fly deployment config.
- Dockerfile changes or Fly-specific Dockerfile only if the existing Dockerfile
  is not sufficient.
- Documentation with:
  - app creation
  - volume creation
  - secrets setup
  - deploy command
  - verification commands
  - warning about single-machine storage

## Google Cloud Run Plan

Cloud Run is the best demonstration of a scale-to-zero concurrent container that
does not require a VM. It can run the zhorten HTTP server normally and route
multiple simultaneous requests into the same container instance.

### Runtime Shape

- Build a container image for Cloud Run.
- Deploy as a Cloud Run service.
- Set minimum instances to `0`.
- Set maximum instances to `1` for the initial educational deployment.
- Set container concurrency high enough to demonstrate the async-server shape.
  A conservative initial value is `80`; a later experiment can raise it.
- Use Cloud Run's required `PORT` environment variable or adapt startup config
  so the service listens on `0.0.0.0:$PORT`.
- Set secure cookies for HTTPS.
- Provide admin credentials through Google Secret Manager or Cloud Run secrets.

### Google Storage

Do not rely on local disk for durable data in Cloud Run. Local filesystem state
is instance-local and not durable across replacement or scale-to-zero.

Implement a Google-backed database adapter for `zhorten_service::Database`.

Reasonable storage options:

1. **Firestore in Native mode**
   - Best match for a tiny key-value/document store.
   - Store one document per short code.
   - Use a transaction for create-if-absent.
   - Use an atomic increment/update for click counts and last-click timestamp.
   - Query all link documents for the dashboard.

2. **Cloud Storage JSON/object store**
   - Simpler conceptually, but trickier for concurrent updates.
   - Could store one object per code.
   - Needs conditional writes/generation checks to avoid lost updates.
   - Click counters become awkward under concurrent redirects.

Prefer **Firestore** for the Google implementation. It is closer to the existing
`Database` trait and is a better teaching example for managed key-value/document
storage. Keep the storage adapter narrow so it can be replaced later.

### Firestore Data Model

Suggested collection:

```text
links/{code}
```

Suggested document fields:

```text
code: string
url: string
clicks: integer
created_at: integer
last_clicked_at: integer | null
```

The serialized shape should continue to round-trip to `zhorten_core::api::LinkRecord`
where practical.

Operation mapping:

- `dashboard`
  - Read all documents in `links`.
  - Convert to `LinkRecord`.
  - Sort by `created_at` descending, matching current sled behavior.
  - Sum total clicks.

- `create_link`
  - Validate already happens in `zhorten-service`.
  - Use create-if-absent semantics or a transaction.
  - Return `Ok(None)` on conflict.
  - Return `Ok(Some(record))` on success.

- `remove_link`
  - Delete `links/{code}`.
  - Treat missing documents as success, matching current behavior.

- `follow_link`
  - Read `links/{code}`.
  - If missing, return `Ok(None)`.
  - Return the pre-update URL.
  - Update `clicks = clicks + 1` and `last_clicked_at = clicked_at`.
  - Prefer an atomic update/transaction to avoid lost click increments.

The current sled implementation also has a separate raw click tree for future
analytics. The initial Firestore implementation does not need to preserve that
append-only event log unless the educational series specifically wants to cover
analytics storage. The existing dashboard only needs aggregate count and last
click timestamp.

### Google Deliverables

- `zhorten-google` crate or feature-gated database adapter.
- Firestore-backed `Database` implementation.
- Cloud Run container build/deploy configuration.
- Google IAM notes:
  - Cloud Run service account can read/write Firestore.
  - Cloud Run service account can access configured secrets.
- Documentation with:
  - Google project prerequisites
  - Firestore setup
  - secret setup
  - container build/push
  - Cloud Run deploy
  - verification commands

## Cost and Abuse Controls

These deployments should include simple cost fuses, but avoid expensive managed
firewall features by default.

Recommended controls:

- Set maximum instances to `1` for the initial Cloud Run deployment.
- Keep Fly deployment to one machine while using embedded storage.
- Use platform-level request/concurrency limits where available.
- Keep application logs sparse:
  - log startup, shutdown, errors, and admin mutations
  - do not log every redirect by default
- Consider lightweight in-process rate limiting only as a demonstration, not as
  a substitute for a real edge firewall.
- Add budget alerts in the hosting platform account.

WAF-style protection is intentionally out of scope for the first version. A
managed web application firewall can be added later, but it has fixed monthly
costs that are large compared with this application's expected compute and
database costs.

## Container Expectations

Both experimental deployments should preserve the current production container
qualities:

- Small image.
- Fast startup.
- Non-root runtime user.
- No shell/package manager in the final image if practical.
- Health check or platform-native health probe.
- Static browser bundle embedded in the image.
- Server listens on all interfaces inside the container.

The current root `Dockerfile` is the starting point. Add deployment-specific
Dockerfiles only if the platform requires materially different behavior.

## Configuration Expectations

Keep configuration environment-variable driven.

Existing variables to preserve:

- `ZHORTEN_USERNAME`
- `ZHORTEN_PASSWORD`
- `ZHORTEN_DB`
- `ZHORTEN_CACHE_CAPACITY`
- `ZHORTEN_ADDR`
- `ZHORTEN_SITE_ROOT`
- `ZHORTEN_MCP`
- `ZHORTEN_MCP_HOST`
- `ZHORTEN_SECURE_COOKIES`

New variables may be needed:

- `PORT`, for Cloud Run compatibility.
- Google project/database identifiers if not discoverable from the environment.
- Storage backend selector if using one binary for multiple deployments.

Prefer deployment-specific binaries over requiring users to set a storage backend
selector manually.

## Testing Plan

Storage adapters should receive focused tests that mirror the current sled
database tests.

For Fly:

- Reuse sled database tests where possible.
- Add deployment smoke documentation rather than mocking Fly.
- Verify write persistence across container restart manually or in an integration
  script if practical.

For Google:

- Unit-test conversion between Firestore documents and `LinkRecord`.
- Integration-test against a Firestore emulator if available and not too heavy.
- Test create conflict behavior.
- Test follow-link click increments.
- Test missing delete is harmless.
- Test dashboard sorting and totals.

Server-level smoke tests should verify:

- `/health` or equivalent health route, if present.
- static admin UI serving.
- login works with configured credentials.
- create/list/delete API flow.
- redirect path returns expected location.

## Educational Narrative

The eventual writeup or video series can compare three hosting shapes:

1. **Tiny always-on VM**
   - Lowest conceptual complexity.
   - Embedded storage is easy.
   - Predictable few-dollar monthly floor.

2. **Fly.io sleeping machine**
   - Preserves normal server architecture.
   - Can sleep when idle.
   - Persistent volume keeps embedded storage simple.
   - Single-machine constraint is explicit and acceptable for this app.

3. **Cloud Run scale-to-zero container**
   - Excellent scale-to-zero HTTP container model.
   - Lets one container handle concurrent requests.
   - Requires external durable storage.
   - Demonstrates the cost and complexity tradeoff of managed storage.

The central lesson is that "serverless" is not one shape. Lambda is excellent
for per-event handlers, but zhorten is naturally a tiny concurrent web server.
For this application, scale-to-zero containers are a better teaching target than
function-per-request hosting.

## Open Questions for the Implementing Agent

- Should `zhorten-server` expose a reusable library API for route construction,
  or should the deployment crates duplicate a very small amount of startup code?
- Should the Fly deployment use the exact existing Docker image, or should it
  have a Fly-specific Dockerfile/config pair?
- Which Rust Google client crate should be used for Firestore?
- Is Firestore emulator testing worth adding in this repository, or should Google
  storage tests be split between pure unit tests and documented manual smoke
  tests?
- Should click-event history be preserved for Google storage now, or deferred
  until analytics need it?
- Should Cloud Run maximum instances stay fixed at `1` for the demo, or should
  the docs show how to raise it after switching to a fully shared storage model?

