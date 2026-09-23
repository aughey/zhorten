use axum::http::{HeaderMap, header};
use time::OffsetDateTime;

pub const SESSION_COOKIE: &str = "zhorten_session";
pub const SESSION_TOKEN_LEN: usize = 48;
pub const SESSION_MAX_AGE_SECONDS: i64 = 86_400;
const MAX_CODE_LEN: usize = 32;

pub fn cookie_token(headers: &HeaderMap) -> Option<String> {
    let prefix = format!("{SESSION_COOKIE}=");
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|value| value.strip_prefix(&prefix))
        .filter(|token| {
            token.len() == SESSION_TOKEN_LEN
                && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .map(str::to_owned)
}

pub fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

pub fn valid_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_CODE_LEN
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub fn session_cookie(token: &str, secure: bool, max_age: i64) -> String {
    let secure_attr = if secure { "; Secure" } else { "" };
    format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}{secure_attr}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn valid_code_accepts_only_route_safe_codes() {
        assert!(valid_code("abc-123_DEF"));
        assert!(!valid_code(""));
        assert!(!valid_code("has/slash"));
        assert!(!valid_code("has space"));
        assert!(!valid_code(&"a".repeat(MAX_CODE_LEN + 1)));
    }

    #[test]
    fn cookie_token_reads_a_valid_session_cookie() {
        let token = "a".repeat(SESSION_TOKEN_LEN);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("theme=dark; {SESSION_COOKIE}={token}")).unwrap(),
        );
        assert_eq!(cookie_token(&headers), Some(token));
    }

    #[test]
    fn session_cookie_can_be_marked_secure() {
        let cookie = session_cookie("token", true, SESSION_MAX_AGE_SECONDS);
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Secure"));
    }
}
