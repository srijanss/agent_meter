//! The running popup loads the configured history, works out the busiest
//! weekday in the viewer's local zone and shows it in the header, with the rest
//! of the popup unchanged; no history means no subtitle.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::history::HistoryStore;
use agent_meter::popup::{render_popup_full, render_popup_scrolled};
use agent_meter::snapshot::{parse_snapshot, Snapshot};
use chrono::{TimeZone, Utc, Weekday};
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;

fn dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_busy_wire_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn config(dir: &Path, history: &str) -> PathBuf {
    let path = dir.join("config.toml");
    fs::write(&path, format!("{history}\n[accounts.work]\nprovider = \"claude\"\n")).unwrap();
    path
}

/// Snapshots stamped at midday UTC, which is the same weekday in every zone
/// from UTC-11 to UTC+11, so the expected weekday does not depend on the machine.
fn snapshot_at(day: &str, percent: f64) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"work","plan_label":null,
        "collected_at":"{day}T12:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":{percent},"resets_at":null,
        "source":"provider_reported"}}],"bonus_resets":null}}"#
    ))
    .unwrap()
}

fn jobs(day: &'static str, percent: f64) -> JobFactory {
    Box::new(move |_| Box::new(move || Ok(snapshot_at(day, percent))))
}

fn app(config: PathBuf, jobs: JobFactory) -> App {
    App::new(config, Path::new("/home/u"), jobs, Duration::from_secs(1)).unwrap()
}

fn wait_until(app: &mut App, mut done: impl FnMut(&App) -> bool) {
    let now = Instant::now();
    for _ in 0..400 {
        app.poll(now);
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the awaited effect never arrived");
}

fn rows(app: &App, busiest: Option<Weekday>, scroll: usize, hover: Option<Position>, notice: Option<&str>) -> Vec<String> {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(70, 12)).unwrap();
    terminal
        .draw(|frame| render_popup_full(frame, app.entries(), now, false, hover, notice, scroll, busiest))
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..12).map(|y| (0..70).map(|x| buf[(x, y)].symbol()).collect()).collect()
}

fn plain_rows(app: &App, scroll: usize, hover: Option<Position>, notice: Option<&str>) -> Vec<String> {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(70, 12)).unwrap();
    terminal
        .draw(|frame| render_popup_scrolled(frame, app.entries(), now, false, hover, notice, scroll))
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..12).map(|y| (0..70).map(|x| buf[(x, y)].symbol()).collect()).collect()
}

#[test]
fn the_popup_names_the_busiest_weekday_of_the_configured_history_and_changes_nothing_else() {
    let dir = dir("on");
    let history = dir.join("h.jsonl");
    let store = HistoryStore::new(history.clone(), 100);
    // Friday peaks at 60, Monday at 20.
    store.record(&snapshot_at("2026-10-16", 60.0)).unwrap();
    store.record(&snapshot_at("2026-10-12", 20.0)).unwrap();
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));

    let mut app = app(cfg, jobs("2026-10-13", 1.0));
    wait_until(&mut app, |a| a.busiest_day() == Some(Weekday::Fri));

    let with = rows(&app, app.busiest_day(), 0, Some(Position::new(3, 4)), Some("a notice"));
    let without = plain_rows(&app, 0, Some(Position::new(3, 4)), Some("a notice"));
    assert!(with[1].contains("BUSIEST DAY: Friday"), "{:?}", with[1]);
    assert!(with[1].contains("Agents") && with[1].contains('×'), "{:?}", with[1]);
    for y in (0..12).filter(|&y| y != 1) {
        assert_eq!(with[y], without[y], "row {y} is unchanged by the subtitle");
    }
    // Scroll, hover and notice still render through the same path.
    assert!(with.iter().any(|r| r.contains("a notice")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn new_records_from_a_refresh_can_change_the_busiest_day() {
    let dir = dir("update");
    let history = dir.join("h.jsonl");
    HistoryStore::new(history.clone(), 100).record(&snapshot_at("2026-10-12", 5.0)).unwrap();
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));

    // The refresh reports a heavier Wednesday than the seeded Monday.
    let mut app = app(cfg, jobs("2026-10-14", 70.0));
    wait_until(&mut app, |a| a.busiest_day() == Some(Weekday::Mon));
    app.refresh(Instant::now());
    wait_until(&mut app, |a| a.busiest_day() == Some(Weekday::Wed));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn with_history_off_or_empty_the_header_is_unchanged() {
    let dir = dir("none");
    let mut off = app(config(&dir, ""), jobs("2026-10-14", 70.0));
    off.refresh(Instant::now());
    wait_until(&mut off, |a| !a.refreshing());
    assert_eq!(off.busiest_day(), None);
    assert_eq!(rows(&off, off.busiest_day(), 0, None, None), plain_rows(&off, 0, None, None));

    // History on but nothing recorded: after loading, still nothing to show.
    let history = dir.join("empty.jsonl");
    fs::write(&history, "").unwrap();
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    let mut empty = app(cfg, Box::new(|_| Box::new(|| Err(agent_meter::failure::CollectorError::Unavailable))));
    std::thread::sleep(Duration::from_millis(100));
    empty.poll(Instant::now());
    assert_eq!(empty.busiest_day(), None);
    assert_eq!(rows(&empty, empty.busiest_day(), 0, None, None), plain_rows(&empty, 0, None, None));
    let _ = fs::remove_dir_all(&dir);
}
