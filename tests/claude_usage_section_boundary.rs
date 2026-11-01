use agent_meter::claude_usage::parse_claude_usage;
use agent_meter::snapshot::{Health, Source};
use chrono::{TimeZone, Utc};

#[test]
fn usage_lines_below_the_contributing_heading_are_never_read() {
    let text = "You are currently using your subscription to power your Claude Code usage\n\
\n\
What's contributing to your usage?\n\
Approximate · sessions on this machine only · shares overlap\n\
Current session: 99% used · resets Nov 1, 7:40pm (Australia/Sydney)\n\
Current week (all models): 98% used · resets Nov 6, 10pm (Australia/Sydney)\n\
Current week (Opus): 97% used · resets Nov 6, 10pm (Australia/Sydney)\n";
    let collected = Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap();

    let s = parse_claude_usage(text, collected, "p");

    for w in &s.windows {
        assert_eq!((w.used_percent, w.resets_at, w.source), (None, None, Source::Unknown), "{}", w.id);
        assert!(w.scoped.is_empty(), "{}", w.id);
    }
    assert_eq!((s.source, s.health), (Source::Unknown, Health::Unsupported));
}

#[test]
fn a_matching_clock_model_line_below_the_heading_is_not_attached() {
    let text = "Current session: 38% used · resets Nov 1, 7:40pm (Australia/Sydney)\n\
Current week (all models): 5% used · resets Nov 6, 10pm (Australia/Sydney)\n\
What's contributing to your usage?\n\
Current week (Opus): 97% used · resets Nov 6, 10pm (Australia/Sydney)\n";
    let collected = Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap();

    let s = parse_claude_usage(text, collected, "p");

    assert_eq!(s.windows[1].used_percent, Some(5.0));
    assert!(s.windows[1].scoped.is_empty(), "{:?}", s.windows[1].scoped);
}
