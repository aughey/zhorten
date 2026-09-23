use super::*;
use std::{convert::Infallible, sync::Mutex};

#[derive(Default)]
struct FakeDatabase {
    links: Mutex<Vec<LinkRecord>>,
    followed_at: Mutex<Option<i64>>,
}

impl Database for FakeDatabase {
    type Error = Infallible;

    fn dashboard(&self) -> Result<DashboardData, Self::Error> {
        let links = self.links.lock().unwrap().clone();
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
        if self
            .links
            .lock()
            .unwrap()
            .iter()
            .any(|record| record.code == code)
        {
            return Ok(None);
        }
        let record = LinkRecord {
            code,
            url: url.to_string(),
            clicks: 0,
            created_at,
            last_clicked_at: None,
        };
        self.links.lock().unwrap().push(record.clone());
        Ok(Some(record))
    }

    async fn remove_link(&self, code: &ValidCode) -> Result<(), Self::Error> {
        self.links
            .lock()
            .unwrap()
            .retain(|record| &record.code != code);
        Ok(())
    }

    async fn follow_link(
        &self,
        code: &ValidCode,
        clicked_at: i64,
    ) -> Result<Option<String>, Self::Error> {
        *self.followed_at.lock().unwrap() = Some(clicked_at);
        let url = self
            .links
            .lock()
            .unwrap()
            .iter()
            .find(|record| &record.code == code)
            .map(|record| record.url.clone());
        Ok(url)
    }
}

#[tokio::test]
async fn create_link_validates_inputs_before_inserting() {
    let database = FakeDatabase::default();

    assert!(matches!(
        create_link(
            &database,
            "bad/code".into(),
            "https://example.com/".into(),
            100
        )
        .await,
        Err(CreateLinkError::InvalidCode)
    ));
    assert!(matches!(
        create_link(&database, "docs".into(), "not a url".into(), 100).await,
        Err(CreateLinkError::InvalidUrl)
    ));
    assert!(matches!(
        create_link(&database, "docs".into(), "ftp://example.com/".into(), 100).await,
        Err(CreateLinkError::UnsupportedUrlScheme)
    ));

    let record = create_link(&database, "docs".into(), "https://example.com/".into(), 100).await;
    assert_eq!(record.unwrap().url, "https://example.com/");
    assert!(matches!(
        create_link(
            &database,
            "docs".into(),
            "https://other.example/".into(),
            100
        )
        .await,
        Err(CreateLinkError::Conflict)
    ));
}

#[tokio::test]
async fn remove_link_validates_code_and_delegates_to_database() {
    let database = FakeDatabase::default();
    create_link(&database, "docs".into(), "https://example.com/".into(), 100)
        .await
        .unwrap();

    assert_eq!(
        remove_link(&database, "bad/code".into()).await,
        Err(RemoveLinkError::InvalidCode)
    );
    remove_link(&database, "docs".into()).await.unwrap();
    assert!(list_links(&database).unwrap().links.is_empty());
}

#[tokio::test]
async fn follow_link_validates_code_and_reports_missing_links() {
    let database = FakeDatabase::default();
    create_link(&database, "docs".into(), "https://example.com/".into(), 100)
        .await
        .unwrap();

    assert_eq!(
        follow_link(&database, "bad/code".into(), 200).await,
        Err(FollowLinkError::InvalidCode)
    );
    assert_eq!(
        follow_link(&database, "missing".into(), 200).await,
        Err(FollowLinkError::NotFound)
    );
    assert_eq!(
        follow_link(&database, "docs".into(), 250).await.unwrap(),
        "https://example.com/"
    );
    assert_eq!(*database.followed_at.lock().unwrap(), Some(250));
}
