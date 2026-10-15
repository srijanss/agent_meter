use agent_meter::redact::redact;

#[test]
fn redacts_whole_unquoted_value_across_inner_punctuation() {
    let out = redact("token=abc:def next");
    assert!(!out.contains("def") && !out.contains("abc"), "{out}");
    assert!(out.ends_with(" next"), "{out}");
}

#[test]
fn redacts_whole_quoted_value_containing_spaces() {
    let out = redact(r#"password="two words here" tail"#);
    assert!(!out.contains("words") && !out.contains("here"), "{out}");
    assert_eq!(out, r#"password="[redacted]" tail"#);
}

#[test]
fn redacts_value_containing_brackets_and_braces() {
    let out = redact("secret=a{b}[c] done");
    assert!(!out.contains('b') || out == "secret=[redacted] done", "{out}");
    assert_eq!(out, "secret=[redacted] done");
}
