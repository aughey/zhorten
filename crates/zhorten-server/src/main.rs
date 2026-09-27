use zhorten_core::cli::{Parser, StandaloneSledArgs, validate_mcp, validate_password};
use zhorten_server::{
    ServerOptions, SledServerOptions, healthy, serve, sled_db::Database, sled_router,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = StandaloneSledArgs::parse();
    let address = args.address();
    if args.server.healthcheck {
        std::process::exit(if healthy(address) { 0 } else { 1 });
    }
    if let Err(message) = validate_password(&args.server.password) {
        exit_with_config_error(message);
    }
    if let Err(message) = validate_mcp(&args.mcp) {
        exit_with_config_error(message);
    }

    let database_path = args.database();
    let site_root = args.site_root();
    let secure_cookies = args.secure_cookies();
    let database = Database::open(database_path, args.sled.cache_capacity)
        .expect("unable to open sled database");
    let app = sled_router(
        database,
        SledServerOptions {
            server: ServerOptions {
                site_root,
                username: args.server.username,
                password: args.server.password,
                secure_cookies,
            },
            mcp_token: args.mcp.mcp,
            mcp_hosts: args.mcp.mcp_host,
        },
    );

    serve(address, app).await.expect("server error");
}

fn exit_with_config_error(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2);
}
