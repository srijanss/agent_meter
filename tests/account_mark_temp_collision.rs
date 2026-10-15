#![cfg(unix)]
//! Single test on purpose: temp names are `.<file>.<pid>.<counter>.tmp`, and
//! this test binary's first calls use counters 0, 1, ...
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::symlink;

const CONFIG: &str = "[accounts.a]\nprovider = \"claude\"\n\n[accounts.b]\nprovider = \"claude\"\n";

#[test]
fn colliding_temp_paths_are_skipped_and_never_touched() {
    let dir = std::env::temp_dir().join(format!("agent_meter_collide_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");
    fs::write(&path, CONFIG).unwrap();
    let victim = dir.join("victim.txt");
    fs::write(&victim, "precious").unwrap();
    let pid = std::process::id();
    let planted_link = dir.join(format!(".agents.toml.{pid}.0.tmp"));
    let planted_file = dir.join(format!(".agents.toml.{pid}.1.tmp"));
    symlink(&victim, &planted_link).unwrap();
    fs::write(&planted_file, "stale").unwrap();

    mark_active(&path, "b").expect("marks despite collisions");

    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    assert!(fs::symlink_metadata(&planted_link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&planted_file).unwrap(), "stale");
    assert!(fs::read_to_string(&path).unwrap().contains("active = true"));
    let _ = fs::remove_dir_all(&dir);
}
