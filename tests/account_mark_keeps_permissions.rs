#![cfg(unix)]
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn marking_keeps_a_restricted_config_restricted() {
    let dir = std::env::temp_dir().join(format!("agent_meter_perm_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");
    fs::write(&path, "[accounts.a]\nprovider = \"claude\"\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    mark_active(&path, "a").expect("marks");

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let _ = fs::remove_dir_all(&dir);
}
