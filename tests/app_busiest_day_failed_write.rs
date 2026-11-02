//! A history that cannot be written still reports its failure through the
//! notice, and never invents a busiest weekday.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::snapshot::parse_snapshot;

#[test]
fn a_failed_write_shows_the_notice_and_leaves_no_busiest_day() {
    let dir = std::env::temp_dir().join(format!("agent_meter_failed_write_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // The history's parent is a regular file, so nothing can be created under it.
    let blocker = dir.join("blocker");
    fs::write(&blocker, "x").unwrap();
    let history = blocker.join("h.jsonl");
    let config = dir.join("config.toml");
    fs::write(
        &config,
        format!("[history]\npath = \"{}\"\n\n[accounts.work]\nprovider = \"claude\"\n", history.display()),
    )
    .unwrap();
    let snapshot = parse_snapshot(
        r#"{"schema_version":1,"provider":"claude","profile_id":"work","plan_label":null,
        "collected_at":"2026-10-14T12:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{"id":"s","label":"Session","used_percent":40.0,"resets_at":null,
        "source":"provider_reported"}],"bonus_resets":null}"#,
    )
    .unwrap();
    let jobs: JobFactory = Box::new(move |_| {
        let snapshot = snapshot.clone();
        Box::new(move || Ok(snapshot))
    });
    let mut app = App::new(config, Path::new("/home/u"), jobs, Duration::from_secs(1)).unwrap();
    app.refresh(Instant::now());
    for _ in 0..400 {
        app.poll(Instant::now());
        if app.notice().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(app.notice(), Some("Could not record usage history; check the history path"));
    assert_eq!(app.busiest_day(), None);
    let _ = fs::remove_dir_all(&dir);
}
