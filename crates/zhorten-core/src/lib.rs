use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;

const MAX_CODE_LEN: usize = 32;

/// A short-link code that is safe to use as a route segment and database key.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ValidCode(String);

impl ValidCode {
    /// Borrow the validated code as its underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ValidCode {
    type Error = InvalidCode;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !value.is_empty()
            && value.len() <= MAX_CODE_LEN
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            Ok(Self(value))
        } else {
            Err(InvalidCode)
        }
    }
}

impl TryFrom<&str> for ValidCode {
    type Error = InvalidCode;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl<'de> Deserialize<'de> for ValidCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .try_into()
            .map_err(de::Error::custom)
    }
}

impl fmt::Display for ValidCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Returned when a string cannot be represented as a [`ValidCode`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCode;

impl fmt::Display for InvalidCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("code must be 1-32 letters, numbers, dashes, or underscores")
    }
}

impl std::error::Error for InvalidCode {}

/// A persisted short-link record.
///
/// The code is validated when constructed or deserialized, so persisted and API
/// records cannot represent an invalid route key.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LinkRecord {
    pub code: ValidCode,
    pub url: String,
    pub clicks: u64,
    pub created_at: i64,
    pub last_clicked_at: Option<i64>,
}

/// Data returned by the authenticated dashboard endpoint.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DashboardData {
    pub links: Vec<LinkRecord>,
    pub total_clicks: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_code_accepts_only_route_safe_codes() {
        assert!(ValidCode::try_from("abc-123_DEF").is_ok());
        assert!(ValidCode::try_from("").is_err());
        assert!(ValidCode::try_from("has/slash").is_err());
        assert!(ValidCode::try_from("has space").is_err());
        assert!(ValidCode::try_from("a".repeat(MAX_CODE_LEN + 1)).is_err());
    }

    #[test]
    fn deserialization_enforces_the_valid_code_invariant() {
        let code = serde_json::from_str::<ValidCode>("\"docs\"").unwrap();
        assert_eq!(serde_json::to_string(&code).unwrap(), "\"docs\"");
        assert!(serde_json::from_str::<ValidCode>("\"not/a/code\"").is_err());
    }
}
