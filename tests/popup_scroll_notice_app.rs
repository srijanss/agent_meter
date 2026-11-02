use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::failure::CollectorError;
use chrono::{TimeZone, Utc};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;

const AREA: Rect = Rect { x: 0, y: 0, width: 40, height: 10 };

fn idle_jobs() -> JobFactory {
    Box::new(|_| Box::new(|| Err(CollectorError::Unavailable)))
}

fn key(code: KeyCode) -> Event {
    let mut event = KeyEvent::new(code, KeyModifiers::NONE);
    event.kind = KeyEventKind::Press;
    Event::Key(event)
}

#[test]
fn the_use_link_under_a_switch_failure_notice_cannot_be_chosen() {
    let dir = std::env::temp_dir().join(format!("agent_meter_notice_app_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    let mut config = String::new();
    for n in 1..=8 {
        config.push_str(&format!("[accounts.acct-{n}]\nprovider = \"claude\"\n\n"));
    }
    fs::write(&path, config).unwrap();
    let mut app = App::new(path, Path::new("/home/u"), idle_jobs(), Duration::from_secs(30)).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let send =|app: &mut App, event: Event| app.handle(&event, AREA, now, Instant::now());

    // Two lines down, the acct-3 link sits on the last body row.
    send(&mut app, key(KeyCode::Down)).unwrap();
    send(&mut app, key(KeyCode::Down)).unwrap();
    assert_eq!(app.scroll(), 2);

    // Make the config unwritable so choosing acct-2 (link 1) fails with a notice.
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    assert!(send(&mut app, key(KeyCode::Char('1'))).is_err());
    assert!(app.notice().is_some());

    // The notice now covers that last row, so link 2 is not offered.
    assert!(send(&mut app, key(KeyCode::Char('2'))).is_ok());
    assert!(app.notice().is_some(), "no action ran, so the notice stays");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
}
