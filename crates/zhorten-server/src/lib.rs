pub mod auth;
pub mod handlers;
pub mod helpers;
pub mod mcp;

use axum::{
    Router,
    http::{HeaderValue, header},
    middleware,
    response::{Redirect, Response},
    routing::{delete, get, post},
};
use axum_login::{AuthManagerLayerBuilder, login_required};
use handlers::{AppState, create_link, follow_link, list_links, login, logout, remove_link};
use mcp::BearerToken;
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    path::PathBuf,
    time::Duration,
};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer, cookie::SameSite};
use zhorten_database::SharedDatabase;
use zhorten_service::Database;

#[derive(Clone)]
pub struct ServerOptions {
    pub site_root: PathBuf,
    pub username: String,
    pub password: String,
    pub secure_cookies: bool,
}

#[derive(Clone)]
pub struct LocalServerOptions {
    pub server: ServerOptions,
    pub mcp_token: Option<String>,
    pub mcp_hosts: Vec<String>,
}

/// Compose the static site, public redirects, authentication endpoints, and protected API.
pub fn router<D>(database: D, options: ServerOptions) -> Router
where
    D: Database + Clone + Send + Sync + 'static,
    D::Error: Send + Sync + 'static,
{
    let index = options.site_root.join("index.html");
    let backend = auth::Backend::new(options.username, options.password);
    let state = AppState { database };

    // The session store is intentionally in-memory: restarting the service logs
    // administrators out without touching the persistent link database.
    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_name("zhorten_session")
        .with_http_only(true)
        .with_same_site(SameSite::Strict)
        .with_secure(options.secure_cookies)
        .with_expiry(Expiry::OnInactivity(time::Duration::days(1)));
    let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();

    // Only the JSON administration API requires a session. Redirects and the
    // browser shell stay public so short links keep working without auth.
    let protected_api = Router::new()
        .route("/api/links", get(list_links::<D>).post(create_link::<D>))
        .route("/api/links/{code}", delete(remove_link::<D>))
        .route_layer(login_required!(auth::Backend));

    Router::new()
        .route_service("/", ServeFile::new(index.clone()))
        .route_service("/admin", ServeFile::new(index))
        .nest_service("/assets", ServeDir::new(options.site_root.join("assets")))
        .route("/z/{code}", get(follow_link::<D>))
        .route("/{code}", get(follow_link::<D>))
        .route("/api/login", post(login::<D>))
        .route("/api/logout", post(logout))
        .route(
            "/favicon.ico",
            get(|| async { Redirect::permanent("/assets/favicon.svg") }),
        )
        .merge(protected_api)
        .layer(axum::middleware::from_fn(security_headers))
        .layer(TraceLayer::new_for_http())
        .layer(auth_layer)
        .with_state(state)
}

/// Compose the local-database server, including the optional MCP endpoint.
pub fn local_router(database: SharedDatabase, options: LocalServerOptions) -> Router {
    let mut router = router(database.clone(), options.server);
    if let Some(token) = options.mcp_token {
        tracing::info!("MCP endpoint enabled at /mcp");
        let mcp_router = Router::new()
            .nest_service("/mcp", mcp::service(database, options.mcp_hosts))
            .layer(middleware::from_fn_with_state(
                BearerToken(token),
                mcp::bearer_auth,
            ));
        router = router.merge(mcp_router);
    }
    router
}

pub async fn serve(address: SocketAddr, app: Router) -> std::io::Result<()> {
    tracing::info!("zhorten listening on http://{}", address);
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
}

/// Probe the configured listener for container health checks.
pub fn healthy(address: SocketAddr) -> bool {
    // Docker may ask the process to bind on all interfaces. For an internal
    // health probe, connect through the matching loopback family instead.
    let ip = match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    let Ok(mut stream) =
        TcpStream::connect_timeout(&SocketAddr::new(ip, address.port()), Duration::from_secs(2))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if stream
        .write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut response = [0; 12];
    stream.read(&mut response).is_ok_and(|read| {
        response[..read].starts_with(b"HTTP/1.1 200")
            || response[..read].starts_with(b"HTTP/1.0 200")
    })
}

/// Wait for Ctrl-C so axum can drain in-flight requests before exiting.
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Attach browser security headers to every response, including static assets.
async fn security_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src https://fonts.gstatic.com; img-src 'self' data:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'",
        ),
    );
    response
}
