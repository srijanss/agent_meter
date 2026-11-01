use std::time::{Duration, Instant};

use agent_meter::input::{action_for, Action};
use agent_meter::popup::{refresh_button, render_popup_with};
use agent_meter::refresh::{accounts_to_refresh, RefreshGate};
use chrono::{TimeZone, Utc};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;

const AREA: Rect = Rect { x: 0, y: 0, width: 48, height: 20 };

fn render(refreshing: bool) -> Buffer {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    terminal.draw(|frame| render_popup_with(frame, &[], now, refreshing)).unwrap();
    terminal.backend().buffer().clone()
}

fn header_text(buf: &Buffer) -> String {
    (0..buf.area.width).map(|x| buf[(x, 1)].symbol()).collect()
}

fn click(button: MouseButton, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(button),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn key(code: KeyCode, kind: KeyEventKind) -> Event {
    let mut event = KeyEvent::new(code, KeyModifiers::NONE);
    event.kind = kind;
    Event::Key(event)
}

#[test]
fn the_refresh_button_and_r_key_request_a_refresh_and_nothing_else_does() {
    let button = refresh_button(AREA);
    assert!(button.width > 0);
    let glyph = header_text(&render(false)).chars().position(|c| c == '↻').expect("refresh glyph is drawn") as u16;
    assert!(button.contains((glyph, 1).into()), "the button covers the drawn glyph");

    for dx in [0, button.width - 1] {
        assert_eq!(
            action_for(&click(MouseButton::Left, button.x + dx, button.y), AREA),
            Some(Action::Refresh)
        );
    }
    assert_eq!(action_for(&click(MouseButton::Right, button.x, button.y), AREA), None);

    assert_eq!(action_for(&key(KeyCode::Char('r'), KeyEventKind::Press), AREA), Some(Action::Refresh));
    assert_eq!(action_for(&key(KeyCode::Char('r'), KeyEventKind::Release), AREA), None);
    assert_eq!(action_for(&key(KeyCode::Char('x'), KeyEventKind::Press), AREA), None);
}

#[test]
fn the_header_shows_an_in_progress_indicator_only_while_refreshing() {
    let idle = header_text(&render(false));
    let busy = header_text(&render(true));
    assert!(idle.contains('↻') && !idle.contains('…'), "{idle}");
    assert!(busy.contains('…') && !busy.contains('↻'), "{busy}");
    assert!(busy.contains('×'), "close stays available while refreshing");
}

#[test]
fn a_refresh_starts_only_accounts_not_in_flight_or_rate_limited() {
    let mut gate = RefreshGate::new(Duration::from_secs(30));
    let t0 = Instant::now();
    let accounts = ["a".to_string(), "b".to_string(), "c".to_string()];

    // First press starts everything, and the popup has something to show as in progress.
    assert!(!gate.any_in_flight());
    assert_eq!(accounts_to_refresh(&mut gate, &accounts, t0), accounts.to_vec());
    assert!(gate.any_in_flight());
    // A second press while all are in flight starts nothing.
    assert!(accounts_to_refresh(&mut gate, &accounts, t0).is_empty());

    // "a" finishes; right after, it is rate limited, so still nothing starts.
    gate.finish("a", t0);
    assert!(accounts_to_refresh(&mut gate, &accounts, t0 + Duration::from_secs(1)).is_empty());
    // Once the interval has passed, only "a" can start (b and c are still running).
    assert_eq!(
        accounts_to_refresh(&mut gate, &accounts, t0 + Duration::from_secs(31)),
        vec!["a".to_string()]
    );
}
