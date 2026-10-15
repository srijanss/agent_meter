#![cfg(unix)]
use std::time::Duration;

use agent_meter::runner::{CommandRunner, CommandSpec, Limits, SystemRunner};

#[test]
fn extreme_limits_do_not_panic() {
    let spec = CommandSpec::new("sh", ["-c", "printf ok"]).unwrap();
    let limits = Limits { timeout: Duration::MAX, max_output: usize::MAX };
    let out = SystemRunner.run(&spec, &limits).expect("runs");
    assert_eq!(out.stdout, "ok");
}
