use time::OffsetDateTime;

const MAX_CODE_LEN: usize = 32;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_code_accepts_only_route_safe_codes() {
        assert!(valid_code("abc-123_DEF"));
        assert!(!valid_code(""));
        assert!(!valid_code("has/slash"));
        assert!(!valid_code("has space"));
        assert!(!valid_code(&"a".repeat(MAX_CODE_LEN + 1)));
    }
}
