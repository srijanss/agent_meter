use agent_meter::claude_usage::parse_claude_usage;
use chrono::{DateTime, TimeZone, Utc};

fn resets(text: &str, collected: DateTime<Utc>) -> Option<DateTime<Utc>> {
    parse_claude_usage(text, collected, "p").windows[0].resets_at
}

#[test]
fn an_ambiguous_or_skipped_time_that_is_already_past_resolves_to_next_year() {
    // Both occurrences of Nov 1 01:30 New York 2026 are over by December; 2027's is unambiguous (EDT).
    let overlap = "Current session: 1% used · resets Nov 1, 1:30am (America/New_York)\n";
    let december = Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap();
    assert_eq!(resets(overlap, december), Some(Utc.with_ymd_and_hms(2027, 11, 1, 5, 30, 0).unwrap()));

    // Mar 8 02:30 New York 2026 does not exist; once it has passed, 2027's does (EST).
    let gap = "Current session: 1% used · resets Mar 8, 2:30am (America/New_York)\n";
    let later = Utc.with_ymd_and_hms(2026, 3, 20, 0, 0, 0).unwrap();
    assert_eq!(resets(gap, later), Some(Utc.with_ymd_and_hms(2027, 3, 8, 7, 30, 0).unwrap()));
}

#[test]
fn a_still_future_ambiguous_or_skipped_time_stays_unknown() {
    let overlap = "Current session: 1% used · resets Nov 1, 1:30am (America/New_York)\n";
    // Between the two occurrences: one is still ahead, but which is unknowable.
    let between = Utc.with_ymd_and_hms(2026, 11, 1, 6, 0, 0).unwrap();
    assert_eq!(resets(overlap, between), None);

    let gap = "Current session: 1% used · resets Mar 8, 2:30am (America/New_York)\n";
    let before = Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap();
    assert_eq!(resets(gap, before), None);
}
