use agent_meter::snapshot::{parse_snapshot, ScopedAllowance};

fn snapshot(window_extra: &str) -> String {
    format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"p","collected_at":"2026-10-14T00:00:00Z",
        "source":"provider_reported","health":"ok","bonus_resets":null,
        "windows":[{{"id":"weekly","label":"Weekly","used_percent":5.0,"resets_at":null,
        "source":"provider_reported"{window_extra}}}]}}"#
    )
}

#[test]
fn windows_carry_optional_model_scoped_allowances() {
    let with = parse_snapshot(&snapshot(
        r#","scoped":[{"label":"Opus","used_percent":12.5},{"label":"Sonnet","used_percent":null}]"#,
    ))
    .unwrap();
    assert_eq!(
        with.windows[0].scoped,
        vec![
            ScopedAllowance { label: "Opus".into(), used_percent: Some(12.5) },
            ScopedAllowance { label: "Sonnet".into(), used_percent: None },
        ]
    );

    // Snapshots that predate scoped allowances still parse, with none.
    let without = parse_snapshot(&snapshot("")).unwrap();
    assert!(without.windows[0].scoped.is_empty());
}
