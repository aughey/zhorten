use clap::Parser;
use std::{net::SocketAddr, path::PathBuf};
use zhorten_server::{
    ServerOptions, SledServerOptions, healthy, serve, sled_db::Database, sled_router,
};

#[derive(Parser)]
#[command(version, about = "Fly.io deployment wrapper for zhorten")]
struct Args {
    #[arg(long, env = "ZHORTEN_USERNAME", default_value = "admin")]
    username: String,
    #[arg(long, env = "ZHORTEN_PASSWORD", hide_env_values = true)]
    password: String,
    #[arg(long, env = "ZHORTEN_DB", default_value = "/data/zhorten.db")]
    database: String,
    #[arg(long, env = "ZHORTEN_CACHE_CAPACITY", default_value_t = 64 * 1024 * 1024)]
    cache_capacity: u64,
    #[arg(long, env = "ZHORTEN_ADDR", default_value = "0.0.0.0:3000")]
    address: SocketAddr,
    #[arg(long, env = "ZHORTEN_SITE_ROOT", default_value = "/app/site")]
    site_root: PathBuf,
    #[arg(long, env = "ZHORTEN_MCP", hide_env_values = true)]
    mcp: Option<String>,
    #[arg(
        long,
        env = "ZHORTEN_MCP_HOST",
        value_delimiter = ',',
        requires = "mcp"
    )]
    mcp_host: Vec<String>,
    #[arg(long, env = "ZHORTEN_SECURE_COOKIES", default_value_t = true)]
    secure_cookies: bool,
    #[arg(long, hide = true)]
    healthcheck: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    if args.healthcheck {
        std::process::exit(if healthy(args.address) { 0 } else { 1 });
    }
    if args.password.trim().is_empty() {
        eprintln!("error: --password (or ZHORTEN_PASSWORD) must not be empty");
        std::process::exit(2);
    }

    let database =
        Database::open(&args.database, args.cache_capacity).expect("unable to open sled database");
    let app = sled_router(
        database,
        SledServerOptions {
            server: ServerOptions {
                site_root: args.site_root,
                username: args.username,
                password: args.password,
                secure_cookies: args.secure_cookies,
            },
            mcp_token: args.mcp,
            mcp_hosts: args.mcp_host,
        },
    );

    serve(args.address, app).await.expect("server error");
}
