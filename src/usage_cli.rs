//! The read-only `herdr-agents usage --json` command: the same normalized
//! snapshots the popup shows, for independent local integrations. It reads the
//! accounts' usage and prints it; it never writes the config or opens a socket.

use std::fmt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::account::Provider;
use crate::app::{parse_args, App, AppError, JobFactory, USAGE};
use crate::popup::AccountEntry;

/// How long `usage --json` waits before giving up. It exceeds every collector's
/// own limit, so each child process has been reaped by its collector by then.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(90);

/// Version of the `usage --json` envelope (each snapshot has its own version).
pub const REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// The interactive popup, with an optional config path.
    Popup(Option<PathBuf>),
    /// `usage --json`, with an optional config path.
    UsageJson(Option<PathBuf>),
}

pub fn parse_command(args: impl IntoIterator<Item = String>) -> Result<Command, AppError> {
    let args: Vec<String> = args.into_iter().collect();
    if args.first().map(String::as_str) != Some("usage") {
        return parse_args(args).map(Command::Popup);
    }
    match &args[1..] {
        [json] if json == "--json" => Ok(Command::UsageJson(None)),
        [json, config, path] if json == "--json" && config == "--config" && !path.is_empty() => {
            Ok(Command::UsageJson(Some(PathBuf::from(path))))
        }
        _ => Err(AppError::new(USAGE)),
    }
}

fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
    }
}

/// Renders the accounts as JSON. Only the account name, provider and active
/// mark are included: never its home directory or any other config value.
pub fn usage_json(entries: &[AccountEntry], now: DateTime<Utc>) -> String {
    let accounts: Vec<Value> = entries
        .iter()
        .map(|entry| {
            let age = entry
                .snapshot
                .as_ref()
                .map(|snapshot| (now - snapshot.collected_at).num_seconds().max(0));
            json!({
                "name": entry.account.name,
                "provider": provider_name(entry.account.provider),
                "active": entry.account.active,
                "age_seconds": age,
                "snapshot": entry.snapshot,
            })
        })
        .collect();
    json!({
        "schema_version": REPORT_SCHEMA_VERSION,
        "generated_at": now,
        "accounts": accounts,
    })
    .to_string()
}

/// Why `usage --json` produced no report. Each kind has its own exit code so
/// a script can tell them apart without parsing stderr.
#[derive(Debug)]
pub enum UsageError {
    /// Bad arguments or config: a problem with how it was started.
    Invalid(AppError),
    /// Collectors were still running when the time bound passed.
    Timeout,
    /// Every collector finished but none reported usage.
    NoData,
}

impl UsageError {
    pub fn exit_code(&self) -> u8 {
        match self {
            UsageError::Invalid(_) => 2,
            UsageError::Timeout => 3,
            UsageError::NoData => 4,
        }
    }
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsageError::Invalid(error) => error.fmt(f),
            UsageError::Timeout => f.write_str("timed out waiting for usage"),
            UsageError::NoData => f.write_str("no account reported usage"),
        }
    }
}

impl std::error::Error for UsageError {}

impl From<AppError> for UsageError {
    fn from(error: AppError) -> Self {
        UsageError::Invalid(error)
    }
}

/// Collects every account's usage and renders it, giving up after `timeout`.
/// One account reporting is enough for a report; the others show a null snapshot.
pub fn usage_report(
    config: &Path,
    user_home: &Path,
    jobs: JobFactory,
    timeout: Duration,
) -> Result<String, UsageError> {
    let mut app = App::new(config.to_path_buf(), user_home, jobs, Duration::ZERO)?;
    // A bound too large to represent is no bound at all.
    let deadline = Instant::now().checked_add(timeout);
    app.refresh(Instant::now());
    loop {
        let now = Instant::now();
        app.poll(now);
        if !app.refreshing() {
            break;
        }
        if deadline.is_some_and(|deadline| now >= deadline) {
            return Err(UsageError::Timeout);
        }
        thread::sleep(Duration::from_millis(10));
    }
    if app.entries().iter().all(|entry| entry.snapshot.is_none()) {
        return Err(UsageError::NoData);
    }
    Ok(usage_json(app.entries(), Utc::now()))
}
