use agent_meter::countdown::{reset_display, ResetDisplay};
use chrono::{Duration, TimeZone, Utc};

#[test]
fn past_reset_timestamp_is_stale_never_negative() {
    let now = Utc.with_ymd_and_hms(2026, 10, 15, 0, 0, 0).unwrap();

    assert_eq!(reset_display(now - Duration::minutes(5), now), ResetDisplay::Stale);
    assert_eq!(reset_display(now, now), ResetDisplay::Stale);
    assert_eq!(
        reset_display(now + Duration::hours(2), now),
        ResetDisplay::Remaining("2h 0m".to_string())
    );
}
