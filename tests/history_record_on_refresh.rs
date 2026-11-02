//! A completed refresh records each reported account into the configured
//! history; recording problems are a notice, never a failed refresh.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_meter::app::{App, JobFactory};
use agent_meter::failure::CollectorError;
use agent_meter::history::HistoryStore;
use chrono::{TimeZone, Utc};
use agent_meter::snapshot::{parse_snapshot, Snapshot};

fn dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_rec_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn config(dir: &Path, history: &str) -> PathBuf {
    let path = dir.join("config.toml");
    fs::write(
        &path,
        format!("{history}\n[accounts.work]\nprovider = \"claude\"\n[accounts.home]\nprovider = \"claude\"\n"),
    )
    .unwrap();
    path
}

fn snapshot(name: &str) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"{name}","plan_label":"Pro",
        "collected_at":"2026-10-14T01:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":10.0,"resets_at":"2026-10-14T23:00:00Z",
        "source":"provider_reported"}}],"bonus_resets":null}}"#
    ))
    .unwrap()
}

fn ok_jobs() -> JobFactory {
    Box::new(|account| {
        let name = account.name.clone();
        Box::new(move || Ok(snapshot(&name)))
    })
}

fn failing_jobs() -> JobFactory {
    Box::new(|_| Box::new(|| Err(CollectorError::Unavailable)))
}

fn settle(app: &mut App, now: Instant) {
    for _ in 0..200 {
        app.poll(now);
        if !app.refreshing() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("collection did not finish");
}

/// Polls until `done` holds: recording happens off the polling thread, so its
/// effects arrive a little after the collections finish.
fn wait_until(app: &mut App, now: Instant, mut done: impl FnMut(&App) -> bool) {
    for _ in 0..400 {
        app.poll(now);
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the awaited recording effect never arrived");
}

fn run_refresh(config: PathBuf, jobs: JobFactory) -> App {
    let mut app = App::new(config, Path::new("/home/u"), jobs, Duration::from_secs(1)).unwrap();
    let now = Instant::now();
    app.refresh(now);
    settle(&mut app, now);
    app
}

#[test]
fn each_reported_account_is_recorded_once_per_completed_refresh() {
    let dir = dir("on");
    let history = dir.join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    let mut app = run_refresh(cfg, ok_jobs());
    let store = HistoryStore::new(history, 100);
    let now = Instant::now();
    wait_until(&mut app, now, |_| store.load().len() == 2);

    let mut accounts: Vec<String> = store.load().into_iter().map(|r| r.account).collect();
    accounts.sort();
    assert_eq!(accounts, ["home", "work"]);
    // More polls neither duplicate records nor raise a notice.
    for _ in 0..5 {
        app.poll(now);
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(store.load().len(), 2);
    assert_eq!(app.notice(), None);

    // A later refresh adds exactly one record per successful account.
    let later = now + Duration::from_secs(5);
    app.refresh(later);
    settle(&mut app, later);
    wait_until(&mut app, later, |_| store.load().len() == 4);
    std::thread::sleep(Duration::from_millis(50));
    app.poll(later);
    assert_eq!(store.load().len(), 4);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_recorded_when_history_is_off_or_collection_fails() {
    let dir = dir("off");
    let app = run_refresh(config(&dir, ""), ok_jobs());
    assert_eq!(app.notice(), None);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "only the config file exists");

    let history = dir.join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    run_refresh(cfg, failing_jobs());
    assert!(HistoryStore::new(history, 100).load().is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_recording_failure_is_a_fixed_notice_and_the_refresh_still_shows_its_data() {
    let dir = dir("fail");
    // The history path sits below a regular file, so it can never be written.
    fs::write(dir.join("blocker"), "x").unwrap();
    let bad = dir.join("blocker").join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", bad.display()));
    let mut app = run_refresh(cfg, ok_jobs());

    assert!(app.entries().iter().all(|e| e.snapshot.is_some()), "data shown despite the failure");
    wait_until(&mut app, Instant::now(), |app| app.notice().is_some());
    let notice = app.notice().expect("a notice reports the recording failure");
    assert!(notice.contains("history"), "{notice}");
    assert!(!notice.contains("blocker"), "no path in the notice: {notice}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_busy_history_file_never_holds_up_polling_or_the_data_shown() {
    let dir = dir("busy");
    let history = dir.join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    // Another process holds the history's lock, as a concurrent recorder would.
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("h.jsonl.lock"))
        .unwrap();
    lock.lock().unwrap();
    // It is released after a while, so a blocking implementation fails the
    // timing check below instead of hanging the test.
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        lock.unlock().unwrap();
    });

    let started = Instant::now();
    let mut app = App::new(cfg, Path::new("/home/u"), ok_jobs(), Duration::from_secs(1)).unwrap();
    app.refresh(started);
    settle(&mut app, started);
    assert!(started.elapsed() < Duration::from_millis(1000), "polling waited on the history lock");
    assert!(app.entries().iter().all(|e| e.snapshot.is_some()), "data shown while history is busy");

    // Once the lock is released the waiting records are written.
    release.join().unwrap();
    let store = HistoryStore::new(history, 100);
    wait_until(&mut app, Instant::now(), |_| store.load().len() == 2);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn closing_the_app_flushes_accepted_history_without_waiting_forever() {
    let dir = dir("flush");
    let history = dir.join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("h.jsonl.lock"))
        .unwrap();
    lock.lock().unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        lock.unlock().unwrap();
    });

    let now = Instant::now();
    let mut app = App::new(cfg, Path::new("/home/u"), ok_jobs(), Duration::from_secs(1)).unwrap();
    app.refresh(now);
    settle(&mut app, now);
    drop(app);

    // Dropping the app waited for the accepted records to reach the file.
    assert_eq!(HistoryStore::new(history, 100).load().len(), 2);
    release.join().unwrap();
    let _ = fs::remove_dir_all(&dir);
}

/// Jobs whose snapshots carry the current value of `round` as their
/// `collected_at` second, so every refresh's records are distinguishable.
fn round_jobs(round: Arc<AtomicU64>) -> JobFactory {
    Box::new(move |account| {
        let name = account.name.clone();
        let round = round.clone();
        Box::new(move || {
            let mut s = snapshot(&name);
            s.collected_at = Utc.timestamp_opt(1_800_000_000 + round.load(Ordering::SeqCst) as i64, 0).unwrap();
            Ok(s)
        })
    })
}

fn locked(dir: &Path) -> fs::File {
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("h.jsonl.lock"))
        .unwrap();
    lock.lock().unwrap();
    lock
}

