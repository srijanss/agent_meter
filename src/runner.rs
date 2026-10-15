//! Injectable command runner: collectors run external CLIs (claude, codex)
//! through this seam with validated argument vectors (never shell strings), an
//! allow-listed per-account environment, a timeout and an output cap.

use std::io::Read;
use std::process::{Child, Command, Stdio};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::failure::CollectorError;

/// Environment variables a collector may set for an account.
const ALLOWED_ENV: [&str; 2] = ["CLAUDE_CONFIG_DIR", "CODEX_HOME"];
/// Host variables a child still needs to locate itself and its config.
const INHERITED_ENV: [&str; 4] = ["PATH", "HOME", "USER", "LANG"];

/// A command to run. Construction validates it; the error carries no input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidCommand;

impl CommandSpec {
    /// `program` must be a bare executable name resolved through `PATH`.
    pub fn new<I, S>(program: &str, args: I) -> Result<Self, InvalidCommand>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let bare = !program.is_empty()
            && !program.starts_with('-')
            && program.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
            && program != "."
            && program != "..";
        if !bare {
            return Err(InvalidCommand);
        }
        let args: Vec<String> = args.into_iter().map(Into::into).collect();
        if args.iter().any(|a| a.contains('\0')) {
            return Err(InvalidCommand);
        }
        Ok(Self { program: program.to_string(), args, env: Vec::new() })
    }

    /// Adds an account environment variable (only `CLAUDE_CONFIG_DIR` or `CODEX_HOME`).
    pub fn with_env(mut self, name: &str, value: &str) -> Result<Self, InvalidCommand> {
        if !ALLOWED_ENV.contains(&name) || value.contains('\0') {
            return Err(InvalidCommand);
        }
        self.env.retain(|(n, _)| n != name);
        self.env.push((name.to_string(), value.to_string()));
        Ok(self)
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub timeout: Duration,
    pub max_output: usize,
}

/// Captured result of a command that ran to completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub success: bool,
}

pub trait CommandRunner {
    /// Failures: `Unavailable` (cannot start, timed out), `Malformed` (output
    /// over the cap or not UTF-8). Child stderr is discarded, never surfaced.
    fn run(&self, spec: &CommandSpec, limits: &Limits) -> Result<Output, CollectorError>;
}

/// Reads at most `cap + 1` bytes (the extra byte lets callers detect overflow
/// by length). A read failure is an error, never silently truncated output.
pub fn read_capped<R: Read>(reader: R, cap: usize) -> Result<Vec<u8>, CollectorError> {
    let mut buf = Vec::new();
    reader
        .take((cap as u64).saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|_| CollectorError::Unavailable)?;
    Ok(buf)
}

/// Kills the child and every descendant in its process group, then reaps it.
fn terminate(child: &mut Child) {
    #[cfg(unix)]
    {
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        const SIGKILL: i32 = 9;
        if let Ok(pid) = i32::try_from(child.id()) {
            // SAFETY: plain syscall; a negative pid addresses the group we created.
            unsafe { kill(-pid, SIGKILL) };
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs real processes.
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, spec: &CommandSpec, limits: &Limits) -> Result<Output, CollectorError> {
        let mut command = Command::new(&spec.program);
        command.args(&spec.args).env_clear();
        for name in INHERITED_ENV {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        for (name, value) in &spec.env {
            command.env(name, value);
        }
        // Its own process group, so the whole tree can be signalled on failure.
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| CollectorError::Unavailable)?;
        let stdout = child.stdout.take().ok_or(CollectorError::Internal)?;

        // Read one byte past the cap so oversize output is detectable, and stop
        // reading there so a flooding child cannot exhaust memory.
        let cap = limits.max_output;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(read_capped(stdout, cap));
        });

        // An unrepresentable timeout means "effectively never", not a panic.
        let now = Instant::now();
        let deadline = now
            .checked_add(limits.timeout)
            .unwrap_or_else(|| now + Duration::from_secs(100 * 365 * 24 * 3600));
        let bytes = loop {
            let wait = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(wait.min(Duration::from_millis(20))) {
                Ok(Ok(buf)) => break buf,
                Ok(Err(error)) => {
                    terminate(&mut child);
                    return Err(error);
                }
                Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                Err(_) => {
                    terminate(&mut child);
                    return Err(CollectorError::Unavailable);
                }
            }
        };
        if bytes.len() > cap {
            terminate(&mut child);
            return Err(CollectorError::Malformed);
        }
        // Output closed; the child is exiting. Bound the wait as well.
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                _ => {
                    terminate(&mut child);
                    return Err(CollectorError::Unavailable);
                }
            }
        };
        let stdout = String::from_utf8(bytes).map_err(|_| CollectorError::Malformed)?;
        Ok(Output { stdout, success: status.success() })
    }
}
