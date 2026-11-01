//! Collects one Codex account's limits by speaking JSON-RPC to
//! `codex app-server` over stdio: initialize, initialized, then
//! `account/rateLimits/read`. The session is dropped (and so the child reaped)
//! on every path out of this function.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::account::{Account, Provider};
use crate::codex_limits::parse_codex_rate_limits;
use crate::failure::CollectorError;
use crate::runner::CommandSpec;
use crate::session::{LineSession, SessionStarter};
use crate::snapshot::Snapshot;

/// Per-reply wait.
const REPLY_TIMEOUT: Duration = Duration::from_secs(8);
/// Notifications and noise tolerated while waiting for one reply.
const MAX_LINES_PER_REPLY: usize = 64;

pub fn collect_codex(
    starter: &dyn SessionStarter,
    account: &Account,
    now: DateTime<Utc>,
) -> Result<Snapshot, CollectorError> {
    if account.provider != Provider::Codex {
        return Err(CollectorError::Internal);
    }
    let (var, home) = account.home_env();
    // Never substitute replacement characters: that would point the CLI at another directory.
    let home = home.to_str().ok_or(CollectorError::Internal)?;
    let spec = CommandSpec::new("codex", ["-s", "read-only", "-a", "on-request", "app-server"])
        .and_then(|spec| spec.with_env(var, home))
        .map_err(|_| CollectorError::Internal)?;
    let mut session = starter.start(&spec)?;

    send(&mut *session, &json!({"id": 1, "method": "initialize",
        "params": {"clientInfo": {"name": "agent-meter", "version": env!("CARGO_PKG_VERSION")}}}))?;
    reply(&mut *session, 1)?;
    send(&mut *session, &json!({"method": "initialized"}))?;
    send(&mut *session, &json!({"id": 2, "method": "account/rateLimits/read"}))?;
    let result = reply(&mut *session, 2)?;
    if !result.is_object() {
        return Err(CollectorError::Malformed);
    }
    Ok(parse_codex_rate_limits(&result, now, account.profile_id()))
}

fn send(session: &mut dyn LineSession, message: &Value) -> Result<(), CollectorError> {
    session.send(&message.to_string())
}

/// The `result` of the reply with this id. Notifications and unrelated lines
/// are skipped, up to a bound.
fn reply(session: &mut dyn LineSession, id: i64) -> Result<Value, CollectorError> {
    // One deadline per reply, however many unrelated lines arrive before it.
    let deadline = Instant::now() + REPLY_TIMEOUT;
    for _ in 0..MAX_LINES_PER_REPLY {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let line = session.recv(remaining)?;
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        if message.get("id").and_then(Value::as_i64) != Some(id) {
            continue;
        }
        return match message.get("error") {
            Some(error) => Err(error_kind(error)),
            None => message.get("result").cloned().ok_or(CollectorError::Malformed),
        };
    }
    Err(CollectorError::Unavailable)
}

/// Only a fixed mapping is surfaced; the server's message text never is.
fn error_kind(error: &Value) -> CollectorError {
    let message = error.get("message").and_then(Value::as_str).unwrap_or_default().to_lowercase();
    if message.contains("authentication required") {
        CollectorError::SignInRequired
    } else {
        CollectorError::Unavailable
    }
}
