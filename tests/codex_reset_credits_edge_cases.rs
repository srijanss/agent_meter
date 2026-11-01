use agent_meter::codex_limits::parse_codex_rate_limits;
use agent_meter::snapshot::{BonusResets, Source};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

fn credit(status: &str, expires_at: Value) -> Value {
    json!({"id": "c", "grantedAt": 1, "resetType": "codexRateLimits", "status": status, "expiresAt": expires_at})
}

fn bonus(credits: Value) -> Option<BonusResets> {
    let result = json!({
        "rateLimits": {"primary": {"usedPercent": 5, "windowDurationMins": 300}},
        "rateLimitResetCredits": credits
    });
    parse_codex_rate_limits(&result, Utc.timestamp_opt(1793000000, 0).unwrap(), "p").bonus_resets
}

fn at(secs: i64) -> Option<chrono::DateTime<Utc>> {
    Some(Utc.timestamp_opt(secs, 0).unwrap())
}

#[test]
fn the_available_count_is_authoritative_when_the_list_is_capped_or_missing() {
    // The backend may cap the list: 5 available but only 2 detail rows.
    let capped = bonus(json!({"availableCount": 5, "credits": [
        credit("available", json!(1795000000)), credit("available", json!(1794000000))]}))
    .unwrap();
    assert_eq!((capped.remaining, capped.expires_at), (Some(5), at(1794000000)));

    // No detail rows at all: only the count is known, so there is no expiry to show.
    let count_only = bonus(json!({"availableCount": 3, "credits": null})).unwrap();
    assert_eq!((count_only.remaining, count_only.expires_at), (Some(3), None));
    let no_key = bonus(json!({"availableCount": 3})).unwrap();
    assert_eq!((no_key.remaining, no_key.expires_at), (Some(3), None));
}

#[test]
fn without_a_usable_count_the_available_rows_are_counted() {
    let rows = json!([credit("available", json!(1794000000)), credit("available", json!(null)), credit("redeemed", json!(1))]);
    for bad_count in [json!(null), json!(-1), json!("2"), json!(2.5), json!(u64::MAX)] {
        let b = bonus(json!({"availableCount": bad_count.clone(), "credits": rows.clone()}))
            .unwrap_or_else(|| panic!("{bad_count}"));
        assert_eq!(b.remaining, Some(2), "{bad_count}: counted from the rows");
        assert_eq!(b.expires_at, at(1794000000), "{bad_count}");
    }
    // Nothing usable anywhere: not reported rather than an invented zero.
    for nothing in [json!({"availableCount": null, "credits": null}), json!({}), json!({"availableCount": "x"})] {
        assert_eq!(bonus(nothing.clone()), None, "{nothing}");
    }
}

#[test]
fn expiry_ignores_credits_that_are_not_available_or_never_expire() {
    // Never-expiring available credits have no expiry; others' expiries still count.
    let mixed = bonus(json!({"availableCount": 2, "credits": [
        credit("available", json!(null)), credit("available", json!(1795000000)),
        credit("redeemed", json!(1700000000))]}))
    .unwrap();
    assert_eq!(mixed.expires_at, at(1795000000));
    let never = bonus(json!({"availableCount": 1, "credits": [credit("available", json!(null))]})).unwrap();
    assert_eq!(never.expires_at, None);
    // Bad timestamps are ignored, not fatal.
    let bad = bonus(json!({"availableCount": 2, "credits": [
        credit("available", json!("soon")), credit("available", json!(i64::MAX)),
        credit("available", json!(1794000000))]}))
    .unwrap();
    assert_eq!(bad.expires_at, at(1794000000));
}

#[test]
fn malformed_shapes_never_panic_and_are_not_reported() {
    for bad in [json!("many"), json!(7), json!([1, 2]), json!(true)] {
        assert_eq!(bonus(bad.clone()), None, "{bad}");
    }
    // With a valid count, unreadable rows are skipped for the expiry only.
    let b = bonus(json!({"availableCount": 1, "credits": [
        1, "x", null, {"status": 5}, {"id": "no-status"}, credit("available", json!(1794000000))]}))
    .unwrap();
    assert_eq!((b.remaining, b.expires_at), (Some(1), at(1794000000)));
    // A non-array credits value is treated like missing rows.
    let b = bonus(json!({"availableCount": 4, "credits": "none"})).unwrap();
    assert_eq!((b.remaining, b.source), (Some(4), Source::ProviderReported));
}
