use agent_meter::redact::redact;

#[test]
fn camel_case_secret_keys_are_redacted() {
    for line in ["accessToken=abc123", "clientSecret=abc123", "sessionId=abc123", "apiKey=abc123"] {
        let out = redact(line);
        assert!(!out.contains("abc123"), "{line} leaked: {out}");
    }
    assert!(!redact(r#"{"accessToken": "abc123"}"#).contains("abc123"));
}

#[test]
fn lookalike_words_are_not_redacted() {
    for line in ["monkey=banana", "keyboard=qwerty", "turkey=bird", "tokenizer=bpe"] {
        assert_eq!(redact(line), line);
    }
}
