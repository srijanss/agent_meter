use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::failure::{AccountOutcome, CollectorError};
use agent_meter::outcome::{entry_for, unavailable_reason};
use agent_meter::snapshot::parse_snapshot;

fn account(name: &str) -> Account {
    Account { name: name.into(), provider: Provider::Claude, home: PathBuf::from("/x"), active: false }
}

#[test]
fn a_failed_collector_keeps_the_account_listed_without_a_snapshot() {
    for error in [
        CollectorError::Unavailable,
        CollectorError::SignInRequired,
        CollectorError::RateLimited,
        CollectorError::Malformed,
        CollectorError::Internal,
    ] {
        let entry = entry_for(account("Work"), AccountOutcome::Failed(error));
        assert_eq!(entry.account.name, "Work", "{error:?}: still listed");
        assert!(entry.snapshot.is_none(), "{error:?}: usage is unavailable, not zero");
    }
}

#[test]
fn a_measured_account_carries_its_snapshot() {
    let snapshot = parse_snapshot(
        r#"{"schema_version":1,"provider":"claude","profile_id":"p","plan_label":null,
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok","windows":[]}"#,
    )
    .unwrap();
    let entry = entry_for(account("Work"), AccountOutcome::Snapshot(snapshot.clone()));
    assert_eq!(entry.snapshot, Some(snapshot));
}

#[test]
fn each_failure_has_a_distinct_plain_reason_for_the_hover() {
    assert_eq!(unavailable_reason(CollectorError::Unavailable), "CLI not found or provider unreachable");
    assert_eq!(unavailable_reason(CollectorError::SignInRequired), "Sign-in required");
    let reasons: std::collections::HashSet<_> = [
        CollectorError::Unavailable,
        CollectorError::SignInRequired,
        CollectorError::RateLimited,
        CollectorError::Malformed,
        CollectorError::Internal,
    ]
    .into_iter()
    .map(unavailable_reason)
    .collect();
    assert_eq!(reasons.len(), 5);
}
