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
