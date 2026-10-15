#![cfg(unix)]
use std::time::{Duration, Instant};

use agent_meter::failure::CollectorError;
use agent_meter::runner::{CommandRunner, CommandSpec, Limits, Output, SystemRunner};

fn limits() -> Limits {
    Limits { timeout: Duration::from_secs(5), max_output: 4096 }
}

fn sh(script: &str) -> CommandSpec {
    CommandSpec::new("sh", ["-c", script]).expect("valid")
}

#[test]
fn spec_rejects_shell_strings_paths_and_foreign_env() {
    assert!(CommandSpec::new("", ["x"]).is_err());
    assert!(CommandSpec::new("claude --version", ["x"]).is_err(), "no shell strings");
    assert!(CommandSpec::new("/bin/sh", ["x"]).is_err(), "bare executable names only");
    assert!(CommandSpec::new("../codex", ["x"]).is_err());
    assert!(CommandSpec::new("claude", ["a\0b"]).is_err());
    let spec = CommandSpec::new("claude", ["-p"]).unwrap();
    assert!(spec.with_env("CLAUDE_CONFIG_DIR", "/tmp/x").is_ok());
    let spec = CommandSpec::new("claude", ["-p"]).unwrap();
    assert!(spec.with_env("LD_PRELOAD", "/tmp/evil.so").is_err());
    let spec = CommandSpec::new("claude", ["-p"]).unwrap();
    assert!(spec.with_env("CODEX_HOME", "bad\0value").is_err());
}

#[test]
fn captures_stdout_and_success() {
    let out = SystemRunner.run(&sh("printf hello"), &limits()).expect("runs");
    assert_eq!(out, Output { stdout: "hello".into(), success: true });
}

#[test]
fn passes_only_the_declared_account_environment() {
    let spec = sh("printf '%s|%s' \"$CODEX_HOME\" \"${AGENT_METER_LEAK:-unset}\"")
        .with_env("CODEX_HOME", "/accounts/work")
        .unwrap();
    let out = SystemRunner.run(&spec, &limits()).expect("runs");
    assert_eq!(out.stdout, "/accounts/work|unset");
}

#[test]
fn nonzero_exit_is_reported_not_hidden() {
    let out = SystemRunner.run(&sh("exit 3"), &limits()).expect("runs");
    assert!(!out.success);
}

#[test]
fn a_missing_executable_is_unavailable() {
    let spec = CommandSpec::new("agent-meter-no-such-cli", ["x"]).unwrap();
    assert_eq!(SystemRunner.run(&spec, &limits()), Err(CollectorError::Unavailable));
}

#[test]
fn a_hung_command_times_out_and_is_killed() {
    let tight = Limits { timeout: Duration::from_millis(200), max_output: 4096 };
    let started = Instant::now();
    let result = SystemRunner.run(&sh("sleep 10"), &tight);
    assert_eq!(result, Err(CollectorError::Unavailable));
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn output_beyond_the_cap_is_rejected() {
    let small = Limits { timeout: Duration::from_secs(5), max_output: 1000 };
    let started = Instant::now();
    let result = SystemRunner.run(&sh("yes | head -c 1000000"), &small);
    assert_eq!(result, Err(CollectorError::Malformed));
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_fake_runner_can_stand_in() {
    struct Fake;
    impl CommandRunner for Fake {
        fn run(&self, spec: &CommandSpec, _: &Limits) -> Result<Output, CollectorError> {
            assert_eq!(spec.program(), "codex");
            Ok(Output { stdout: "canned".into(), success: true })
        }
    }
    let runner: &dyn CommandRunner = &Fake;
    let spec = CommandSpec::new("codex", ["app-server"]).unwrap();
    assert_eq!(runner.run(&spec, &limits()).unwrap().stdout, "canned");
}
