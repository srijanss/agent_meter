use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{render_popup, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;

fn window(label: &str, used: &str, resets: Option<&str>) -> String {
    let resets = resets.map_or("null".to_string(), |r| format!("\"{r}\""));
    format!(
        r#"{{"id":"{label}","label":"{label}","used_percent":{used},"resets_at":{resets},"source":"provider_reported"}}"#
    )
}

fn render() -> Buffer {
    let windows = [
        window("Session", "5.0", Some("2026-10-14T00:39:00Z")),
        window("Weekly", "89.0", Some("2026-10-15T18:00:00Z")),
        window("Opus", "90.0", Some("2026-10-14T05:30:00Z")),
        window("Sonnet", "null", None),
    ]
    .join(",");
    let snapshot = parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"p","plan_label":"Promax",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{windows}],"bonus_resets":null}}"#
    ))
    .unwrap();
    let entries = [AccountEntry {
        account: Account {
            name: "Main".into(),
            provider: Provider::Codex,
            home: PathBuf::from("/x"),
            active: true,
        },
        snapshot: Some(snapshot),
    }];
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(48, 14)).unwrap();
    terminal.draw(|frame| render_popup(frame, &entries, now)).unwrap();
    terminal.backend().buffer().clone()
}

fn line(buf: &Buffer, y: u16) -> String {
    let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
    row.trim_end_matches('│').trim_end().to_string()
}

fn row_of(buf: &Buffer, label: &str) -> u16 {
    (0..buf.area.height)
        .find(|&y| line(buf, y).contains(label))
        .unwrap_or_else(|| panic!("no row for {label}"))
}

fn meter(buf: &Buffer, y: u16) -> Vec<Color> {
    (0..buf.area.width).filter(|&x| buf[(x, y)].symbol() == "━").map(|x| buf[(x, y)].fg).collect()
}

fn filled(cells: &[Color], color: Color) -> usize {
    cells.iter().filter(|c| **c == color).count()
}

#[test]
fn window_rows_show_title_meter_and_a_fixed_time_column() {
    let buf = render();
    let (session, weekly, opus, sonnet) =
        (row_of(&buf, "Session"), row_of(&buf, "Weekly"), row_of(&buf, "Opus"), row_of(&buf, "Sonnet"));
    assert!(session < weekly && weekly < opus && opus < sonnet, "rows keep snapshot order");

    // Title at the left, time left right-aligned in a column shared by every row.
    assert!(line(&buf, session).ends_with("39m"), "{:?}", line(&buf, session));
    assert!(line(&buf, weekly).ends_with("1d 18h"), "{:?}", line(&buf, weekly));
    assert!(line(&buf, opus).ends_with("5h 30m"), "{:?}", line(&buf, opus));
    let time_col = |y: u16| line(&buf, y).chars().count();
    assert_eq!(time_col(session), time_col(weekly));
    assert_eq!(time_col(weekly), time_col(opus));

    // Every row's meter has the same width, so fills are comparable.
    let width = meter(&buf, session).len();
    assert!(width >= 10, "meter has room: {width}");
    for y in [weekly, opus, sonnet] {
        assert_eq!(meter(&buf, y).len(), width, "row {y}");
    }

    // Fill is proportional to usage and drawn in the normal color below 90%.
    assert_eq!(filled(&meter(&buf, session), Color::White), (0.05 * width as f64).round() as usize);
    assert_eq!(filled(&meter(&buf, weekly), Color::White), (0.89 * width as f64).round() as usize);
    assert_eq!(filled(&meter(&buf, weekly), Color::Red), 0, "89% is not urgent");

    // At 90% and above the fill switches to the urgent color.
    assert_eq!(filled(&meter(&buf, opus), Color::Red), (0.90 * width as f64).round() as usize);
    assert_eq!(filled(&meter(&buf, opus), Color::White), 0);

    // Unknown usage is an empty track with no countdown, never an empty-looking 0%.
    assert_eq!(filled(&meter(&buf, sonnet), Color::DarkGray), width);
    assert!(line(&buf, sonnet).ends_with('━'), "no time column text: {:?}", line(&buf, sonnet));

    // Usage is shown by the meters alone.
    assert!(!(0..buf.area.height).any(|y| line(&buf, y).contains('%')));
}
