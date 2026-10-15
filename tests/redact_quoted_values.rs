use agent_meter::redact::redact;

#[test]
fn keeps_balanced_quotes_around_redacted_values() {
    assert_eq!(redact("access_token=\"tok_123\" next"), "access_token=\"[redacted]\" next");
    assert_eq!(redact("api_key='abc',"), "api_key='[redacted]',");
    assert_eq!(redact("token=\"\""), "token=\"\"");
}
