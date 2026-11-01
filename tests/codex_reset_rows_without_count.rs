use agent_meter::codex_limits::parse_codex_rate_limits;
use agent_meter::snapshot::BonusResets;
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

fn bonus(credits: Value) -> Option<BonusResets> {
    let result = json!({
        "rateLimits": {"primary": {"usedPercent": 5, "windowDurationMins": 300}},
        "rateLimitResetCredits": credits
    });
    parse_codex_rate_limits(&result, Utc.timestamp_opt(1793000000, 0).unwrap(), "p").bonus_resets
}

fn row(status: Value) -> Value {
    json!({"id": "c", "grantedAt": 1, "resetType": "codexRateLimits", "status": status})
}

#[test]
fn rows_that_cannot_be_interpreted_never_become_a_definite_count() {
    // No usable availableCount, so the rows are all there is to go on.
    for rows in [
        json!([null, {"status": 5}]),
        json!([1, "x", []]),
        json!([{"id": "no-status"}]),
        json!([row(json!("available")), null]),
        json!([row(json!("available")), {"status": 7}, row(json!("redeemed"))]),
        json!([row(json!(null))]),
    ] {
        assert_eq!(
            bonus(json!({"availableCount": null, "credits": rows.clone()})),
            None,
            "{rows}: a lower bound is not a count"
        );
    }
}

#[test]
fn a_fully_interpretable_list_is_counted_exactly() {
    // Every row has a readable status (including statuses this code has never seen).
    let counted = |rows: Value| bonus(json!({"credits": rows})).map(|b| b.remaining);
    assert_eq!(counted(json!([])), Some(Some(0)), "details fetched, none available");
    assert_eq!(counted(json!([row(json!("redeemed"))])), Some(Some(0)));
    assert_eq!(counted(json!([row(json!("available")), row(json!("expired-someday"))])), Some(Some(1)));
    assert_eq!(counted(json!([row(json!("available")), row(json!("available"))])), Some(Some(2)));

    // A valid availableCount still wins over unreadable rows.
    let b = bonus(json!({"availableCount": 3, "credits": [null, {"status": 5}]})).unwrap();
    assert_eq!(b.remaining, Some(3));
}
