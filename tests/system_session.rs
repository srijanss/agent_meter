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
    let dir = private_dir();
    let pid_file = dir.join("pid");
    // A grandchild that would outlive the shell, recording its pid. The path is a
    // positional argument, never spliced into the script text.
    let pid_path = pid_file.to_str().unwrap();
    let spec = CommandSpec::new(
        "sh",
        ["-c", r#"sleep 60 & echo $! > "$1"; echo ready; wait"#, "sh", pid_path],
    )
    .unwrap();
    let mut session = SystemSessionStarter.start(&spec).unwrap();
    assert_eq!(session.recv(Duration::from_secs(10)).unwrap(), "ready");
    let pid: i32 = std::fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
    drop(session);
    // Bounded polling: the kill is asynchronous with the reaping of the group.
    let mut alive = true;
    for _ in 0..50 {
        alive = is_alive(pid);
        if !alive {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!alive, "grandchild {pid} survived the session");
}

/// A fresh directory only this user can enter: randomized name, created with
/// `create_dir` (fails if it exists, never follows a pre-planted path), mode 0700.
fn private_dir() -> std::path::PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    let base = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    for attempt in 0..100u32 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(attempt);
        let dir = base.join(format!("agent-meter-drop-{}-{nanos}-{attempt}", std::process::id()));
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => return dir,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("cannot create private test dir: {e}"),
        }
    }
    panic!("no unused private test dir name");
}

/// Portable liveness via `ps`: a missing or zombie process is gone. If `ps`
/// itself cannot be run the test fails; it never counts as termination.
fn is_alive(pid: i32) -> bool {
    let output = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("ps must be runnable to check liveness");
    let stat = String::from_utf8_lossy(&output.stdout);
    let stat = stat.trim();
    !(stat.is_empty() || stat.starts_with('Z'))
}

#[test]
fn sending_to_a_child_that_never_reads_never_blocks_and_drop_still_cleans_up() {
    let mut session = SystemSessionStarter.start(&sh("sleep 60")).unwrap();
    let line = "x".repeat(16 * 1024);
    let started = std::time::Instant::now();
    // Far more than any pipe buffer holds: later sends are refused, none block.
    let results: Vec<_> = (0..64).map(|_| session.send(&line)).collect();
    assert!(started.elapsed() < Duration::from_secs(5), "send blocked");
    assert!(results[0].is_ok());
    assert!(results.iter().any(Result::is_err), "backlog must be bounded");

    let dropped = std::time::Instant::now();
    drop(session);
    assert!(dropped.elapsed() < Duration::from_secs(5), "drop blocked behind a stuck write");
}
