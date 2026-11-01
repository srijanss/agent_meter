use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::{render_popup, AccountEntry};
use agent_meter::snapshot::parse_snapshot;
use chrono::{TimeZone, Utc};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

fn entries(name: &str, plan: &str) -> Vec<AccountEntry> {
    let windows = [
        r#"{"id":"s","label":"Session","used_percent":95.0,"resets_at":"2026-10-14T00:39:00Z","source":"provider_reported"}"#,
        r#"{"id":"w","label":"Weekly","used_percent":40.0,"resets_at":"2026-10-20T00:00:00Z","source":"provider_reported"}"#,
    ]
    .join(",");
    let snapshot = |provider: &str| {
        parse_snapshot(&format!(
            r#"{{"schema_version":1,"provider":"{provider}","profile_id":"p","plan_label":"{plan}",
            "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
            "windows":[{windows}],"bonus_resets":null}}"#
        ))
        .unwrap()
    };
    let account = |name: &str, provider, active| Account {
        name: name.into(),
        provider,
        home: PathBuf::from("/x"),
        active,
    };
    vec![
        AccountEntry { account: account(name, Provider::Claude, true), snapshot: Some(snapshot("claude")) },
        AccountEntry { account: account("Second", Provider::Claude, false), snapshot: None },
        AccountEntry { account: account("Third", Provider::Claude, false), snapshot: Some(snapshot("claude")) },
        AccountEntry { account: account("Lone", Provider::Codex, true), snapshot: Some(snapshot("codex")) },
    ]
}

fn render(width: u16, height: u16, entries: &[AccountEntry]) -> Buffer {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_popup(frame, entries, now)).unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn every_terminal_size_renders_without_panicking_and_keeps_the_border() {
    let data = entries("Secondary", "Max 20x");
    for width in 0..=60 {
        for height in 0..=26 {
            let buf = render(width, height, &data);
            if width >= 2 && height >= 2 {
                let accent = buf[(0, 0)].fg;
                for (x, y) in [(width - 1, 0), (0, height - 1), (width - 1, height - 1)] {
                    assert_eq!(buf[(x, y)].fg, accent, "corner ({x},{y}) at {width}x{height}");
                }
                for y in 1..height - 1 {
                    assert_eq!(buf[(0, y)].symbol(), "│", "left border row {y} at {width}x{height}");
                    assert_eq!(
                        buf[(width - 1, y)].symbol(),
                        "│",
                        "right border row {y} at {width}x{height}"
                    );
                }
            }
        }
    }
}

#[test]
fn long_and_wide_unicode_names_and_plans_stay_inside_the_border() {
    let long = "ĺöñğ-名前-アカウント-🙂-".repeat(6);
    let data = entries(&long, &long);
    for (width, height) in [(20, 20), (30, 26), (48, 26), (80, 30)] {
        let buf = render(width, height, &data);
        for y in 1..height - 1 {
            assert_eq!(buf[(width - 1, y)].symbol(), "│", "right border row {y} at {width}x{height}");
        }
    }
}
