#![cfg(unix)]
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::symlink;

const CONFIG: &str = "[accounts.a]\nprovider = \"claude\"\n\n[accounts.b]\nprovider = \"claude\"\n";

#[test]
fn a_planted_temp_symlink_never_redirects_the_write() {
    let dir = std::env::temp_dir().join(format!("agent_meter_symlink_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");
    fs::write(&path, CONFIG).unwrap();
    let victim = dir.join("victim.txt");
    fs::write(&victim, "precious").unwrap();
    // The old predictable temp name.
    symlink(&victim, dir.join(".agents.toml.tmp")).unwrap();

    mark_active(&path, "b").expect("marks");

    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    assert!(fs::read_to_string(&path).unwrap().contains("active = true"));
    let _ = fs::remove_dir_all(&dir);
}
