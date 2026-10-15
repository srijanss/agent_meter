#![cfg(unix)]
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::PermissionsExt;

const CONFIG: &str = "[accounts.a]\nprovider = \"claude\"\n# sk-sentinel-secret\n";

#[test]
fn a_failed_write_keeps_the_original_and_leaks_nothing() {
    let dir = std::env::temp_dir().join(format!("agent_meter_wfail_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");
    fs::write(&path, CONFIG).unwrap();
    // A read-only directory makes temp-file creation fail.
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
    if fs::File::create(dir.join("probe")).is_ok() {
        // Running as a user who ignores directory modes (root): cannot simulate.
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let _ = fs::remove_dir_all(&dir);
        return;
    }

    let err = mark_active(&path, "a").expect_err("write must fail");

    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), CONFIG);
    let names: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names.len(), 1, "no temp file left: {names:?}");
    assert!(!err.to_string().contains("sentinel"), "{err}");
    assert!(!format!("{err:?}").contains("sentinel"), "{err:?}");
    let _ = fs::remove_dir_all(&dir);
}
