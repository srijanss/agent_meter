use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::failure::CollectorError;

const INTERVAL: Duration = Duration::from_secs(60);

/// Jobs that count their start, then wait for a message before failing.
fn held_jobs(started: Arc<AtomicUsize>, release: Receiver<()>) -> JobFactory {
    let release = Arc::new(Mutex::new(release));
    Box::new(move |_| {
        let started = started.clone();
        let release = release.clone();
        Box::new(move || {
            started.fetch_add(1, Ordering::SeqCst);
            let _ = release.lock().unwrap().recv_timeout(Duration::from_secs(10));
            Err(CollectorError::Unavailable)
        })
    })
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..400 {
        if done() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {what}");
}

#[test]
fn ticks_and_manual_refreshes_do_not_duplicate_a_running_job_and_a_manual_result_sets_the_schedule() {
    let dir = tempfile_dir();
    let path = dir.join("config.toml");
    fs::write(&path, "[accounts.work]\nprovider = \"claude\"\n").unwrap();
    let started = Arc::new(AtomicUsize::new(0));
    let (release, held) = channel();
    let mut app = App::new(path, Path::new("/home/u"), held_jobs(started.clone(), held), Duration::from_secs(1))
        .unwrap()
        .with_auto_refresh(INTERVAL);
    let count = || started.load(Ordering::SeqCst);
    let t0 = Instant::now();

    app.tick(t0);
    wait_for("the job to start", || count() == 1);

    // While it runs, neither more ticks nor a manual refresh start another.
    app.tick(t0 + Duration::from_secs(1000));
    app.refresh(t0 + Duration::from_secs(1000));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(count(), 1);
    assert!(app.refreshing());

    // It fails at t1; a manual refresh after the gate interval runs again.
    let t1 = t0 + Duration::from_secs(1001);
    release.send(()).unwrap();
    wait_for("the failure", || {
        app.poll(t1);
        !app.refreshing()
    });
    let t2 = t1 + Duration::from_secs(2);
    app.refresh(t2);
    wait_for("the manual job to start", || count() == 2);
    let t3 = t2 + Duration::from_secs(1);
    release.send(()).unwrap();
    wait_for("the manual failure", || {
        app.poll(t3);
        !app.refreshing()
    });

    // Two failures in a row: the next automatic refresh is 4x the interval
    // after the manual one finished.
    app.tick(t3 + Duration::from_secs(239));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(count(), 2);
    app.tick(t3 + Duration::from_secs(240));
    wait_for("the automatic job to start", || count() == 3);
    let _ = release.send(());
    let _ = fs::remove_dir_all(&dir);
}

fn tempfile_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_inflight_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
