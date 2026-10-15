use agent_meter::redact::redact;

#[test]
fn redacts_value_containing_an_escaped_quote() {
    let out = redact(r#"{"password":"prefix\"remaining-secret","name":"ok"}"#);
    assert!(!out.contains("remaining-secret") && !out.contains("prefix"), "{out}");
    assert!(out.contains(r#""name":"ok""#), "{out}");
}

#[test]
fn escaped_backslash_before_closing_quote_still_ends_the_value() {
    let out = redact(r#"{"token":"abc\\","password":"second"}"#);
    assert!(!out.contains("abc") && !out.contains("second"), "{out}");
}
