use agent_meter::snapshot::parse_snapshot;

fn json(version: &str, source: &str) -> String {
    format!(
        r#"{{"schema_version":{version},"provider":"claude","profile_id":"p","plan_label":null,
        "collected_at":"2026-10-14T00:00:00Z","source":"{source}","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":42.0,"resets_at":null,"source":"{source}"}}],
        "bonus_resets":null}}"#
    )
}

#[test]
fn only_schema_version_one_with_a_known_source_yields_a_snapshot() {
    let ok = parse_snapshot(&json("1", "provider_reported")).unwrap();
    assert_eq!(ok.windows[0].used_percent, Some(42.0));

    for version in ["0", "2", "99", "-1", "\"1\"", "1.5", "null"] {
        let err = parse_snapshot(&json(version, "provider_reported"));
        assert!(err.is_err(), "schema_version {version} must be rejected");
    }
    assert!(parse_snapshot(&json("1", "guessed")).is_err(), "unknown source must be rejected");

    let message = parse_snapshot(&json("2", "provider_reported")).unwrap_err().to_string();
    assert!(message.contains("schema_version"), "{message}");
}
