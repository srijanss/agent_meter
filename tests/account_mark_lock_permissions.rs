#![cfg(unix)]
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn mode_of(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn the_lock_gets_the_configs_permissions_so_shared_users_can_lock() {
    let dir = std::env::temp_dir().join(format!("agent_meter_lockperm_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    // A group-shared config: the lock must be usable by the same users.
    let shared = dir.join("shared.toml");
    fs::write(&shared, "[accounts.a]\nprovider = \"claude\"\n").unwrap();
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o660)).unwrap();
    mark_active(&shared, "a").expect("marks shared config");
    assert_eq!(mode_of(&dir.join(".shared.toml.lock")), 0o660);

    // A private config: the lock must be no broader than the config.
    let private = dir.join("private.toml");
    fs::write(&private, "[accounts.a]\nprovider = \"claude\"\n").unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o600)).unwrap();
    mark_active(&private, "a").expect("marks private config");
    assert_eq!(mode_of(&dir.join(".private.toml.lock")), 0o600);

    let _ = fs::remove_dir_all(&dir);
}
