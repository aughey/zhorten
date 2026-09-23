use std::{fmt, path::Path};
use zhorten_core::{DashboardData, LinkRecord, ValidCode};

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

    /// Build and insert a new short link if the validated code is available.
    pub async fn create_link(
        &self,
        code: ValidCode,
        url: String,
        created_at: i64,
    ) -> Result<Option<LinkRecord>, Error> {
        if self.links.contains_key(code.as_str().as_bytes())? {
            return Ok(None);
        }
        let record = LinkRecord {
            code,
            url,
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
    pub async fn remove_link(&self, code: &ValidCode) -> Result<(), Error> {
        self.links.remove(code.as_str().as_bytes())?;
        self.db.flush_async().await?;
        Ok(())
    }

    /// Resolve a short code and record the click as part of the redirect path.
    ///
    /// The returned URL comes from the pre-update record, while the persisted
    /// record is updated atomically with a saturated click count.
    pub fn follow_link(&self, code: &ValidCode, clicked_at: i64) -> Result<Option<String>, Error> {
        let Some(bytes) = self.links.get(code.as_str().as_bytes())? else {
            return Ok(None);
        };
        let record = serde_json::from_slice::<LinkRecord>(&bytes)?;
        self.links
            .update_and_fetch(code.as_str().as_bytes(), |previous| {
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
            code: ValidCode::try_from("docs").unwrap(),
            url: "https://example.com/".into(),
            clicks: 0,
            created_at: 100,
            last_clicked_at: None,
        };

        assert_eq!(
            database
                .create_link(record.code.clone(), record.url.clone(), record.created_at)
                .await
                .unwrap()
                .unwrap()
                .code,
            record.code
        );
        assert!(
            database
                .create_link(record.code.clone(), record.url.clone(), record.created_at)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            database.follow_link(&record.code, 200).unwrap(),
            Some(record.url.clone())
        );

        let dashboard = database.dashboard().unwrap();
        assert_eq!(dashboard.total_clicks, 1);
        assert_eq!(dashboard.links[0].last_clicked_at, Some(200));

        database.remove_link(&record.code).await.unwrap();
        assert!(database.follow_link(&record.code, 300).unwrap().is_none());
    }
}
