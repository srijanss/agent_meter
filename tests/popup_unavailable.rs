use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{render_popup, AccountEntry};
use agent_meter::snapshot::{parse_snapshot, Snapshot};
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn snapshot(windows: &str) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"p","plan_label":"Promax",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{windows}],"bonus_resets":null}}"#
    ))
    .unwrap()
}

fn lone_codex(snapshot: Option<Snapshot>) -> Vec<String> {
    let entries = [AccountEntry {
        account: Account {
            name: "Main".into(),
            provider: Provider::Codex,
            home: PathBuf::from("/x"),
            active: true,
        },
        snapshot,
    }];
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(48, 10)).unwrap();
    terminal.draw(|frame| render_popup(frame, &entries, now)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            row.trim_end_matches('│').trim_end().to_string()
        })
        .collect()
}

fn unavailable_under_section(lines: &[String]) -> bool {
    let section = lines.iter().position(|l| l.contains("Codex")).expect("section row");
    lines[section + 1].contains("Usage unavailable")
}

#[test]
fn an_account_without_usable_data_says_so_instead_of_vanishing() {
    let null_window =
        r#"{"id":"weekly","label":"Weekly","used_percent":null,"resets_at":null,"source":"unknown"}"#;
    assert!(unavailable_under_section(&lone_codex(None)), "no snapshot yet");
    assert!(unavailable_under_section(&lone_codex(Some(snapshot("")))), "no windows");
    assert!(
        unavailable_under_section(&lone_codex(Some(snapshot(null_window)))),
        "every window unknown"
    );

    let measured =
        r#"{"id":"weekly","label":"Weekly","used_percent":10.0,"resets_at":null,"source":"provider_reported"}"#;
    let lines = lone_codex(Some(snapshot(measured)));
    assert!(!lines.join("\n").contains("unavailable"), "measured data is not flagged");
}
