//! The busiest weekday is the viewer's local one: in a zone ahead of UTC by
//! more than twelve hours, midday UTC already belongs to the next day.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::history::HistoryStore;
use agent_meter::snapshot::parse_snapshot;
use chrono::Weekday;

#[test]
fn midday_utc_counts_as_the_next_weekday_at_utc_plus_14() {
    // The only test in this binary, so changing the zone affects nothing else.
    std::env::set_var("TZ", "Pacific/Kiritimati");
    let dir = std::env::temp_dir().join(format!("agent_meter_far_east_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let history = dir.join("h.jsonl");
    let monday_noon = parse_snapshot(
        r#"{"schema_version":1,"provider":"claude","profile_id":"work","plan_label":null,
        "collected_at":"2026-10-12T12:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{"id":"s","label":"Session","used_percent":30.0,"resets_at":null,
        "source":"provider_reported"}],"bonus_resets":null}"#,
    )
    .unwrap();
    HistoryStore::new(history.clone(), 100).record(&monday_noon).unwrap();
    let config = dir.join("config.toml");
    fs::write(
        &config,
        format!("[history]\npath = \"{}\"\n\n[accounts.work]\nprovider = \"claude\"\n", history.display()),
    )
    .unwrap();
    let jobs: JobFactory = Box::new(|_| Box::new(|| Err(agent_meter::failure::CollectorError::Unavailable)));
    let mut app = App::new(config, Path::new("/home/u"), jobs, Duration::from_secs(1)).unwrap();
    for _ in 0..400 {
        app.poll(Instant::now());
        if app.busiest_day().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(app.busiest_day(), Some(Weekday::Tue));
    let _ = fs::remove_dir_all(&dir);
}
