use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{render_popup, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

fn render(weekly_scoped: &str) -> Buffer {
    let snapshot = parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"p","plan_label":"Max",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[
          {{"id":"s","label":"Session","used_percent":10.0,"resets_at":"2026-10-14T03:00:00Z","source":"provider_reported"}},
          {{"id":"w","label":"Weekly","used_percent":40.0,"resets_at":"2026-10-15T18:00:00Z","source":"provider_reported",
            "scoped":{weekly_scoped}}}
        ],"bonus_resets":null}}"#
    ))
    .unwrap();
    let entries = [AccountEntry {
        account: Account {
            name: "Main".into(),
            provider: Provider::Claude,
            home: PathBuf::from("/x"),
            active: true,
        },
        snapshot: Some(snapshot),
    }];
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(48, 10)).unwrap();
    terminal.draw(|frame| render_popup(frame, &entries, now)).unwrap();
    terminal.backend().buffer().clone()
}

fn line(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

fn row_of(buf: &Buffer, label: &str) -> u16 {
    (0..buf.area.height).find(|&y| line(buf, y).contains(label)).unwrap_or_else(|| panic!("no row for {label}"))
}

/// Columns of the meter on a row (track cells and tick alike).
fn meter_cells(buf: &Buffer, y: u16) -> Vec<(u16, String)> {
    (0..buf.area.width)
        .map(|x| (x, buf[(x, y)].symbol().to_string()))
        .filter(|(_, s)| s == "━" || s == "┃")
        .collect()
}

fn ticks(buf: &Buffer, y: u16) -> Vec<usize> {
    meter_cells(buf, y).iter().enumerate().filter(|(_, (_, s))| s == "┃").map(|(i, _)| i).collect()
}

#[test]
fn a_scoped_allowance_is_a_tick_across_its_windows_meter_not_a_row() {
    let buf = render(r#"[{"label":"Opus","used_percent":90.0}]"#);
    let (session, weekly) = (row_of(&buf, "Session"), row_of(&buf, "Weekly"));
    let width = meter_cells(&buf, session).len();
    assert!(width >= 10, "meter has room: {width}");

    // One tick on the window it shares a clock with, at the scoped used percent.
    assert_eq!(meter_cells(&buf, weekly).len(), width, "tick replaces a track cell");
    assert_eq!(ticks(&buf, weekly), vec![(0.9 * width as f64).floor() as usize]);
    assert!(ticks(&buf, session).is_empty(), "other windows are untouched");

    // Not a separate row.
    assert!(!(0..buf.area.height).any(|y| line(&buf, y).contains("Opus")));
}

#[test]
fn an_unknown_scoped_percent_or_no_scoped_allowance_draws_no_tick() {
    let unknown = render(r#"[{"label":"Opus","used_percent":null}]"#);
    assert!(ticks(&unknown, row_of(&unknown, "Weekly")).is_empty());
    let none = render("[]");
    assert!(ticks(&none, row_of(&none, "Weekly")).is_empty());
}

#[test]
fn a_known_scoped_percent_is_shown_even_when_every_parent_percent_is_unknown() {
    let snapshot = parse_snapshot(
        r#"{"schema_version":1,"provider":"claude","profile_id":"p","plan_label":"Max",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{"id":"w","label":"Weekly","used_percent":null,"resets_at":null,
          "source":"provider_reported","scoped":[{"label":"Opus","used_percent":50.0}]}],
        "bonus_resets":null}"#,
    )
    .unwrap();
    let entries = [AccountEntry {
        account: Account { name: "Main".into(), provider: Provider::Claude, home: PathBuf::from("/x"), active: true },
        snapshot: Some(snapshot),
    }];
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(48, 10)).unwrap();
    terminal.draw(|frame| render_popup(frame, &entries, now)).unwrap();
    let buf = terminal.backend().buffer().clone();

    let weekly = row_of(&buf, "Weekly");
    let width = meter_cells(&buf, weekly).len();
    assert_eq!(ticks(&buf, weekly), vec![width / 2]);
    assert!(!(0..buf.area.height).any(|y| line(&buf, y).contains("unavailable")));
}

#[test]
fn a_full_scoped_allowance_stays_inside_the_meter() {
    let buf = render(r#"[{"label":"Opus","used_percent":100.0}]"#);
    let weekly = row_of(&buf, "Weekly");
    let width = meter_cells(&buf, weekly).len();
    assert_eq!(ticks(&buf, weekly), vec![width - 1]);
}
