#![cfg(unix)]
use agent_meter::account::mark_active_racing;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

#[test]
fn a_symlink_planted_after_the_absence_check_never_changes_the_victim() {
    let dir = std::env::temp_dir().join(format!("agent_meter_lockrace_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");
    let config = "[accounts.a]\nprovider = \"claude\"\n";
    fs::write(&path, config).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    let victim = dir.join("victim.txt");
    fs::write(&victim, "precious").unwrap();
    fs::set_permissions(&victim, fs::Permissions::from_mode(0o600)).unwrap();

    let lock = dir.join(".agents.toml.lock");
    let result = mark_active_racing(&path, "a", &|| {
        symlink(&victim, &lock).unwrap();
    });

    assert!(result.is_err());
    assert_eq!(fs::metadata(&victim).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    assert_eq!(fs::read_to_string(&path).unwrap(), config);
    let _ = fs::remove_dir_all(&dir);
}
