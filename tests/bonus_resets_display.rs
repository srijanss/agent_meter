use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{render_popup, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render(bonus: &str) -> Vec<String> {
    let snapshot = parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"p","plan_label":"Promax",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{{"id":"weekly","label":"Weekly","used_percent":52.0,
                     "resets_at":"2026-10-18T19:00:00Z","source":"provider_reported"}}],
        "bonus_resets":{bonus}}}"#
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
    let mut terminal = Terminal::new(TestBackend::new(52, 12)).unwrap();
    terminal.draw(|frame| render_popup(frame, &entries, now)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            row.trim_end_matches('│').trim_end().to_string()
        })
        .collect()
}

fn row_with(lines: &[String], needle: &str) -> Option<usize> {
    lines.iter().position(|l| l.contains(needle))
}

#[test]
fn a_reported_bonus_count_is_drawn_under_the_windows() {
    // 20d 7h after now (2026-10-14T00:00Z).
    let lines = render(
        r#"{"remaining":2,"expires_at":"2026-11-03T07:00:00Z","source":"provider_reported"}"#,
    );
    let weekly = row_with(&lines, "Weekly").expect("weekly row");
    let bonus = row_with(&lines, "free reset").expect("bonus line");
    assert_eq!(bonus, weekly + 1, "directly under the account's windows");
    assert!(lines[bonus].contains("2 free resets · next expires in 20d 7h"), "{:?}", lines[bonus]);
    assert!(lines[bonus].trim_start_matches('│').starts_with("2 free resets"), "left aligned");
}

#[test]
fn zero_is_drawn_as_zero_and_not_reported_draws_nothing() {
    let zero = render(r#"{"remaining":0,"expires_at":null,"source":"provider_reported"}"#);
    let line = zero.iter().find(|l| l.contains("free reset")).expect("zero is a real value");
    assert!(line.contains("0 free resets") && !line.contains("expires"), "{line:?}");

    // Not reported: no line at all, and certainly not a 0.
    for absent in ["null", r#"{"remaining":null,"expires_at":null,"source":"unknown"}"#] {
        let lines = render(absent);
        assert_eq!(row_with(&lines, "free reset"), None, "{absent}");
        assert_eq!(row_with(&lines, "expires"), None, "{absent}");
    }
}
