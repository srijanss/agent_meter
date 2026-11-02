use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_meter::account::parse_accounts;
use agent_meter::app::{App, JobFactory};
use agent_meter::failure::CollectorError;
use agent_meter::popup::{render_popup_status, AccountEntry};
use chrono::{TimeZone, Utc};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;

const CONFIG: &str = "[accounts.claude-a]\nprovider = \"claude\"\nactive = true\n\n[accounts.claude-b]\nprovider = \"claude\"\nhome = \"/data/b\"\n";
const AREA: Rect = Rect { x: 0, y: 0, width: 48, height: 14 };

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

fn entries() -> Vec<AccountEntry> {
    parse_accounts(CONFIG, Path::new("/home/u"))
        .unwrap()
        .into_iter()
        .map(|account| AccountEntry { account, snapshot: None })
        .collect()
}

fn draw(width: u16, height: u16, notice: Option<&str>) -> Buffer {
    let entries = entries();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| render_popup_status(frame, &entries, now(), false, None, notice))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn rows(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

#[test]
fn a_notice_is_the_popups_bottom_row_inside_its_border_and_never_without_one() {
    let plain = rows(&draw(48, 14, None));
    assert!(!plain.iter().any(|r| r.contains("Could not")));

    let with = rows(&draw(48, 14, Some("Could not switch account")));
    let last_inside = &with[12];
    assert!(last_inside.contains("Could not switch account"), "{with:?}");
    assert!(last_inside.starts_with('│') && last_inside.ends_with('│'), "{last_inside:?}");
    assert!(with[13].starts_with('└'), "the border stays intact: {with:?}");
    // The accounts above are still drawn.
    assert!(with.iter().any(|r| r.contains("claude-a")));
}

#[test]
fn a_notice_is_clipped_to_the_popup_and_control_characters_are_not_drawn() {
    let narrow = rows(&draw(20, 14, Some("Could not switch account to the other one")));
    assert!(narrow[12].contains("Could not"), "{narrow:?}");
    assert!(narrow.iter().all(|r| r.chars().count() == 20));

    let hostile = rows(&draw(48, 14, Some("bad\u{1b}[2Jtext\nnext")));
    assert!(hostile.iter().all(|r| !r.contains('\u{1b}') && !r.contains('\n')), "{hostile:?}");
    assert!(hostile[12].contains("bad"), "{hostile:?}");

    // Tiny frames never panic.
    for (w, h) in [(0, 0), (1, 1), (2, 2), (5, 3), (10, 4)] {
        let _ = draw(w, h, Some("Could not switch account"));
    }
}

fn idle_jobs() -> JobFactory {
    Box::new(|_| Box::new(|| Err(CollectorError::Unavailable)))
}

fn key(code: KeyCode) -> Event {
    let mut event = KeyEvent::new(code, KeyModifiers::NONE);
    event.kind = KeyEventKind::Press;
    Event::Key(event)
}

#[test]
fn a_failed_switch_leaves_a_notice_without_config_values_and_the_next_action_clears_it() {
    let dir = std::env::temp_dir().join(format!("agent_meter_notice_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    fs::write(&path, CONFIG).unwrap();
    let mut app = App::new(path.clone(), Path::new("/home/u"), idle_jobs(), Duration::from_secs(30)).unwrap();
    let t0 = Instant::now();
    assert_eq!(app.notice(), None);

    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("\n[accounts.broken]\nprovider = \"claude\"\nunsupported = \"SECRETVALUE\"\n");
    fs::write(&path, text).unwrap();

    assert!(app.handle(&key(KeyCode::Char('1')), AREA, now(), t0).is_err());
    let notice = app.notice().expect("the failure is shown").to_string();
    assert!(notice.contains("Could not switch"), "{notice}");
    assert!(!notice.contains("SECRETVALUE") && !notice.contains(dir.to_str().unwrap()), "{notice}");
    assert_eq!(app.entries().len(), 2, "state is intact");

    // Keys that do nothing leave it; the next real action clears it.
    app.handle(&key(KeyCode::Char('z')), AREA, now(), t0).unwrap();
    assert!(app.notice().is_some());
    app.handle(&key(KeyCode::Char('r')), AREA, now(), t0).unwrap();
    assert_eq!(app.notice(), None);
}
