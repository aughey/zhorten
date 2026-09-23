use crate::{
    auth::Auth,
    db::Database,
    helpers::{SESSION_MAX_AGE_SECONDS, now, session_cookie, valid_code},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use zhorten_core::{DashboardData, LinkRecord};

#[derive(Clone)]
pub struct AppState {
    pub database: Database,
    pub auth: Auth,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
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

pub async fn login(State(state): State<AppState>, Json(body): Json<LoginRequest>) -> HandlerResult {
    if !state
        .auth
        .credentials_are_valid(&body.username, &body.password)
    {
        return unauthorized::<DashboardData>().map(IntoResponse::into_response);
    }
    let token = state.auth.create_session().map_err(internal_error)?;
    let data = state.database.dashboard().map_err(internal_error)?;
    let mut response = Json(data).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie(
            &token,
            state.auth.secure_cookies(),
            SESSION_MAX_AGE_SECONDS,
        ))
        .expect("session cookie is a valid header value"),
    );
    Ok(response)
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    state.auth.remove_session(&headers);
    let mut response = Json(serde_json::json!({"ok": true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie("", state.auth.secure_cookies(), 0))
            .expect("expired session cookie is a valid header value"),
    );
    response
}

pub async fn list_links(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<DashboardData> {
    if !state.auth.authorized(&headers) {
        return unauthorized();
    }
    state.database.dashboard().map(Json).map_err(internal_error)
}

pub async fn create_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateRequest>,
) -> ApiResult<LinkRecord> {
    if !state.auth.authorized(&headers) {
        return unauthorized();
    }
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

pub async fn remove_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(code): Path<String>,
) -> ApiResult<serde_json::Value> {
    if !state.auth.authorized(&headers) {
        return unauthorized();
    }
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
