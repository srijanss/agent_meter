use agent_meter::popup::render_popup;
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;

fn render(width: u16, height: u16) -> Buffer {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_popup(frame, &[], now)).unwrap();
    terminal.backend().buffer().clone()
}

fn line(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

#[test]
fn popup_has_an_accent_border_and_a_header_with_refresh_and_close() {
    let buf = render(48, 10);

    let accent = buf[(0, 0)].fg;
    assert_ne!(accent, Color::Reset, "the border uses an accent color");
    for (x, y) in [(47, 0), (0, 9), (47, 9)] {
        assert_eq!(buf[(x, y)].fg, accent, "all four corners share the accent");
    }

    let header = line(&buf, 1);
    assert!(header.contains("Agents"), "title: {header:?}");
    let refresh = header.find('↻').expect("refresh button");
    let close = header.find('×').expect("close button");
    assert!(header.find("Agents").unwrap() < refresh, "title is left of the buttons");
    assert!(refresh < close, "refresh is left of close");
    assert!(header.trim_end_matches('│').trim_end().ends_with('×'), "close is right-most");
}
