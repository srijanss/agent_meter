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
