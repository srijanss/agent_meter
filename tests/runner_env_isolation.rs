#![cfg(unix)]
use std::process::Command;
use std::time::Duration;

use agent_meter::runner::{CommandRunner, CommandSpec, Limits, SystemRunner};

const PROBE: &str = "AGENT_METER_ENV_PROBE";
const SENTINEL: &str = "AGENT_METER_LEAK";

/// Runs only inside the helper process spawned by the test below, where the
/// host environment really does contain the sentinel.
#[test]
fn env_probe() {
    if std::env::var_os(PROBE).is_none() {
        return;
    }
    assert!(std::env::var_os(SENTINEL).is_some(), "helper must start with the sentinel set");
    let spec = CommandSpec::new("sh", ["-c", "printf '%s' \"${AGENT_METER_LEAK:-unset}\""]).unwrap();
    let limits = Limits { timeout: Duration::from_secs(5), max_output: 1024 };
    let out = SystemRunner.run(&spec, &limits).expect("runs");
    assert_eq!(out.stdout, "unset", "host environment leaked into the child");
}

#[test]
fn host_environment_does_not_reach_the_child() {
    let mut helper = Command::new(std::env::current_exe().unwrap());
    helper.args(["--exact", "env_probe", "--test-threads=1"]);
    helper.env(PROBE, "1");
    helper.env(SENTINEL, "super-secret");
    let status = helper.status().expect("helper runs");
    assert!(status.success());
}
