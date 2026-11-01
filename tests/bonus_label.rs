use agent_meter::display::bonus_label;
use agent_meter::snapshot::{BonusResets, Source};
use chrono::{DateTime, TimeZone, Utc};

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

fn bonus(remaining: Option<u32>, expires_at: Option<DateTime<Utc>>) -> BonusResets {
    BonusResets { remaining, expires_at, source: Source::ProviderReported }
}

fn after(hours: i64, minutes: i64) -> Option<DateTime<Utc>> {
    Some(now() + chrono::Duration::hours(hours) + chrono::Duration::minutes(minutes))
}

#[test]
fn counts_and_expiry_are_worded_as_in_the_reference() {
    assert_eq!(
        bonus_label(&bonus(Some(2), after(20 * 24 + 7, 0)), now()).as_deref(),
        Some("2 free resets · next expires in 20d 7h")
    );
    assert_eq!(
        bonus_label(&bonus(Some(5), after(3, 30)), now()).as_deref(),
        Some("5 free resets · next expires in 3h 30m")
    );
    assert_eq!(
        bonus_label(&bonus(Some(3), after(0, 45)), now()).as_deref(),
        Some("3 free resets · next expires in 45m")
    );
    // One is singular.
    assert_eq!(
        bonus_label(&bonus(Some(1), after(24, 0)), now()).as_deref(),
        Some("1 free reset · next expires in 1d 0h")
    );
}

#[test]
fn zero_is_a_real_value_and_never_mentions_an_expiry() {
    assert_eq!(bonus_label(&bonus(Some(0), None), now()).as_deref(), Some("0 free resets"));
    assert_eq!(
        bonus_label(&bonus(Some(0), after(10, 0)), now()).as_deref(),
        Some("0 free resets"),
        "nothing left to expire"
    );
}

#[test]
fn unknown_values_are_omitted_never_shown_as_zero() {
    assert_eq!(bonus_label(&bonus(None, None), now()), None, "not reported");
    assert_eq!(bonus_label(&bonus(None, after(5, 0)), now()), None, "an expiry alone is not a count");

    // No expiry, or one that already passed (stale data): just the count.
    assert_eq!(bonus_label(&bonus(Some(2), None), now()).as_deref(), Some("2 free resets"));
    assert_eq!(
        bonus_label(&bonus(Some(2), after(-1, 0)), now()).as_deref(),
        Some("2 free resets")
    );
    assert_eq!(bonus_label(&bonus(Some(2), Some(now())), now()).as_deref(), Some("2 free resets"));
}
