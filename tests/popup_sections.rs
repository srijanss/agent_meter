use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{render_popup, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

fn entry(name: &str, provider: Provider, active: bool, plan: &str) -> AccountEntry {
    let provider_id = if provider == Provider::Claude { "claude" } else { "codex" };
    let snapshot = parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"{provider_id}","profile_id":"p","plan_label":"{plan}",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[],"bonus_resets":null}}"#
    ))
    .unwrap();
    AccountEntry {
        account: Account { name: name.into(), provider, home: PathBuf::from("/x"), active },
        snapshot: Some(snapshot),
    }
}

fn render(entries: &[AccountEntry]) -> Buffer {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(48, 20)).unwrap();
    terminal.draw(|frame| render_popup(frame, entries, now)).unwrap();
    terminal.backend().buffer().clone()
}

fn lines(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            row.trim_end_matches('│').trim_end().to_string()
        })
        .collect()
}

fn row_with(lines: &[String], needle: &str) -> usize {
    lines
        .iter()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no row contains {needle:?} in:\n{}", lines.join("\n")))
}

#[test]
fn sections_group_accounts_by_provider_with_the_reference_account_lines() {
    let lines = lines(&render(&[
        entry("Secondary", Provider::Claude, true, "Max 20x"),
        entry("Primary", Provider::Claude, false, "Max 20x"),
        entry("Main", Provider::Codex, true, "Promax"),
    ]));

    let claude = row_with(&lines, "Claude Code");
    let secondary = row_with(&lines, "Secondary");
    let primary = row_with(&lines, "Primary");
    let codex = row_with(&lines, "Codex");
    assert!(row_with(&lines, "Agents") < claude && claude < secondary && secondary < primary);
    assert!(primary < codex, "Claude's section comes before Codex's");

    assert!(lines[claude].contains("✻ Claude Code"), "provider mark: {:?}", lines[claude]);
    assert!(lines[codex].contains("◍ Codex"), "provider mark: {:?}", lines[codex]);

    assert!(lines[secondary].contains("Secondary · Max 20x"), "{:?}", lines[secondary]);
    assert!(lines[secondary].ends_with("ACTIVE"), "{:?}", lines[secondary]);
    assert!(lines[primary].contains("Primary · Max 20x"), "{:?}", lines[primary]);
    assert!(lines[primary].ends_with("Use"), "{:?}", lines[primary]);

    // A provider with one account puts its plan on the section row instead.
    assert!(lines[codex].ends_with("Promax"), "{:?}", lines[codex]);
    assert!(!lines.join("\n").contains("Main"), "a lone account's name is not repeated");
}

#[test]
fn a_provider_with_no_accounts_has_no_section() {
    let lines = lines(&render(&[entry("Main", Provider::Codex, true, "Promax")]));
    assert!(!lines.join("\n").contains("Claude Code"));
}
