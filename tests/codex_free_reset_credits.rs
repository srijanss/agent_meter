use agent_meter::codex_limits::parse_codex_rate_limits;
use agent_meter::snapshot::{BonusResets, Source};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

fn credit(status: &str, expires_at: Value) -> Value {
    json!({
        "id": "c", "grantedAt": 1790000000, "resetType": "codexRateLimits",
        "status": status, "expiresAt": expires_at, "title": null, "description": null
    })
}

fn parse(credits: Value) -> agent_meter::snapshot::Snapshot {
    let result = json!({
        "rateLimits": {"planType": "promax",
            "primary": {"usedPercent": 5, "windowDurationMins": 300, "resetsAt": 1793900000}},
        "rateLimitResetCredits": credits
    });
    parse_codex_rate_limits(&result, Utc.timestamp_opt(1793000000, 0).unwrap(), "main")
}

#[test]
fn available_reset_credits_become_the_bonus_count_and_next_expiry() {
    let snapshot = parse(json!({
        "availableCount": 2,
        "credits": [
            credit("available", json!(1795000000)),
            credit("available", json!(1794000000)),
            credit("redeemed", json!(1793500000)),
            credit("redeeming", json!(1793400000)),
            credit("unknown", json!(1793300000)),
        ]
    }));
    assert_eq!(
        snapshot.bonus_resets,
        Some(BonusResets {
            remaining: Some(2),
            // The earliest expiry among the *available* credits only.
            expires_at: Some(Utc.timestamp_opt(1794000000, 0).unwrap()),
            source: Source::ProviderReported,
        })
    );
}

#[test]
fn absent_credits_are_not_reported_but_zero_available_is_zero() {
    // Not reported: the field is absent or null. Never shown as 0.
    let result = json!({"rateLimits": {"primary": {"usedPercent": 5, "windowDurationMins": 300}}});
    let absent = parse_codex_rate_limits(&result, Utc.timestamp_opt(0, 0).unwrap(), "p");
    assert_eq!(absent.bonus_resets, None);
    assert_eq!(parse(json!(null)).bonus_resets, None);

    // Reported, and none are available: that is a real zero.
    let none_available = parse(json!({"availableCount": 0, "credits": []}));
    assert_eq!(
        none_available.bonus_resets,
        Some(BonusResets { remaining: Some(0), expires_at: None, source: Source::ProviderReported })
    );
    let only_redeemed = parse(json!({"availableCount": 0, "credits": [credit("redeemed", json!(1794000000))]}));
    assert_eq!(only_redeemed.bonus_resets.unwrap().remaining, Some(0));
}
