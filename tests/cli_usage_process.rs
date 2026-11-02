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

/// A stand-in `codex app-server` that answers the two requests the collector sends.
fn install_fake_codex(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("codex");
    fs::write(
        &script,
        "#!/bin/sh\n\
         while read -r line; do\n\
           case \"$line\" in\n\
             *'\"id\":1'*) echo '{\"id\":1,\"result\":{}}' ;;\n\
             *'\"id\":2'*) echo '{\"id\":2,\"result\":{\"rateLimits\":{\"planType\":\"plus\",\"primary\":{\"usedPercent\":25,\"windowDurationMins\":300,\"resetsAt\":1900000000}}}}' ;;\n\
           esac\n\
         done\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_partial_report_is_one_json_line_with_exit_0_and_nothing_else_on_stdout() {
    let home = dir("partial");
    install_fake_codex(&home);
    let config = home.join("config.toml");
    fs::write(
        &config,
        "[accounts.work]\nprovider = \"codex\"\n[accounts.broken]\nprovider = \"claude\"\n",
    )
    .unwrap();

    let out = run(&home, &["usage", "--json", "--config", config.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "{:?}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains('\u{1b}'), "no terminal escapes: {text:?}");
    assert_eq!(text.matches('\n').count(), 1, "one line: {text:?}");
    assert!(text.ends_with('\n'));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let accounts = value["accounts"].as_array().unwrap();
    let find = |name: &str| accounts.iter().find(|a| a["name"] == name).unwrap();
    assert_eq!(find("work")["snapshot"]["provider"], "codex");
    assert_eq!(find("work")["snapshot"]["windows"][0]["used_percent"], 25.0);
    assert!(find("broken")["snapshot"].is_null());
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn no_account_reporting_exits_4_with_nothing_on_stdout() {
    let home = dir("ok");
    let config = home.join("config.toml");
    fs::write(&config, "[accounts.work]\nprovider = \"claude\"\n").unwrap();

    let out = run(&home, &["usage", "--json", "--config", config.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(4), "no data is its own exit code");
    assert!(out.stdout.is_empty(), "no report is printed: {:?}", out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no account reported usage"), "{stderr}");
    let _ = fs::remove_dir_all(&home);
}
