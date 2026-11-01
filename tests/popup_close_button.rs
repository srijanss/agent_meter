use agent_meter::popup::{close_button, render_popup};
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;

fn drawn(area: Rect, symbol: &str) -> Option<Position> {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|frame| render_popup(frame, &[], now)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| Position::new(x, y)))
        .find(|&p| buf[p].symbol() == symbol)
}

#[test]
fn close_button_rect_is_exactly_where_the_close_button_is_drawn() {
    for width in 4..=70 {
        for height in 3..=12 {
            let area = Rect::new(0, 0, width, height);
            let rect = close_button(area);
            assert!(area.contains(Position::new(rect.x, rect.y)) || rect.is_empty(), "{area:?}");
            assert_eq!(rect.intersection(area), rect, "inside the popup at {width}x{height}");

            match drawn(area, "×") {
                Some(cross) => {
                    assert!(rect.contains(cross), "× at {cross:?} is in {rect:?} ({width}x{height})");
                    assert_eq!((rect.width, rect.height), (3, 1), "button is ' × ' on one row");
                    if let Some(refresh) = drawn(area, "↻") {
                        assert!(!rect.contains(refresh), "refresh is outside the close rect");
                    }
                }
                None => assert!(rect.is_empty(), "nothing drawn, nothing clickable ({width}x{height})"),
            }
        }
    }
}

#[test]
fn close_button_follows_the_popup_origin() {
    let at_origin = close_button(Rect::new(0, 0, 40, 10));
    let moved = close_button(Rect::new(5, 3, 40, 10));
    assert_eq!((moved.x - 5, moved.y - 3), (at_origin.x, at_origin.y));
}
