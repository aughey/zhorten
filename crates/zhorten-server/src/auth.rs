use axum_login::{AuthUser, AuthnBackend, UserId};
use password_auth::{generate_hash, verify_password};
use serde::Deserialize;
use std::{fmt, sync::Arc};

#[derive(Clone)]
pub struct User {
    username: String,
    password_hash: String,
}

impl fmt::Debug for User {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("User")
            .field("username", &self.username)
            .field("password_hash", &"[redacted]")
            .finish()
    }
}

impl AuthUser for User {
    type Id = String;

    fn id(&self) -> Self::Id {
        self.username.clone()
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.password_hash.as_bytes()
    }
}

#[derive(Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Clone)]
pub struct Backend {
    user: Arc<User>,
}

impl Backend {
    pub fn new(username: String, password: String) -> Self {
        Self {
            user: Arc::new(User {
                username,
                password_hash: generate_hash(password),
            }),
        }
    }
}

#[derive(Debug)]
pub struct Error(tokio::task::JoinError);

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "password verification task failed: {}", self.0)
    }
}

impl std::error::Error for Error {}

impl AuthnBackend for Backend {
    type User = User;
    type Credentials = Credentials;
    type Error = Error;

    async fn authenticate(&self, credentials: Credentials) -> Result<Option<User>, Error> {
        let user = Arc::clone(&self.user);
        tokio::task::spawn_blocking(move || {
            let username_matches = credentials.username == user.username;
            let password_matches =
                verify_password(credentials.password, &user.password_hash).is_ok();
            (username_matches && password_matches).then(|| (*user).clone())
        })
        .await
        .map_err(Error)
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<User>, Error> {
        Ok((user_id == &self.user.username).then(|| (*self.user).clone()))
    }
}

pub type AuthSession = axum_login::AuthSession<Backend>;

#[cfg(test)]
mod tests {
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
}
