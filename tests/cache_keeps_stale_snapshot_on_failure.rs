use agent_meter::cache::SnapshotCache;
use agent_meter::snapshot::{parse_snapshot, Snapshot};

fn snapshot(used: f64) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"p","collected_at":"2026-10-14T00:00:00Z",
        "source":"provider_reported","health":"ok","bonus_resets":null,
        "windows":[{{"id":"weekly","label":"Weekly","used_percent":{used},"source":"provider_reported"}}]}}"#
    ))
    .unwrap()
}

#[test]
fn failed_refresh_keeps_previous_snapshot_flagged_stale() {
    let mut cache = SnapshotCache::default();
    assert!(cache.current().is_none());

    cache.record(Ok(snapshot(40.0)));
    let live = cache.current().unwrap();
    assert!(!live.is_stale());
    assert_eq!(live.error(), None);

    cache.record(Err("codex: sign-in required".to_string()));
    let stale = cache.current().unwrap();
    assert!(stale.is_stale());
    assert_eq!(stale.error(), Some("codex: sign-in required"));
    assert_eq!(stale.snapshot().windows[0].used_percent, Some(40.0));

    cache.record(Ok(snapshot(55.0)));
    let recovered = cache.current().unwrap();
    assert!(!recovered.is_stale());
    assert_eq!(recovered.snapshot().windows[0].used_percent, Some(55.0));
}

#[test]
fn failure_with_nothing_cached_stays_empty_and_remembers_error() {
    let mut cache = SnapshotCache::default();
    cache.record(Err("claude: CLI not found".to_string()));
    assert!(cache.current().is_none());
    assert_eq!(cache.last_error(), Some("claude: CLI not found"));
}
