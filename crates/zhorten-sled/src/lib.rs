use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path};
use url::Url;
use zhorten_core::{
    ValidCode,
    api::{DashboardData, LinkRecord},
};
use zhorten_service::ClickContext;

#[derive(Clone)]
pub struct Database {
    db: sled::Db,
    links: sled::Tree,
    clicks: sled::Tree,
    analytics_enabled: bool,
}

#[derive(Debug)]
pub enum Error {
    Storage(sled::Error),
    InvalidRecord(serde_json::Error),
}

impl Database {
    /// Open the embedded database and its named trees.
    ///
    pub fn open(path: impl AsRef<Path>, cache_capacity: u64) -> Result<Self, Error> {
        Self::open_with_analytics(path, cache_capacity, false)
    }

    pub fn open_with_analytics(
        path: impl AsRef<Path>,
        cache_capacity: u64,
        analytics_enabled: bool,
    ) -> Result<Self, Error> {
        let db = sled::Config::new()
            .path(path)
            .cache_capacity(cache_capacity)
            .open()?;
        Self::from_db(db, analytics_enabled)
    }

    #[cfg(test)]
    fn temporary(analytics_enabled: bool) -> Result<Self, Error> {
        let db = sled::Config::new().temporary(true).open()?;
        Self::from_db(db, analytics_enabled)
    }

    fn from_db(db: sled::Db, analytics_enabled: bool) -> Result<Self, Error> {
        let links = db.open_tree("links")?;
        let clicks = db.open_tree("clicks")?;
        Ok(Self {
            db,
            links,
            clicks,
            analytics_enabled,
        })
    }
}

#[derive(Deserialize, Serialize)]
struct ClickEvent {
    code: String,
    clicked_at: i64,
    client_ip: Option<String>,
    user_agent: Option<String>,
}

#[async_trait]
impl zhorten_service::Database for Database {
    type Error = Error;

    /// Load the dashboard view from persisted link records.
    ///
    /// Corrupt records are skipped so one bad value does not make the whole
    /// administration page unusable.
    async fn dashboard(&self) -> Result<DashboardData, Self::Error> {
        let mut links: Vec<LinkRecord> = self
            .links
            .iter()
            .values()
            .filter_map(Result::ok)
            .filter_map(|value| serde_json::from_slice(&value).ok())
            .collect();
        links.sort_by_key(|record| std::cmp::Reverse(record.created_at));
        let total_clicks = links.iter().map(|record| record.clicks).sum();
        Ok(DashboardData {
            links,
            total_clicks,
        })
    }

    /// Build and insert a new short link if the validated code is available.
    async fn create_link(
        &self,
        code: ValidCode,
        url: Url,
        created_at: i64,
    ) -> Result<Option<LinkRecord>, Self::Error> {
        if self.links.contains_key(code.as_str().as_bytes())? {
            return Ok(None);
        }
        let record = LinkRecord {
            code,
            url: url.to_string(),
            clicks: 0,
            created_at,
            last_clicked_at: None,
        };
        self.links.insert(
            record.code.as_str().as_bytes(),
            serde_json::to_vec(&record)?.as_slice(),
        )?;
        self.db.flush_async().await?;
        Ok(Some(record))
    }

    /// Delete a short link by code.
    async fn remove_link(&self, code: &ValidCode) -> Result<(), Self::Error> {
        self.links.remove(code.as_str().as_bytes())?;
        self.db.flush_async().await?;
        Ok(())
    }

    /// Resolve a short code and record the click as part of the redirect path.
    ///
    /// The returned URL comes from the pre-update record, while the persisted
    /// record is updated atomically with a saturated click count.
    async fn follow_link(
        &self,
        code: &ValidCode,
        context: ClickContext,
    ) -> Result<Option<String>, Self::Error> {
        let Some(bytes) = self.links.get(code.as_str().as_bytes())? else {
            return Ok(None);
        };
        let record = serde_json::from_slice::<LinkRecord>(&bytes)?;
        let clicked_at = context.clicked_at;
        self.links
            .update_and_fetch(code.as_str().as_bytes(), |previous| {
                let mut current =
                    previous.and_then(|value| serde_json::from_slice::<LinkRecord>(value).ok())?;
                current.clicks = current.clicks.saturating_add(1);
                current.last_clicked_at = Some(clicked_at);
                serde_json::to_vec(&current).ok()
            })?;

        if self.analytics_enabled {
            // The separate click tree is intentionally best-effort: redirecting
            // is more important than preserving a raw analytics event if this
            // insert fails after the aggregate count has already been updated.
            let id = self.db.generate_id().unwrap_or_default();
            let event = ClickEvent {
                code: code.as_str().to_owned(),
                clicked_at,
                client_ip: context.client_ip,
                user_agent: context.user_agent,
            };
            if let Ok(bytes) = serde_json::to_vec(&event) {
                let _ = self
                    .clicks
                    .insert(format!("{code}:{id:020}").as_bytes(), bytes.as_slice());
            }
        }
        Ok(Some(record.url))
    }
}

impl From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Self::Storage(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidRecord(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "{error}"),
            Self::InvalidRecord(error) => write!(formatter, "invalid database record: {error}"),
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
        assert_eq!(database.clicks.len(), 0);

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
        assert_eq!(database.clicks.len(), 1);
    }
}
