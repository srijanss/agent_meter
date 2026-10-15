#![cfg(unix)]
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::MetadataExt;

#[test]
fn marking_the_already_active_account_does_not_rewrite_the_config() {
    let dir = std::env::temp_dir().join(format!("agent_meter_noop_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");
    // Formatting a rewrite would normalise; a no-op must leave it byte for byte.
    let config = "# keep me\n[accounts.a]\nprovider = \"claude\"\nactive = true\n\n[accounts.b]\nprovider   = \"claude\"\n";
    fs::write(&path, config).unwrap();
    let inode = fs::metadata(&path).unwrap().ino();

    mark_active(&path, "a").expect("already active is a success");

    assert_eq!(fs::read_to_string(&path).unwrap(), config);
    assert_eq!(fs::metadata(&path).unwrap().ino(), inode, "file was replaced");
    let _ = fs::remove_dir_all(&dir);
}
