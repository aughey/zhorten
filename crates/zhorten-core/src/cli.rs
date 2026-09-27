use std::{
    env,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

pub use clap::Parser;
use clap::{Args, ValueHint};

pub const DEFAULT_FIRESTORE_COLLECTION: &str = "links";

#[derive(Args)]
pub struct ServerArgs {
    #[arg(long, env = "ZHORTEN_USERNAME", default_value = "admin")]
    pub username: String,
    #[arg(long, env = "ZHORTEN_PASSWORD", hide_env_values = true)]
    pub password: String,
    #[arg(long, env = "ZHORTEN_ADDR")]
    pub address: Option<SocketAddr>,
    #[arg(long, env = "ZHORTEN_SITE_ROOT", value_hint = ValueHint::DirPath)]
    pub site_root: Option<PathBuf>,
    /// Add the Secure attribute to session cookies.
    ///
    /// Enable this when zhorten is served through HTTPS. Leave it disabled for
    /// plain HTTP local development, where browsers will reject Secure cookies.
    #[arg(
        long,
        env = "ZHORTEN_SECURE_COOKIES",
        num_args = 0..=1,
        default_missing_value = "true"
    )]
    pub secure_cookies: Option<bool>,
    #[arg(long, hide = true)]
    pub healthcheck: bool,
}

#[derive(Args)]
pub struct SledArgs {
    #[arg(long, env = "ZHORTEN_DB", value_hint = ValueHint::AnyPath)]
    pub database: Option<String>,
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
pub struct StandaloneSledArgs {
    #[command(flatten)]
    pub server: ServerArgs,
    #[command(flatten)]
    pub sled: SledArgs,
    #[command(flatten)]
    pub mcp: McpArgs,
}

#[derive(Parser)]
#[command(version, about = "Fly.io deployment wrapper for zhorten")]
pub struct FlySledArgs {
    #[command(flatten)]
    pub server: ServerArgs,
    #[command(flatten)]
    pub sled: SledArgs,
    #[command(flatten)]
    pub mcp: McpArgs,
}

#[derive(Parser)]
#[command(version, about = "Google Cloud Run deployment wrapper for zhorten")]
pub struct GoogleCloudRunArgs {
    #[command(flatten)]
    pub server: ServerArgs,
    #[arg(long, env = "ZHORTEN_GOOGLE_PROJECT")]
    pub google_project: Option<String>,
    #[arg(
        long,
        env = "ZHORTEN_FIRESTORE_COLLECTION",
        default_value = DEFAULT_FIRESTORE_COLLECTION
    )]
    pub firestore_collection: String,
    #[arg(long, env = "PORT")]
    pub port: Option<u16>,
}

impl StandaloneSledArgs {
    pub fn address(&self) -> SocketAddr {
        self.server
            .address
            .unwrap_or_else(|| "127.0.0.1:3000".parse().expect("default address"))
    }

    pub fn site_root(&self) -> PathBuf {
        self.server
            .site_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("./target/site"))
    }

    pub fn secure_cookies(&self) -> bool {
        self.server.secure_cookies.unwrap_or(false)
    }

    pub fn database(&self) -> String {
        self.sled
            .database
            .clone()
            .unwrap_or_else(|| "./data/zhorten.db".into())
    }
}

impl FlySledArgs {
    pub fn address(&self) -> SocketAddr {
        self.server
            .address
            .unwrap_or_else(|| "0.0.0.0:3000".parse().expect("default address"))
    }

    pub fn site_root(&self) -> PathBuf {
        self.server
            .site_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("/app/site"))
    }

    pub fn secure_cookies(&self) -> bool {
        self.server.secure_cookies.unwrap_or(true)
    }

    pub fn database(&self) -> String {
        self.sled
            .database
            .clone()
            .unwrap_or_else(|| "/data/zhorten.db".into())
    }
}

impl GoogleCloudRunArgs {
    pub fn address(&self) -> SocketAddr {
        self.server
            .address
            .unwrap_or_else(|| SocketAddr::from((Ipv4Addr::UNSPECIFIED, self.port.unwrap_or(3000))))
    }

    pub fn site_root(&self) -> PathBuf {
        self.server
            .site_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("/app/site"))
    }

    pub fn secure_cookies(&self) -> bool {
        self.server.secure_cookies.unwrap_or(true)
    }

    pub fn project_id(&self) -> Option<String> {
        self.google_project
            .clone()
            .or_else(default_google_project_id)
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

fn default_google_project_id() -> Option<String> {
    env::var("GOOGLE_CLOUD_PROJECT")
        .or_else(|_| env::var("GCP_PROJECT"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_run_port_sets_listener_address() {
        let args = GoogleCloudRunArgs {
            server: ServerArgs {
                username: "admin".into(),
                password: "password".into(),
                address: None,
                site_root: None,
                secure_cookies: None,
                healthcheck: false,
            },
            google_project: None,
            firestore_collection: DEFAULT_FIRESTORE_COLLECTION.into(),
            port: Some(8080),
        };

        assert_eq!(args.address(), SocketAddr::from(([0, 0, 0, 0], 8080)));
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
