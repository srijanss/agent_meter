//! Runs the real `herdr-agents` binary: the machine-readable command must keep
//! stdout to one JSON document and report bad starts with exit code 2.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_proc_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs with an empty PATH so no real provider CLI can be found.
fn run(home: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-agents"));
    command.args(args).env_clear().env("HOME", home).env("PATH", home);
    command.output().unwrap()
}

#[test]
fn bad_arguments_or_config_exit_2_with_nothing_on_stdout() {
    let home = dir("bad");
    let missing = home.join("missing.toml");
    let malformed = home.join("malformed.toml");
    fs::write(&malformed, "this is = not [valid").unwrap();
    let empty = home.join("empty.toml");
    fs::write(&empty, "").unwrap();

    let cases: Vec<Vec<&str>> = vec![
        vec!["usage"],
        vec!["usage", "--json", "extra"],
        vec!["usage", "--json", "--config", missing.to_str().unwrap()],
        vec!["usage", "--json", "--config", malformed.to_str().unwrap()],
        vec!["usage", "--json", "--config", empty.to_str().unwrap()],
    ];
    for args in cases {
        let out = run(&home, &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
        assert!(!out.stderr.is_empty(), "{args:?}");
    }
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn a_failed_collector_still_yields_one_json_document_with_a_null_snapshot() {
    let home = dir("ok");
    let config = home.join("config.toml");
    fs::write(&config, "[accounts.work]\nprovider = \"claude\"\n").unwrap();

    let out = run(&home, &["usage", "--json", "--config", config.to_str().unwrap()]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains('\u{1b}'), "no terminal escapes: {text:?}");
    assert_eq!(text.matches('\n').count(), 1, "one line: {text:?}");
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["accounts"][0]["name"], "work");
    assert!(value["accounts"][0]["snapshot"].is_null());
    let _ = fs::remove_dir_all(&home);
}
