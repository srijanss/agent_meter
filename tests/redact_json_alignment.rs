use agent_meter::redact::redact;

#[test]
fn redacts_every_secret_pair_in_one_object() {
    let out = redact(r#"{"token":"first","password":"second","name":"ok"}"#);
    assert!(!out.contains("first") && !out.contains("second"), "{out}");
    assert_eq!(out, r#"{"token":"[redacted]","password":"[redacted]","name":"ok"}"#);
}

#[test]
fn keeps_following_pairs_after_a_redacted_value() {
    let out = redact(r#"{"api_key":"k1","note":"password","secret":"s1"}"#);
    assert!(!out.contains("k1") && !out.contains("s1"), "{out}");
    assert!(out.contains(r#""note":"password""#), "{out}");
}
