use std::path::Path;

use agent_meter::account::parse_accounts;
use agent_meter::popup::{render_popup_hover, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{DateTime, TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;

const CONFIG: &str = "[accounts.claude-a]\nprovider = \"claude\"\nactive = true\n";

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

fn entries() -> Vec<AccountEntry> {
    let snapshot = parse_snapshot(
        r#"{"schema_version":1,"provider":"claude","profile_id":"p","plan_label":"Pro",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[
          {"id":"s","label":"Session","used_percent":42.0,"resets_at":"2026-10-14T05:00:00Z","source":"provider_reported"},
          {"id":"w","label":"Weekly","used_percent":null,"resets_at":null,"source":"unknown"}],
        "bonus_resets":null}"#,
    )
    .unwrap();
    parse_accounts(CONFIG, Path::new("/home/u"))
        .unwrap()
        .into_iter()
        .map(|account| AccountEntry { account, snapshot: Some(snapshot.clone()) })
        .collect()
}

fn render(area: Rect, hover: Option<Position>) -> Buffer {
    let entries = entries();
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|frame| render_popup_hover(frame, &entries, now(), false, hover)).unwrap();
    terminal.backend().buffer().clone()
}

fn rows(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

/// Where `word` is drawn, read from the rendered rows.
fn find(buf: &Buffer, word: &str) -> Position {
    rows(buf)
        .iter()
        .enumerate()
        .find_map(|(y, row)| row.find(word).map(|x| Position::new(row[..x].chars().count() as u16, y as u16)))
        .unwrap_or_else(|| panic!("{word} not drawn"))
}

fn drawn(buf: &Buffer, text: &str) -> bool {
    rows(buf).iter().any(|row| row.contains(text))
}

const AREA: Rect = Rect { x: 0, y: 0, width: 48, height: 20 };

#[test]
fn hovering_a_window_row_shows_its_tooltip_and_nothing_else_does() {
    let plain = render(AREA, None);
    assert!(!drawn(&plain, "Source:"), "no tooltip without a hover");

    let session = find(&plain, "Session");
    let hovered = render(AREA, Some(session));
    assert!(drawn(&hovered, "Session 42% used"), "headline: {:?}", rows(&hovered));
    assert!(drawn(&hovered, "Source: provider_reported"));

    // A window with no known usage says so; it never reads as 0%.
    let weekly = render(AREA, Some(find(&plain, "Weekly")));
    assert!(drawn(&weekly, "Weekly usage unknown"));
    assert!(!drawn(&weekly, "0% used"));

    // Hovering anywhere that is not a window row shows no tooltip.
    for at in [find(&plain, "Agents"), find(&plain, "Claude Code"), Position::new(0, 0), Position::new(200, 200)] {
        assert!(!drawn(&render(AREA, Some(at)), "Source:"), "hover at {at:?}");
    }
}

#[test]
fn the_tooltip_stays_inside_the_popup_at_every_edge_and_size() {
    let plain = render(AREA, None);
    let session = find(&plain, "Session");
    // Hover at the far right and bottom edges, and in tiny popups: never panics or draws outside.
    for (width, height) in [(48, 20), (30, 8), (12, 6), (5, 3), (1, 1), (0, 0)] {
        let area = Rect::new(0, 0, width, height);
        for at in [session, Position::new(width.saturating_sub(1), session.y), Position::new(1, height.saturating_sub(1))] {
            let _ = render(area, Some(at));
        }
    }
    // When the hovered row is near the bottom, the tooltip is still fully drawn.
    let short = Rect::new(0, 0, 48, 9);
    let at = find(&render(short, None), "Weekly");
    assert!(drawn(&render(short, Some(at)), "Weekly usage unknown"));
}

#[test]
fn the_tooltip_is_as_wide_as_its_text_is_in_terminal_columns() {
    // Wide characters take two columns each; the box must not clip them.
    let snapshot = parse_snapshot(
        r#"{"schema_version":1,"provider":"claude","profile_id":"p","plan_label":"Pro",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{"id":"s","label":"日本語日本語日本語日本語日本語日本語日本語","used_percent":42.0,"resets_at":null,"source":"provider_reported"}],
        "bonus_resets":null}"#,
    )
    .unwrap();
    let entries: Vec<AccountEntry> = parse_accounts(CONFIG, Path::new("/home/u"))
        .unwrap()
        .into_iter()
        .map(|account| AccountEntry { account, snapshot: Some(snapshot.clone()) })
        .collect();
    let draw = |hover| {
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|frame| render_popup_hover(frame, &entries, now(), false, hover)).unwrap();
        terminal.backend().buffer().clone()
    };
    let at = find(&draw(None), "日");
    let tip = draw(Some(at));
    assert!(drawn(&tip, "% used"), "the whole headline is visible: {:?}", rows(&tip));
    assert!(drawn(&tip, "Source: provider_reported"));
}

