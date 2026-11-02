//! The app works out the busiest weekday of its configured history in the
//! background, so reading it never touches the history file.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::history::HistoryStore;
use agent_meter::snapshot::{parse_snapshot, Snapshot};
use chrono::Weekday;

fn dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_appbusy_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Midday UTC is the same weekday in every zone from UTC-11 to UTC+11.
fn snapshot_at(day: &str, percent: f64) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"work","plan_label":null,
        "collected_at":"{day}T12:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":{percent},"resets_at":null,
        "source":"provider_reported"}}],"bonus_resets":null}}"#
    ))
    .unwrap()
}

fn app(dir: &Path, history: Option<&Path>, report: (&'static str, f64)) -> App {
    let table = history.map(|p| format!("[history]\npath = \"{}\"\n", p.display())).unwrap_or_default();
    let config = dir.join("config.toml");
    fs::write(&config, format!("{table}\n[accounts.work]\nprovider = \"claude\"\n")).unwrap();
    let jobs: JobFactory = Box::new(move |_| Box::new(move || Ok(snapshot_at(report.0, report.1))));
    App::new(config, Path::new("/home/u"), jobs, Duration::from_secs(1)).unwrap()
}

fn wait_until(app: &mut App, mut done: impl FnMut(&App) -> bool) {
    for _ in 0..400 {
        app.poll(Instant::now());
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the awaited effect never arrived");
}

#[test]
fn the_busiest_day_comes_from_the_history_and_follows_new_records() {
    let dir = dir("follow");
    let history = dir.join("h.jsonl");
    let store = HistoryStore::new(history.clone(), 100);
    store.record(&snapshot_at("2026-10-12", 5.0)).unwrap();

    let mut app = app(&dir, Some(&history), ("2026-10-14", 70.0));
    assert_eq!(app.busiest_day(), None, "nothing is known until the history has been read");
    wait_until(&mut app, |a| a.busiest_day() == Some(Weekday::Mon));
    app.refresh(Instant::now());
    wait_until(&mut app, |a| a.busiest_day() == Some(Weekday::Wed));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_history_or_an_empty_one_has_no_busiest_day() {
    let dir = dir("none");
    let mut off = app(&dir, None, ("2026-10-14", 70.0));
    off.refresh(Instant::now());
    wait_until(&mut off, |a| !a.refreshing());
    assert_eq!(off.busiest_day(), None);

    let empty = dir.join("empty.jsonl");
    fs::write(&empty, "").unwrap();
    let mut app = app(&dir, Some(&empty), ("2026-10-14", 0.0));
    std::thread::sleep(Duration::from_millis(100));
    app.poll(Instant::now());
    assert_eq!(app.busiest_day(), None);
    let _ = fs::remove_dir_all(&dir);
}
