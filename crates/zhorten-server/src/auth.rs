use crate::helpers::{SESSION_MAX_AGE_SECONDS, SESSION_TOKEN_LEN, cookie_token, now};
use axum::http::HeaderMap;
use rand::{RngExt, distr::Alphanumeric};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(Clone, Debug)]
struct Session {
    expires_at: i64,
}

#[derive(Clone)]
pub struct Auth {
    username: Arc<String>,
    password: Arc<String>,
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    secure_cookies: bool,
}

impl Auth {
    pub fn new(username: String, password: String, secure_cookies: bool) -> Self {
        Self {
            username: Arc::new(username),
            password: Arc::new(password),
            sessions: Default::default(),
            secure_cookies,
        }
    }

    pub fn credentials_are_valid(&self, username: &str, password: &str) -> bool {
        constant_time_eq::constant_time_eq(username.as_bytes(), self.username.as_bytes())
            && constant_time_eq::constant_time_eq(password.as_bytes(), self.password.as_bytes())
    }

    pub fn create_session(&self) -> Result<String, &'static str> {
        let token: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(SESSION_TOKEN_LEN)
            .map(char::from)
            .collect();
        self.sessions
            .write()
            .map_err(|_| "session store lock poisoned")?
            .insert(
                token.clone(),
                Session {
                    expires_at: now() + SESSION_MAX_AGE_SECONDS,
                },
            );
        Ok(token)
    }

    pub fn authorized(&self, headers: &HeaderMap) -> bool {
        cookie_token(headers).is_some_and(|token| self.session_is_authorized(&token, now()))
    }

    pub fn remove_session(&self, headers: &HeaderMap) {
        let Some(token) = cookie_token(headers) else {
            return;
        };
        match self.sessions.write() {
            Ok(mut sessions) => {
                sessions.remove(&token);
            }
            Err(error) => tracing::error!(%error, "session store lock poisoned"),
        }
    }

    pub fn secure_cookies(&self) -> bool {
        self.secure_cookies
    }

    fn session_is_authorized(&self, token: &str, current_time: i64) -> bool {
        let session = match self.sessions.read() {
            Ok(sessions) => sessions.get(token).cloned(),
            Err(error) => {
                tracing::error!(%error, "session store lock poisoned");
                return false;
            }
        };
        let Some(session) = session else {
            return false;
        };
        if session.expires_at > current_time {
            return true;
        }
        match self.sessions.write() {
            Ok(mut sessions) => {
                sessions.remove(token);
            }
            Err(error) => tracing::error!(%error, "session store lock poisoned"),
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::SESSION_COOKIE;
    use axum::http::{HeaderValue, header};

    #[test]
    fn session_lifecycle_authorizes_then_removes_token() {
        let auth = Auth::new("admin".into(), "secret".into(), true);
        assert!(auth.credentials_are_valid("admin", "secret"));
        assert!(!auth.credentials_are_valid("admin", "wrong"));

        let token = auth.create_session().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}")).unwrap(),
        );
        assert!(auth.authorized(&headers));

        auth.remove_session(&headers);
        assert!(!auth.authorized(&headers));
    }
}
