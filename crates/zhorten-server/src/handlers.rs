use crate::{
    auth::{AuthSession, Credentials},
    db::Database,
    helpers::{now, valid_code},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use zhorten_core::{DashboardData, LinkRecord};

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
    let data = state.database.dashboard().map_err(internal_error)?;
    Ok(Json(data).into_response())
}

/// End the current browser session.
pub async fn logout(mut auth_session: AuthSession) -> HandlerResult {
    auth_session.logout().await.map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true})).into_response())
}

/// Return the current dashboard state for an authenticated administrator.
pub async fn list_links(State(state): State<AppState>) -> ApiResult<DashboardData> {
    state.database.dashboard().map(Json).map_err(internal_error)
}

/// Create a short link after validating both the route code and destination URL.
pub async fn create_link(
    State(state): State<AppState>,
    Json(body): Json<CreateRequest>,
) -> ApiResult<LinkRecord> {
    if !valid_code(&body.code) {
        return bad_request("Code must be 1-32 letters, numbers, dashes, or underscores.");
    }
    let parsed = url::Url::parse(&body.url).map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "Enter a valid http:// or https:// URL.",
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return bad_request("Only http:// and https:// URLs are allowed.");
    }
    let record = LinkRecord {
        code: body.code,
        url: parsed.to_string(),
        clicks: 0,
        created_at: now(),
        last_clicked_at: None,
    };
    if !state
        .database
        .create_link(&record)
        .await
        .map_err(internal_error)?
    {
        return Err(error(
            StatusCode::CONFLICT,
            "That short code is already in use.",
        ));
    }
    Ok(Json(record))
}

/// Remove an existing link. Missing links are treated as a successful no-op.
pub async fn remove_link(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<serde_json::Value> {
    if !valid_code(&code) {
        return bad_request("Code must be 1-32 letters, numbers, dashes, or underscores.");
    }
    state
        .database
        .remove_link(&code)
        .await
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// Resolve a public short code and redirect to its stored destination.
pub async fn follow_link(State(state): State<AppState>, Path(code): Path<String>) -> Response {
    if !valid_code(&code) {
        return not_found();
    }
    match state.database.follow_link(&code, now()) {
        Ok(Some(url)) => Redirect::temporary(&url).into_response(),
        Ok(None) => not_found(),
        Err(error) => {
            tracing::error!(%error, "database request failed");
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

fn bad_request<T>(message: &str) -> ApiResult<T> {
    Err(error(StatusCode::BAD_REQUEST, message))
}

fn error(status: StatusCode, message: &str) -> (StatusCode, Json<ErrorBody>) {
    (
        status,
        Json(ErrorBody {
            error: message.into(),
        }),
    )
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, Json<ErrorBody>) {
    tracing::error!(%error, "request failed");
    self::error(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error.")
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Short link not found").into_response()
}
