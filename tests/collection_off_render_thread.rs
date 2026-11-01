use std::sync::mpsc;
use std::time::{Duration, Instant};

use agent_meter::collection::Collections;
use agent_meter::failure::{AccountOutcome, CollectorError};
use agent_meter::snapshot::{parse_snapshot, Snapshot};

fn snapshot(profile: &str) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"codex","profile_id":"{profile}","collected_at":"2026-10-14T00:00:00Z",
        "source":"provider_reported","health":"ok","windows":[],"bonus_resets":null}}"#
    ))
    .unwrap()
}

/// Polls until `want` results have arrived or the deadline passes.
fn drain(collections: &Collections, want: usize) -> Vec<(String, AccountOutcome)> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut got = Vec::new();
    while got.len() < want && Instant::now() < deadline {
        got.extend(collections.poll());
        std::thread::sleep(Duration::from_millis(5));
    }
    got
}

#[test]
fn a_hung_collection_never_blocks_spawn_or_poll_and_other_accounts_still_land() {
    let collections = Collections::new();
    let (release, hold) = mpsc::channel::<()>();

    let started = Instant::now();
    collections.spawn("hung".into(), move || {
        let _ = hold.recv();
        Err(CollectorError::Unavailable)
    });
    collections.spawn("quick".into(), || Ok(snapshot("quick")));
    assert!(collections.poll().len() <= 1, "poll never waits for the hung job");
    assert!(started.elapsed() < Duration::from_secs(1), "the caller was blocked");

    // The quick account's result arrives while the other is still hung.
    let first = drain(&collections, 1);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].0, "quick");
    assert!(matches!(first[0].1, AccountOutcome::Snapshot(_)));
    assert!(collections.poll().is_empty(), "the hung account has not reported");

    // Releasing it delivers its failure.
    release.send(()).unwrap();
    let second = drain(&collections, 1);
    assert_eq!(second, vec![("hung".to_string(), AccountOutcome::Failed(CollectorError::Unavailable))]);
}

#[test]
fn a_panicking_collection_reports_an_internal_failure_and_the_worker_survives() {
    let collections = Collections::new();
    collections.spawn("bad".into(), || panic!("collector bug"));
    let bad = drain(&collections, 1);
    assert_eq!(bad, vec![("bad".to_string(), AccountOutcome::Failed(CollectorError::Internal))]);

    collections.spawn("next".into(), || Err(CollectorError::RateLimited));
    let next = drain(&collections, 1);
    assert_eq!(next, vec![("next".to_string(), AccountOutcome::Failed(CollectorError::RateLimited))]);
}
