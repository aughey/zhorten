//! The purpose of handlers is to setup the binding between the axum http routes and the business logic
//! implemented in the `zhorten-service` crate.  No business logic should be implemented here.  This is
//! simply a translation layer to demartial the request and response types between the two layers.

use crate::{
    auth::{AuthSession, Credentials},
    helpers::now,
};
use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use std::net::SocketAddr;
use zhorten_core::api::{ApiError, CreateRequest, DashboardData, LinkRecord};
use zhorten_service::{ClickContext, CreateLinkError, Database, FollowLinkError, RemoveLinkError};

#[derive(Clone)]
pub struct AppState<D> {
    pub database: D,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;
type HandlerResult = Result<Response, (StatusCode, Json<ApiError>)>;

/// Authenticate the configured admin user and return the first dashboard payload.
pub async fn login<D>(
    mut auth_session: AuthSession,
    State(state): State<AppState<D>>,
    Json(credentials): Json<Credentials>,
) -> HandlerResult
where
    D: Database,
{
    let Some(user) = auth_session
        .authenticate(credentials)
        .await
        .map_err(internal_error)?
    else {
        return unauthorized::<DashboardData>().map(IntoResponse::into_response);
    };
    auth_session.login(&user).await.map_err(internal_error)?;
    let data = zhorten_service::list_links(&state.database)
        .await
        .map_err(internal_error)?;
    Ok(Json(data).into_response())
}

/// End the current browser session.
pub async fn logout(mut auth_session: AuthSession) -> HandlerResult {
    auth_session.logout().await.map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true})).into_response())
}

/// Return the current dashboard state for an authenticated administrator.
pub async fn list_links<D>(State(state): State<AppState<D>>) -> ApiResult<DashboardData>
where
    D: Database,
{
    zhorten_service::list_links(&state.database)
        .await
        .map(Json)
        .map_err(internal_error)
}

/// Create a short link after validating both the route code and destination URL.
pub async fn create_link<D>(
    State(state): State<AppState<D>>,
    Json(body): Json<CreateRequest>,
) -> ApiResult<LinkRecord>
where
    D: Database,
{
    zhorten_service::create_link(&state.database, body.code, body.url, now())
        .await
        .map(Json)
        .map_err(create_link_error)
}

/// Remove an existing link. Missing links are treated as a successful no-op.
pub async fn remove_link<D>(
    State(state): State<AppState<D>>,
    Path(code): Path<String>,
) -> ApiResult<serde_json::Value>
where
    D: Database,
{
    zhorten_service::remove_link(&state.database, code)
        .await
        .map(|_| Json(serde_json::json!({"ok": true})))
        .map_err(remove_link_error)
}

/// Resolve a public short code and redirect to its stored destination.
pub async fn follow_link<D>(
    State(state): State<AppState<D>>,
    Path(code): Path<String>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response
where
    D: Database,
{
    let context = click_context(&headers, remote_addr);
    match zhorten_service::follow_link(&state.database, code, context).await {
        Ok(url) => Redirect::temporary(&url).into_response(),
        Err(FollowLinkError::InvalidCode | FollowLinkError::NotFound) => not_found(),
        Err(FollowLinkError::Database(error)) => {
            tracing::error!(%error, "request failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn click_context(headers: &HeaderMap, remote_addr: SocketAddr) -> ClickContext {
    ClickContext {
        clicked_at: now(),
        client_ip: forwarded_for(headers).or_else(|| Some(remote_addr.ip().to_string())),
        user_agent: headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
    }
}

fn forwarded_for(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn unauthorized<T>() -> ApiResult<T> {
    Err(error(
        StatusCode::UNAUTHORIZED,
        "Invalid username or password.",
    ))
}

fn error(status: StatusCode, message: &str) -> (StatusCode, Json<ApiError>) {
    (
        status,
        Json(ApiError {
            error: message.into(),
        }),
    )
}

fn create_link_error(
    failure: CreateLinkError<impl std::fmt::Display>,
) -> (StatusCode, Json<ApiError>) {
    match failure {
        CreateLinkError::InvalidCode => error(
            StatusCode::BAD_REQUEST,
            "Code must be 1-32 letters, numbers, dashes, or underscores.",
        ),
        CreateLinkError::InvalidUrl => error(
            StatusCode::BAD_REQUEST,
            "Enter a valid http:// or https:// URL.",
        ),
        CreateLinkError::UnsupportedUrlScheme => error(
            StatusCode::BAD_REQUEST,
            "Only http:// and https:// URLs are allowed.",
        ),
        CreateLinkError::Conflict => {
            error(StatusCode::CONFLICT, "That short code is already in use.")
        }
        CreateLinkError::Database(error) => internal_error(error),
    }
}

fn remove_link_error(
    failure: RemoveLinkError<impl std::fmt::Display>,
) -> (StatusCode, Json<ApiError>) {
    match failure {
        RemoveLinkError::InvalidCode => error(
            StatusCode::BAD_REQUEST,
            "Code must be 1-32 letters, numbers, dashes, or underscores.",
        ),
        RemoveLinkError::Database(error) => internal_error(error),
    }
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, Json<ApiError>) {
    tracing::error!(%error, "request failed");
    self::error(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.")
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Short link not found").into_response()
}
