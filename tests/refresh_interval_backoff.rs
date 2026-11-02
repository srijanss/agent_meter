use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::failure::CollectorError;
use agent_meter::snapshot::parse_snapshot;

const INTERVAL: Duration = Duration::from_secs(60);

fn config(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_backoff_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    fs::write(&path, "[accounts.work]\nprovider = \"claude\"\n").unwrap();
    path
}

/// Jobs that always fail, counting their runs.
fn counting_jobs(runs: Arc<AtomicUsize>) -> JobFactory {
    Box::new(move |_| {
        let runs = runs.clone();
        Box::new(move || {
            runs.fetch_add(1, Ordering::SeqCst);
            Err(CollectorError::Unavailable)
        })
    })
}

fn settle(app: &mut App, at: Instant) {
    for _ in 0..200 {
        app.poll(at);
        if !app.refreshing() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("collection did not finish");
}

#[test]
fn automatic_refresh_waits_longer_after_each_consecutive_failure() {
    let runs = Arc::new(AtomicUsize::new(0));
    let mut app = App::new(config("fail"), Path::new("/home/u"), counting_jobs(runs.clone()), Duration::from_secs(1))
        .unwrap()
        .with_auto_refresh(INTERVAL);
    let t0 = Instant::now();
    let count = || runs.load(Ordering::SeqCst);

    app.tick(t0);
    settle(&mut app, t0);
    assert_eq!(count(), 1, "the first automatic refresh is due at once");

    // One failure: wait twice the interval.
    app.tick(t0 + Duration::from_secs(119));
    settle(&mut app, t0 + Duration::from_secs(119));
    assert_eq!(count(), 1);
    app.tick(t0 + Duration::from_secs(120));
    settle(&mut app, t0 + Duration::from_secs(120));
    assert_eq!(count(), 2);

    // Two failures: wait four times the interval.
    app.tick(t0 + Duration::from_secs(359));
    settle(&mut app, t0 + Duration::from_secs(359));
    assert_eq!(count(), 2);
    app.tick(t0 + Duration::from_secs(360));
    settle(&mut app, t0 + Duration::from_secs(360));
    assert_eq!(count(), 3);
}

/// Jobs that fail twice, then succeed.
fn recovering_jobs(runs: Arc<AtomicUsize>) -> JobFactory {
    Box::new(move |_| {
        let runs = runs.clone();
        Box::new(move || {
            if runs.fetch_add(1, Ordering::SeqCst) < 2 {
                return Err(CollectorError::Unavailable);
            }
            Ok(parse_snapshot(
                r#"{"schema_version":1,"provider":"claude","profile_id":"work","collected_at":"2026-10-14T00:00:00Z",
                "source":"provider_reported","health":"ok","windows":[],"bonus_resets":null}"#,
            )
            .unwrap())
        })
    })
}

#[test]
fn a_success_returns_automatic_refresh_to_the_plain_interval() {
    let runs = Arc::new(AtomicUsize::new(0));
    let mut app = App::new(config("recover"), Path::new("/home/u"), recovering_jobs(runs.clone()), Duration::from_secs(1))
        .unwrap()
        .with_auto_refresh(INTERVAL);
    let t0 = Instant::now();
    let count = || runs.load(Ordering::SeqCst);
    let mut step = |offset: u64| {
        let at = t0 + Duration::from_secs(offset);
        app.tick(at);
        settle(&mut app, at);
    };

    step(0);
    step(120);
    step(360); // the third run succeeds
    assert_eq!(count(), 3);
    step(419);
    assert_eq!(count(), 3, "a success waits one plain interval");
    step(420);
    assert_eq!(count(), 4);
}
