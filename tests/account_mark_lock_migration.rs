#![cfg(unix)]
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn an_existing_owner_only_lock_is_widened_to_the_configs_permissions() {
    let dir = std::env::temp_dir().join(format!("agent_meter_lockmig_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shared.toml");
    fs::write(&path, "[accounts.a]\nprovider = \"claude\"\n\n[accounts.b]\nprovider = \"claude\"\n")
        .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();
    // Left behind by the earlier version, which always created locks 0600.
    let lock = dir.join(".shared.toml.lock");
    fs::write(&lock, "").unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o600)).unwrap();

    mark_active(&path, "b").expect("marks");

    assert_eq!(fs::metadata(&lock).unwrap().permissions().mode() & 0o777, 0o660);
    let _ = fs::remove_dir_all(&dir);
}
