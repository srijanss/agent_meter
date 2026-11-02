//! The read-only `herdr-agents usage --json` command: the same normalized
//! snapshots the popup shows, for independent local integrations. It reads the
//! accounts' usage and prints it; it never writes the config or opens a socket.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::account::Provider;
use crate::app::{parse_args, App, AppError, JobFactory, USAGE};
use crate::popup::AccountEntry;

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

/// Collects every account's usage and renders it.
pub fn usage_report(config: &Path, user_home: &Path, jobs: JobFactory) -> Result<String, AppError> {
    let mut app = App::new(config.to_path_buf(), user_home, jobs, Duration::ZERO)?;
    app.refresh(Instant::now());
    loop {
        app.poll(Instant::now());
        if !app.refreshing() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(usage_json(app.entries(), Utc::now()))
}
