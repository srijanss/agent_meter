use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{render_popup, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn screen(second_source: &str) -> Vec<String> {
    let snapshot = parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"p","plan_label":null,
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[
          {{"id":"s","label":"Session","used_percent":10.0,"resets_at":null,"source":"provider_reported"}},
          {{"id":"w","label":"Weekly","used_percent":40.0,"resets_at":null,"source":"{second_source}"}}
        ],"bonus_resets":null}}"#
    ))
    .unwrap();
    let entries = [AccountEntry {
        account: Account { name: "Main".into(), provider: Provider::Codex, home: PathBuf::from("/x"), active: true },
        snapshot: Some(snapshot),
    }];
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(48, 12)).unwrap();
    terminal.draw(|frame| render_popup(frame, &entries, now)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()).collect()
}

#[test]
fn a_local_estimate_window_is_labelled_an_estimate_and_provider_windows_are_not() {
    let lines = screen("local_estimate");
    let weekly = lines.iter().position(|l| l.contains("Weekly")).unwrap();
    assert!(lines[weekly + 1].contains("local estimate"), "{:?}", lines[weekly + 1]);
    assert!(lines[weekly + 1].contains("not provider"), "{:?}", lines[weekly + 1]);
    let session = lines.iter().position(|l| l.contains("Session")).unwrap();
    assert!(!lines[session + 1].contains("estimate"));

    let reported = screen("provider_reported");
    assert!(!reported.iter().any(|l| l.contains("estimate")));
}

#[test]
fn an_estimate_window_with_only_scoped_usage_is_still_labelled() {
    let snapshot = parse_snapshot(
        r#"{"schema_version":1,"provider":"claude","profile_id":"p","plan_label":null,
        "collected_at":"2026-10-14T00:00:00Z","source":"local_estimate","health":"ok",
        "windows":[{"id":"w","label":"Weekly","used_percent":null,"resets_at":null,
          "source":"local_estimate","scoped":[{"label":"Opus","used_percent":50.0}]}],
        "bonus_resets":null}"#,
    )
    .unwrap();
    let entries = [AccountEntry {
        account: Account { name: "Main".into(), provider: Provider::Claude, home: PathBuf::from("/x"), active: true },
        snapshot: Some(snapshot),
    }];
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(48, 12)).unwrap();
    terminal.draw(|frame| render_popup(frame, &entries, now)).unwrap();
    let buf = terminal.backend().buffer().clone();
    let text: String = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_string())
        .collect();
    assert!(text.contains("local estimate"));
}
