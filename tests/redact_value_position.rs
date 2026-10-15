use agent_meter::redact::redact;

#[test]
fn replaces_the_value_not_a_matching_key() {
    assert_eq!(redact("token=token"), "token=[redacted]");
    assert_eq!(redact("api_key=key"), "api_key=[redacted]");
    assert_eq!(redact("secret=ret"), "secret=[redacted]");
}
