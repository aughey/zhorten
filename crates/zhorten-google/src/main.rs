mod firestore_db;

use firestore_db::FirestoreDatabase;
use zhorten_core::cli::{GoogleCloudRunArgs, Parser, validate_password};
use zhorten_server::{ServerOptions, healthy, router, serve};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = GoogleCloudRunArgs::parse();
    let address = args.address();
    if args.server.healthcheck {
        std::process::exit(if healthy(address) { 0 } else { 1 });
    }
    if let Err(message) = validate_password(&args.server.password) {
        exit_with_config_error(message);
    }

    let project_id = args.project_id().unwrap_or_else(|| {
        exit_with_config_error(
            "set ZHORTEN_GOOGLE_PROJECT, GOOGLE_CLOUD_PROJECT, or GCP_PROJECT for Firestore",
        )
    });
    let site_root = args.site_root();
    let secure_cookies = args.secure_cookies();
    let database = FirestoreDatabase::new(project_id, args.firestore_collection)
        .await
        .expect("unable to connect to Firestore");
    database.ping().await.expect("Firestore ping failed");

    let app = router(
        database,
        ServerOptions {
            site_root,
            username: args.server.username,
            password: args.server.password,
            secure_cookies,
        },
    );
    serve(address, app).await.expect("server error");
}

fn exit_with_config_error(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2);
}
