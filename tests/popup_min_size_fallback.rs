use agent_meter::input::{action_for, Action};
use agent_meter::popup::{close_button, refresh_button, render_popup};
use chrono::{TimeZone, Utc};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn header(width: u16, height: u16) -> String {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_popup(frame, &[], now)).unwrap();
    let buf = terminal.backend().buffer().clone();
    if height < 3 {
        return String::new();
    }
    (0..width).map(|x| buf[(x, 1)].symbol()).collect()
}

fn click(column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn at_any_size_the_buttons_are_whole_or_absent_and_the_keys_still_work() {
    for width in 0..=40u16 {
        for height in 0..=8u16 {
            let area = Rect::new(0, 0, width, height);
            let text = header(width, height);
            let (refresh, close) = (refresh_button(area), close_button(area));
            let drawn = text.contains('↻') && text.contains('×');

            // Buttons are drawn together, or neither is drawn nor clickable.
            assert_eq!(text.contains('↻'), text.contains('×'), "{width}x{height}: {text:?}");
            if drawn {
                for button in [refresh, close] {
                    assert_eq!((button.width, button.height, button.y), (3, 1, 1), "{width}x{height}");
                    assert!(button.right() <= width && button.bottom() <= height, "{width}x{height}");
                }
                // Each hitbox is the three cells around its drawn glyph, and they do not overlap.
                let cells: Vec<char> = text.chars().collect();
                assert_eq!(cells[refresh.x as usize + 1], '↻', "{width}x{height}: {text:?}");
                assert_eq!(cells[close.x as usize + 1], '×', "{width}x{height}: {text:?}");
                assert!(refresh.right() <= close.x, "{width}x{height}");
            } else {
                assert_eq!((refresh.area(), close.area()), (0, 0), "{width}x{height}");
            }

            // Clicks act only on the buttons; with no buttons, no click acts.
            for row in 0..height {
                for column in 0..width {
                    let expected = if !drawn {
                        None
                    } else if refresh.contains((column, row).into()) {
                        Some(Action::Refresh)
                    } else if close.contains((column, row).into()) {
                        Some(Action::Close)
                    } else {
                        None
                    };
                    assert_eq!(action_for(&click(column, row), area), expected, "{width}x{height} at ({column},{row})");
                }
            }

            // The keyboard fallbacks never depend on the size.
            assert_eq!(action_for(&key(KeyCode::Char('r')), area), Some(Action::Refresh));
            assert_eq!(action_for(&key(KeyCode::Esc), area), Some(Action::Close));
        }
    }
    // The smallest area with a header row has room for both buttons.
    assert!(header(9, 4).contains('↻') && header(9, 4).contains('×'));
}
