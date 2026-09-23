use std::{fmt, path::Path};
use zhorten_core::{DashboardData, LinkRecord};

#[derive(Clone)]
pub struct Database {
    db: sled::Db,
    links: sled::Tree,
    clicks: sled::Tree,
}

#[derive(Debug)]
pub enum Error {
    Storage(sled::Error),
    InvalidRecord(serde_json::Error),
}

impl Database {
    /// Open the embedded database and its named trees.
    ///
    /// `links` is the authoritative store for current short links. `clicks`
    /// keeps append-only timestamps for future analytics without changing the
    /// dashboard API today.
    pub fn open(path: impl AsRef<Path>, cache_capacity: u64) -> Result<Self, Error> {
        let db = sled::Config::new()
            .path(path)
            .cache_capacity(cache_capacity)
            .open()?;
        let links = db.open_tree("links")?;
        let clicks = db.open_tree("clicks")?;
        Ok(Self { db, links, clicks })
    }

    /// Load the dashboard view from persisted link records.
    ///
    /// Corrupt records are skipped so one bad value does not make the whole
    /// administration page unusable.
    pub fn dashboard(&self) -> Result<DashboardData, Error> {
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

    /// Insert a new short link if the code has not already been claimed.
    pub async fn create_link(&self, record: &LinkRecord) -> Result<bool, Error> {
        if self.links.contains_key(record.code.as_bytes())? {
            return Ok(false);
        }
        self.links.insert(
            record.code.as_bytes(),
            serde_json::to_vec(record)?.as_slice(),
        )?;
        self.db.flush_async().await?;
        Ok(true)
    }

    /// Delete a short link by code.
    pub async fn remove_link(&self, code: &str) -> Result<(), Error> {
        self.links.remove(code.as_bytes())?;
        self.db.flush_async().await?;
        Ok(())
    }

    /// Resolve a short code and record the click as part of the redirect path.
    ///
    /// The returned URL comes from the pre-update record, while the persisted
    /// record is updated atomically with a saturated click count.
    pub fn follow_link(&self, code: &str, clicked_at: i64) -> Result<Option<String>, Error> {
        let Some(bytes) = self.links.get(code.as_bytes())? else {
            return Ok(None);
        };
        let record = serde_json::from_slice::<LinkRecord>(&bytes)?;
        self.links.update_and_fetch(code.as_bytes(), |previous| {
            let mut current =
                previous.and_then(|value| serde_json::from_slice::<LinkRecord>(value).ok())?;
            current.clicks = current.clicks.saturating_add(1);
            current.last_clicked_at = Some(clicked_at);
            serde_json::to_vec(&current).ok()
        })?;

        // The separate click tree is intentionally best-effort: redirecting is
        // more important than preserving a raw analytics event if this insert
        // fails after the aggregate count has already been updated.
        let id = self.db.generate_id().unwrap_or_default();
        let _ = self.clicks.insert(
            format!("{code}:{id:020}").as_bytes(),
            clicked_at.to_be_bytes().as_slice(),
        );
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

    fn temporary_database() -> Database {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let links = db.open_tree("links").unwrap();
        let clicks = db.open_tree("clicks").unwrap();
        Database { db, links, clicks }
    }

    #[tokio::test]
    async fn link_lifecycle_updates_dashboard_and_clicks() {
        let database = temporary_database();
        let record = LinkRecord {
            code: "docs".into(),
            url: "https://example.com/".into(),
            clicks: 0,
            created_at: 100,
            last_clicked_at: None,
        };

        assert!(database.create_link(&record).await.unwrap());
        assert!(!database.create_link(&record).await.unwrap());
        assert_eq!(
            database.follow_link("docs", 200).unwrap(),
            Some(record.url.clone())
        );

        let dashboard = database.dashboard().unwrap();
        assert_eq!(dashboard.total_clicks, 1);
        assert_eq!(dashboard.links[0].last_clicked_at, Some(200));

        database.remove_link("docs").await.unwrap();
        assert!(database.follow_link("docs", 300).unwrap().is_none());
    }
}
