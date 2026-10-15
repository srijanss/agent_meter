#![cfg(unix)]
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use agent_meter::failure::CollectorError;
use agent_meter::runner::{CommandRunner, CommandSpec, Limits, SystemRunner};

/// Running, not merely present: a killed orphan stays a zombie until its
/// reaper collects it, which says nothing about whether termination worked.
fn alive(pid: &str) -> bool {
    if let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) {
        // State is the first field after the parenthesised command name.
        let state = stat.rsplit_once(')').and_then(|(_, rest)| rest.trim_start().chars().next());
        return !matches!(state, Some('Z') | Some('X'));
    }
    Command::new("ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .map(|o| {
            let stat = String::from_utf8_lossy(&o.stdout);
            o.status.success() && !stat.trim().is_empty() && !stat.trim_start().starts_with('Z')
        })
        .unwrap_or(false)
}

/// Scratch directory plus the descendant's pid; the descendant is killed on
/// drop so a failing assertion never leaves it running.
struct Scene {
    dir: PathBuf,
}

impl Scene {
    fn new(tag: &str) -> Self {
        // A quote in the path proves it is passed as data, never spliced into the script.
        let dir = std::env::temp_dir()
            .join(format!("agent_meter_pgrp_'_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn pidfile(&self) -> PathBuf {
        self.dir.join("child.pid")
    }

    fn pid(&self) -> String {
        fs::read_to_string(self.pidfile()).expect("descendant started").trim().to_string()
    }

    fn assert_descendant_gone(&self, what: &str) {
        let pid = self.pid();
        let deadline = Instant::now() + Duration::from_secs(3);
        while alive(&pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!alive(&pid), "descendant {pid} survived {what}");
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        if let Ok(pid) = fs::read_to_string(self.pidfile()) {
            let _ = Command::new("kill").args(["-9", pid.trim()]).output();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn run_with_descendant(scene: &Scene, tail: &str, limits: &Limits) -> Result<(), CollectorError> {
    // The descendant keeps the stdout pipe open and outlives its parent shell.
    let script = format!("sleep 30 & echo $! > \"$1\"; {tail}");
    let path = scene.pidfile();
    let spec =
        CommandSpec::new("sh", ["-c", script.as_str(), "sh", path.to_str().unwrap()]).unwrap();
    SystemRunner.run(&spec, limits).map(|_| ())
}

#[test]
fn a_timed_out_command_takes_its_descendants_down_with_it() {
    let scene = Scene::new("timeout");
    let limits = Limits { timeout: Duration::from_millis(500), max_output: 4096 };

    let started = Instant::now();
    let result = run_with_descendant(&scene, "wait", &limits);
    assert_eq!(result, Err(CollectorError::Unavailable));
    assert!(started.elapsed() < Duration::from_secs(3));

    scene.assert_descendant_gone("the timeout");
}

#[test]
fn an_oversized_command_takes_its_descendants_down_with_it() {
    let scene = Scene::new("overflow");
    let limits = Limits { timeout: Duration::from_secs(5), max_output: 100 };

    let started = Instant::now();
    // Overflow the cap while the descendant is still running.
    let result = run_with_descendant(&scene, "yes | head -c 100000; wait", &limits);
    assert_eq!(result, Err(CollectorError::Malformed));
    assert!(started.elapsed() < Duration::from_secs(3));

    scene.assert_descendant_gone("the output overflow");
}
