use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use agent_meter::app::JobFactory;
use agent_meter::{claude_collector, codex_collector};
use agent_meter::failure::CollectorError;
use agent_meter::snapshot::parse_snapshot;
use agent_meter::usage_cli::{usage_report, UsageError, DEFAULT_TIMEOUT};

fn snapshot_json(name: &str) -> String {
    format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"{name}","plan_label":"Pro",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":42.0,"resets_at":null,
        "source":"provider_reported"}}],"bonus_resets":null}}"#
    )
}

/// Accounts named `ok*` report a snapshot, `slow*` block for a long time, the rest fail.
fn jobs() -> JobFactory {
    Box::new(|account| {
        let name = account.name.clone();
        Box::new(move || {
            if name.starts_with("slow") {
                thread::sleep(Duration::from_secs(3));
                Err(CollectorError::Unavailable)
            } else if name.starts_with("ok") {
                Ok(parse_snapshot(&snapshot_json(&name)).unwrap())
            } else {
                Err(CollectorError::Unavailable)
            }
        })
    })
}

fn config(tag: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_exit_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    fs::write(&path, text).unwrap();
    path
}

const LONG: Duration = Duration::from_secs(30);

#[test]
fn each_failure_kind_has_its_own_nonzero_exit_code() {
    let invalid = usage_report(Path::new("/nonexistent/config.toml"), Path::new("/home/u"), jobs(), LONG)
        .unwrap_err();
    assert!(matches!(invalid, UsageError::Invalid(_)));
    assert_eq!(invalid.exit_code(), 2);
    assert_eq!(UsageError::Timeout.exit_code(), 3);
    assert_eq!(UsageError::NoData.exit_code(), 4);
}

#[test]
fn no_account_reporting_is_no_data_but_a_partial_report_succeeds() {
    let none = config("none", "[accounts.a]\nprovider = \"claude\"\n[accounts.b]\nprovider = \"codex\"\n");
    let err = usage_report(&none, Path::new("/home/u"), jobs(), LONG).unwrap_err();
    assert!(matches!(err, UsageError::NoData), "{err:?}");

    let some = config("some", "[accounts.ok1]\nprovider = \"claude\"\n[accounts.b]\nprovider = \"codex\"\n");
    let text = usage_report(&some, Path::new("/home/u"), jobs(), LONG).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["accounts"].as_array().unwrap().len(), 2);
}

#[test]
fn a_collector_that_never_finishes_ends_in_a_timeout_within_the_bound() {
    let slow = config("slow", "[accounts.slow1]\nprovider = \"claude\"\n");
    let started = Instant::now();
    let err = usage_report(&slow, Path::new("/home/u"), jobs(), Duration::from_millis(100)).unwrap_err();
    assert!(matches!(err, UsageError::Timeout), "{err:?}");
    assert!(started.elapsed() < Duration::from_secs(2), "returned at the bound, not when the job ended");
}

#[test]
fn the_default_bound_outlasts_every_collectors_own_limits_so_none_is_orphaned() {
    // A child process is only guaranteed reaped by its collector's own deadline,
    // so giving up earlier than that could leave one running after we exit.
    assert!(DEFAULT_TIMEOUT > claude_collector::MAX_COLLECTION);
    assert!(DEFAULT_TIMEOUT > codex_collector::MAX_COLLECTION);
}

#[test]
fn an_unrepresentable_timeout_means_no_bound_rather_than_a_panic() {
    let some = config("max", "[accounts.ok1]\nprovider = \"claude\"\n");
    let text = usage_report(&some, Path::new("/home/u"), jobs(), Duration::MAX).unwrap();
    assert!(text.contains("\"ok1\""));
}
