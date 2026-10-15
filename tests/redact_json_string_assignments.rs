use agent_meter::redact::redact;

#[test]
fn assignment_inside_json_string_keeps_json_valid_and_hides_secret() {
    let out = redact(r#"{"note":"password=\"abc\" ok","n":1}"#);
    assert!(!out.contains("abc"), "leaked: {out}");
    let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(v["n"], 1);
}
