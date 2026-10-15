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
fn a_new_lock_joins_the_configs_group_not_the_creators_default() {
    let primary = id("-g")[0];
    let Some(other) = id("-G").into_iter().find(|g| *g != primary) else {
        return; // no second group to prove it with
    };
    let dir = std::env::temp_dir().join(format!("agent_meter_lockgrp_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shared.toml");
    fs::write(&path, "[accounts.a]\nprovider = \"claude\"\n").unwrap();
    chown(&path, None, Some(other)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();

    mark_active(&path, "a").expect("marks");

    let lock = fs::metadata(dir.join(".shared.toml.lock")).unwrap();
    assert_eq!(lock.gid(), other);
    assert_eq!(lock.permissions().mode() & 0o777, 0o660);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_lock_that_cannot_join_the_configs_group_drops_group_and_other_access() {
    // Without privilege a gid we are not in cannot be assigned; the lock must
    // then not stay group-accessible to the creator's unrelated default group.
    let groups = id("-G");
    let Some(foreign) = (60000u32..60100).find(|g| !groups.contains(g)) else { return };
    let dir = std::env::temp_dir().join(format!("agent_meter_lockgrp2_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shared.toml");
    fs::write(&path, "[accounts.a]\nprovider = \"claude\"\n").unwrap();
    if chown(&path, None, Some(foreign)).is_err() {
        let _ = fs::remove_dir_all(&dir);
        return; // cannot arrange a foreign group without privilege
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();

    mark_active(&path, "a").expect("marks");

    let lock = fs::metadata(dir.join(".shared.toml.lock")).unwrap();
    assert_eq!(lock.permissions().mode() & 0o070, 0, "group bits kept for the wrong group");
    let _ = fs::remove_dir_all(&dir);
}
