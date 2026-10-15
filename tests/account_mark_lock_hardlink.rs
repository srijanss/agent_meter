#![cfg(unix)]
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn a_hard_linked_lock_is_rejected_and_the_victim_keeps_its_permissions() {
    let dir = std::env::temp_dir().join(format!("agent_meter_lockhl_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");
    let config = "[accounts.a]\nprovider = \"claude\"\n\n[accounts.b]\nprovider = \"claude\"\n";
    fs::write(&path, config).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    let victim = dir.join("victim.txt");
    fs::write(&victim, "precious").unwrap();
    fs::set_permissions(&victim, fs::Permissions::from_mode(0o600)).unwrap();
    fs::hard_link(&victim, dir.join(".agents.toml.lock")).unwrap();

    let result = mark_active(&path, "b");

    assert!(result.is_err());
    assert_eq!(fs::metadata(&victim).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    assert_eq!(fs::read_to_string(&path).unwrap(), config);
    let _ = fs::remove_dir_all(&dir);
}
