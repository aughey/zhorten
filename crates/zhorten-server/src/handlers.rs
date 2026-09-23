use crate::{
    auth::{AuthSession, Credentials},
    db::Database,
    helpers::now,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use zhorten_core::{DashboardData, LinkRecord};
use zhorten_service::{CreateLinkError, FollowLinkError, RemoveLinkError};

#[derive(Clone)]
pub struct AppState {
    pub database: Database,
}

#[derive(Deserialize)]
pub struct CreateRequest {
    code: String,
    url: String,
}

#[derive(Serialize)]
pub struct ErrorBody {
    error: String,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorBody>)>;
type HandlerResult = Result<Response, (StatusCode, Json<ErrorBody>)>;

/// Authenticate the configured admin user and return the first dashboard payload.
pub async fn login(
    mut auth_session: AuthSession,
    State(state): State<AppState>,
    Json(credentials): Json<Credentials>,
) -> HandlerResult {
    let Some(user) = auth_session
        .authenticate(credentials)
        .await
        .map_err(internal_error)?
    else {
        return unauthorized::<DashboardData>().map(IntoResponse::into_response);
    };
    auth_session.login(&user).await.map_err(internal_error)?;
    let data = zhorten_service::list_links(&state.database).map_err(internal_error)?;
    Ok(Json(data).into_response())
}

/// End the current browser session.
pub async fn logout(mut auth_session: AuthSession) -> HandlerResult {
    auth_session.logout().await.map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true})).into_response())
}

/// Return the current dashboard state for an authenticated administrator.
pub async fn list_links(State(state): State<AppState>) -> ApiResult<DashboardData> {
    zhorten_service::list_links(&state.database)
        .map(Json)
        .map_err(internal_error)
}

/// Create a short link after validating both the route code and destination URL.
pub async fn create_link(
    State(state): State<AppState>,
    Json(body): Json<CreateRequest>,
) -> ApiResult<LinkRecord> {
    zhorten_service::create_link(&state.database, body.code, body.url, now())
        .await
        .map(Json)
        .map_err(create_link_error)
}

/// Remove an existing link. Missing links are treated as a successful no-op.
pub async fn remove_link(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<serde_json::Value> {
    zhorten_service::remove_link(&state.database, code)
        .await
        .map(|_| Json(serde_json::json!({"ok": true})))
        .map_err(remove_link_error)
}

/// Resolve a public short code and redirect to its stored destination.
pub async fn follow_link(State(state): State<AppState>, Path(code): Path<String>) -> Response {
    match zhorten_service::follow_link(&state.database, code, now()) {
        Ok(url) => Redirect::temporary(&url).into_response(),
        Err(FollowLinkError::InvalidCode | FollowLinkError::NotFound) => not_found(),
        Err(FollowLinkError::Database(error)) => {
            tracing::error!(%error, "request failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn unauthorized<T>() -> ApiResult<T> {
    Err(error(
        StatusCode::UNAUTHORIZED,
        "Invalid username or password.",
    ))
}

fn error(status: StatusCode, message: &str) -> (StatusCode, Json<ErrorBody>) {
    (
        status,
        Json(ErrorBody {
            error: message.into(),
        }),
    )
}

fn create_link_error(
    failure: CreateLinkError<impl std::fmt::Display>,
) -> (StatusCode, Json<ErrorBody>) {
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
) -> (StatusCode, Json<ErrorBody>) {
    match failure {
        RemoveLinkError::InvalidCode => error(
            StatusCode::BAD_REQUEST,
            "Code must be 1-32 letters, numbers, dashes, or underscores.",
        ),
        RemoveLinkError::Database(error) => internal_error(error),
    }
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, Json<ErrorBody>) {
    tracing::error!(%error, "request failed");
    self::error(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.")
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Short link not found").into_response()
}
