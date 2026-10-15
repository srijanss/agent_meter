use agent_meter::countdown::ResetDisplay;
use agent_meter::display::{account_view, AccountView};
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};

fn snapshot(windows: &str) -> String {
    format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"p","collected_at":"2026-10-14T00:00:00Z",
        "source":"provider_reported","health":"ok","windows":[{windows}],"bonus_resets":null}}"#
    )
}

#[test]
fn null_used_percent_is_unknown_not_zero() {
    let json = snapshot(
        r#"{"id":"session","label":"Session","used_percent":0.0,"resets_at":"2026-10-14T03:00:00Z","source":"provider_reported"},
           {"id":"weekly","label":"Weekly","used_percent":null,"resets_at":"2026-10-18T00:00:00Z","source":"unknown"}"#,
    );
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let AccountView::Rows(rows) = account_view(&parse_snapshot(&json).unwrap(), now) else {
        panic!("expected rows");
    };
    assert_eq!(rows[0].fill, Some(0.0));
    assert_eq!(rows[0].reset, Some(ResetDisplay::Remaining("3h 0m".into())));
    assert_eq!(rows[1].label, "Weekly");
    assert_eq!(rows[1].fill, None);
    assert_eq!(rows[1].reset, None);
}

#[test]
fn account_without_measured_windows_is_unavailable() {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    for windows in [
        String::new(),
        r#"{"id":"weekly","label":"Weekly","used_percent":null,"source":"unknown"}"#.to_string(),
    ] {
        let s = parse_snapshot(&snapshot(&windows)).unwrap();
        assert_eq!(account_view(&s, now), AccountView::Unavailable);
    }
}
