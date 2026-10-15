#![cfg(unix)]
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::{chown, MetadataExt, PermissionsExt};
use std::process::Command;

fn id(flag: &str) -> Vec<u32> {
    let out = Command::new("id").arg(flag).output().expect("id");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect()
}

#[test]
fn an_existing_lock_in_the_wrong_group_is_moved_to_the_configs_group() {
    let primary = id("-g")[0];
    let others: Vec<u32> = id("-G").into_iter().filter(|g| *g != primary).collect();
    if others.len() < 2 {
        return; // needs two supplementary groups to arrange a mismatch
    }
    let (config_group, lock_group) = (others[0], others[1]);
    let dir = std::env::temp_dir().join(format!("agent_meter_lockex_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shared.toml");
    fs::write(&path, "[accounts.a]\nprovider = \"claude\"\n\n[accounts.b]\nprovider = \"claude\"\n")
        .unwrap();
    chown(&path, None, Some(config_group)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();
    let lock = dir.join(".shared.toml.lock");
    fs::write(&lock, "").unwrap();
    chown(&lock, None, Some(lock_group)).unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o660)).unwrap();

    mark_active(&path, "b").expect("marks");

    let meta = fs::metadata(&lock).unwrap();
    assert_eq!(meta.gid(), config_group);
    assert_eq!(meta.permissions().mode() & 0o777, 0o660);
    let _ = fs::remove_dir_all(&dir);
}
