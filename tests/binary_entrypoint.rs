use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_meter::account::Account;
use agent_meter::app::{default_config_path, load_accounts, App, JobFactory};
use agent_meter::failure::CollectorError;
use agent_meter::snapshot::{parse_snapshot, Snapshot};
use chrono::{TimeZone, Utc};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

const AREA: Rect = Rect { x: 0, y: 0, width: 48, height: 20 };
const HOME: &str = "/home/u";
const CONFIG: &str = r#"
[accounts.claude-a]
provider = "claude"
active = true

[accounts.claude-b]
provider = "claude"
home = "/data/b"

[accounts.codex-x]
provider = "codex"
active = true
"#;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn config_in(dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("config.toml");
    fs::write(&path, text).unwrap();
    path
}

fn snapshot(provider: &str, used: f64) -> Snapshot {
    parse_snapshot(&format!(
        r#"{{"schema_version":1,"provider":"{provider}","profile_id":"p","plan_label":"Pro",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":{used},"resets_at":null,"source":"provider_reported"}}],
        "bonus_resets":null}}"#
    ))
    .unwrap()
}

/// Scripted collectors: results per account, a count of started jobs, and a
/// gate that keeps jobs "hung" until released.
#[derive(Clone)]
struct Script {
    results: Arc<Mutex<HashMap<String, Result<Snapshot, CollectorError>>>>,
    started: Arc<AtomicUsize>,
    release: Arc<AtomicBool>,
}

impl Script {
    fn new(release: bool) -> Self {
        Self {
            results: Arc::default(),
            started: Arc::default(),
            release: Arc::new(AtomicBool::new(release)),
        }
    }

    fn set(&self, account: &str, result: Result<Snapshot, CollectorError>) {
        self.results.lock().unwrap().insert(account.to_string(), result);
    }

    fn factory(&self) -> JobFactory {
        let script = self.clone();
        Box::new(move |account: &Account| {
            script.started.fetch_add(1, Ordering::SeqCst);
            let script = script.clone();
            let name = account.name.clone();
            Box::new(move || {
                while !script.release.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                script.results.lock().unwrap().get(&name).cloned().unwrap_or(Err(CollectorError::Unavailable))
            })
        })
    }
}

