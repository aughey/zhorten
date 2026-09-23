use crate::ValidCode;
use serde::{Deserialize, Serialize};

/// A persisted short-link record.
///
/// The code is validated when constructed or deserialized, so persisted and API
/// records cannot represent an invalid route key.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LinkRecord {
    pub code: ValidCode,
    pub url: String,
    pub clicks: u64,
    pub created_at: i64,
    pub last_clicked_at: Option<i64>,
}

/// Data returned by the authenticated dashboard endpoint.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DashboardData {
    pub links: Vec<LinkRecord>,
    pub total_clicks: u64,
}

/// Request body for creating a short link.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateRequest {
    pub code: String,
    pub url: String,
}

/// Request body for administrator login.
#[derive(Clone, Deserialize, Serialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Error payload returned by JSON API endpoints.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiError {
    pub error: String,
}
