use time::OffsetDateTime;

/// Return the current UTC timestamp in the format stored by link records.
pub fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}
