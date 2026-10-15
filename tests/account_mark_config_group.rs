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
fn rewriting_the_config_keeps_its_group_and_permissions() {
    let primary = id("-g")[0];
    let Some(other) = id("-G").into_iter().find(|g| *g != primary) else {
        return; // no second group to prove it with
    };
    let dir = std::env::temp_dir().join(format!("agent_meter_cfggrp_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shared.toml");
    fs::write(&path, "[accounts.a]\nprovider = \"claude\"\n\n[accounts.b]\nprovider = \"claude\"\n")
        .unwrap();
    chown(&path, None, Some(other)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();

    mark_active(&path, "b").expect("marks");

    let meta = fs::metadata(&path).unwrap();
    assert_eq!(meta.gid(), other);
    assert_eq!(meta.permissions().mode() & 0o777, 0o660);
    assert!(fs::read_to_string(&path).unwrap().contains("active = true"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_group_preservation_leaves_the_shared_config_untouched() {
    use agent_meter::account::{mark_active_failing_at, WriteStage};
    let primary = id("-g")[0];
    let Some(other) = id("-G").into_iter().find(|g| *g != primary) else { return };
    let dir = std::env::temp_dir().join(format!("agent_meter_cfggrp2_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shared.toml");
    let text = "[accounts.a]\nprovider = \"claude\"\n\n[accounts.b]\nprovider = \"claude\"\n";
    fs::write(&path, text).unwrap();
    chown(&path, None, Some(other)).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).unwrap();

    let result = mark_active_failing_at(&path, "b", WriteStage::GroupChange);

    assert!(result.is_err(), "must not replace a shared config it cannot keep shared");
    let meta = fs::metadata(&path).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    assert_eq!(meta.gid(), other);
    assert_eq!(meta.permissions().mode() & 0o777, 0o660);
    let leftovers = fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(leftovers, 0);
    let _ = fs::remove_dir_all(&dir);
}
