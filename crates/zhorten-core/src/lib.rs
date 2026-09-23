pub mod api;

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

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
