#![cfg(unix)]
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

use agent_meter::failure::CollectorError;
use agent_meter::runner::{CommandRunner, CommandSpec, Limits, SystemRunner};

fn alive(pid: &str) -> bool {
    Command::new("kill").args(["-0", pid]).output().map(|o| o.status.success()).unwrap_or(false)
}

#[test]
fn a_timed_out_command_takes_its_descendants_down_with_it() {
    let dir = std::env::temp_dir().join(format!("agent_meter_pgrp_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let pidfile = dir.join("child.pid");
    // The descendant keeps the stdout pipe open and outlives its parent shell.
    let script = format!("sleep 30 & echo $! > '{}'; wait", pidfile.display());
    let spec = CommandSpec::new("sh", ["-c", script.as_str()]).unwrap();
    let limits = Limits { timeout: Duration::from_millis(500), max_output: 4096 };

    let started = Instant::now();
    let result = SystemRunner.run(&spec, &limits);
    assert_eq!(result, Err(CollectorError::Unavailable));
    assert!(started.elapsed() < Duration::from_secs(3));

    let pid = fs::read_to_string(&pidfile).expect("descendant started").trim().to_string();
    let deadline = Instant::now() + Duration::from_secs(3);
    while alive(&pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!alive(&pid), "descendant {pid} survived the timeout");
    let _ = fs::remove_dir_all(&dir);
}
