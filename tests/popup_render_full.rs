//! `render_popup_full` is the one render path taking every popup input at once:
//! refresh state, hover, notice, scroll and the header's busiest weekday.

use agent_meter::popup::{render_popup_busiest, render_popup_full, render_popup_scrolled};
use chrono::{TimeZone, Utc, Weekday};
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;

fn draw(f: impl Fn(&mut ratatui::Frame)) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
    terminal.draw(|frame| f(frame)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..8).map(|y| (0..60).map(|x| buf[(x, y)].symbol()).collect()).collect()
}

#[test]
fn the_full_render_matches_the_narrower_ones_for_the_inputs_they_share() {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let hover = Some(Position::new(2, 3));

    let full = draw(|f| render_popup_full(f, &[], now, true, hover, Some("note"), 2, None));
    let scrolled = draw(|f| render_popup_scrolled(f, &[], now, true, hover, Some("note"), 2));
    assert_eq!(full, scrolled);

    let full = draw(|f| render_popup_full(f, &[], now, false, None, None, 0, Some(Weekday::Tue)));
    let busiest = draw(|f| render_popup_busiest(f, &[], now, Some(Weekday::Tue)));
    assert_eq!(full, busiest);
    assert!(full[1].contains("BUSIEST DAY: Tuesday"), "{:?}", full[1]);
}
