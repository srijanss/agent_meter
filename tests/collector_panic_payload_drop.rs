use agent_meter::failure::{collect_all, AccountOutcome, CollectorError, Collect};
use agent_meter::snapshot::parse_snapshot;

struct PanicsOnDrop;

impl Drop for PanicsOnDrop {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            panic!("payload destructor panics");
        }
    }
}

#[test]
fn a_panic_payload_whose_drop_panics_does_not_stop_later_accounts() {
    let next = parse_snapshot(
        r#"{"schema_version":1,"provider":"codex","profile_id":"next","collected_at":"2026-10-14T00:00:00Z",
        "source":"provider_reported","health":"ok","windows":[],"bonus_resets":null}"#,
    )
    .unwrap();
    let accounts: Vec<(String, Collect)> = vec![
        ("bad".into(), Box::new(|| std::panic::panic_any(PanicsOnDrop))),
        ("next".into(), Box::new(move || Ok(next))),
    ];

    let outcomes = collect_all(accounts);

    assert_eq!(outcomes.len(), 2);
    assert_eq!(outcomes[0].1, AccountOutcome::Failed(CollectorError::Internal));
    assert!(matches!(outcomes[1].1, AccountOutcome::Snapshot(_)));
}

struct PanicsWithPanickingPayload;

impl Drop for PanicsWithPanickingPayload {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            std::panic::panic_any(PanicsOnDrop);
        }
    }
}

#[test]
fn a_nested_panic_payload_whose_drop_panics_does_not_stop_later_accounts() {
    let accounts: Vec<(String, Collect)> = vec![
        ("bad".into(), Box::new(|| std::panic::panic_any(PanicsWithPanickingPayload))),
        ("next".into(), Box::new(|| Err(CollectorError::RateLimited))),
    ];

    let outcomes = collect_all(accounts);

    assert_eq!(outcomes.len(), 2);
    assert_eq!(outcomes[0].1, AccountOutcome::Failed(CollectorError::Internal));
    assert_eq!(outcomes[1].0, "next");
    assert_eq!(outcomes[1].1, AccountOutcome::Failed(CollectorError::RateLimited));
}
