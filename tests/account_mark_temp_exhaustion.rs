#![cfg(unix)]
//! Single test on purpose: this binary's first call uses temp counters 0..16.
use agent_meter::account::mark_active;
use std::fs;
use std::os::unix::fs::symlink;

const CONFIG: &str = "[accounts.a]\nprovider = \"claude\"\n\n[accounts.b]\nprovider = \"claude\"\n";

#[test]
fn exhausting_every_temp_candidate_fails_and_changes_nothing() {
    let dir = std::env::temp_dir().join(format!("agent_meter_exhaust_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.toml");
    fs::write(&path, CONFIG).unwrap();
    let victim = dir.join("victim.txt");
    fs::write(&victim, "precious").unwrap();
    let pid = std::process::id();
    for n in 0..16 {
        let planted = dir.join(format!(".agents.toml.{pid}.{n}.tmp"));
        if n % 2 == 0 {
            symlink(&victim, &planted).unwrap();
        } else {
            fs::write(&planted, format!("stale-{n}")).unwrap();
        }
    }

    let err = mark_active(&path, "b").expect_err("no free temp name");

    assert_eq!(err.to_string(), "cannot write config");
    assert_eq!(fs::read_to_string(&path).unwrap(), CONFIG);
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    for n in 0..16 {
        let planted = dir.join(format!(".agents.toml.{pid}.{n}.tmp"));
        let meta = fs::symlink_metadata(&planted).unwrap();
        if n % 2 == 0 {
            assert!(meta.file_type().is_symlink());
        } else {
            assert_eq!(fs::read_to_string(&planted).unwrap(), format!("stale-{n}"));
        }
    }
    let _ = fs::remove_dir_all(&dir);
}
