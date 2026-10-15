use agent_meter::failure::{collect_all, AccountOutcome, CollectorError, Collect};
use agent_meter::snapshot::{parse_snapshot, Snapshot};

fn ok_snapshot(profile: &str) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"{profile}","collected_at":"2026-10-14T00:00:00Z",
        "source":"provider_reported","health":"ok","windows":[],"bonus_resets":null}}"#
    ))
    .unwrap()
}

#[test]
fn one_failing_account_does_not_affect_the_others() {
    let accounts: Vec<(String, Collect)> = vec![
        ("a".into(), Box::new(|| Ok(ok_snapshot("a")))),
        ("b".into(), Box::new(|| Err(CollectorError::SignInRequired))),
        ("c".into(), Box::new(|| Err(CollectorError::RateLimited))),
        ("d".into(), Box::new(|| Err(CollectorError::Malformed))),
        ("e".into(), Box::new(|| Err(CollectorError::Unavailable))),
        ("f".into(), Box::new(|| panic!("collector bug"))),
        ("g".into(), Box::new(|| Ok(ok_snapshot("g")))),
    ];

    let outcomes = collect_all(accounts);

    let ids: Vec<&str> = outcomes.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c", "d", "e", "f", "g"]);
    assert!(matches!(outcomes[0].1, AccountOutcome::Snapshot(_)));
    assert_eq!(outcomes[1].1, AccountOutcome::Failed(CollectorError::SignInRequired));
    assert_eq!(outcomes[2].1, AccountOutcome::Failed(CollectorError::RateLimited));
    assert_eq!(outcomes[3].1, AccountOutcome::Failed(CollectorError::Malformed));
    assert_eq!(outcomes[4].1, AccountOutcome::Failed(CollectorError::Unavailable));
    assert_eq!(outcomes[5].1, AccountOutcome::Failed(CollectorError::Internal));
    assert!(matches!(outcomes[6].1, AccountOutcome::Snapshot(_)));
}
