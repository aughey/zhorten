# zhorten-server

`zhorten-server` is the Axum server for zhorten. It adapts HTTP requests to the transport-agnostic functions in `zhorten-service`, serves the compiled browser app, handles authentication, and provides the sled-backed database implementation.

## Responsibilities

- Start the Axum HTTP server and compose routes, middleware, static files, and graceful shutdown.
- Serve the client-side rendered Leptos app from the assembled site directory.
- Expose redirect routes at `/z/{code}` and `/{code}`.
- Expose JSON API routes for login, logout, listing links, creating links, and deleting links.
- Protect administration API routes with `axum-login` sessions.
- Optionally expose authenticated MCP tools at `/mcp` when started with `--mcp TOKEN`.
- Implement `zhorten_service::Database` using an embedded sled database.
- Translate service errors into HTTP responses and JSON `ApiError` payloads.

## Architecture Boundary

This crate is an adapter between HTTP transport and the service API. App logic belongs in `zhorten-service`; shared datatypes belong in `zhorten-core`; browser UI belongs in `zhorten-app`.

Handlers should stay thin:

1. Extract request data with Axum.
2. Call the appropriate `zhorten-service` function.
3. Translate the result into an HTTP redirect, JSON response, or status code.

No link validation or business policy should be reimplemented in handlers. The server may own transport-specific policy such as sessions, cookies, security headers, logging, static-file routing, and command-line configuration.

## Important Files

- `src/main.rs` parses configuration, opens storage, configures authentication, builds the router, and starts Axum.
- `src/handlers.rs` binds Axum extractors and responses to `zhorten-service`.
- `src/mcp.rs` exposes authenticated MCP tools for listing, generating, creating, and deleting short links.
- `src/db.rs` implements the service `Database` trait with sled.
- `src/auth.rs` defines the single configured administrator login backend.
- `src/helpers.rs` contains small server-side utilities.

## Run

The server expects an assembled site directory containing `index.html` and browser assets:

```bash
ZHORTEN_PASSWORD='choose-a-long-random-password' \
ZHORTEN_SITE_ROOT='./target/site' \
cargo run --package zhorten-server --bin zhorten
```

See the workspace README for full source and Docker build instructions.

