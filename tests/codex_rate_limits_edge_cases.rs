use agent_meter::codex_limits::parse_codex_rate_limits;
use agent_meter::snapshot::{Health, Source};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

fn parse(rate_limits: Value) -> agent_meter::snapshot::Snapshot {
    parse_codex_rate_limits(&json!({ "rateLimits": rate_limits }), Utc.timestamp_opt(1793000000, 0).unwrap(), "p")
}

fn window(mins: Value) -> Value {
    json!({"usedPercent": 10, "windowDurationMins": mins, "resetsAt": 1793900000})
}

#[test]
fn window_titles_come_from_the_duration() {
    let labels = |mins: Value| {
        let s = parse(json!({"primary": window(mins)}));
        (s.windows[0].id.clone(), s.windows[0].label.clone())
    };
    assert_eq!(labels(json!(300)), ("session".into(), "Session".into()));
    assert_eq!(labels(json!(10080)), ("weekly".into(), "Weekly".into()));
    assert_eq!(labels(json!(60)), ("1h".into(), "1h window".into()));
    assert_eq!(labels(json!(1440)), ("24h".into(), "24h window".into()));
    assert_eq!(labels(json!(90)), ("90m".into(), "90m window".into()));
    // Unknown or nonsensical lengths fall back to the slot, never a guessed title.
    assert_eq!(labels(json!(null)), ("primary".into(), "Primary".into()));
    assert_eq!(labels(json!(0)), ("primary".into(), "Primary".into()));
    assert_eq!(labels(json!(-5)), ("primary".into(), "Primary".into()));
    assert_eq!(labels(json!("300")), ("primary".into(), "Primary".into()));

    let both = parse(json!({"primary": window(json!(null)), "secondary": window(json!(null))}));
    assert_eq!(both.windows[1].label, "Secondary");
}

#[test]
fn missing_or_malformed_data_is_unknown_never_a_panic() {
    for bad in [json!(null), json!("nope"), json!([1, 2]), json!({}), json!({"primary": null, "secondary": null})] {
        let s = parse(bad.clone());
        assert!(s.windows.is_empty(), "{bad}");
        assert_eq!((s.source, s.health), (Source::Unknown, Health::Unsupported), "{bad}");
    }
    // A result with no rateLimits at all.
    let s = parse_codex_rate_limits(&json!({"unrelated": 1}), Utc.timestamp_opt(0, 0).unwrap(), "p");
    assert!(s.windows.is_empty());
    let s = parse_codex_rate_limits(&json!(null), Utc.timestamp_opt(0, 0).unwrap(), "p");
    assert!(s.windows.is_empty());

    // Bad fields inside a window leave just those fields unknown.
    let s = parse(json!({"primary": {"usedPercent": "lots", "windowDurationMins": 300, "resetsAt": "soon"}}));
    let w = &s.windows[0];
    assert_eq!((w.used_percent, w.resets_at), (None, None));
    let s = parse(json!({"primary": {"usedPercent": -4, "windowDurationMins": 300, "resetsAt": i64::MAX}}));
    assert_eq!((s.windows[0].used_percent, s.windows[0].resets_at), (None, None), "negative and out-of-range");
    // A missing percentage means the window is unknown, and so is its reset.
    let s = parse(json!({"primary": {"windowDurationMins": 300, "resetsAt": 1793900000}}));
    assert_eq!(s.windows[0].used_percent, None);
    assert_eq!(s.windows[0].source, Source::Unknown);
}

#[test]
fn plan_names_are_made_readable_and_unknown_is_dropped() {
    let plan = |p: Value| parse(json!({"planType": p, "primary": window(json!(300))})).plan_label;
    assert_eq!(plan(json!("promax")).as_deref(), Some("Promax"));
    assert_eq!(plan(json!("plus")).as_deref(), Some("Plus"));
    assert_eq!(plan(json!("self_serve_business_prolite")).as_deref(), Some("Self Serve Business Prolite"));
    assert_eq!(plan(json!("unknown")), None);
    assert_eq!(plan(json!("")), None);
    assert_eq!(plan(json!(null)), None);
    assert_eq!(plan(json!(7)), None);
}

#[test]
fn only_the_single_bucket_view_is_used() {
    let result = json!({
        "rateLimits": {"primary": window(json!(300))},
        "rateLimitsByLimitId": {"other": {"primary": {"usedPercent": 99, "windowDurationMins": 60, "resetsAt": 1}}}
    });
    let s = parse_codex_rate_limits(&result, Utc.timestamp_opt(1793000000, 0).unwrap(), "p");
    assert_eq!(s.windows.len(), 1);
    assert_eq!(s.windows[0].used_percent, Some(10.0));
}
