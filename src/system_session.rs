//! Real child-process `LineSession`: same hygiene as `SystemRunner` (own
//! process group, cleared environment, stderr discarded), with a reader thread
//! so `recv` can time out. Dropping the session kills the whole group and reaps it.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::Duration;

use crate::failure::CollectorError;
use crate::runner::{configured_command, terminate, CommandSpec};
use crate::session::{LineSession, SessionStarter};

/// Longest line accepted from the child.
const MAX_LINE: usize = 1024 * 1024;
/// Longest line `send` accepts; well under the smallest common pipe buffer (64 KiB).
const MAX_SEND: usize = 16 * 1024;
/// Lines buffered ahead of `recv`; a flooding child blocks instead of growing memory.
const QUEUE: usize = 64;
/// Lines queued for the writer thread; beyond this `send` reports Unavailable.
const OUTBOX: usize = 8;

pub struct SystemSessionStarter;

impl SessionStarter for SystemSessionStarter {
    fn start(&self, spec: &CommandSpec) -> Result<Box<dyn LineSession>, CollectorError> {
        let mut child = configured_command(spec)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| CollectorError::Unavailable)?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            terminate(&mut child);
            return Err(CollectorError::Internal);
        };
        let (tx, lines) = mpsc::sync_channel(QUEUE);
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut buf = Vec::new();
                let read = (&mut reader).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut buf);
                let item = match read {
                    Ok(0) | Err(_) => return,
                    Ok(_) if buf.len() > MAX_LINE => Err(CollectorError::Malformed),
                    Ok(_) => String::from_utf8(buf)
                        .map(|line| line.trim_end_matches(['\r', '\n']).to_string())
                        .map_err(|_| CollectorError::Malformed),
                };
                let stop = item.is_err();
                if tx.send(item).is_err() || stop {
                    return;
                }
            }
        });
        // A writer thread owns stdin, so a child that never reads can only stall
        // that thread; `send` never waits. Dropping the session kills the group,
        // which ends a blocked write with EPIPE.
        let (out, queued) = mpsc::sync_channel::<String>(OUTBOX);
        thread::spawn(move || {
            let mut stdin: ChildStdin = stdin;
            while let Ok(line) = queued.recv() {
                if writeln!(stdin, "{line}").and_then(|()| stdin.flush()).is_err() {
                    return;
                }
            }
        });
        Ok(Box::new(ProcessSession { child, out, lines }))
    }
}

struct ProcessSession {
    child: Child,
    out: SyncSender<String>,
    lines: Receiver<Result<String, CollectorError>>,
}

impl LineSession for ProcessSession {
    fn send(&mut self, line: &str) -> Result<(), CollectorError> {
        if line.len() > MAX_SEND {
            return Err(CollectorError::Internal);
        }
        // Never blocks: a full outbox or a dead writer means the child is not listening.
        self.out.try_send(line.to_string()).map_err(|_| CollectorError::Unavailable)
    }

    fn recv(&mut self, timeout: Duration) -> Result<String, CollectorError> {
        // A timeout and a closed stream (child exited) are both "no answer".
        self.lines.recv_timeout(timeout).map_err(|_| CollectorError::Unavailable)?
    }
}

impl Drop for ProcessSession {
    fn drop(&mut self) {
        terminate(&mut self.child);
    }
}
