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
