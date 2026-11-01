use std::path::Path;

use agent_meter::account::parse_accounts;
use agent_meter::input::{action_in, Action};
use agent_meter::popup::{render_popup, use_links, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{DateTime, TimeZone, Utc};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;

const CONFIG: &str = r#"
[accounts.claude-a]
provider = "claude"
active = true

[accounts.claude-b]
provider = "claude"
home = "/data/b"

[accounts.claude-c]
provider = "claude"
home = "/data/c"

[accounts.codex-x]
provider = "codex"
active = true
home = "/data/x"

[accounts.codex-y]
provider = "codex"
home = "/data/y"
"#;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

fn snapshot(provider: &str, windows: usize) -> agent_meter::snapshot::Snapshot {
    let windows: Vec<String> = (0..windows)
        .map(|i| {
            format!(
                r#"{{"id":"w{i}","label":"Win{i}","used_percent":10.0,"resets_at":"2026-10-14T05:00:00Z","source":"provider_reported"}}"#
            )
        })
        .collect();
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"{provider}","profile_id":"p","plan_label":"Pro",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{}],"bonus_resets":null}}"#,
        windows.join(",")
    ))
    .unwrap()
}

/// Entries for the config; `windows` gives each account's window count in
/// config order (`None` = no snapshot yet).
fn entries(windows: [Option<usize>; 5]) -> Vec<AccountEntry> {
    parse_accounts(CONFIG, Path::new("/home/u"))
        .unwrap()
        .into_iter()
        .zip(windows)
        .map(|(account, count)| {
            let snapshot = count.map(|n| snapshot(if account.name.starts_with("codex") { "codex" } else { "claude" }, n));
            AccountEntry { account, snapshot }
        })
        .collect()
}

fn render(entries: &[AccountEntry], area: Rect) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|frame| render_popup(frame, entries, now())).unwrap();
    terminal.backend().buffer().clone()
}

/// Every place the word "Use" is drawn, read cell by cell from the buffer.
fn drawn_uses(buf: &Buffer) -> Vec<(u16, u16)> {
    let mut found = Vec::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width.saturating_sub(2) {
            if buf[(x, y)].symbol() == "U" && buf[(x + 1, y)].symbol() == "s" && buf[(x + 2, y)].symbol() == "e" {
                found.push((x, y));
            }
        }
    }
    found
}

fn names(area: Rect, entries: &[AccountEntry]) -> Vec<String> {
    use_links(area, entries, now()).into_iter().map(|l| l.account).collect()
}

fn key(c: char) -> Event {
    let mut event = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
    event.kind = KeyEventKind::Press;
    Event::Key(event)
}

fn click(column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

#[test]
fn links_are_ordered_across_providers_and_follow_the_drawn_rows_as_snapshots_change() {
    let tall = Rect::new(0, 0, 48, 40);
    // Different window counts move the Use rows; links must track what is drawn.
    for windows in [
        [None, None, None, None, None],
        [Some(2), Some(1), Some(3), Some(2), Some(1)],
        [Some(0), Some(3), None, Some(1), Some(2)],
    ] {
        let entries = entries(windows);
        let links = use_links(tall, &entries, now());
        let drawn = drawn_uses(&render(&entries, tall));

        assert_eq!(
            links.iter().map(|l| l.account.as_str()).collect::<Vec<_>>(),
            ["claude-b", "claude-c", "codex-y"],
            "{windows:?}"
        );
        assert_eq!(
            links.iter().map(|l| (l.area.x, l.area.y)).collect::<Vec<_>>(),
            drawn,
            "links sit exactly on the drawn Use labels for {windows:?}"
        );

        // Digits pick links top to bottom; a number past the last link picks nothing.
        for (i, expected) in ["claude-b", "claude-c", "codex-y"].iter().enumerate() {
            let digit = char::from_digit(i as u32 + 1, 10).unwrap();
            assert_eq!(
                action_in(&key(digit), tall, &entries, now()),
                Some(Action::Use((*expected).into()))
            );
        }
        assert_eq!(action_in(&key('4'), tall, &entries, now()), None);
        assert_eq!(action_in(&key('9'), tall, &entries, now()), None);
    }
}

#[test]
fn a_popup_at_a_nonzero_origin_has_its_links_shifted_with_it() {
    let entries = entries([Some(2), Some(1), Some(1), Some(1), Some(1)]);
    let at_origin = use_links(Rect::new(0, 0, 48, 40), &entries, now());
    let moved = use_links(Rect::new(7, 3, 48, 40), &entries, now());
    assert_eq!(at_origin.len(), moved.len());
    assert!(!moved.is_empty());
    for (a, m) in at_origin.iter().zip(&moved) {
        assert_eq!(a.account, m.account);
        assert_eq!((m.area.x, m.area.y), (a.area.x + 7, a.area.y + 3));
    }
    let first = &moved[0];
    assert_eq!(
        action_in(&click(first.area.x, first.area.y), Rect::new(7, 3, 48, 40), &entries, now()),
        Some(Action::Use(first.account.clone()))
    );
    // The same cell is not a link when the popup sits at the origin.
    assert_eq!(action_in(&click(first.area.x, first.area.y), Rect::new(0, 0, 48, 40), &entries, now()), None);
}

#[test]
fn links_below_the_visible_body_cannot_be_selected() {
    let entries = entries([Some(1), Some(1), Some(1), Some(1), Some(1)]);
    let full = Rect::new(0, 0, 48, 40);
    let all = names(full, &entries);
    assert_eq!(all.len(), 3);

    for height in 3..=16 {
        let area = Rect::new(0, 0, 48, height);
        let visible = names(area, &entries);
        let drawn = drawn_uses(&render(&entries, area));
        assert_eq!(visible.len(), drawn.len(), "height {height}: links are exactly the drawn labels");
        assert_eq!(visible, all[..visible.len()], "height {height}: clipping only removes the bottom links");

        // Digits beyond the visible links do nothing, and a hidden link's row is not clickable.
        let hidden = all.len() - visible.len();
        for n in (visible.len() + 1)..=all.len() {
            let digit = char::from_digit(n as u32, 10).unwrap();
            assert_eq!(action_in(&key(digit), area, &entries, now()), None, "height {height} digit {n}");
        }
        if hidden > 0 {
            let hidden_row = use_links(full, &entries, now())[visible.len()].area;
            assert_eq!(action_in(&click(hidden_row.x, hidden_row.y), area, &entries, now()), None);
        }
    }
}

#[test]
fn a_body_too_narrow_to_show_use_has_no_links() {
    let entries = entries([Some(1), Some(1), Some(1), Some(1), Some(1)]);
    // Width 5 leaves a 3-column body, exactly the label; narrower leaves none.
    let exact = Rect::new(0, 0, 5, 40);
    let links = use_links(exact, &entries, now());
    assert_eq!(links.len(), 3);
    assert!(links.iter().all(|l| l.area.width == 3 && l.area.x == 1));
    for width in 0..5 {
        let narrow = Rect::new(0, 0, width, 40);
        assert!(use_links(narrow, &entries, now()).is_empty(), "width {width}");
        assert_eq!(action_in(&key('1'), narrow, &entries, now()), None, "width {width}");
    }
}
