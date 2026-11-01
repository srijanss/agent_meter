use agent_meter::codex_limits::parse_codex_rate_limits;
use agent_meter::snapshot::{Health, Source};
use chrono::{TimeZone, Utc};
use serde_json::json;

#[test]
fn rate_limits_read_result_becomes_session_and_weekly_windows() {
    // Shape of a real `account/rateLimits/read` result (codex-cli 0.162).
    let result = json!({
        "rateLimits": {
            "limitId": "codex",
            "limitName": null,
            "planType": "promax",
            "primary":   {"usedPercent": 12, "windowDurationMins": 300,   "resetsAt": 1793520000},
            "secondary": {"usedPercent": 52, "windowDurationMins": 10080, "resetsAt": 1793900000},
            "credits": {"hasCredits": false, "unlimited": false, "balance": null}
        },
        "rateLimitsByLimitId": null,
        "rateLimitResetCredits": null
    });
    let collected = Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap();

    let snapshot = parse_codex_rate_limits(&result, collected, "main");

    assert_eq!(snapshot.provider, "codex");
    assert_eq!(snapshot.profile_id, "main");
    assert_eq!(snapshot.collected_at, collected);
    assert_eq!(snapshot.plan_label.as_deref(), Some("Promax"));
    assert_eq!((snapshot.source, snapshot.health), (Source::ProviderReported, Health::Ok));
    assert_eq!(snapshot.bonus_resets, None, "reset credits are a separate feature");

    let [session, weekly] = snapshot.windows.as_slice() else { panic!("{:?}", snapshot.windows) };
    assert_eq!((session.id.as_str(), session.label.as_str()), ("session", "Session"));
    assert_eq!(session.used_percent, Some(12.0));
    assert_eq!(session.resets_at, Some(Utc.timestamp_opt(1793520000, 0).unwrap()));
    assert_eq!(session.source, Source::ProviderReported);
    assert_eq!((weekly.id.as_str(), weekly.label.as_str()), ("weekly", "Weekly"));
    assert_eq!(weekly.used_percent, Some(52.0));
    assert_eq!(weekly.resets_at, Some(Utc.timestamp_opt(1793900000, 0).unwrap()));
}

#[test]
fn a_weekly_only_plan_lists_just_the_weekly_window() {
    // Plans without a session limit report only one window; nothing is invented for the other.
    let result = json!({"rateLimits": {
        "planType": "free",
        "primary": {"usedPercent": 3, "windowDurationMins": 10080, "resetsAt": 1793900000},
        "secondary": null
    }});
    let snapshot = parse_codex_rate_limits(&result, Utc.timestamp_opt(1793000000, 0).unwrap(), "p");
    let [weekly] = snapshot.windows.as_slice() else { panic!("{:?}", snapshot.windows) };
    assert_eq!(weekly.label, "Weekly");
    assert_eq!(snapshot.plan_label.as_deref(), Some("Free"));
}
