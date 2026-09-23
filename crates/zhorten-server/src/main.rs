mod auth;
mod db;
mod handlers;
mod helpers;

use axum::{
    Router,
    http::{HeaderValue, header},
    response::{Redirect, Response},
    routing::{delete, get, post},
};
use axum_login::{AuthManagerLayerBuilder, login_required};
use clap::Parser;
use db::Database;
use handlers::{AppState, create_link, follow_link, list_links, login, logout, remove_link};
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

#[derive(Parser)]
#[command(version, about = "A tiny self-hosted URL shortener")]
struct Args {
    #[arg(long, env = "ZHORTEN_USERNAME", default_value = "admin")]
    username: String,
    #[arg(long, env = "ZHORTEN_PASSWORD", hide_env_values = true)]
    password: String,
    #[arg(long, env = "ZHORTEN_DB", default_value = "./data/zhorten.db")]
    database: String,
    #[arg(long, env = "ZHORTEN_CACHE_CAPACITY", default_value_t = 64 * 1024 * 1024)]
    cache_capacity: u64,
    #[arg(long, env = "ZHORTEN_ADDR", default_value = "127.0.0.1:3000")]
    address: SocketAddr,
    #[arg(long, env = "ZHORTEN_SITE_ROOT", default_value = "./target/site")]
    site_root: PathBuf,
    /// Add the Secure attribute to session cookies.
    ///
    /// Enable this when zhorten is served through HTTPS. Leave it disabled for
    /// plain HTTP local development, where browsers will reject Secure cookies.
    #[arg(long, env = "ZHORTEN_SECURE_COOKIES", default_value_t = false)]
    secure_cookies: bool,
    #[arg(long, hide = true)]
    healthcheck: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = Args::parse();
    if args.healthcheck {
        std::process::exit(if healthy(args.address) { 0 } else { 1 });
    }
    if args.password.trim().is_empty() {
        eprintln!("error: --password (or ZHORTEN_PASSWORD) must not be empty");
        std::process::exit(2);
    }

    let database =
        Database::open(&args.database, args.cache_capacity).expect("unable to open sled database");
    let backend = auth::Backend::new(args.username, args.password);
    let state = AppState { database };
    let app = router(args.site_root, state, backend, args.secure_cookies);

    println!("zhorten listening on http://{}", args.address);
    let listener = tokio::net::TcpListener::bind(args.address)
        .await
        .expect("bind address");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

fn router(
    site_root: PathBuf,
    state: AppState,
    backend: auth::Backend,
    secure_cookies: bool,
) -> Router {
    let index = site_root.join("index.html");
    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_name("zhorten_session")
        .with_http_only(true)
        .with_same_site(SameSite::Strict)
        .with_secure(secure_cookies)
        .with_expiry(Expiry::OnInactivity(time::Duration::days(1)));
    let auth_layer = AuthManagerLayerBuilder::new(backend, session_layer).build();
    let protected_api = Router::new()
        .route("/api/links", get(list_links).post(create_link))
        .route("/api/links/{code}", delete(remove_link))
        .route_layer(login_required!(auth::Backend));

    Router::new()
        .route_service("/", ServeFile::new(index.clone()))
        .route_service("/admin", ServeFile::new(index))
        .nest_service("/assets", ServeDir::new(site_root.join("assets")))
        .route("/z/{code}", get(follow_link))
        .route("/{code}", get(follow_link))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .merge(protected_api)
        .route(
            "/favicon.ico",
            get(|| async { Redirect::permanent("/assets/favicon.svg") }),
        )
        .layer(axum::middleware::from_fn(security_headers))
        .layer(TraceLayer::new_for_http())
        .layer(auth_layer)
        .with_state(state)
}

fn healthy(address: SocketAddr) -> bool {
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

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

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