/// One stale window carrying three scoped allowances: the tallest tooltip.
fn tall_entries() -> Vec<AccountEntry> {
    let snapshot = parse_snapshot(
        r#"{"schema_version":1,"provider":"claude","profile_id":"p","plan_label":"Pro",
        "collected_at":"2026-10-13T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{"id":"w","label":"Weekly","used_percent":42.0,"resets_at":"2026-10-20T00:00:00Z",
          "source":"provider_reported",
          "scoped":[{"label":"Opus","used_percent":10.0},{"label":"Sonnet","used_percent":20.0},{"label":"Haiku","used_percent":null}]}],
        "bonus_resets":null}"#,
    )
    .unwrap();
    parse_accounts(CONFIG, Path::new("/home/u"))
        .unwrap()
        .into_iter()
        .map(|account| AccountEntry { account, snapshot: Some(snapshot.clone()) })
        .collect()
}

fn draw_tall(height: u16, hover: Option<Position>) -> Buffer {
    let entries = tall_entries();
    let mut terminal = Terminal::new(TestBackend::new(48, height)).unwrap();
    terminal.draw(|frame| render_popup_hover(frame, &entries, now(), false, hover)).unwrap();
    terminal.backend().buffer().clone()
}

fn row_of(buf: &Buffer, y: u16) -> String {
    rows(buf)[y as usize].clone()
}

#[test]
fn a_tall_stale_tooltip_is_bordered_and_placed_below_above_or_clipped_by_the_frame() {
    let lines = ["Weekly 42% used", "Source: provider_reported", "Last updated", "Opus 10% used", "Sonnet 20% used", "Haiku usage unknown"];

    // Room below: the box opens on the row under the hovered one, fully drawn.
    let roomy = draw_tall(30, None);
    let at = find(&roomy, "Weekly");
    let below = draw_tall(30, Some(at));
    for line in lines {
        assert!(drawn(&below, line), "{line}: {:?}", rows(&below));
    }
    assert!(row_of(&below, at.y + 1).contains('┌'), "top border below the row: {:?}", rows(&below));
    let bottom = rows(&below).iter().position(|r| r.contains("Haiku")).unwrap() as u16 + 1;
    assert!(row_of(&below, bottom).contains('└'), "bottom border: {:?}", rows(&below));

    // No room below but room above: the box ends on the row above the hovered one.
    let short = Rect::new(0, 0, 48, 9);
    let at = find(&render(short, None), "Weekly");
    let above = render(short, Some(at));
    assert!(drawn(&above, "Weekly usage unknown"), "{:?}", rows(&above));
    assert!(row_of(&above, at.y - 1).contains('└'), "bottom border above the row: {:?}", rows(&above));

    // Taller than the frame: it is clipped to the frame without panicking and
    // still starts with its top border at the top edge.
    let tiny = draw_tall(6, None);
    let at = find(&tiny, "Weekly");
    let clipped = draw_tall(6, Some(at));
    assert!(row_of(&clipped, 0).contains('┌'), "{:?}", rows(&clipped));
}
