use agent_meter::redact::redact;

#[test]
fn removes_emails_tokens_headers_and_org_ids() {
    let raw = "user alice@example.com org 123e4567-e89b-12d3-a456-426614174000\n\
               Authorization: Bearer abc.def.ghi\n\
               Cookie: sessionKey=sk-ant-oat01-SECRET; other=1\n\
               token sk-ant-api03-ZZZZ and access_token=\"tok_123\"";

    let out = redact(raw);

    for secret in [
        "alice@example.com",
        "123e4567-e89b-12d3-a456-426614174000",
        "abc.def.ghi",
        "sk-ant-oat01-SECRET",
        "sessionKey",
        "sk-ant-api03-ZZZZ",
        "tok_123",
    ] {
        assert!(!out.contains(secret), "{secret} leaked in: {out}");
    }
    assert!(out.contains("[redacted]"));
}

#[test]
fn leaves_ordinary_usage_text_untouched() {
    let text = "Current session: 34% used · resets Oct 15, 6:49pm (Australia/Sydney)";
    assert_eq!(redact(text), text);
}
