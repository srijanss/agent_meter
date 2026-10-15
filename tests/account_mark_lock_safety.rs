#![cfg(unix)]
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

const CONFIG: &str = "[accounts.a]\nprovider = \"claude\"\n";

fn setup(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("agent_meter_lock_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");
    fs::write(&path, CONFIG).unwrap();
    (dir, path)
}

#[test]
fn a_planted_lock_symlink_is_rejected_and_never_followed() {
    let (dir, path) = setup("link");
    let victim = dir.join("victim.txt");
    fs::write(&victim, "precious").unwrap();
    symlink(&victim, dir.join(".agents.toml.lock")).unwrap();

    let result = mark_active(&path, "a");

    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    assert_eq!(fs::read_to_string(&path).unwrap(), CONFIG);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_planted_lock_fifo_fails_promptly_instead_of_hanging() {
    let (dir, path) = setup("fifo");
    let fifo = dir.join(".agents.toml.lock");
    let made = Command::new("mkfifo").arg(&fifo).status().map(|s| s.success()).unwrap_or(false);
    if !made {
        let _ = fs::remove_dir_all(&dir);
        return;
    }
    let (tx, rx) = mpsc::channel();
    let worker_path = path.clone();
    std::thread::spawn(move || {
        let _ = tx.send(mark_active(&worker_path, "a"));
    });

    let result = rx.recv_timeout(Duration::from_secs(5)).expect("mark_active hung on a FIFO");

    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), CONFIG);
    let _ = fs::remove_dir_all(&dir);
}
