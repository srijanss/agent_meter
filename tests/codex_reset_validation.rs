use agent_meter::codex_limits::parse_codex_rate_limits;
use agent_meter::snapshot::Source;
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

fn parse_with_reset(resets_at: Option<Value>) -> agent_meter::snapshot::Window {
    let mut window = json!({"usedPercent": 40, "windowDurationMins": 300});
    if let Some(value) = resets_at {
        window["resetsAt"] = value;
    }
    let result = json!({"rateLimits": {"primary": window}});
    parse_codex_rate_limits(&result, Utc.timestamp_opt(1793000000, 0).unwrap(), "p")
        .windows
        .remove(0)
}

#[test]
fn a_bad_reset_time_leaves_usage_intact_and_only_the_reset_unknown() {
    let bad: [(&str, Option<Value>); 9] = [
        ("absent", None),
        ("null", Some(json!(null))),
        ("string", Some(json!("soon"))),
        ("numeric string", Some(json!("1793900000"))),
        ("float", Some(json!(1793900000.5))),
        ("i64::MAX", Some(json!(i64::MAX))),
        ("i64::MIN", Some(json!(i64::MIN))),
        ("object", Some(json!({"at": 1}))),
        ("array", Some(json!([1793900000]))),
    ];
    for (name, value) in bad {
        let window = parse_with_reset(value);
        assert_eq!(window.used_percent, Some(40.0), "{name}: usage is kept");
        assert_eq!(window.source, Source::ProviderReported, "{name}: still provider reported");
        assert_eq!(window.resets_at, None, "{name}: the reset is unknown");
    }
}

#[test]
fn a_valid_reset_time_is_converted_exactly() {
    let window = parse_with_reset(Some(json!(1793900000)));
    assert_eq!(window.resets_at, Some(Utc.timestamp_opt(1793900000, 0).unwrap()));
    // The epoch itself is a valid instant, not "missing".
    let window = parse_with_reset(Some(json!(0)));
    assert_eq!(window.resets_at, Some(Utc.timestamp_opt(0, 0).unwrap()));
}
