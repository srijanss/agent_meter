use agent_meter::redact::redact;

#[test]
fn redacts_uuid_after_a_tab() {
    let out = redact("user\t123e4567-e89b-12d3-a456-426614174000");
    assert!(!out.contains("123e4567"), "{out}");
}

#[test]
fn redacts_json_string_values_of_secret_keys() {
    let out = redact(r#"{"api_key":"secret-value","name":"ok"}"#);
    assert!(!out.contains("secret-value"), "{out}");
    assert!(out.contains(r#""name":"ok""#), "{out}");
}

#[test]
fn redacts_json_values_with_spaces_after_colon() {
    let out = redact("{\"access_token\": \"abc123\"}");
    assert!(!out.contains("abc123"), "{out}");
}
