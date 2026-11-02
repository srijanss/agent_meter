use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::failure::CollectorError;
use agent_meter::popup::render_popup_scrolled;
use chrono::{TimeZone, Utc};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

const AREA: Rect = Rect { x: 0, y: 0, width: 40, height: 10 };

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

fn idle_jobs() -> JobFactory {
    Box::new(|_| Box::new(|| Err(CollectorError::Unavailable)))
}

fn app(tag: &str) -> App {
    let dir = std::env::temp_dir().join(format!("agent_meter_scroll_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    let mut config = String::new();
    for n in 1..=8 {
        config.push_str(&format!("[accounts.acct-{n}]\nprovider = \"claude\"\n\n"));
    }
    fs::write(&path, config).unwrap();
    App::new(path, Path::new("/home/u"), idle_jobs(), Duration::from_secs(30)).unwrap()
}

fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    terminal
        .draw(|frame| {
            render_popup_scrolled(
                frame,
                app.entries(),
                now(),
                false,
                None,
                app.notice(),
                app.scroll(),
            )
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn wheel(kind: MouseEventKind) -> Event {
    Event::Mouse(MouseEvent { kind, column: 5, row: 5, modifiers: KeyModifiers::NONE })
}

fn key(code: KeyCode) -> Event {
    let mut event = KeyEvent::new(code, KeyModifiers::NONE);
    event.kind = KeyEventKind::Press;
    Event::Key(event)
}

fn send(app: &mut App, event: Event) {
    app.handle(&event, AREA, now(), Instant::now()).unwrap();
}

#[test]
fn content_taller_than_the_popup_scrolls_with_wheel_and_keys_and_stops_at_the_ends() {
    let mut app = app("all");
    assert!(screen(&app).contains("acct-1"));
    assert!(!screen(&app).contains("acct-8"), "does not fit at first");

    // Wheel down far past the end: the last account shows, and it clamps.
    for _ in 0..40 {
        send(&mut app, wheel(MouseEventKind::ScrollDown));
    }
    let bottom = screen(&app);
    assert!(bottom.contains("acct-8"), "{bottom}");
    assert!(!bottom.contains("acct-1"), "{bottom}");
    let end = app.scroll();
    send(&mut app, wheel(MouseEventKind::ScrollDown));
    assert_eq!(app.scroll(), end, "cannot scroll past the last row");

    // Keyboard: PageUp moves up by a page, Down/Up by a line, and the top clamps.
    send(&mut app, key(KeyCode::PageUp));
    assert!(app.scroll() < end);
    send(&mut app, key(KeyCode::Up));
    send(&mut app, key(KeyCode::Down));
    for _ in 0..10 {
        send(&mut app, key(KeyCode::PageUp));
    }
    assert_eq!(app.scroll(), 0);
    send(&mut app, wheel(MouseEventKind::ScrollUp));
    assert_eq!(app.scroll(), 0, "cannot scroll above the first row");
    send(&mut app, key(KeyCode::PageDown));
    assert!(app.scroll() > 0);
    assert!(screen(&app).contains('│'), "the border and header stay drawn");
}
