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

#[test]
fn each_window_keeps_its_own_source_even_inside_a_provider_reported_snapshot() {
    let dir = dir("window_source");
    let store = HistoryStore::new(dir.join("history.jsonl"), 10);
    let mixed = parse_snapshot(
        r#"{"schema_version":1,"provider":"claude","profile_id":"work","plan_label":null,
        "collected_at":"2026-10-14T01:00:00Z","source":"provider_reported","health":"ok",
        "windows":[
          {"id":"s","label":"Session","used_percent":10.0,"resets_at":null,"source":"provider_reported"},
          {"id":"w","label":"Weekly","used_percent":20.0,"resets_at":null,"source":"local_estimate"}],
        "bonus_resets":null}"#,
    )
    .unwrap();
    store.record(&mixed).unwrap();

    let windows = &store.load()[0].windows;
    assert_eq!(windows[0].source, Source::ProviderReported);
    assert_eq!(windows[1].source, Source::LocalEstimate);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn concurrent_recorders_lose_no_records_and_leave_no_temporary_files() {
    let dir = dir("concurrent");
    let path = dir.join("history.jsonl");
    let handles: Vec<_> = (0..8)
        .map(|worker| {
            let path = path.clone();
            std::thread::spawn(move || {
                let store = HistoryStore::new(path, 1000);
                for n in 0..5 {
                    store.record(&snapshot("work", "provider_reported", worker, n as f64)).unwrap();
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 40, "every record survives, none is torn");
    assert_eq!(HistoryStore::new(path, 1000).load().len(), 40);
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != "history.jsonl" && name != "history.jsonl.lock")
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_history_is_never_replaced_by_a_single_record() {
    let dir = dir("unreadable");
    let path = dir.join("history.jsonl");
    let store = HistoryStore::new(path.clone(), 10);
    store.record(&snapshot("work", "provider_reported", 1, 1.0)).unwrap();

    // One line of invalid UTF-8 costs only that line.
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(b"\xff\xfe\n");
    fs::write(&path, bytes).unwrap();
    assert_eq!(store.load().len(), 1);
    store.record(&snapshot("work", "provider_reported", 2, 2.0)).unwrap();
    assert_eq!(store.load().len(), 2, "the earlier record survives");

    // A history that cannot be read at all is an error, never an empty start.
    let unreadable = dir.join("as_directory");
    fs::create_dir(&unreadable).unwrap();
    let broken = HistoryStore::new(unreadable.clone(), 10);
    assert!(broken.record(&snapshot("work", "provider_reported", 3, 3.0)).is_err());
    assert!(unreadable.is_dir());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn retention_bounds_of_zero_and_one_are_honoured() {
    let dir = dir("edges");
    let none = HistoryStore::new(dir.join("none.jsonl"), 0);
    none.record(&snapshot("work", "provider_reported", 1, 1.0)).unwrap();
    assert!(none.load().is_empty());

    let one = HistoryStore::new(dir.join("one.jsonl"), 1);
    one.record(&snapshot("work", "provider_reported", 1, 1.0)).unwrap();
    one.record(&snapshot("work", "provider_reported", 2, 2.0)).unwrap();
    let kept = one.load();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].windows[0].used_percent, Some(2.0));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_replacement_keeps_the_original_and_a_later_record_succeeds() {
    let dir = dir("failed_replace");
    let path = dir.join("history.jsonl");
    let store = HistoryStore::new(path.clone(), 10);
    store.record(&snapshot("work", "provider_reported", 1, 1.0)).unwrap();
    let before = fs::read(&path).unwrap();

    // A directory that refuses new files makes the replacement fail.
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
    let failed = store.record(&snapshot("work", "provider_reported", 2, 2.0));
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();

    if failed.is_ok() {
        // Running with privileges that ignore directory permissions (root).
        let _ = fs::remove_dir_all(&dir);
        return;
    }
    assert_eq!(fs::read(&path).unwrap(), before, "the original is byte-for-byte intact");
    store.record(&snapshot("work", "provider_reported", 3, 3.0)).unwrap();
    assert_eq!(store.load().len(), 2);
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != "history.jsonl" && name != "history.jsonl.lock")
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn record_all_appends_a_batch_in_order_and_respects_the_bound() {
    let dir = dir("batch");
    let store = HistoryStore::new(dir.join("history.jsonl"), 3);
    store.record_all(&[]).unwrap();
    assert!(store.load().is_empty(), "an empty batch records nothing");

    store.record_all(&[snapshot("a", "provider_reported", 1, 1.0), snapshot("b", "provider_reported", 2, 2.0)]).unwrap();
    store.record_all(&[snapshot("c", "provider_reported", 3, 3.0), snapshot("d", "provider_reported", 4, 4.0)]).unwrap();

    let accounts: Vec<String> = store.load().into_iter().map(|r| r.account).collect();
    assert_eq!(accounts, ["b", "c", "d"], "oldest dropped beyond the bound");
    let _ = fs::remove_dir_all(&dir);
}
