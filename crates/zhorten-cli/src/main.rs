use std::time::{SystemTime, UNIX_EPOCH};
use zhorten_core::cli::{CliAction, CliSledArgs, Parser};
use zhorten_sled::Database;

#[tokio::main]
async fn main() {
    let args = CliSledArgs::parse();
    let database = Database::open(args.database(), args.sled.cache_capacity)
        .expect("unable to open sled database");

    let result = match args.action {
        CliAction::List => list_links(&database).await,
        CliAction::Create { code, url } => create_link(&database, code, url).await,
        CliAction::Follow { code } => follow_link(&database, code).await,
        CliAction::Delete { code } => delete_link(&database, code).await,
    };

    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn list_links(database: &Database) -> Result<(), String> {
    let dashboard = zhorten_service::list_links(database)
        .await
        .map_err(|error| error.to_string())?;
    print_json(&dashboard)
}

async fn create_link(database: &Database, code: String, url: String) -> Result<(), String> {
    let record = zhorten_service::create_link(database, code, url, now())
        .await
        .map_err(create_error)?;
    print_json(&record)
}

async fn follow_link(database: &Database, code: String) -> Result<(), String> {
    let url = zhorten_service::follow_link(database, code, now())
        .await
        .map_err(follow_error)?;
    println!("{url}");
    Ok(())
}

async fn delete_link(database: &Database, code: String) -> Result<(), String> {
    zhorten_service::remove_link(database, code)
        .await
        .map_err(remove_error)?;
    print_json(&serde_json::json!({ "ok": true }))
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    println!("{json}");
    Ok(())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn create_error(error: zhorten_service::CreateLinkError<zhorten_sled::Error>) -> String {
    match error {
        zhorten_service::CreateLinkError::InvalidCode => {
            "code must be 1-32 letters, numbers, dashes, or underscores".into()
        }
        zhorten_service::CreateLinkError::InvalidUrl => {
            "enter a valid http:// or https:// URL".into()
        }
        zhorten_service::CreateLinkError::UnsupportedUrlScheme => {
            "only http:// and https:// URLs are allowed".into()
        }
        zhorten_service::CreateLinkError::Conflict => "that short code is already in use".into(),
        zhorten_service::CreateLinkError::Database(error) => error.to_string(),
    }
}

fn follow_error(error: zhorten_service::FollowLinkError<zhorten_sled::Error>) -> String {
    match error {
        zhorten_service::FollowLinkError::InvalidCode => {
            "code must be 1-32 letters, numbers, dashes, or underscores".into()
        }
        zhorten_service::FollowLinkError::NotFound => "short link not found".into(),
        zhorten_service::FollowLinkError::Database(error) => error.to_string(),
    }
}

fn remove_error(error: zhorten_service::RemoveLinkError<zhorten_sled::Error>) -> String {
    match error {
        zhorten_service::RemoveLinkError::InvalidCode => {
            "code must be 1-32 letters, numbers, dashes, or underscores".into()
        }
        zhorten_service::RemoveLinkError::Database(error) => error.to_string(),
    }
}
