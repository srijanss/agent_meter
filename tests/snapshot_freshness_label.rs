use agent_meter::freshness::{freshness, Freshness};
use agent_meter::snapshot::parse_snapshot;
use chrono::{Duration, TimeZone, Utc};

fn snapshot_collected_at(ts: &str) -> agent_meter::snapshot::Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"p","collected_at":"{ts}",
        "source":"provider_reported","health":"ok","windows":[],"bonus_resets":null}}"#
    ))
    .unwrap()
}

#[test]
fn snapshots_older_than_threshold_are_stale_with_age() {
    let s = snapshot_collected_at("2026-10-14T00:00:00Z");
    let threshold = Duration::minutes(10);
    let at = |mins: i64| Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap() + Duration::minutes(mins);

    assert_eq!(freshness(&s, at(3), threshold), Freshness::Fresh);
    assert_eq!(freshness(&s, at(10), threshold), Freshness::Fresh);
    assert_eq!(freshness(&s, at(11), threshold), Freshness::Stale { age: "11m".into() });
    assert_eq!(freshness(&s, at(26 * 60), threshold), Freshness::Stale { age: "1d 2h".into() });
}

#[test]
fn collected_in_the_future_is_fresh() {
    let s = snapshot_collected_at("2026-10-14T01:00:00Z");
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    assert_eq!(freshness(&s, now, Duration::minutes(10)), Freshness::Fresh);
}
