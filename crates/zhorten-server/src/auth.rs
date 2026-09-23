use axum_login::{AuthUser, AuthnBackend, UserId};
use password_auth::{generate_hash, verify_password};
use std::{fmt, sync::Arc};
use zhorten_core::api::LoginRequest;

#[derive(Clone)]
pub struct User {
    username: String,
    password_hash: String,
}

// `User` appears in traces and test failures through axum-login, so redact the
// derived output instead of relying on callers to remember not to log secrets.
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
        // axum-login stores this hash in the session so a password change would
        // invalidate existing sessions. This service has one startup-time user,
        // but keeping the hook wired correctly avoids a future footgun.
        self.password_hash.as_bytes()
    }
}

/// Login credentials accepted by the authentication backend.
pub type Credentials = LoginRequest;

#[derive(Clone)]
pub struct Backend {
    user: Arc<User>,
}

impl Backend {
    /// Build the in-memory authentication backend for the configured admin user.
    ///
    /// The password is hashed at startup and never persisted with the link data.
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

    /// Verify credentials off the async runtime because Argon2 is intentionally CPU-heavy.
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

/// Session extractor used by request handlers after axum-login layers are installed.
pub type AuthSession = axum_login::AuthSession<Backend>;

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
