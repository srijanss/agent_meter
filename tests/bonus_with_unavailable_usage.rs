use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{render_popup, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render(windows: &str, bonus: &str) -> Vec<String> {
    let snapshot = parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"p","plan_label":"Promax",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{windows}],"bonus_resets":{bonus}}}"#
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

const NULL_WINDOW: &str =
    r#"{"id":"weekly","label":"Weekly","used_percent":null,"resets_at":null,"source":"unknown"}"#;
const TWO_RESETS: &str =
    r#"{"remaining":2,"expires_at":"2026-11-03T07:00:00Z","source":"provider_reported"}"#;
const ZERO_RESETS: &str = r#"{"remaining":0,"expires_at":null,"source":"provider_reported"}"#;

fn position(lines: &[String], needle: &str) -> Option<usize> {
    lines.iter().position(|l| l.contains(needle))
}

#[test]
fn a_reported_bonus_count_survives_unavailable_usage() {
    for (case, windows) in [("no windows", ""), ("all usage unknown", NULL_WINDOW)] {
        let lines = render(windows, TWO_RESETS);
        let unavailable = position(&lines, "Usage unavailable").unwrap_or_else(|| panic!("{case}"));
        let bonus = position(&lines, "2 free resets").unwrap_or_else(|| panic!("{case}: bonus hidden"));
        assert_eq!(bonus, unavailable + 1, "{case}: the bonus line follows the unavailable row");
        assert!(lines[bonus].contains("next expires in 20d 7h"), "{case}: {:?}", lines[bonus]);

        // A real zero is shown as well.
        let zero = render(windows, ZERO_RESETS);
        assert!(position(&zero, "0 free resets").is_some(), "{case}: zero is a value");
    }
}

#[test]
fn not_reported_bonus_still_draws_nothing_when_usage_is_unavailable() {
    for bonus in ["null", r#"{"remaining":null,"expires_at":null,"source":"unknown"}"#] {
        let lines = render(NULL_WINDOW, bonus);
        assert!(position(&lines, "Usage unavailable").is_some());
        assert_eq!(position(&lines, "free reset"), None, "{bonus}");
    }
}
