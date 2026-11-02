//! The optional local usage history: snapshots recorded over time, with no
//! credentials and no claim that a local figure is subscription-wide usage.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use agent_meter::history::HistoryStore;
use agent_meter::snapshot::{parse_snapshot, Snapshot, Source};
use chrono::{TimeZone, Utc};

fn dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_hist_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn snapshot(name: &str, source: &str, hour: u32, percent: f64) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"{name}","plan_label":"Pro",
        "collected_at":"2026-10-14T{hour:02}:00:00Z","source":"{source}","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":{percent},"resets_at":"2026-10-14T23:00:00Z",
        "source":"{source}"}}],"bonus_resets":null}}"#
    ))
    .unwrap()
}

#[test]
fn recorded_snapshots_load_back_in_order_and_a_missing_store_is_empty() {
    let dir = dir("order");
    let store = HistoryStore::new(dir.join("history.jsonl"), 100);
    assert!(store.load().is_empty());

    store.record(&snapshot("work", "provider_reported", 1, 10.0)).unwrap();
    store.record(&snapshot("work", "provider_reported", 2, 20.0)).unwrap();

    let records = store.load();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].account, "work");
    assert_eq!(records[0].provider, "claude");
    assert_eq!(records[0].recorded_at, Utc.with_ymd_and_hms(2026, 10, 14, 1, 0, 0).unwrap());
    assert_eq!(records[1].windows[0].used_percent, Some(20.0));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_local_estimate_stays_marked_as_one_and_nothing_sensitive_is_stored() {
    let dir = dir("source");
    let path = dir.join("history.jsonl");
    let store = HistoryStore::new(path.clone(), 100);
    store.record(&snapshot("work", "local_estimate", 1, 5.0)).unwrap();

    assert_eq!(store.load()[0].source, Source::LocalEstimate);
    // Only usage figures are kept: no plan, no paths, no tokens, no account home.
    let text = fs::read_to_string(&path).unwrap();
    for forbidden in ["plan", "Pro", "home", "token", "secret", "bonus"] {
        assert!(!text.contains(forbidden), "{forbidden} leaked into {text}");
    }
    // The file is private to the user.
    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_store_is_bounded_and_skips_lines_it_cannot_read() {
    let dir = dir("bound");
    let path = dir.join("history.jsonl");
    let store = HistoryStore::new(path.clone(), 3);
    for hour in 0..5 {
        store.record(&snapshot("work", "provider_reported", hour, hour as f64)).unwrap();
    }
    let kept: Vec<_> = store.load().iter().map(|r| r.windows[0].used_percent).collect();
    assert_eq!(kept, vec![Some(2.0), Some(3.0), Some(4.0)], "the newest are kept");

    // A hand-edited or truncated line is skipped, not fatal, and not propagated.
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("{not json\n");
    fs::write(&path, text).unwrap();
    assert_eq!(store.load().len(), 3);
    store.record(&snapshot("work", "provider_reported", 6, 6.0)).unwrap();
    assert!(!fs::read_to_string(&path).unwrap().contains("not json"));
    let _ = fs::remove_dir_all(&dir);
}
