use agent_meter::redact::redact;

#[test]
fn numeric_json_secret_values_are_redacted() {
    let out = redact(r#"{"pin": 1234, "ok": 5}"#);
    assert!(!out.contains("1234"), "leaked: {out}");
    assert!(out.contains(r#""ok": 5"#), "over-redacted: {out}");
}

#[test]
fn secret_value_on_next_line_is_redacted() {
    let out = redact("{\"token\":\n  \"abc123\"}");
    assert!(!out.contains("abc123"), "leaked: {out}");
}

#[test]
fn redacted_json_stays_parseable() {
    let out = redact(r#"{"token": "abc", "pin": 1234, "n": 1}"#);
    serde_json::from_str::<serde_json::Value>(&out).expect("still valid JSON");
}
