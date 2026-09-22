use serde::{Deserialize, Serialize};

/// A persisted short-link record.
///
/// The server validates `code` and `url` before writing records. Client-side
/// code treats these as display data from the API and never as trusted HTML.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LinkRecord {
    pub code: String,
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
