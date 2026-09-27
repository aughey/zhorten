use std::{fmt, sync::Arc};

use async_trait::async_trait;
use zhorten_core::{
    ValidCode,
    api::{DashboardData, LinkRecord},
    cli::{DatabaseArgs, DatabaseBackend},
};
use zhorten_service::ClickContext;

pub type SharedDatabase = Arc<dyn zhorten_service::Database<Error = Error>>;

#[derive(Debug)]
pub enum Error {
    Sled(zhorten_sled::Error),
    Sqlite(zhorten_sqlite::Error),
}

#[derive(Clone)]
enum LocalDatabase {
    Sled(zhorten_sled::Database),
    Sqlite(zhorten_sqlite::Database),
}

pub fn open(config: &DatabaseArgs) -> Result<SharedDatabase, Error> {
    let analytics_enabled = config.analytics.is_enabled();
    let database = match config.backend {
        DatabaseBackend::Sled => LocalDatabase::Sled(zhorten_sled::Database::open_with_analytics(
            &config.database,
            config.cache_capacity,
            analytics_enabled,
        )?),
        DatabaseBackend::Sqlite => LocalDatabase::Sqlite(
            zhorten_sqlite::Database::open_with_analytics(&config.database, analytics_enabled)?,
        ),
    };
    Ok(Arc::new(database))
}

#[async_trait]
impl zhorten_service::Database for LocalDatabase {
    type Error = Error;

    async fn dashboard(&self) -> Result<DashboardData, Self::Error> {
        match self {
            Self::Sled(database) => database.dashboard().await.map_err(Error::Sled),
            Self::Sqlite(database) => database.dashboard().await.map_err(Error::Sqlite),
        }
    }

    async fn create_link(
        &self,
        code: ValidCode,
        url: url::Url,
        created_at: i64,
    ) -> Result<Option<LinkRecord>, Self::Error> {
        match self {
            Self::Sled(database) => database
                .create_link(code, url, created_at)
                .await
                .map_err(Error::Sled),
            Self::Sqlite(database) => database
                .create_link(code, url, created_at)
                .await
                .map_err(Error::Sqlite),
        }
    }

    async fn remove_link(&self, code: &ValidCode) -> Result<(), Self::Error> {
        match self {
            Self::Sled(database) => database.remove_link(code).await.map_err(Error::Sled),
            Self::Sqlite(database) => database.remove_link(code).await.map_err(Error::Sqlite),
        }
    }

    async fn follow_link(
        &self,
        code: &ValidCode,
        context: ClickContext,
    ) -> Result<Option<String>, Self::Error> {
        match self {
            Self::Sled(database) => database
                .follow_link(code, context)
                .await
                .map_err(Error::Sled),
            Self::Sqlite(database) => database
                .follow_link(code, context)
                .await
                .map_err(Error::Sqlite),
        }
    }
}

impl From<zhorten_sled::Error> for Error {
    fn from(error: zhorten_sled::Error) -> Self {
        Self::Sled(error)
    }
}

impl From<zhorten_sqlite::Error> for Error {
    fn from(error: zhorten_sqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sled(error) => write!(formatter, "{error}"),
            Self::Sqlite(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for Error {}
