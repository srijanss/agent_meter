use agent_meter::claude_usage::parse_claude_usage;
use chrono::{DateTime, TimeZone, Utc};

fn resets(text: &str, collected: DateTime<Utc>) -> Option<DateTime<Utc>> {
    parse_claude_usage(text, collected, "p").windows[0].resets_at
}

fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
}

#[test]
fn month_names_must_match_whole_tokens() {
    let collected = at(2026, 1, 1, 0, 0);
    let line = |month: &str| format!("Current session: 1% used · resets {month} 6, 10pm (UTC)\n");

    // Abbreviations and full names, in any case.
    for name in ["Nov", "nov", "NOV", "November", "november", "Sep", "Sept", "September"] {
        assert!(resets(&line(name), collected).is_some(), "{name} should parse");
    }
    // A valid prefix followed by junk is not a month.
    for name in ["Marching", "NovemberXYZ", "Nove", "Novv", "Sepx", "Mayhem", "Janu", "Decem", "N"] {
        assert_eq!(resets(&line(name), collected), None, "{name} must stay unknown");
    }
}

#[test]
fn a_skipped_time_resolves_to_next_year_as_soon_as_it_has_passed() {
    let gap = "Current session: 1% used · resets Mar 8, 2:30am (America/New_York)\n";
    // 2026-03-08 02:30 New York does not exist (clocks jump 02:00 EST to 03:00 EDT at 07:00Z).
    // Both readings of it (07:30Z on the old clock, 06:30Z on the new) are over by 12:00Z.
    assert_eq!(resets(gap, at(2026, 3, 8, 12, 0)), Some(at(2027, 3, 8, 7, 30)));
    // Until the later reading (07:30Z) has passed, a reset in doubt stays unknown.
    assert_eq!(resets(gap, at(2026, 3, 8, 7, 0)), None);
    assert_eq!(resets(gap, at(2026, 3, 1, 0, 0)), None);
    // Exactly at the later reading it has passed too.
    assert_eq!(resets(gap, at(2026, 3, 8, 7, 30)), Some(at(2027, 3, 8, 7, 30)));
}

#[test]
fn overlap_boundaries_are_exact() {
    let overlap = "Current session: 1% used · resets Nov 1, 1:30am (America/New_York)\n";
    // The two occurrences are 05:30Z (EDT) and 06:30Z (EST).
    assert_eq!(resets(overlap, at(2026, 10, 31, 0, 0)), None, "before both");
    assert_eq!(resets(overlap, at(2026, 11, 1, 5, 30)), None, "at the first: the second is ahead");
    assert_eq!(resets(overlap, at(2026, 11, 1, 6, 29)), None, "just before the second");
    assert_eq!(
        resets(overlap, at(2026, 11, 1, 6, 30)),
        Some(at(2027, 11, 1, 5, 30)),
        "exactly at the later occurrence both are over: next year, which is unambiguous"
    );
}
