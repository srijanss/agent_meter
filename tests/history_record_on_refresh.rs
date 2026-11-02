//! A completed refresh records each reported account into the configured
//! history; recording problems are a notice, never a failed refresh.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::failure::CollectorError;
use agent_meter::history::HistoryStore;
use agent_meter::snapshot::{parse_snapshot, Snapshot};

fn dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_rec_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn config(dir: &Path, history: &str) -> PathBuf {
    let path = dir.join("config.toml");
    fs::write(
        &path,
        format!("{history}\n[accounts.work]\nprovider = \"claude\"\n[accounts.home]\nprovider = \"claude\"\n"),
    )
    .unwrap();
    path
}

fn snapshot(name: &str) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"{name}","plan_label":"Pro",
        "collected_at":"2026-10-14T01:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":10.0,"resets_at":"2026-10-14T23:00:00Z",
        "source":"provider_reported"}}],"bonus_resets":null}}"#
    ))
    .unwrap()
}

fn ok_jobs() -> JobFactory {
    Box::new(|account| {
        let name = account.name.clone();
        Box::new(move || Ok(snapshot(&name)))
    })
}

fn failing_jobs() -> JobFactory {
    Box::new(|_| Box::new(|| Err(CollectorError::Unavailable)))
}

fn run_refresh(config: PathBuf, jobs: JobFactory) -> App {
    let mut app = App::new(config, Path::new("/home/u"), jobs, Duration::from_secs(1)).unwrap();
    let now = Instant::now();
    app.refresh(now);
    for _ in 0..200 {
        app.poll(now);
        if !app.refreshing() {
            return app;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("collection did not finish");
}

#[test]
fn each_reported_account_is_recorded_once_per_completed_refresh() {
    let dir = dir("on");
    let history = dir.join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    let app = run_refresh(cfg, ok_jobs());

    let mut accounts: Vec<String> =
        HistoryStore::new(history, 100).load().into_iter().map(|r| r.account).collect();
    accounts.sort();
    assert_eq!(accounts, ["home", "work"]);
    assert_eq!(app.notice(), None);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_recorded_when_history_is_off_or_collection_fails() {
    let dir = dir("off");
    let app = run_refresh(config(&dir, ""), ok_jobs());
    assert_eq!(app.notice(), None);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "only the config file exists");

    let history = dir.join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    run_refresh(cfg, failing_jobs());
    assert!(HistoryStore::new(history, 100).load().is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_recording_failure_is_a_fixed_notice_and_the_refresh_still_shows_its_data() {
    let dir = dir("fail");
    // The history path sits below a regular file, so it can never be written.
    fs::write(dir.join("blocker"), "x").unwrap();
    let bad = dir.join("blocker").join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", bad.display()));
    let app = run_refresh(cfg, ok_jobs());

    assert!(app.entries().iter().all(|e| e.snapshot.is_some()), "data shown despite the failure");
    let notice = app.notice().expect("a notice reports the recording failure");
    assert!(notice.contains("history"), "{notice}");
    assert!(!notice.contains("blocker"), "no path in the notice: {notice}");
    let _ = fs::remove_dir_all(&dir);
}
