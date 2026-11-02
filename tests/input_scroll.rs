use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::input::{action_in, action_scrolled, Action};
use agent_meter::popup::{page_size, AccountEntry};
use chrono::{TimeZone, Utc};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

const AREA: Rect = Rect { x: 0, y: 0, width: 40, height: 10 };

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

fn entries() -> Vec<AccountEntry> {
    (1..=8)
        .map(|n| AccountEntry {
            account: Account {
                name: format!("acct-{n}"),
                provider: Provider::Claude,
                home: PathBuf::from("/x"),
                active: n == 1,
            },
            snapshot: None,
        })
        .collect()
}

fn key(code: KeyCode, kind: KeyEventKind) -> Event {
    let mut event = KeyEvent::new(code, KeyModifiers::NONE);
    event.kind = kind;
    Event::Key(event)
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE })
}

fn act(event: &Event, scroll: usize) -> Option<Action> {
    action_scrolled(event, AREA, &entries(), now(), scroll)
}

#[test]
fn wheel_and_arrow_and_page_keys_ask_to_scroll_by_lines_and_pages() {
    let press = KeyEventKind::Press;
    assert_eq!(act(&mouse(MouseEventKind::ScrollDown, 5, 5), 0), Some(Action::Scroll(3)));
    assert_eq!(act(&mouse(MouseEventKind::ScrollUp, 5, 5), 0), Some(Action::Scroll(-3)));
    assert_eq!(act(&key(KeyCode::Down, press), 0), Some(Action::Scroll(1)));
    assert_eq!(act(&key(KeyCode::Up, press), 0), Some(Action::Scroll(-1)));
    let page = page_size(AREA) as isize;
    assert_eq!(act(&key(KeyCode::PageDown, press), 0), Some(Action::Scroll(page)));
    assert_eq!(act(&key(KeyCode::PageUp, press), 0), Some(Action::Scroll(-page)));
    // Releases do not scroll a second time.
    assert_eq!(act(&key(KeyCode::Down, KeyEventKind::Release), 0), None);
}

#[test]
fn use_links_and_number_keys_follow_the_scroll_offset_and_action_in_is_unscrolled() {
    let press = KeyEventKind::Press;
    // Unscrolled, the only visible link is acct-2 on the row below the first account.
    assert_eq!(act(&key(KeyCode::Char('1'), press), 0), Some(Action::Use("acct-2".into())));
    // Scrolled by 5, the first visible link is acct-3.
    assert_eq!(act(&key(KeyCode::Char('1'), press), 5), Some(Action::Use("acct-3".into())));

    let unscrolled = agent_meter::popup::use_links_scrolled(AREA, &entries(), now(), 5);
    let link = &unscrolled[0];
    let click = mouse(
        MouseEventKind::Down(MouseButton::Left),
        link.area.x,
        link.area.y,
    );
    assert_eq!(act(&click, 5), Some(Action::Use("acct-3".into())));

    assert_eq!(
        action_in(&key(KeyCode::Char('1'), press), AREA, &entries(), now()),
        Some(Action::Use("acct-2".into()))
    );
}
