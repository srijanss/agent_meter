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
    use agent_meter::account::{mark_active_failing_at, WriteStage};
    let primary = id("-g")[0];
    let Some(other) = id("-G").into_iter().find(|g| *g != primary) else { return };
    let dir = std::env::temp_dir().join(format!("agent_meter_lockgrp2_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shared.toml");
    // Already active, so the config is not rewritten and keeps its own group.
    fs::write(&path, "[accounts.a]\nprovider = \"claude\"\nactive = true\n").unwrap();
    chown(&path, None, Some(other)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    // A pre-existing lock in a different group than the config's.
    let lock = dir.join(".shared.toml.lock");
    fs::write(&lock, "").unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o660)).unwrap();
    assert_ne!(
        fs::metadata(&lock).unwrap().gid(),
        fs::metadata(&path).unwrap().gid(),
        "the mismatch must exist before the injected failure"
    );

    mark_active_failing_at(&path, "a", WriteStage::GroupChange).expect("marks");
    assert_eq!(fs::metadata(&lock).unwrap().permissions().mode() & 0o077, 0);

    // Repeating still meets the (unchanged) mismatch and keeps the restriction.
    assert_ne!(fs::metadata(&lock).unwrap().gid(), fs::metadata(&path).unwrap().gid());
    mark_active_failing_at(&path, "a", WriteStage::GroupChange).expect("marks again");
    assert_eq!(fs::metadata(&lock).unwrap().permissions().mode() & 0o077, 0);
    let _ = fs::remove_dir_all(&dir);
}
