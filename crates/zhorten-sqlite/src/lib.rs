use std::{
    fmt,
    path::Path,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use url::Url;
use zhorten_core::{
    ValidCode,
    api::{DashboardData, LinkRecord},
};
use zhorten_service::ClickContext;

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
    analytics_enabled: bool,
}

#[derive(Debug)]
pub enum Error {
    Storage(rusqlite::Error),
    LockPoisoned,
    InvalidCode(String),
    InvalidClickCount(i64),
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::open_with_analytics(path, false)
    }

    pub fn open_with_analytics(
        path: impl AsRef<Path>,
        analytics_enabled: bool,
    ) -> Result<Self, Error> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection, analytics_enabled)
    }

    #[cfg(test)]
    fn temporary(analytics_enabled: bool) -> Result<Self, Error> {
        let connection = Connection::open_in_memory()?;
        Self::from_connection(connection, analytics_enabled)
    }

    fn from_connection(connection: Connection, analytics_enabled: bool) -> Result<Self, Error> {
        connection.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS links (
                code TEXT PRIMARY KEY NOT NULL,
                url TEXT NOT NULL,
                clicks INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                last_clicked_at INTEGER
            );
            CREATE TABLE IF NOT EXISTS click_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                code TEXT NOT NULL,
                clicked_at INTEGER NOT NULL,
                client_ip TEXT,
                user_agent TEXT
            );
            ",
        )?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            analytics_enabled,
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, Error> {
        self.connection.lock().map_err(|_| Error::LockPoisoned)
    }
}

#[async_trait]
impl zhorten_service::Database for Database {
    type Error = Error;

    async fn dashboard(&self) -> Result<DashboardData, Self::Error> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT code, url, clicks, created_at, last_clicked_at FROM links ORDER BY created_at DESC",
        )?;
        let links = statement
            .query_map([], |row| {
                let code: String = row.get(0)?;
                let clicks: i64 = row.get(2)?;
                Ok((
                    code,
                    row.get::<_, String>(1)?,
                    clicks,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .map(|row| {
                let (code, url, clicks, created_at, last_clicked_at) = row?;
                let code =
                    ValidCode::try_from(code.clone()).map_err(|_| Error::InvalidCode(code))?;
                let clicks = u64::try_from(clicks).map_err(|_| Error::InvalidClickCount(clicks))?;
                Ok(LinkRecord {
                    code,
                    url,
                    clicks,
                    created_at,
                    last_clicked_at,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let total_clicks = links.iter().map(|record| record.clicks).sum();
        Ok(DashboardData {
            links,
            total_clicks,
        })
    }

    async fn create_link(
        &self,
        code: ValidCode,
        url: Url,
        created_at: i64,
    ) -> Result<Option<LinkRecord>, Self::Error> {
        let connection = self.lock()?;
        let record = LinkRecord {
            code,
            url: url.to_string(),
            clicks: 0,
            created_at,
            last_clicked_at: None,
        };
        let inserted = connection.execute(
            "INSERT OR IGNORE INTO links (code, url, clicks, created_at, last_clicked_at) VALUES (?1, ?2, 0, ?3, NULL)",
            params![record.code.as_str(), record.url, record.created_at],
        )?;
        Ok((inserted == 1).then_some(record))
    }

    async fn remove_link(&self, code: &ValidCode) -> Result<(), Self::Error> {
        let connection = self.lock()?;
        connection.execute("DELETE FROM links WHERE code = ?1", params![code.as_str()])?;
        Ok(())
    }

    async fn follow_link(
        &self,
        code: &ValidCode,
        context: ClickContext,
    ) -> Result<Option<String>, Self::Error> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let url = transaction
            .query_row(
                "SELECT url FROM links WHERE code = ?1",
                params![code.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(url) = url else {
            return Ok(None);
        };
        transaction.execute(
            "UPDATE links SET clicks = clicks + 1, last_clicked_at = ?2 WHERE code = ?1",
            params![code.as_str(), context.clicked_at],
        )?;
        if self.analytics_enabled {
            transaction.execute(
                "INSERT INTO click_events (code, clicked_at, client_ip, user_agent) VALUES (?1, ?2, ?3, ?4)",
                params![
                    code.as_str(),
                    context.clicked_at,
                    context.client_ip,
                    context.user_agent
                ],
            )?;
        }
        transaction.commit()?;
        Ok(Some(url))
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Storage(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "{error}"),
            Self::LockPoisoned => write!(formatter, "database lock poisoned"),
            Self::InvalidCode(code) => write!(formatter, "invalid code stored in database: {code}"),
            Self::InvalidClickCount(clicks) => {
                write!(
                    formatter,
                    "invalid click count stored in database: {clicks}"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;
    use zhorten_service::Database as _;

    #[tokio::test]
    async fn link_lifecycle_updates_dashboard_and_clicks() {
        let database = Database::temporary(false).unwrap();
        let code = ValidCode::try_from("docs").unwrap();
        let url = Url::parse("https://example.com/").unwrap();

        let record = database
            .create_link(code.clone(), url.clone(), 100)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.code, code);
        assert_eq!(record.url, url.as_str());
        assert!(
            database
                .create_link(code, url, 100)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            database
                .follow_link(&record.code, ClickContext::new(200))
                .await
                .unwrap(),
            Some(record.url.clone())
        );

        let dashboard = database.dashboard().await.unwrap();
        assert_eq!(dashboard.total_clicks, 1);
        assert_eq!(dashboard.links[0].last_clicked_at, Some(200));

        database.remove_link(&record.code).await.unwrap();
        assert!(
            database
                .follow_link(&record.code, ClickContext::new(300))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn analytics_events_are_optional() {
        let database = Database::temporary(false).unwrap();
        let code = ValidCode::try_from("docs").unwrap();
        database
            .create_link(
                code.clone(),
                Url::parse("https://example.com/").unwrap(),
                100,
            )
            .await
            .unwrap();
        database
            .follow_link(&code, ClickContext::new(200))
            .await
            .unwrap();
        assert_eq!(
            database
                .lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM click_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );

        let database = Database::temporary(true).unwrap();
        let code = ValidCode::try_from("docs").unwrap();
        database
            .create_link(
                code.clone(),
                Url::parse("https://example.com/").unwrap(),
                100,
            )
            .await
            .unwrap();
        database
            .follow_link(
                &code,
                ClickContext {
                    clicked_at: 200,
                    client_ip: Some("127.0.0.1".into()),
                    user_agent: Some("zhorten-test".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(
            database
                .lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM click_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
