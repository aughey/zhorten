use std::{net::SocketAddr, path::PathBuf};

pub use clap::Parser;
use clap::{ArgAction, Args, Subcommand, ValueEnum, ValueHint};

#[derive(Args)]
pub struct ServerArgs {
    #[arg(long, env = "ZHORTEN_USERNAME")]
    pub username: String,
    #[arg(long, env = "ZHORTEN_PASSWORD", hide_env_values = true)]
    pub password: String,
    #[arg(long, env = "ZHORTEN_ADDR")]
    pub address: SocketAddr,
    #[arg(long, env = "ZHORTEN_SITE_ROOT", value_hint = ValueHint::DirPath)]
    pub site_root: Option<PathBuf>,
    /// Add the Secure attribute to session cookies.
    ///
    /// Enable this when zhorten is served through HTTPS. Set it explicitly to
    /// false for plain HTTP local development, where browsers reject Secure cookies.
    #[arg(long, env = "ZHORTEN_SECURE_COOKIES", action = ArgAction::Set)]
    pub secure_cookies: Option<bool>,
    #[arg(long, hide = true)]
    pub healthcheck: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum DatabaseBackend {
    Sled,
    Sqlite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AnalyticsMode {
    Disabled,
    Enabled,
}

impl AnalyticsMode {
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Args)]
pub struct DatabaseArgs {
    #[arg(
        long = "database-backend",
        env = "ZHORTEN_DATABASE_BACKEND",
        value_enum,
        default_value_t = DatabaseBackend::Sled
    )]
    pub backend: DatabaseBackend,
    #[arg(long, env = "ZHORTEN_DB", value_hint = ValueHint::AnyPath)]
    pub database: String,
    #[arg(
        long = "analytics",
        env = "ZHORTEN_ANALYTICS",
        value_enum,
        default_value_t = AnalyticsMode::Disabled
    )]
    pub analytics: AnalyticsMode,
    #[arg(long, env = "ZHORTEN_CACHE_CAPACITY", default_value_t = 64 * 1024 * 1024)]
    pub cache_capacity: u64,
}

#[derive(Args)]
pub struct McpArgs {
    /// Enable the MCP endpoint at /mcp using this bearer token.
    #[arg(long, env = "ZHORTEN_MCP", hide_env_values = true)]
    pub mcp: Option<String>,
    /// Allow this hostname to access the MCP endpoint. May be repeated.
    #[arg(
        long,
        env = "ZHORTEN_MCP_HOST",
        value_delimiter = ',',
        requires = "mcp"
    )]
    pub mcp_host: Vec<String>,
}

#[derive(Parser)]
#[command(version, about = "A tiny self-hosted URL shortener")]
pub struct StandaloneArgs {
    #[command(flatten)]
    pub server: ServerArgs,
    #[command(flatten)]
    pub database: DatabaseArgs,
    #[command(flatten)]
    pub mcp: McpArgs,
}

#[derive(Parser)]
#[command(
    version,
    about = "Run one zhorten service operation without starting a web server"
)]
pub struct CliArgs {
    #[command(flatten)]
    pub database: DatabaseArgs,
    #[command(subcommand)]
    pub action: CliAction,
}

#[derive(Subcommand)]
pub enum CliAction {
    /// Print dashboard data as JSON.
    #[command(alias = "dashboard")]
    List,
    /// Create one short link.
    Create {
        /// Route-safe short code.
        code: String,
        /// Destination URL.
        url: String,
    },
    /// Resolve a short code, increment its click count, and print the destination URL.
    Follow {
        /// Route-safe short code.
        code: String,
    },
    /// Delete one short link. Missing links are treated as a successful no-op.
    #[command(alias = "remove")]
    Delete {
        /// Route-safe short code.
        code: String,
    },
}

#[derive(Parser)]
#[command(version, about = "Google Cloud Run deployment wrapper for zhorten")]
pub struct GoogleCloudRunArgs {
    #[command(flatten)]
    pub server: ServerArgs,
    #[arg(long, env = "ZHORTEN_GOOGLE_PROJECT")]
    pub google_project: String,
    #[arg(long, env = "ZHORTEN_FIRESTORE_COLLECTION")]
    pub firestore_collection: String,
}

impl StandaloneArgs {
    pub fn site_root(&self) -> PathBuf {
        self.server
            .site_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("./target/site"))
    }

    pub fn secure_cookies(&self) -> bool {
        self.server.secure_cookies.unwrap_or(false)
    }
}

impl GoogleCloudRunArgs {
    pub fn site_root(&self) -> PathBuf {
        self.server
            .site_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("/app/site"))
    }

    pub fn secure_cookies(&self) -> bool {
        self.server.secure_cookies.unwrap_or(true)
    }
}

pub fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.trim().is_empty() {
        Err("--password (or ZHORTEN_PASSWORD) must not be empty")
    } else {
        Ok(())
    }
}

pub fn validate_mcp(mcp: &McpArgs) -> Result<(), &'static str> {
    if mcp
        .mcp
        .as_deref()
        .is_some_and(|token| token.trim().is_empty())
    {
        Err("--mcp (or ZHORTEN_MCP) must not be empty when provided")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_args_carry_explicit_listener_address() {
        let args = GoogleCloudRunArgs {
            server: ServerArgs {
                username: "admin".into(),
                password: "password".into(),
                address: SocketAddr::from(([127, 0, 0, 1], 8080)),
                site_root: None,
                secure_cookies: None,
                healthcheck: false,
            },
            google_project: "example-project".into(),
            firestore_collection: "links".into(),
        };

        assert_eq!(
            args.server.address,
            SocketAddr::from(([127, 0, 0, 1], 8080))
        );
        assert_eq!(args.site_root(), PathBuf::from("/app/site"));
        assert!(args.secure_cookies());
    }

    #[test]
    fn validates_non_empty_passwords_and_mcp_tokens() {
        assert!(validate_password("secret").is_ok());
        assert!(validate_password(" ").is_err());

        assert!(
            validate_mcp(&McpArgs {
                mcp: None,
                mcp_host: Vec::new(),
            })
            .is_ok()
        );
        assert!(
            validate_mcp(&McpArgs {
                mcp: Some(" ".into()),
                mcp_host: Vec::new(),
            })
            .is_err()
        );
    }
}
