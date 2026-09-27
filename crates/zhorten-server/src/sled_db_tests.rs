use super::*;
use zhorten_service::Database as _;

fn temporary_database() -> Database {
    let db = sled::Config::new().temporary(true).open().unwrap();
    let links = db.open_tree("links").unwrap();
    let clicks = db.open_tree("clicks").unwrap();
    Database { db, links, clicks }
}

#[tokio::test]
async fn link_lifecycle_updates_dashboard_and_clicks() {
    let database = temporary_database();
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
        database.follow_link(&record.code, 200).await.unwrap(),
        Some(record.url.clone())
    );

    let dashboard = database.dashboard().unwrap();
    assert_eq!(dashboard.total_clicks, 1);
    assert_eq!(dashboard.links[0].last_clicked_at, Some(200));

    database.remove_link(&record.code).await.unwrap();
    assert!(
        database
            .follow_link(&record.code, 300)
            .await
            .unwrap()
            .is_none()
    );
}
