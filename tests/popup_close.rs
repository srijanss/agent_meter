use agent_meter::input::{action_for, Action};
use agent_meter::popup::render_popup;
use chrono::{TimeZone, Utc};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;

const AREA: Rect = Rect { x: 0, y: 0, width: 48, height: 20 };

fn render() -> Buffer {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    terminal.draw(|frame| render_popup(frame, &[], now)).unwrap();
    terminal.backend().buffer().clone()
}

/// Where a symbol is drawn on the header row, found from the rendered buffer.
fn find(buf: &Buffer, symbol: &str) -> (u16, u16) {
    (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .find(|&(x, y)| buf[(x, y)].symbol() == symbol)
        .unwrap_or_else(|| panic!("{symbol} is not drawn"))
}

fn click(button: MouseButton, (column, row): (u16, u16)) -> Event {
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
fn popup_closes_on_the_close_button_and_on_escape_only() {
    let buf = render();
    let close = find(&buf, "×");
    let refresh = find(&buf, "↻");
    let title = find(&buf, "A");

    // A left click on the close button closes, including its padding.
    assert_eq!(action_for(&click(MouseButton::Left, close), AREA), Some(Action::Close));
    for dx in [-1i32, 1] {
        let edge = ((close.0 as i32 + dx) as u16, close.1);
        assert_eq!(action_for(&click(MouseButton::Left, edge), AREA), Some(Action::Close), "{edge:?}");
    }

    // Esc closes.
    assert_eq!(action_for(&key(KeyCode::Esc, KeyEventKind::Press), AREA), Some(Action::Close));

    // Nothing else does: other buttons, other areas, other mouse buttons, other keys.
    assert_eq!(action_for(&click(MouseButton::Left, refresh), AREA), None, "refresh is not close");
    assert_eq!(action_for(&click(MouseButton::Left, title), AREA), None, "title is not close");
    assert_eq!(action_for(&click(MouseButton::Left, (close.0, close.1 + 1)), AREA), None);
    assert_eq!(action_for(&click(MouseButton::Right, close), AREA), None, "right click");
    assert_eq!(action_for(&key(KeyCode::Char('q'), KeyEventKind::Press), AREA), None);
    assert_eq!(action_for(&key(KeyCode::Enter, KeyEventKind::Press), AREA), None);
    assert_eq!(action_for(&key(KeyCode::Esc, KeyEventKind::Release), AREA), None, "release");
    assert_eq!(action_for(&key(KeyCode::Esc, KeyEventKind::Repeat), AREA), None, "repeat");
    let up = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: close.0,
        row: close.1,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(action_for(&up, AREA), None, "button release");
    assert_eq!(action_for(&Event::Resize(10, 10), AREA), None);
}
