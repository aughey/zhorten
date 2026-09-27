use clap::Parser;
use firestore::{
    FirestoreDb, FirestoreTransactionOps,
    errors::{BackoffError, FirestoreError},
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{env, fmt, net::SocketAddr, path::PathBuf};
use url::Url;
use zhorten_core::{
    ValidCode,
    api::{DashboardData, LinkRecord},
};
use zhorten_server::{ServerOptions, healthy, router, serve};

const DEFAULT_COLLECTION: &str = "links";

#[derive(Parser)]
#[command(version, about = "Google Cloud Run deployment wrapper for zhorten")]
struct Args {
    #[arg(long, env = "ZHORTEN_USERNAME", default_value = "admin")]
    username: String,
    #[arg(long, env = "ZHORTEN_PASSWORD", hide_env_values = true)]
    password: String,
    #[arg(long, env = "ZHORTEN_GOOGLE_PROJECT")]
    google_project: Option<String>,
    #[arg(long, env = "ZHORTEN_FIRESTORE_COLLECTION", default_value = DEFAULT_COLLECTION)]
    firestore_collection: String,
    #[arg(long, env = "ZHORTEN_ADDR")]
    address: Option<SocketAddr>,
    #[arg(long, env = "PORT")]
    port: Option<u16>,
    #[arg(long, env = "ZHORTEN_SITE_ROOT", default_value = "/app/site")]
    site_root: PathBuf,
    #[arg(long, env = "ZHORTEN_SECURE_COOKIES", default_value_t = true)]
    secure_cookies: bool,
    #[arg(long, hide = true)]
    healthcheck: bool,
}

#[derive(Clone)]
struct FirestoreDatabase {
    db: FirestoreDb,
    collection: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FirestoreLink {
    code: String,
    url: String,
    clicks: u64,
    created_at: i64,
    last_clicked_at: Option<i64>,
}

#[derive(Debug)]
enum Error {
    Firestore(FirestoreError),
    InvalidCode(String),
}

impl FirestoreDatabase {
    async fn new(project_id: String, collection: String) -> Result<Self, Error> {
        Ok(Self {
            db: FirestoreDb::new(project_id).await?,
            collection,
        })
    }
}

impl zhorten_service::Database for FirestoreDatabase {
    type Error = Error;

    async fn dashboard(&self) -> Result<DashboardData, Self::Error> {
        let mut links = Vec::new();
        let mut stream = self
            .db
            .fluent()
            .list()
            .from(&self.collection)
            .obj::<FirestoreLink>()
            .stream_all()
            .await?;
        while let Some(record) = stream.next().await {
            links.push(record.try_into()?);
        }
        Ok(Self::record_to_dashboard(links))
    }

    async fn create_link(
        &self,
        code: ValidCode,
        url: Url,
        created_at: i64,
    ) -> Result<Option<LinkRecord>, Self::Error> {
        let record = LinkRecord {
            code,
            url: url.to_string(),
            clicks: 0,
            created_at,
            last_clicked_at: None,
        };
        let firestore_record = FirestoreLink::from(record.clone());
        match self
            .db
            .fluent()
            .insert()
            .into(&self.collection)
            .document_id(record.code.as_str())
            .object(&firestore_record)
            .execute::<FirestoreLink>()
            .await
        {
            Ok(_) => Ok(Some(record)),
            Err(FirestoreError::DataConflictError(_)) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn remove_link(&self, code: &ValidCode) -> Result<(), Self::Error> {
        self.db
            .fluent()
            .delete()
            .from(&self.collection)
            .document_id(code.as_str())
            .execute()
            .await?;
        Ok(())
    }

    async fn follow_link(
        &self,
        code: &ValidCode,
        clicked_at: i64,
    ) -> Result<Option<String>, Self::Error> {
        let collection = self.collection.clone();
        let code = code.to_string();
        self.db
            .run_transaction(move |db, transaction| {
                let collection = collection.clone();
                let code = code.clone();
                Box::pin(async move {
                    let Some(mut record) = db
                        .fluent()
                        .select()
                        .by_id_in(&collection)
                        .obj::<FirestoreLink>()
                        .one(&code)
                        .await
                        .map_err(BackoffError::transient)?
                    else {
                        return Ok::<Option<String>, BackoffError<FirestoreError>>(None);
                    };
                    let url = record.url.clone();
                    record.clicks = record.clicks.saturating_add(1);
                    record.last_clicked_at = Some(clicked_at);
                    transaction
                        .update_object(
                            &collection,
                            &code,
                            &record,
                            Some(vec!["clicks".into(), "last_clicked_at".into()]),
                            None,
                            vec![],
                        )
                        .map_err(BackoffError::permanent)?;
                    Ok::<Option<String>, BackoffError<FirestoreError>>(Some(url))
                })
            })
            .await
            .map_err(Into::into)
    }
}

impl FirestoreDatabase {
    fn record_to_dashboard(mut links: Vec<LinkRecord>) -> DashboardData {
        links.sort_by_key(|record| std::cmp::Reverse(record.created_at));
        let total_clicks = links.iter().map(|record| record.clicks).sum();
        DashboardData {
            links,
            total_clicks,
        }
    }
}

impl From<LinkRecord> for FirestoreLink {
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

impl TryFrom<FirestoreLink> for LinkRecord {
    type Error = Error;

    fn try_from(record: FirestoreLink) -> Result<Self, Self::Error> {
        let code = ValidCode::try_from(record.code.clone())
            .map_err(|_| Error::InvalidCode(record.code.clone()))?;
        Ok(Self {
            code,
            url: record.url,
            clicks: record.clicks,
            created_at: record.created_at,
            last_clicked_at: record.last_clicked_at,
        })
    }
}

impl From<FirestoreError> for Error {
    fn from(error: FirestoreError) -> Self {
        Self::Firestore(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Firestore(error) => write!(formatter, "{error}"),
            Self::InvalidCode(code) => write!(formatter, "invalid stored code: {code}"),
        }
    }
}

impl std::error::Error for Error {}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let address = resolve_address(args.address, args.port);
    if args.healthcheck {
        std::process::exit(if healthy(address) { 0 } else { 1 });
    }
    if args.password.trim().is_empty() {
        eprintln!("error: --password (or ZHORTEN_PASSWORD) must not be empty");
        std::process::exit(2);
    }

    let project_id = args.google_project.or_else(default_project_id).unwrap_or_else(|| {
        eprintln!(
            "error: set ZHORTEN_GOOGLE_PROJECT, GOOGLE_CLOUD_PROJECT, or GCP_PROJECT for Firestore"
        );
        std::process::exit(2);
    });
    let database = FirestoreDatabase::new(project_id, args.firestore_collection)
        .await
        .expect("unable to connect to Firestore");
    database.db.ping().await.expect("Firestore ping failed");

    let app = router(
        database,
        ServerOptions {
            site_root: args.site_root,
            username: args.username,
            password: args.password,
            secure_cookies: args.secure_cookies,
        },
    );
    serve(address, app).await.expect("server error");
}

fn resolve_address(address: Option<SocketAddr>, port: Option<u16>) -> SocketAddr {
    address.unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], port.unwrap_or(3000))))
}

fn default_project_id() -> Option<String> {
    env::var("GOOGLE_CLOUD_PROJECT")
        .or_else(|_| env::var("GCP_PROJECT"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firestore_record_round_trips_valid_link_record() {
        let record = LinkRecord {
            code: ValidCode::try_from("docs").unwrap(),
            url: "https://example.com/".into(),
            clicks: 3,
            created_at: 100,
            last_clicked_at: Some(200),
        };

        let firestore = FirestoreLink::from(record.clone());
        assert_eq!(LinkRecord::try_from(firestore).unwrap(), record);
    }

    #[test]
    fn invalid_firestore_code_is_rejected() {
        let record = FirestoreLink {
            code: "bad/code".into(),
            url: "https://example.com/".into(),
            clicks: 0,
            created_at: 100,
            last_clicked_at: None,
        };

        assert!(matches!(
            LinkRecord::try_from(record),
            Err(Error::InvalidCode(code)) if code == "bad/code"
        ));
    }

    #[test]
    fn cloud_run_port_sets_listener_address() {
        assert_eq!(
            resolve_address(None, Some(8080)),
            SocketAddr::from(([0, 0, 0, 0], 8080))
        );
    }
}
