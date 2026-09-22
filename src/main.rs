#![recursion_limit = "512"]

use axum::{
    Json, Router,
    extract::{FromRef, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post},
};
use clap::Parser;
use leptos::prelude::*;
use leptos_axum::{LeptosRoutes, generate_route_list};
use leptos_meta::MetaTags;
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{Arc, RwLock},
};
use time::OffsetDateTime;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use zhorten::{App, DashboardData, LinkRecord};

#[derive(Parser, Debug)]
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
}

#[derive(Clone)]
struct AppState {
    db: sled::Db,
    username: Arc<String>,
    password: Arc<String>,
    sessions: Arc<RwLock<HashSet<String>>>,
    leptos_options: LeptosOptions,
}

impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options.clone()
    }
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}
#[derive(Deserialize)]
struct CreateRequest {
    code: String,
    url: String,
}
#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorBody>)>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = Args::parse();
    if args.password.trim().is_empty() {
        eprintln!("error: --password (or ZHORTEN_PASSWORD) must not be empty");
        std::process::exit(2);
    }
    let db = sled::Config::new()
        .path(&args.database)
        .cache_capacity(args.cache_capacity)
        .open()
        .expect("unable to open sled database");
    let conf = get_configuration(None).expect("Leptos configuration");
    let mut leptos_options = conf.leptos_options;
    if leptos_options.output_name.is_empty() {
        leptos_options.output_name = "zhorten".into();
    }
    let state = AppState {
        db,
        username: Arc::new(args.username),
        password: Arc::new(args.password),
        sessions: Default::default(),
        leptos_options: leptos_options.clone(),
    };
    let routes = generate_route_list(App);

    let stylesheet = std::path::PathBuf::from(leptos_options.site_root.as_ref()).join("style.css");
    let app = Router::new()
        .route("/z/{code}", get(follow_link))
        .route("/{code}", get(follow_link))
        .route_service("/style.css", ServeFile::new(stylesheet))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/links", get(list_links).post(create_link))
        .route("/api/links/{code}", delete(remove_link))
        .leptos_routes(&state, routes, {
            let opts = leptos_options.clone();
            move || shell(opts.clone())
        })
        .fallback_service(ServeDir::new(leptos_options.site_root.as_ref()))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    println!("zhorten listening on http://{}", args.address);
    let listener = tokio::net::TcpListener::bind(args.address)
        .await
        .expect("bind address");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body><App/></body>
        </html>
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|v| v.strip_prefix("zhorten_session=").map(str::to_owned))
}

fn authorized(state: &AppState, headers: &HeaderMap) -> bool {
    cookie_token(headers)
        .is_some_and(|token| state.sessions.read().is_ok_and(|s| s.contains(&token)))
}

fn unauthorized<T>() -> ApiResult<T> {
    Err((
        StatusCode::UNAUTHORIZED,
        Json(ErrorBody {
            error: "Invalid username or password.".into(),
        }),
    ))
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<ErrorBody>)> {
    let valid_user =
        constant_time_eq::constant_time_eq(body.username.as_bytes(), state.username.as_bytes());
    let valid_pass =
        constant_time_eq::constant_time_eq(body.password.as_bytes(), state.password.as_bytes());
    if !(valid_user && valid_pass) {
        return unauthorized::<DashboardData>().map(IntoResponse::into_response);
    }
    let token: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();
    state.sessions.write().unwrap().insert(token.clone());
    let data = dashboard(&state).map_err(internal_error)?;
    let mut response = Json(data).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "zhorten_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400"
        ))
        .unwrap(),
    );
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = cookie_token(&headers) {
        state.sessions.write().unwrap().remove(&token);
    }
    let mut response = Json(serde_json::json!({"ok": true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("zhorten_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"),
    );
    response
}

async fn list_links(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<DashboardData> {
    if !authorized(&state, &headers) {
        return unauthorized();
    }
    dashboard(&state).map(Json).map_err(internal_error)
}

async fn create_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateRequest>,
) -> ApiResult<LinkRecord> {
    if !authorized(&state, &headers) {
        return unauthorized();
    }
    if body.code.is_empty()
        || body.code.len() > 32
        || !body
            .code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: "Code must be 1–32 letters, numbers, dashes, or underscores.".into(),
            }),
        ));
    }
    let parsed = url::Url::parse(&body.url).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: "Enter a valid http:// or https:// URL.".into(),
            }),
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: "Only http:// and https:// URLs are allowed.".into(),
            }),
        ));
    }
    let links = state.db.open_tree("links").map_err(internal_error)?;
    if links
        .contains_key(body.code.as_bytes())
        .map_err(internal_error)?
    {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorBody {
                error: "That short code is already in use.".into(),
            }),
        ));
    }
    let record = LinkRecord {
        code: body.code.clone(),
        url: body.url,
        clicks: 0,
        created_at: now(),
        last_clicked_at: None,
    };
    links
        .insert(body.code.as_bytes(), serde_json::to_vec(&record).unwrap())
        .map_err(internal_error)?;
    state.db.flush_async().await.map_err(internal_error)?;
    Ok(Json(record))
}

async fn remove_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(code): Path<String>,
) -> ApiResult<serde_json::Value> {
    if !authorized(&state, &headers) {
        return unauthorized();
    }
    state
        .db
        .open_tree("links")
        .map_err(internal_error)?
        .remove(code.as_bytes())
        .map_err(internal_error)?;
    state.db.flush_async().await.map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn follow_link(State(state): State<AppState>, Path(code): Path<String>) -> Response {
    let links = match state.db.open_tree("links") {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let Some(bytes) = links.get(code.as_bytes()).ok().flatten() else {
        return (StatusCode::NOT_FOUND, "Short link not found").into_response();
    };
    let Ok(record) = serde_json::from_slice::<LinkRecord>(&bytes) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let clicked_at = now();
    if links
        .update_and_fetch(code.as_bytes(), |previous| {
            let mut current =
                previous.and_then(|value| serde_json::from_slice::<LinkRecord>(value).ok())?;
            current.clicks = current.clicks.saturating_add(1);
            current.last_clicked_at = Some(clicked_at);
            serde_json::to_vec(&current).ok()
        })
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let events = match state.db.open_tree("clicks") {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let id = state.db.generate_id().unwrap_or_default();
    let _ = events.insert(
        format!("{code}:{id:020}").as_bytes(),
        clicked_at.to_be_bytes().as_slice(),
    );
    Redirect::temporary(&record.url).into_response()
}

fn dashboard(state: &AppState) -> Result<DashboardData, sled::Error> {
    let links = state.db.open_tree("links")?;
    let mut result: Vec<LinkRecord> = links
        .iter()
        .values()
        .filter_map(Result::ok)
        .filter_map(|v| serde_json::from_slice(&v).ok())
        .collect();
    result.sort_by_key(|r| std::cmp::Reverse(r.created_at));
    let total_clicks = result.iter().map(|r| r.clicks).sum();
    Ok(DashboardData {
        links: result,
        total_clicks,
    })
}

fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}
fn internal_error<E: std::fmt::Display>(error: E) -> (StatusCode, Json<ErrorBody>) {
    tracing::error!(%error, "database error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody {
            error: "Internal server error.".into(),
        }),
    )
}
