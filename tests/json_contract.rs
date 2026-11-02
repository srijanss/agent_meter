//! The `usage --json` output keeps to the v1 snapshot contract: every snapshot
//! in it parses back through the same validator the collectors' output goes
//! through, and unknown/null/stale values mean what they mean in the popup.

use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::popup::AccountEntry;
use agent_meter::snapshot::{parse_snapshot, Health, Source};
use agent_meter::usage_cli::usage_json;
use chrono::{TimeZone, Utc};
use serde_json::Value;

const FULL: &str = r#"{"schema_version":1,"provider":"claude","profile_id":"work","plan_label":"Max",
    "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
    "windows":[
      {"id":"s","label":"Session","used_percent":42.5,"resets_at":"2026-10-14T05:00:00Z",
       "source":"provider_reported","scoped":[{"label":"Opus","used_percent":null}]},
      {"id":"w","label":"Weekly","used_percent":null,"resets_at":null,"source":"unknown"}],
    "bonus_resets":{"remaining":2,"expires_at":null,"source":"provider_reported"}}"#;

const UNSUPPORTED: &str = r#"{"schema_version":1,"provider":"codex","profile_id":"x","plan_label":null,
    "collected_at":"2026-10-14T00:00:00Z","source":"unknown","health":"unsupported",
    "windows":[],"bonus_resets":null}"#;

fn entry(name: &str, json: Option<&str>) -> AccountEntry {
    AccountEntry {
        account: Account {
            name: name.into(),
            provider: Provider::Claude,
            home: PathBuf::from("/x"),
            active: false,
        },
        snapshot: json.map(|j| parse_snapshot(j).unwrap()),
    }
}

fn report(entries: &[AccountEntry]) -> Value {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 1, 0, 0).unwrap();
    serde_json::from_str(&usage_json(entries, now)).unwrap()
}

#[test]
fn every_reported_snapshot_round_trips_through_the_v1_contract() {
    let entries = [entry("a", Some(FULL)), entry("b", Some(UNSUPPORTED))];
    let value = report(&entries);
    for (reported, original) in value["accounts"].as_array().unwrap().iter().zip(&entries) {
        let text = reported["snapshot"].to_string();
        let parsed = parse_snapshot(&text).expect("the CLI's own output must satisfy the contract");
        assert_eq!(Some(&parsed), original.snapshot.as_ref());
    }
}

#[test]
fn unknown_and_null_values_stay_null_rather_than_becoming_zero() {
    let value = report(&[entry("a", Some(FULL)), entry("b", Some(UNSUPPORTED)), entry("c", None)]);
    let accounts = value["accounts"].as_array().unwrap();

    let weekly = &accounts[0]["snapshot"]["windows"][1];
    assert!(weekly["used_percent"].is_null());
    assert!(weekly["resets_at"].is_null());
    assert_eq!(weekly["source"], "unknown");
    assert!(accounts[0]["snapshot"]["windows"][0]["scoped"][0]["used_percent"].is_null());

    let unsupported = parse_snapshot(&accounts[1]["snapshot"].to_string()).unwrap();
    assert_eq!(unsupported.health, Health::Unsupported);
    assert_eq!(unsupported.source, Source::Unknown);
    assert!(unsupported.windows.is_empty());

    // An account with no snapshot has neither a snapshot nor an age: unknown, not fresh.
    assert!(accounts[2]["snapshot"].is_null());
    assert!(accounts[2]["age_seconds"].is_null());
}

#[test]
fn age_lets_a_consumer_apply_the_same_staleness_rule_as_the_popup() {
    // The popup treats a snapshot older than 30 minutes as stale; the report
    // carries the age so a consumer can apply that rule without guessing.
    let at = |h, m| Utc.with_ymd_and_hms(2026, 10, 14, h, m, 0).unwrap();
    let entries = [entry("a", Some(FULL))];
    let fresh = serde_json::from_str::<Value>(&usage_json(&entries, at(0, 29))).unwrap();
    let stale = serde_json::from_str::<Value>(&usage_json(&entries, at(0, 31))).unwrap();
    assert_eq!(fresh["accounts"][0]["age_seconds"], 29 * 60);
    assert_eq!(stale["accounts"][0]["age_seconds"], 31 * 60);
    assert_eq!(agent_meter::popup::STALE_AFTER.num_seconds(), 30 * 60);
    assert!(fresh["accounts"][0]["age_seconds"].as_i64().unwrap() < agent_meter::popup::STALE_AFTER.num_seconds());
    assert!(stale["accounts"][0]["age_seconds"].as_i64().unwrap() > agent_meter::popup::STALE_AFTER.num_seconds());
}
