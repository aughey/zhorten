use super::*;

#[tokio::test]
async fn backend_authenticates_only_the_configured_user() {
    let backend = Backend::new("admin".into(), "secret".into());

    let user = backend
        .authenticate(Credentials {
            username: "admin".into(),
            password: "secret".into(),
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.id(), "admin");
    assert!(
        backend
            .authenticate(Credentials {
                username: "admin".into(),
                password: "wrong".into(),
            })
            .await
            .unwrap()
            .is_none()
    );
    assert!(backend.get_user(&"admin".into()).await.unwrap().is_some());
}
