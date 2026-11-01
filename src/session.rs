//! Line-oriented conversation with a child process (for the Codex app-server,
//! which answers over stdio only while its stdin stays open).
//!
//! Like `CommandRunner`, this is a seam: collectors speak to the traits and
//! tests script them. Dropping a session must terminate and reap the child.

use std::time::Duration;

use crate::failure::CollectorError;
use crate::runner::CommandSpec;

pub trait LineSession {
    /// Sends one line (a newline is added).
    fn send(&mut self, line: &str) -> Result<(), CollectorError>;

    /// Next line from the child. `Unavailable` when it timed out or exited.
    fn recv(&mut self, timeout: Duration) -> Result<String, CollectorError>;
}

pub trait SessionStarter {
    /// `Unavailable` when the program cannot be started.
    fn start(&self, spec: &CommandSpec) -> Result<Box<dyn LineSession>, CollectorError>;
}
