use agent_meter::snapshot::parse_snapshot;
use agent_meter::tooltip::window_tooltip;
use chrono::{Duration, TimeZone, Utc};

fn snapshot(collected_at: &str) -> agent_meter::snapshot::Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"p","plan_label":"Max",
        "collected_at":"{collected_at}","source":"provider_reported","health":"ok",
        "windows":[
          {{"id":"w","label":"Weekly","used_percent":63.2,"resets_at":"2026-10-17T08:00:00Z","source":"provider_reported",
            "scoped":[{{"label":"Opus","used_percent":90.0}},{{"label":"Sonnet","used_percent":null}}]}},
          {{"id":"s","label":"Session","used_percent":null,"resets_at":null,"source":"unknown"}}
        ],"bonus_resets":null}}"#
    ))
    .unwrap()
}

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

#[test]
fn a_fresh_window_tooltip_gives_usage_reset_source_and_scoped_allowances() {
    let snap = snapshot("2026-10-14T00:00:00Z");
    let lines = window_tooltip(&snap, 0, now(), Duration::minutes(10)).unwrap();
    assert_eq!(
        lines,
        vec![
            "Weekly 63% used, resets in 3d 8h",
            "Source: provider_reported",
            "Opus 90% used",
            "Sonnet usage unknown",
        ]
    );
}

#[test]
fn a_stale_snapshot_adds_how_long_ago_it_was_updated() {
    let snap = snapshot("2026-10-13T23:49:00Z");
    let lines = window_tooltip(&snap, 0, now(), Duration::minutes(10)).unwrap();
    assert!(lines.contains(&"Last updated 11m ago".to_string()), "{lines:?}");
    let fresh = window_tooltip(&snapshot("2026-10-14T00:00:00Z"), 0, now(), Duration::minutes(10)).unwrap();
    assert!(!fresh.iter().any(|l| l.contains("Last updated")));
}

#[test]
fn unknown_usage_is_never_shown_as_zero_and_a_missing_window_has_no_tooltip() {
    let snap = snapshot("2026-10-14T00:00:00Z");
    let lines = window_tooltip(&snap, 1, now(), Duration::minutes(10)).unwrap();
    assert_eq!(lines, vec!["Session usage unknown", "Source: unknown"]);
    assert!(!lines.iter().any(|l| l.contains("0%")));
    assert_eq!(window_tooltip(&snap, 5, now(), Duration::minutes(10)), None);
}

#[test]
fn a_reset_already_in_the_past_is_not_shown_as_a_negative_countdown() {
    let snap = snapshot("2026-10-14T00:00:00Z");
    let later = Utc.with_ymd_and_hms(2026, 10, 18, 0, 0, 0).unwrap();
    let lines = window_tooltip(&snap, 0, later, Duration::days(30)).unwrap();
    assert_eq!(lines[0], "Weekly 63% used, reset time passed");
}
