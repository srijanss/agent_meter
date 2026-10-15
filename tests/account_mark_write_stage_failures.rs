use agent_meter::account::{mark_active_failing_at, WriteStage};
use std::fs;

const CONFIG: &str =
    "[accounts.a]\nprovider = \"claude\"\n# sk-sentinel-secret\n\n[accounts.b]\nprovider = \"claude\"\n";

#[test]
fn a_failure_at_any_write_stage_keeps_the_original_and_cleans_up() {
    for stage in [
        WriteStage::PermissionCopy,
        WriteStage::Write,
        WriteStage::Sync,
        WriteStage::Rename,
    ] {
        let dir = std::env::temp_dir()
            .join(format!("agent_meter_stage_{}_{stage:?}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("agents.toml");
        fs::write(&path, CONFIG).unwrap();

        let err = mark_active_failing_at(&path, "b", stage).expect_err("injected failure");

        assert_eq!(fs::read(&path).unwrap(), CONFIG.as_bytes(), "{stage:?}: original changed");
        let mut names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert!(
            names.iter().all(|n| !n.ends_with(".tmp")),
            "{stage:?}: temp file left: {names:?}"
        );
        let shown = format!("{err} {err:?}");
        assert!(!shown.contains("sentinel"), "{stage:?}: {shown}");
        assert!(!shown.contains(dir.to_string_lossy().as_ref()), "{stage:?}: {shown}");
        assert_eq!(err.to_string(), "cannot write config", "{stage:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}
