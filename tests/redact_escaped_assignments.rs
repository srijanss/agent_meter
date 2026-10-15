use agent_meter::redact::redact;

#[test]
fn escaped_quotes_inside_quoted_assignments_do_not_end_the_secret() {
    let out = redact(r#"password="prefix\"remaining-secret" next=1"#);
    assert!(!out.contains("remaining-secret"), "leaked: {out}");
    assert!(!out.contains("prefix"), "leaked: {out}");
    assert!(out.contains("next=1"), "over-redacted: {out}");
}

#[test]
fn escaped_backslash_before_closing_quote_still_ends_the_value() {
    let out = redact(r#"token="abc\\" next=1"#);
    assert!(!out.contains("abc"), "leaked: {out}");
    assert!(out.contains("next=1"), "over-redacted: {out}");
}
