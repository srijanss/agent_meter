use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{
    max_scroll, page_size, render_popup_scrolled, render_popup_status, use_links_scrolled,
    AccountEntry,
};
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

const AREA: Rect = Rect { x: 0, y: 0, width: 40, height: 10 };

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

fn entries(count: usize) -> Vec<AccountEntry> {
    (1..=count)
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

fn draw(entries: &[AccountEntry], notice: Option<&str>, scroll: usize) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    terminal
        .draw(|f| render_popup_scrolled(f, entries, now(), false, None, notice, scroll))
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

#[test]
fn scroll_zero_is_the_unscrolled_popup_and_content_that_fits_cannot_scroll() {
    let few = entries(2);
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    terminal
        .draw(|f| render_popup_status(f, &few, now(), false, None, None))
        .unwrap();
    let plain = terminal.backend().buffer().clone();
    let expected: Vec<String> = (0..plain.area.height)
        .map(|y| (0..plain.area.width).map(|x| plain[(x, y)].symbol()).collect())
        .collect();
    assert_eq!(draw(&few, None, 0), expected);

    assert_eq!(max_scroll(AREA, &few, now(), false), 0);
    assert_eq!(draw(&few, None, 99), expected, "scrolling a short list changes nothing");
}

#[test]
fn scrolling_moves_the_body_up_keeps_header_and_border_and_clamps_at_the_end() {
    let many = entries(8);
    let max = max_scroll(AREA, &many, now(), false);
    assert!(max > 0);

    let top = draw(&many, None, 0);
    assert!(top.iter().any(|r| r.contains("acct-1")));
    assert!(!top.iter().any(|r| r.contains("acct-8")));

    let one = draw(&many, None, 1);
    assert_ne!(top, one);
    assert!(one[1].contains("Agents"), "header stays: {one:?}");

    let end = draw(&many, None, max);
    assert!(end.iter().any(|r| r.contains("acct-8")), "{end:?}");
    assert!(!end.iter().any(|r| r.contains("acct-1")));
    assert!(end[0].starts_with('┌') && end[9].starts_with('└'));
    assert_eq!(draw(&many, None, max + 50), end, "past the end clamps");

    // With a notice the bottom body row is taken, so the end is one row further.
    let with_notice_max = max_scroll(AREA, &many, now(), true);
    assert_eq!(with_notice_max, max + 1);
    let noticed = draw(&many, Some("Could not switch account"), with_notice_max);
    assert!(noticed.iter().any(|r| r.contains("acct-8")), "{noticed:?}");
    assert!(noticed[8].contains("Could not switch"));
}

#[test]
fn page_size_is_the_body_height_and_use_links_follow_what_is_visible() {
    assert_eq!(page_size(AREA), 6);
    assert_eq!(page_size(Rect::new(0, 0, 40, 0)), 1, "never zero");

    let many = entries(8);
    let top = use_links_scrolled(AREA, &many, now(), 0);
    assert_eq!(top.first().map(|l| l.account.as_str()), Some("acct-2"));

    // Scrolling by three rows moves a link up three rows and reveals the next.
    let shifted = use_links_scrolled(AREA, &many, now(), 3);
    let names: Vec<&str> = shifted.iter().map(|l| l.account.as_str()).collect();
    assert_eq!(names, ["acct-2", "acct-3"]);
    assert_eq!(shifted[0].area.y, top[0].area.y - 3);
    assert!(shifted.iter().all(|l| l.area.y >= 3 && l.area.y < 9));

    // A link scrolled out of the body is gone, so it can't be clicked or numbered.
    let later = use_links_scrolled(AREA, &many, now(), 5);
    assert_eq!(later.first().map(|l| l.account.as_str()), Some("acct-3"));

    // Links agree with what is drawn at that scroll offset.
    let screen = draw(&many, None, 3);
    for link in &shifted {
        assert!(screen[link.area.y as usize].contains("Use"), "{screen:?}");
    }
}