#[test]
fn a_full_history_queue_keeps_polling_responsive_reports_it_and_recovers() {
    let dir = dir("overflow");
    let history = dir.join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    let lock = locked(&dir);
    let round = Arc::new(AtomicU64::new(0));
    let mut app = App::new(cfg, Path::new("/home/u"), round_jobs(round.clone()), Duration::from_secs(1)).unwrap();
    let start = Instant::now();
    // The writer is stuck on the lock, so far more refreshes than the queue
    // holds are submitted; each must return at once.
    for n in 1..=30u64 {
        round.store(n, Ordering::SeqCst);
        let at = start + Duration::from_secs(5 * n);
        app.refresh(at);
        settle(&mut app, at);
    }
    assert!(start.elapsed() < Duration::from_secs(5), "polling waited on a full history queue");
    assert!(app.notice().is_some(), "dropped batches are reported");
    assert!(app.entries().iter().all(|e| e.snapshot.is_some()));

    // Once the lock is released the accepted batches drain, oldest first.
    lock.unlock().unwrap();
    let store = HistoryStore::new(history, 1000);
    let mut at = start + Duration::from_secs(1000);
    // A refresh that is distinguishable from every earlier one, repeated until
    // the queue has room for it. Writes are in order, so once it shows up every
    // batch accepted before it has been written.
    round.store(999, Ordering::SeqCst);
    let recorded = |store: &HistoryStore| store.load().iter().any(|r| r.recorded_at.timestamp() == 1_800_000_999);
    for _ in 0..200 {
        at += Duration::from_secs(5);
        app.refresh(at);
        settle(&mut app, at);
        std::thread::sleep(Duration::from_millis(10));
        if recorded(&store) {
            break;
        }
    }
    wait_until(&mut app, at, |_| recorded(&store));

    // Each refresh yields one or two batches, and at most 9 were ever accepted
    // (8 queued plus 1 in the writer), so only the oldest rounds survive, in order.
    let rounds: Vec<i64> =
        store.load().iter().map(|r| r.recorded_at.timestamp() - 1_800_000_000).filter(|&n| n != 999).collect();
    assert!(!rounds.is_empty() && rounds.len() <= 18, "only accepted batches were kept: {rounds:?}");
    assert!(rounds.windows(2).all(|w| w[0] <= w[1]), "written in order: {rounds:?}");
    assert_eq!(rounds[0], 1, "the oldest batch was kept: {rounds:?}");
    assert!(*rounds.last().unwrap() <= 9, "later batches were dropped, not queued: {rounds:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dropping_the_app_gives_up_after_the_flush_limit_and_the_worker_still_finishes() {
    let dir = dir("deadline");
    let history = dir.join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    let lock = locked(&dir);
    let now = Instant::now();
    let mut app = App::new(cfg, Path::new("/home/u"), ok_jobs(), Duration::from_secs(1)).unwrap();
    for n in 1..=3u64 {
        let at = now + Duration::from_secs(5 * n);
        app.refresh(at);
        settle(&mut app, at);
    }

    // The lock is held well past the flush limit: closing must not wait for it.
    let started = Instant::now();
    drop(app);
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(1500), "gave up before the flush limit: {waited:?}");
    assert!(waited < Duration::from_secs(4), "closing waited on a stuck history: {waited:?}");

    // The worker keeps going in the background and writes what was accepted.
    lock.unlock().unwrap();
    let store = HistoryStore::new(history, 100);
    for _ in 0..400 {
        if store.load().len() == 6 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(store.load().len(), 6, "every accepted batch is written after the lock frees");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_mixed_refresh_records_only_the_accounts_that_reported() {
    let dir = dir("mixed");
    let history = dir.join("h.jsonl");
    let cfg = config(&dir, &format!("[history]\npath = \"{}\"\n", history.display()));
    let jobs: JobFactory = Box::new(|account| {
        let name = account.name.clone();
        Box::new(move || {
            if name == "home" {
                Err(CollectorError::Unavailable)
            } else {
                Ok(snapshot(&name))
            }
        })
    });
    let mut app = run_refresh(cfg, jobs);
    let store = HistoryStore::new(history, 100);
    let now = Instant::now();
    wait_until(&mut app, now, |_| store.load().len() == 1);

    // Later refreshes add one record for the reporting account, never a stale
    // copy for the failing one.
    for round in 1..=2u64 {
        let at = now + Duration::from_secs(5 * round);
        app.refresh(at);
        settle(&mut app, at);
        wait_until(&mut app, at, |_| store.load().len() == 1 + round as usize);
    }
    std::thread::sleep(Duration::from_millis(50));
    let records = store.load();
    assert_eq!(records.len(), 3);
    assert!(records.iter().all(|r| r.account == "work"));
    let _ = fs::remove_dir_all(&dir);
}
