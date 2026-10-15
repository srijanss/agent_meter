use agent_meter::countdown::countdown_label;
use chrono::{Duration, TimeZone, Utc};

#[test]
fn formats_time_until_reset_like_omarchy() {
    let now = Utc.with_ymd_and_hms(2026, 10, 15, 0, 0, 0).unwrap();
    let label = |delta: Duration| countdown_label(now + delta, now);

    assert_eq!(label(Duration::days(2) + Duration::hours(3) + Duration::minutes(10)), "2d 3h");
    assert_eq!(label(Duration::days(1)), "1d 0h");
    assert_eq!(label(Duration::hours(5) + Duration::minutes(20)), "5h 20m");
    assert_eq!(label(Duration::hours(1)), "1h 0m");
    assert_eq!(label(Duration::minutes(59)), "59m");
    assert_eq!(label(Duration::seconds(10)), "1m");
}
