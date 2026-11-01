use agent_meter::claude_usage::parse_claude_usage;
use agent_meter::snapshot::{Health, Source};
use chrono::{TimeZone, Utc};

/// Real `claude -p /usage --output-format json` result text (claude 2.1.300).
const SAMPLE: &str = "You are currently using your subscription to power your Claude Code usage\n\
\n\
Current session: 38% used · resets Nov 1, 7:40pm (Australia/Sydney)\n\
Current week (all models): 5% used · resets Nov 6, 10pm (Australia/Sydney)\n\
\n\
What's contributing to your usage?\n\
Approximate · sessions on this machine only · shares overlap\n\
\n\
Last 24h · 407 requests · 2 sessions\n\
  89%  Run /implement-and-review on a cheaper model\n\
Current session: 99% used · resets Nov 1, 1:00am (UTC)\n\
  63%  Start fresh when you switch tasks\n";

#[test]
fn real_usage_text_becomes_session_and_weekly_windows() {
    let collected = Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap();
    let snapshot = parse_claude_usage(SAMPLE, collected, "primary");

    assert_eq!(snapshot.provider, "claude");
    assert_eq!(snapshot.profile_id, "primary");
    assert_eq!(snapshot.collected_at, collected);
    assert_eq!(snapshot.source, Source::ProviderReported);
    assert_eq!(snapshot.health, Health::Ok);
    assert_eq!(snapshot.bonus_resets, None, "Claude reports no bonus resets");
    assert_eq!(snapshot.plan_label, None, "the plan comes from auth status, not usage text");

    let [session, weekly] = snapshot.windows.as_slice() else { panic!("{:?}", snapshot.windows) };
    assert_eq!((session.id.as_str(), session.label.as_str()), ("session", "Session"));
    assert_eq!(session.used_percent, Some(38.0));
    // 7:40pm on Nov 1 in Sydney (AEDT, UTC+11).
    assert_eq!(session.resets_at, Some(Utc.with_ymd_and_hms(2026, 11, 1, 8, 40, 0).unwrap()));
    assert_eq!(session.source, Source::ProviderReported);

    assert_eq!((weekly.id.as_str(), weekly.label.as_str()), ("weekly", "Weekly"));
    assert_eq!(weekly.used_percent, Some(5.0));
    assert_eq!(weekly.resets_at, Some(Utc.with_ymd_and_hms(2026, 11, 6, 11, 0, 0).unwrap()));
    assert!(weekly.scoped.is_empty() && session.scoped.is_empty());
}

#[test]
fn the_contributing_section_is_ignored_even_if_it_looks_like_a_usage_line() {
    let collected = Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap();
    let snapshot = parse_claude_usage(SAMPLE, collected, "p");
    assert_eq!(snapshot.windows[0].used_percent, Some(38.0), "not the 99% decoy");
}
