use agent_meter::claude_usage::parse_claude_usage;
use chrono::{TimeZone, Utc};

fn resets(text: &str, collected: chrono::DateTime<Utc>) -> Option<chrono::DateTime<Utc>> {
    parse_claude_usage(text, collected, "p").windows[0].resets_at
}

#[test]
fn a_local_time_that_happens_twice_is_unknown_not_guessed() {
    // 2026-11-01 01:30 in New York occurs twice (EDT 05:30Z and EST 06:30Z).
    let text = "Current session: 10% used · resets Nov 1, 1:30am (America/New_York)\n";

    for collected in [
        Utc.with_ymd_and_hms(2026, 10, 31, 0, 0, 0).unwrap(), // before both occurrences
        Utc.with_ymd_and_hms(2026, 11, 1, 6, 0, 0).unwrap(),  // between them
    ] {
        assert_eq!(resets(text, collected), None, "collected {collected}");
    }

    // Unambiguous neighbours on the same night still resolve (EST, after the overlap).
    let after = "Current session: 10% used · resets Nov 1, 3:30am (America/New_York)\n";
    let collected = Utc.with_ymd_and_hms(2026, 10, 31, 0, 0, 0).unwrap();
    assert_eq!(resets(after, collected), Some(Utc.with_ymd_and_hms(2026, 11, 1, 8, 30, 0).unwrap()));
}
