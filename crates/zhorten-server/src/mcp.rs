use crate::{db::Database, helpers::now};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde::{Deserialize, Serialize};
use zhorten_core::api::LinkRecord;
use zhorten_service::{CreateLinkError, RemoveLinkError};

#[derive(Clone)]
pub struct BearerToken(pub String);

#[derive(Clone)]
pub struct ZHortenMcp {
    database: Database,
    tool_router: ToolRouter<Self>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct McpLinkRecord {
    pub code: String,
    pub url: String,
    pub clicks: u64,
    pub created_at: i64,
    pub last_clicked_at: Option<i64>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListLinksOutput {
    pub links: Vec<McpLinkRecord>,
    pub total_clicks: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SuggestCodesRequest {
    pub text: String,
    #[serde(default = "default_suggestion_count")]
    pub count: usize,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SuggestCodesOutput {
    pub codes: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateLinkRequest {
    pub code: String,
    pub url: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CreateLinkOutput {
    pub ok: bool,
    pub record: Option<McpLinkRecord>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BulkCreateLinksRequest {
    pub links: Vec<CreateLinkRequest>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct BulkCreateLinksOutput {
    pub created: usize,
    pub failed: usize,
    pub results: Vec<CreateLinkOutput>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteLinkRequest {
    pub code: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DeleteLinkOutput {
    pub ok: bool,
    pub code: String,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BulkDeleteLinksRequest {
    pub codes: Vec<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct BulkDeleteLinksOutput {
    pub deleted: usize,
    pub failed: usize,
    pub results: Vec<DeleteLinkOutput>,
}

impl ZHortenMcp {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl ZHortenMcp {
    #[tool(description = "List all short links and aggregate click counts.")]
    fn list_links(&self) -> Result<Json<ListLinksOutput>, ErrorData> {
        let data = zhorten_service::list_links(&self.database).map_err(database_error)?;
        Ok(Json(ListLinksOutput {
            links: data.links.into_iter().map(Into::into).collect(),
            total_clicks: data.total_clicks,
        }))
    }

    #[tool(
        description = "Generate route-safe short code suggestions from text or a URL without creating links."
    )]
    fn suggest_codes(
        &self,
        Parameters(request): Parameters<SuggestCodesRequest>,
    ) -> Json<SuggestCodesOutput> {
        Json(SuggestCodesOutput {
            codes: suggest_codes(&request.text, request.count),
        })
    }

    #[tool(description = "Create one short link with an explicit code and destination URL.")]
    async fn create_link(
        &self,
        Parameters(request): Parameters<CreateLinkRequest>,
    ) -> Result<Json<CreateLinkOutput>, ErrorData> {
        Ok(Json(create_link(&self.database, request).await?))
    }

    #[tool(
        description = "Create many short links. Each item reports whether it was created or why it failed."
    )]
    async fn bulk_create_links(
        &self,
        Parameters(request): Parameters<BulkCreateLinksRequest>,
    ) -> Result<Json<BulkCreateLinksOutput>, ErrorData> {
        let mut results = Vec::with_capacity(request.links.len());
        for link in request.links {
            results.push(create_link(&self.database, link).await?);
        }
        let created = results.iter().filter(|result| result.ok).count();
        Ok(Json(BulkCreateLinksOutput {
            created,
            failed: results.len().saturating_sub(created),
            results,
        }))
    }

    #[tool(
        description = "Delete one short link by code. Missing links are treated as successful no-ops."
    )]
    async fn delete_link(
        &self,
        Parameters(request): Parameters<DeleteLinkRequest>,
    ) -> Result<Json<DeleteLinkOutput>, ErrorData> {
        Ok(Json(delete_link(&self.database, request.code).await?))
    }

    #[tool(
        description = "Delete many short links by code. Missing links are treated as successful no-ops."
    )]
    async fn bulk_delete_links(
        &self,
        Parameters(request): Parameters<BulkDeleteLinksRequest>,
    ) -> Result<Json<BulkDeleteLinksOutput>, ErrorData> {
        let mut results = Vec::with_capacity(request.codes.len());
        for code in request.codes {
            results.push(delete_link(&self.database, code).await?);
        }
        let deleted = results.iter().filter(|result| result.ok).count();
        Ok(Json(BulkDeleteLinksOutput {
            deleted,
            failed: results.len().saturating_sub(deleted),
            results,
        }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ZHortenMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Manage zhorten short links through authenticated MCP tools.")
    }
}

pub fn service(
    database: Database,
    allowed_hosts: Vec<String>,
) -> StreamableHttpService<ZHortenMcp, LocalSessionManager> {
    StreamableHttpService::new(
        move || Ok(ZHortenMcp::new(database.clone())),
        Default::default(),
        server_config(allowed_hosts),
    )
}

fn server_config(allowed_hosts: Vec<String>) -> StreamableHttpServerConfig {
    let mut config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None);
    config.allowed_hosts.extend(allowed_hosts);
    config
}

pub async fn bearer_auth(
    State(token): State<BearerToken>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = format!("Bearer {}", token.0);
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == expected);

    if authorized {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn create_link(
    database: &Database,
    request: CreateLinkRequest,
) -> Result<CreateLinkOutput, ErrorData> {
    match zhorten_service::create_link(database, request.code, request.url, now()).await {
        Ok(record) => Ok(CreateLinkOutput {
            ok: true,
            record: Some(record.into()),
            error: None,
        }),
        Err(CreateLinkError::InvalidCode) => Ok(create_error(
            "Code must be 1-32 letters, numbers, dashes, or underscores.",
        )),
        Err(CreateLinkError::InvalidUrl) => {
            Ok(create_error("Enter a valid http:// or https:// URL."))
        }
        Err(CreateLinkError::UnsupportedUrlScheme) => {
            Ok(create_error("Only http:// and https:// URLs are allowed."))
        }
        Err(CreateLinkError::Conflict) => Ok(create_error("That short code is already in use.")),
        Err(CreateLinkError::Database(error)) => Err(database_error(error)),
    }
}

async fn delete_link(database: &Database, code: String) -> Result<DeleteLinkOutput, ErrorData> {
    match zhorten_service::remove_link(database, code.clone()).await {
        Ok(()) => Ok(DeleteLinkOutput {
            ok: true,
            code,
            error: None,
        }),
        Err(RemoveLinkError::InvalidCode) => Ok(DeleteLinkOutput {
            ok: false,
            code,
            error: Some("Code must be 1-32 letters, numbers, dashes, or underscores.".into()),
        }),
        Err(RemoveLinkError::Database(error)) => Err(database_error(error)),
    }
}

fn create_error(message: impl Into<String>) -> CreateLinkOutput {
    CreateLinkOutput {
        ok: false,
        record: None,
        error: Some(message.into()),
    }
}

fn database_error(error: impl std::fmt::Display) -> ErrorData {
    ErrorData::internal_error(format!("database error: {error}"), None)
}

fn default_suggestion_count() -> usize {
    5
}

fn suggest_codes(text: &str, count: usize) -> Vec<String> {
    let base = slugify(text);
    let base = if base.is_empty() {
        "link".to_owned()
    } else {
        base
    };
    let base = trim_code(&base);
    let count = count.clamp(1, 20);
    let mut suggestions = Vec::with_capacity(count);
    suggestions.push(base.clone());
    for index in 2..=count {
        let suffix = format!("-{index}");
        let prefix_len = 32usize.saturating_sub(suffix.len());
        let mut candidate = base.chars().take(prefix_len).collect::<String>();
        candidate.push_str(&suffix);
        suggestions.push(candidate);
    }
    suggestions
}

fn slugify(text: &str) -> String {
    let mut code = String::new();
    let mut previous_separator = false;

    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() {
            code.push(byte.to_ascii_lowercase() as char);
            previous_separator = false;
        } else if matches!(byte, b'-' | b'_' | b' ' | b'.' | b'/' | b':')
            && !code.is_empty()
            && !previous_separator
        {
            code.push('-');
            previous_separator = true;
        }
    }

    while code.ends_with('-') {
        code.pop();
    }
    code
}

fn trim_code(code: &str) -> String {
    code.chars().take(32).collect()
}

impl From<LinkRecord> for McpLinkRecord {
    fn from(record: LinkRecord) -> Self {
        Self {
            code: record.code.to_string(),
            url: record.url,
            clicks: record.clicks,
            created_at: record.created_at,
            last_clicked_at: record.last_clicked_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::server_config;

    #[test]
    fn public_mcp_hosts_extend_loopback_defaults() {
        let config = server_config(vec!["z.washucsc.org".into()]);

        assert!(config.allowed_hosts.contains(&"localhost".into()));
        assert!(config.allowed_hosts.contains(&"127.0.0.1".into()));
        assert!(config.allowed_hosts.contains(&"::1".into()));
        assert!(config.allowed_hosts.contains(&"z.washucsc.org".into()));
    }
}