fn wait_until_idle(app: &mut App, now: Instant) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.refreshing() {
        assert!(Instant::now() < deadline, "collections never finished");
        app.poll(now);
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn used(app: &App, account: &str) -> Option<Option<f64>> {
    app.entries()
        .iter()
        .find(|e| e.account.name == account)
        .map(|e| e.snapshot.as_ref().and_then(|s| s.windows[0].used_percent))
}

fn key(code: KeyCode) -> Event {
    let mut event = KeyEvent::new(code, KeyModifiers::NONE);
    event.kind = KeyEventKind::Press;
    Event::Key(event)
}

fn click(column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

// --- the binary -----------------------------------------------------------

fn run_binary(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_herdr-agents"))
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("binary runs")
}

#[test]
fn the_binary_exits_nonzero_with_a_clear_message_when_it_cannot_start() {
    let dir = scratch("bin_errors");
    let missing = dir.join("nope.toml");
    let secret = config_in(&dir, "[accounts.x]\nprovider = \"claude\"\nbogus = \"SECRETVALUE\"\n");
    let empty = dir.join("empty.toml");
    fs::write(&empty, "").unwrap();
    let fine = dir.join("fine.toml");
    fs::write(&fine, CONFIG).unwrap();

    for (path, message) in [
        (&missing, "cannot read config"),
        (&secret, "not valid TOML"),
        (&empty, "no accounts"),
        // A good config still needs a real terminal; without one it must say so, not hang.
        (&fine, "terminal"),
    ] {
        let out = run_binary(&["--config", path.to_str().unwrap()]);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{message}: exit status");
        assert!(stderr.contains(message), "{message}: stderr was {stderr:?}");
        assert!(!stderr.contains("SECRETVALUE"), "config values never reach the message");
        assert!(out.stdout.is_empty(), "nothing is printed to stdout, which Herdr owns");
    }

    let out = run_binary(&["--bogus"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage"));
}

#[test]
fn the_default_config_lives_under_the_users_config_directory() {
    assert_eq!(
        default_config_path(Path::new(HOME)),
        Path::new("/home/u/.config/agent-meter/config.toml")
    );
}

// --- the app --------------------------------------------------------------

#[test]
fn loading_accounts_reports_problems_without_echoing_config_values() {
    let dir = scratch("load_accounts");
    let ok = config_in(&dir, CONFIG);
    assert_eq!(load_accounts(&ok, Path::new(HOME)).unwrap().len(), 3);
    assert!(load_accounts(&dir.join("absent.toml"), Path::new(HOME)).is_err());
}

#[test]
fn refresh_runs_off_the_event_loop_gates_repeats_and_keeps_stale_data_on_failure() {
    let dir = scratch("app_refresh");
    let path = config_in(&dir, CONFIG);
    let script = Script::new(false);
    script.set("claude-a", Ok(snapshot("claude", 42.0)));
    script.set("claude-b", Err(CollectorError::SignInRequired));
    script.set("codex-x", Ok(snapshot("codex", 7.0)));
    let mut app = App::new(path, Path::new(HOME), script.factory(), Duration::from_secs(30)).unwrap();
    let t0 = Instant::now();

    // Before anything has been collected, every account is listed with no data.
    assert_eq!(app.entries().len(), 3);
    assert!(app.entries().iter().all(|e| e.snapshot.is_none()));
    assert!(!app.refreshing());

    // Starting a refresh returns at once even though every job is hung.
    app.refresh(t0);
    assert!(app.refreshing());
    app.refresh(t0);
    app.refresh(t0);
    std::thread::sleep(Duration::from_millis(20));
    app.poll(t0);
    assert_eq!(script.started.load(Ordering::SeqCst), 3, "repeats while in flight start nothing");
    assert!(app.refreshing());

    script.release.store(true, Ordering::SeqCst);
    wait_until_idle(&mut app, t0);
    assert_eq!(used(&app, "claude-a"), Some(Some(42.0)));
    assert_eq!(used(&app, "codex-x"), Some(Some(7.0)));
    assert_eq!(used(&app, "claude-b"), Some(None), "a failed account stays listed, without data");

    // Right after finishing, a refresh is rate limited.
    app.refresh(t0 + Duration::from_secs(1));
    assert_eq!(script.started.load(Ordering::SeqCst), 3);

    // Later a collector fails: the last good data is kept rather than cleared.
    script.set("claude-a", Err(CollectorError::Unavailable));
    script.set("claude-b", Ok(snapshot("claude", 5.0)));
    let t1 = t0 + Duration::from_secs(60);
    app.refresh(t1);
    assert_eq!(script.started.load(Ordering::SeqCst), 6);
    wait_until_idle(&mut app, t1);
    assert_eq!(used(&app, "claude-a"), Some(Some(42.0)), "stale data survives a failed refresh");
    assert_eq!(used(&app, "claude-b"), Some(Some(5.0)));
}

#[test]
fn events_close_refresh_and_switch_the_active_account() {
    let dir = scratch("app_events");
    let path = config_in(&dir, CONFIG);
    let script = Script::new(true);
    script.set("claude-a", Ok(snapshot("claude", 42.0)));
    script.set("claude-b", Ok(snapshot("claude", 11.0)));
    script.set("codex-x", Ok(snapshot("codex", 7.0)));
    let mut app = App::new(path.clone(), Path::new(HOME), script.factory(), Duration::from_secs(30)).unwrap();
    let t0 = Instant::now();

    // 'r' refreshes.
    app.handle(&key(KeyCode::Char('r')), AREA, now(), t0).unwrap();
    wait_until_idle(&mut app, t0);
    assert_eq!(script.started.load(Ordering::SeqCst), 3);
    assert_eq!(used(&app, "claude-b"), Some(Some(11.0)));

    // Choosing claude-b (the only Use link, key 1) rewrites only the config and keeps the data.
    assert!(app.entries().iter().find(|e| e.account.name == "claude-a").unwrap().account.active);
    app.handle(&key(KeyCode::Char('1')), AREA, now(), t0).unwrap();
    let active: Vec<_> = app.entries().iter().filter(|e| e.account.active).map(|e| e.account.name.as_str()).collect();
    assert_eq!(active, ["claude-b", "codex-x"]);
    assert_eq!(used(&app, "claude-b"), Some(Some(11.0)), "choosing an account keeps its data");
    assert!(fs::read_to_string(&path).unwrap().contains("claude-b"));

    // A click that is not on a link does nothing; Esc quits.
    app.handle(&click(0, 0), AREA, now(), t0).unwrap();
    assert!(!app.should_quit());
    app.handle(&key(KeyCode::Esc), AREA, now(), t0).unwrap();
    assert!(app.should_quit());
}

#[test]
fn a_failed_account_switch_reports_an_error_and_keeps_every_account_and_its_data() {
    let dir = scratch("app_switch_failure");
    let path = config_in(&dir, CONFIG);
    let script = Script::new(true);
    script.set("claude-a", Ok(snapshot("claude", 42.0)));
    script.set("claude-b", Ok(snapshot("claude", 11.0)));
    script.set("codex-x", Ok(snapshot("codex", 7.0)));
    let mut app = App::new(path.clone(), Path::new(HOME), script.factory(), Duration::from_secs(30)).unwrap();
    let t0 = Instant::now();
    app.refresh(t0);
    wait_until_idle(&mut app, t0);

    // The config is edited elsewhere into something this program cannot load.
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("\n[accounts.broken]\nprovider = \"claude\"\nunsupported = \"SECRETVALUE\"\n");
    fs::write(&path, text).unwrap();

    let error = app.handle(&key(KeyCode::Char('1')), AREA, now(), t0);
    assert!(error.is_err(), "the failed switch is reported");
    assert!(!error.unwrap_err().to_string().contains("SECRETVALUE"));
    assert_eq!(app.entries().len(), 3, "the popup keeps its accounts");
    assert_eq!(used(&app, "claude-a"), Some(Some(42.0)));
    assert_eq!(used(&app, "claude-b"), Some(Some(11.0)));
    assert_eq!(used(&app, "codex-x"), Some(Some(7.0)));

    // A switch whose config cannot be rewritten at all also changes nothing.
    fs::remove_file(&path).unwrap();
    assert!(app.handle(&key(KeyCode::Char('1')), AREA, now(), t0).is_err());
    assert_eq!(app.entries().len(), 3);
    assert_eq!(used(&app, "claude-a"), Some(Some(42.0)));
}
