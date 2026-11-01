use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{render_popup, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;

const WIDTH: u16 = 48;
const HEIGHT: u16 = 20;

fn account(name: &str, provider: Provider, active: bool) -> Account {
    Account { name: name.into(), provider, home: PathBuf::from("/x"), active }
}

fn snapshot(provider: &str, plan: &str, windows: &str) -> agent_meter::snapshot::Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"{provider}","profile_id":"p","plan_label":"{plan}",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{windows}],"bonus_resets":null}}"#
    ))
    .unwrap()
}

fn window(id: &str, label: &str, used: f64, resets: Option<&str>) -> String {
    let resets = resets.map_or("null".to_string(), |r| format!("\"{r}\""));
    format!(
        r#"{{"id":"{id}","label":"{label}","used_percent":{used},"resets_at":{resets},"source":"provider_reported"}}"#
    )
}

fn render() -> Buffer {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let secondary = snapshot(
        "claude",
        "Max 20x",
        &[
            window("session", "Session", 5.0, Some("2026-10-14T00:39:00Z")),
            window("weekly", "Weekly", 12.0, Some("2026-10-15T18:00:00Z")),
        ]
        .join(","),
    );
    let primary = snapshot(
        "claude",
        "Max 20x",
        &[window("session", "Session", 0.0, None), window("weekly", "Weekly", 1.0, None)].join(","),
    );
    let codex = snapshot(
        "codex",
        "Promax",
        &window("weekly", "Weekly", 52.0, Some("2026-10-18T19:00:00Z")),
    );
    let entries = vec![
        AccountEntry {
            account: account("Secondary", Provider::Claude, true),
            snapshot: Some(secondary),
        },
        AccountEntry {
            account: account("Primary", Provider::Claude, false),
            snapshot: Some(primary),
        },
        AccountEntry { account: account("Main", Provider::Codex, true), snapshot: Some(codex) },
    ];
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal.draw(|frame| render_popup(frame, &entries, now)).unwrap();
    terminal.backend().buffer().clone()
}

fn line(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

fn lines(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height).map(|y| line(buf, y)).collect()
}

fn row_with(buf: &Buffer, needle: &str) -> u16 {
    (0..buf.area.height)
        .find(|&y| line(buf, y).contains(needle))
        .unwrap_or_else(|| panic!("no row contains {needle:?} in:\n{}", lines(buf).join("\n")))
}

/// The meter cells of a window row as (is_filled, is_urgent) pairs.
fn meter(buf: &Buffer, y: u16) -> Vec<Color> {
    (0..buf.area.width).filter(|&x| buf[(x, y)].symbol() == "━").map(|x| buf[(x, y)].fg).collect()
}

#[test]
fn popup_matches_the_reference_layout() {
    let buf = render();
    let all = lines(&buf).join("\n");

    // Accent border around the whole popup.
    let accent = buf[(0, 0)].fg;
    assert_ne!(accent, Color::Reset, "border is drawn in an accent color");
    assert_eq!(buf[(WIDTH - 1, HEIGHT - 1)].fg, accent);

    // Header: mark, title, Refresh and Close at the right.
    let header = line(&buf, row_with(&buf, "Agents"));
    assert!(header.contains("Agents"));
    let refresh = header.find('↻').expect("refresh button");
    let close = header.find('×').expect("close button");
    assert!(refresh < close, "refresh sits left of close");
    assert!(header.trim_end_matches('│').trim_end().ends_with('×'), "close is right-most");

    // Sections in order, one per provider.
    let claude = row_with(&buf, "Claude Code");
    let secondary = row_with(&buf, "Secondary");
    let primary = row_with(&buf, "Primary");
    let codex = row_with(&buf, "Codex");
    assert!(row_with(&buf, "Agents") < claude && claude < secondary && secondary < primary);
    assert!(primary < codex);

    // Multi-account provider: "Name · Plan" with ACTIVE or Use at the right.
    let secondary_line = line(&buf, secondary);
    assert!(secondary_line.contains("Secondary · Max 20x"));
    assert!(secondary_line.trim_end_matches('│').trim_end().ends_with("ACTIVE"));
    let primary_line = line(&buf, primary);
    assert!(primary_line.contains("Primary · Max 20x"));
    assert!(primary_line.trim_end_matches('│').trim_end().ends_with("Use"));

    // Single-account provider: plan right-aligned on the section row, no account line.
    let codex_line = line(&buf, codex);
    assert!(codex_line.trim_end_matches('│').trim_end().ends_with("Promax"));
    assert!(!all.contains("Main"), "a lone account's name is not repeated");

    // Window rows: title | meter | fixed-width time left, right-aligned.
    let session = line(&buf, secondary + 1);
    assert!(session.contains("Session") && session.trim_end_matches('│').trim_end().ends_with("39m"));
    let weekly = line(&buf, secondary + 2);
    assert!(weekly.contains("Weekly") && weekly.trim_end_matches('│').trim_end().ends_with("1d 18h"));
    let session_end = session.trim_end_matches('│').trim_end().len();
    assert_eq!(session_end, weekly.trim_end_matches('│').trim_end().len(), "time column aligned");

    // No percentage text anywhere; usage is shown only by the meters.
    assert!(!all.contains('%'));

    // Meter fill is proportional: 52% of the Codex weekly track is filled.
    let codex_cells = meter(&buf, codex + 1);
    let filled = codex_cells.iter().filter(|c| **c == Color::White).count();
    assert_eq!(filled, (0.52 * codex_cells.len() as f64).round() as usize);
    assert!(codex_cells.iter().all(|c| *c != Color::Red), "52% is not urgent");

    // Unknown reset times draw no countdown (Primary has none).
    assert!(line(&buf, primary + 1).trim_end_matches('│').trim_end().ends_with('━'));
}
