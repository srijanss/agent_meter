use agent_meter::claude_usage::parse_claude_usage;
use agent_meter::snapshot::{Health, ScopedAllowance, Source};
use chrono::{TimeZone, Utc};

fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
}

#[test]
fn the_year_is_the_next_occurrence_after_collection() {
    let text = "Current session: 10% used · resets Jan 2, 3am (UTC)\n\
                Current week (all models): 20% used · resets Nov 6, 10pm (UTC)\n";
    // Collected on Dec 31: Jan 2 is next year. Nov 6 is 10 months away: same-year? No, Nov 6 already passed.
    let s = parse_claude_usage(text, at(2026, 12, 31, 23, 0), "p");
    assert_eq!(s.windows[0].resets_at, Some(at(2027, 1, 2, 3, 0)));
    assert_eq!(s.windows[1].resets_at, Some(at(2027, 11, 6, 22, 0)));

    // Later the same year the date is still ahead.
    let s = parse_claude_usage(text, at(2026, 6, 1, 0, 0), "p");
    assert_eq!(s.windows[0].resets_at, Some(at(2027, 1, 2, 3, 0)));
    assert_eq!(s.windows[1].resets_at, Some(at(2026, 11, 6, 22, 0)));
}

#[test]
fn twelve_oclock_and_feb_29_are_read_correctly() {
    let text = "Current session: 1% used · resets Feb 29, 12am (UTC)\n\
                Current week (all models): 2% used · resets Mar 1, 12pm (UTC)\n";
    let s = parse_claude_usage(text, at(2027, 3, 5, 0, 0), "p");
    assert_eq!(s.windows[0].resets_at, Some(at(2028, 2, 29, 0, 0)), "next leap year");
    assert_eq!(s.windows[1].resets_at, Some(at(2028, 3, 1, 12, 0)));
}

#[test]
fn a_model_line_on_the_same_clock_is_a_scoped_allowance_on_that_window() {
    let text = "Current session: 10% used · resets Nov 1, 7:40pm (Australia/Sydney)\n\
                Current week (all models): 5% used · resets Nov 6, 10pm (Australia/Sydney)\n\
                Current week (Opus): 12% used · resets Nov 6, 10pm (Australia/Sydney)\n\
                Current week (Other): 40% used · resets Nov 9, 10pm (Australia/Sydney)\n";
    let s = parse_claude_usage(text, at(2026, 11, 1, 0, 0), "p");
    assert_eq!(s.windows.len(), 2, "model lines are not rows of their own");
    assert_eq!(
        s.windows[1].scoped,
        vec![ScopedAllowance { label: "Opus".into(), used_percent: Some(12.0) }],
        "the line on a different clock is not attached"
    );
    assert!(s.windows[0].scoped.is_empty());
}

#[test]
fn anything_unrecognized_is_unknown_never_a_guess() {
    let collected = at(2026, 11, 1, 0, 0);

    let garbage = parse_claude_usage("totally unrelated output\n", collected, "p");
    assert_eq!(garbage.windows.len(), 2, "session and weekly are always listed");
    for w in &garbage.windows {
        assert_eq!((w.used_percent, w.resets_at, w.source), (None, None, Source::Unknown));
    }
    assert_eq!((garbage.source, garbage.health), (Source::Unknown, Health::Unsupported));

    // A known zone it cannot interpret keeps the percentage but not the reset.
    let text = "Current session: 38% used · resets Nov 1, 7:40pm (Mars/Olympus)\n\
                Current week (all models): lots used · resets Nov 6, 10pm (UTC)\n";
    let s = parse_claude_usage(text, collected, "p");
    assert_eq!((s.windows[0].used_percent, s.windows[0].resets_at), (Some(38.0), None));
    assert_eq!(s.windows[0].source, Source::ProviderReported);
    assert_eq!(s.windows[1].used_percent, None, "unparseable percentage");
    assert_eq!(s.windows[1].resets_at, Some(at(2026, 11, 6, 22, 0)));

    // A local time that does not exist (spring-forward gap) is unknown, not shifted.
    let gap = "Current session: 1% used · resets Mar 8, 2:30am (America/New_York)\n";
    let s = parse_claude_usage(gap, at(2026, 3, 1, 0, 0), "p");
    assert_eq!(s.windows[0].resets_at, None);
    assert_eq!(s.windows[0].used_percent, Some(1.0));
}
