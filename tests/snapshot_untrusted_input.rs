use agent_meter::snapshot::{parse_snapshot, MAX_SNAPSHOT_BYTES};

fn json(percent: &str, scoped: &str, collected_at: &str, resets_at: &str) -> String {
    format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"p","plan_label":null,
        "collected_at":{collected_at},"source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":{percent},"resets_at":{resets_at},
        "source":"provider_reported","scoped":[{scoped}]}}],"bonus_resets":null}}"#
    )
}

fn ok_json() -> String {
    json("42.5", "", "\"2026-10-14T00:00:00Z\"", "null")
}

#[test]
fn malformed_out_of_range_or_oversized_collector_output_is_rejected_without_panicking() {
    assert!(parse_snapshot(&ok_json()).is_ok());
    assert!(parse_snapshot(&json("0", "", "\"2026-10-14T00:00:00Z\"", "null")).is_ok());
    assert!(parse_snapshot(&json("100", "", "\"2026-10-14T00:00:00Z\"", "null")).is_ok());

    // Out-of-range or non-finite percentages, on a window or a scoped allowance.
    for bad in ["-0.1", "100.1", "1e999", "1e308"] {
        let err = parse_snapshot(&json(bad, "", "\"2026-10-14T00:00:00Z\"", "null"));
        assert!(err.is_err(), "window percent {bad} must be rejected");
        let scoped = format!(r#"{{"label":"Opus","used_percent":{bad}}}"#);
        let err = parse_snapshot(&json("10", &scoped, "\"2026-10-14T00:00:00Z\"", "null"));
        assert!(err.is_err(), "scoped percent {bad} must be rejected");
    }

    // Wrong types and timestamps without a timezone.
    assert!(parse_snapshot(&json("\"42\"", "", "\"2026-10-14T00:00:00Z\"", "null")).is_err());
    assert!(parse_snapshot(&json("true", "", "\"2026-10-14T00:00:00Z\"", "null")).is_err());
    assert!(parse_snapshot(&json("42", "", "\"2026-10-14T00:00:00\"", "null")).is_err());
    assert!(parse_snapshot(&json("42", "", "\"2026-10-14T00:00:00Z\"", "\"2026-10-14 05:00\"")).is_err());
    assert!(parse_snapshot(&json("42", "", "12345", "null")).is_err());

    // Garbage and oversized payloads fail with an error, not a panic.
    for garbage in ["", "null", "[]", "{", "\u{0}", "\"x\""] {
        assert!(parse_snapshot(garbage).is_err(), "{garbage:?}");
    }
    let padding = " ".repeat(MAX_SNAPSHOT_BYTES);
    assert!(parse_snapshot(&format!("{}{padding}", ok_json())).is_err(), "oversized payload");
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert!(parse_snapshot(&deep).is_err());
}
