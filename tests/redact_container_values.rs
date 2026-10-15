use agent_meter::redact::redact;

fn parse(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or_else(|e| panic!("invalid JSON {s}: {e}"))
}

#[test]
fn array_secret_values_are_redacted_whole() {
    let out = redact(r#"{"token":["abc123"],"x":1}"#);
    assert!(!out.contains("abc123"), "leaked: {out}");
    assert_eq!(parse(&out)["x"], 1);
}

#[test]
fn object_secret_values_are_redacted_whole() {
    let out = redact(r#"{"credential":{"value":"abc123","k":[1,2]},"y":2}"#);
    assert!(!out.contains("abc123"), "leaked: {out}");
    assert_eq!(parse(&out)["y"], 2);
}

#[test]
fn nested_containers_and_brackets_in_strings_are_handled() {
    let out = redact(r#"{"token":[["a]"],{"b":"}c"}],"z":3}"#);
    assert!(!out.contains("}c") && !out.contains("a]"), "leaked: {out}");
    assert_eq!(parse(&out)["z"], 3);
}
