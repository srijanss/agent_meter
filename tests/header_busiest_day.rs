//! The header subtitle names the busiest weekday from local usage history and
//! is simply absent when there is no history to compute it from.

use agent_meter::history::{busiest_day, HistoryRecord, HistoryStore};
use agent_meter::popup::render_popup_busiest;
use agent_meter::snapshot::parse_snapshot;
use chrono::{FixedOffset, TimeZone, Utc, Weekday};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn record(timestamp: &str, percent: f64) -> HistoryRecord {
    let path = std::env::temp_dir().join(format!("agent_meter_busy_{}_{timestamp}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let store = HistoryStore::new(path.clone(), 10);
    let snapshot = parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"work","plan_label":null,
        "collected_at":"{timestamp}","source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":{percent},"resets_at":null,
        "source":"provider_reported"}}],"bonus_resets":null}}"#
    ))
    .unwrap();
    store.record(&snapshot).unwrap();
    let loaded = store.load().remove(0);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_file_name(format!("{}.lock", path.file_name().unwrap().to_string_lossy())));
    loaded
}

#[test]
fn the_weekday_with_the_most_usage_is_the_busiest_and_no_history_has_none() {
    assert_eq!(busiest_day(&[], &Utc), None);

    // Monday peaks at 10; Wednesday peaks at 80 across two snapshots.
    let records = [
        record("2026-10-12T09:00:00Z", 10.0),
        record("2026-10-14T09:00:00Z", 30.0),
        record("2026-10-14T15:00:00Z", 80.0),
    ];
    assert_eq!(busiest_day(&records, &Utc), Some(Weekday::Wed));

    // Figures that were never reported say nothing about a busy day.
    assert_eq!(busiest_day(&[record("2026-10-12T09:00:00Z", 0.0)], &Utc), None);
}

#[test]
fn the_weekday_is_the_local_one_not_the_utc_one() {
    let records = [record("2026-10-14T23:30:00Z", 50.0)];
    assert_eq!(busiest_day(&records, &Utc), Some(Weekday::Wed));
    let ahead = FixedOffset::east_opt(2 * 3600).unwrap();
    assert_eq!(busiest_day(&records, &ahead), Some(Weekday::Thu));
}

fn header(busiest: Option<Weekday>, width: u16) -> String {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(width, 6)).unwrap();
    terminal.draw(|frame| render_popup_busiest(frame, &[], now, busiest)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..width).map(|x| buf[(x, 1)].symbol()).collect()
}

#[test]
fn the_header_shows_the_busiest_day_only_when_known_and_when_it_fits() {
    let shown = header(Some(Weekday::Wed), 60);
    assert!(shown.contains("Agents"), "{shown:?}");
    assert!(shown.contains("BUSIEST DAY: Wednesday"), "{shown:?}");
    assert!(shown.contains('↻') && shown.contains('×'), "buttons stay: {shown:?}");

    let none = header(None, 60);
    assert!(!none.contains("BUSIEST"), "{none:?}");
    assert!(none.contains("Agents"), "{none:?}");

    // Too narrow for the subtitle: it is dropped, never wrapped over the buttons.
    let narrow = header(Some(Weekday::Wed), 30);
    assert!(!narrow.contains("BUSIEST"), "{narrow:?}");
    assert!(narrow.contains("Agents") && narrow.contains('×'), "{narrow:?}");
}
