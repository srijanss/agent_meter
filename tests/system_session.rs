#![cfg(unix)]

use std::time::Duration;

use agent_meter::failure::CollectorError;
use agent_meter::runner::CommandSpec;
use agent_meter::session::{LineSession, SessionStarter};
use agent_meter::system_session::SystemSessionStarter;

fn sh(script: &str) -> CommandSpec {
    CommandSpec::new("sh", ["-c", script]).unwrap()
}

#[test]
fn a_child_is_conversed_with_line_by_line_and_exit_is_unavailable() {
    // Echoes each line back prefixed, then exits after one exchange.
    let spec = sh(r#"read line; echo "got:$line""#);
    let mut session = SystemSessionStarter.start(&spec).unwrap();
    session.send("hello").unwrap();
    assert_eq!(session.recv(Duration::from_secs(5)).unwrap(), "got:hello");
    // The child has exited: no more lines, never a hang.
    assert_eq!(session.recv(Duration::from_secs(5)), Err(CollectorError::Unavailable));

    // A program that does not exist cannot start.
    let missing = CommandSpec::new("agent-meter-no-such-program", Vec::<String>::new()).unwrap();
    assert_eq!(SystemSessionStarter.start(&missing).err(), Some(CollectorError::Unavailable));
}

#[test]
fn a_line_too_big_to_write_without_blocking_is_refused_not_hung() {
    // The child never reads stdin; a payload past the pipe capacity would block forever.
    let mut session = SystemSessionStarter.start(&sh("sleep 30")).unwrap();
    let huge = "x".repeat(1024 * 1024);
    let started = std::time::Instant::now();
    assert_eq!(session.send(&huge), Err(CollectorError::Internal));
    assert!(started.elapsed() < Duration::from_secs(5));
    // Small lines still go through.
    assert_eq!(session.send("{}"), Ok(()));
}

#[test]
fn a_silent_child_times_out_and_hostile_output_is_refused() {
    // Alive but silent: the wait ends at the timeout.
    let mut silent = SystemSessionStarter.start(&sh("sleep 30")).unwrap();
    let started = std::time::Instant::now();
    assert_eq!(silent.recv(Duration::from_millis(200)), Err(CollectorError::Unavailable));
    assert!(started.elapsed() < Duration::from_secs(5));

    // A line over the cap is Malformed, not buffered without bound.
    let mut long = SystemSessionStarter.start(&sh("head -c 2000000 /dev/zero | tr '\\0' x; echo")).unwrap();
    assert_eq!(long.recv(Duration::from_secs(10)), Err(CollectorError::Malformed));

    // Output that is not UTF-8 is Malformed.
    let mut bad = SystemSessionStarter.start(&sh(r"printf '\377\376\n'")).unwrap();
    assert_eq!(bad.recv(Duration::from_secs(10)), Err(CollectorError::Malformed));

    // A child flooding lines faster than they are read does not hang the session.
    let mut flood = SystemSessionStarter.start(&sh("yes line")).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(flood.recv(Duration::from_secs(5)).unwrap(), "line");
}

#[test]
fn dropping_a_session_kills_the_child_and_its_descendants() {
    let dir = std::env::temp_dir().join(format!("agent-meter-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pid_file = dir.join("pid");
    // A grandchild that would outlive the shell, recording its pid.
    let script = format!("sleep 60 & echo $! > '{}'; echo ready; wait", pid_file.display());
    let mut session = SystemSessionStarter.start(&sh(&script)).unwrap();
    assert_eq!(session.recv(Duration::from_secs(10)).unwrap(), "ready");
    let pid: i32 = std::fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
    drop(session);
    std::thread::sleep(Duration::from_millis(300));
    let alive = std::path::Path::new(&format!("/proc/{pid}")).exists()
        && !std::fs::read_to_string(format!("/proc/{pid}/stat")).map(|s| s.contains(") Z")).unwrap_or(true);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!alive, "grandchild {pid} survived the session");
}
