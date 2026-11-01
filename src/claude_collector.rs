//! Collects one Claude account's usage through the `claude` CLI.
//!
//! Auth gates collection: `claude auth status --json` must say the account is
//! signed in before the (slow, local) `/usage` command is run. Identity fields
//! in the auth output (email, org) are never read into the snapshot.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::account::Account;
use crate::claude_usage::parse_claude_usage;
use crate::failure::CollectorError;
use crate::runner::{CommandRunner, CommandSpec, Limits};
use crate::snapshot::Snapshot;

const AUTH_LIMITS: Limits = Limits { timeout: Duration::from_secs(10), max_output: 64 * 1024 };
/// `/usage` runs locally with no model turn but takes about 15s.
const USAGE_LIMITS: Limits = Limits { timeout: Duration::from_secs(45), max_output: 1024 * 1024 };

pub fn collect_claude(
    runner: &dyn CommandRunner,
    account: &Account,
    now: DateTime<Utc>,
) -> Result<Snapshot, CollectorError> {
    let (var, home) = account.home_env();
    let home = home.to_string_lossy();
    let command = |args: &[&str]| {
        CommandSpec::new("claude", args.iter().copied())
            .and_then(|spec| spec.with_env(var, &home))
            .map_err(|_| CollectorError::Internal)
    };

    let auth = runner.run(&command(&["auth", "status", "--json"])?, &AUTH_LIMITS)?;
    let auth: Value = serde_json::from_str(&auth.stdout).map_err(|_| CollectorError::Malformed)?;
    match auth.get("loggedIn").and_then(Value::as_bool) {
        Some(true) => {}
        Some(false) => return Err(CollectorError::SignInRequired),
        None => return Err(CollectorError::Malformed),
    }
    let plan = auth.get("subscriptionType").and_then(Value::as_str).and_then(plan_label);

    let usage = runner.run(&command(&["-p", "/usage", "--output-format", "json"])?, &USAGE_LIMITS)?;
    let usage: Value = serde_json::from_str(&usage.stdout).map_err(|_| CollectorError::Malformed)?;
    if usage.get("is_error").and_then(Value::as_bool) == Some(true) {
        return Err(CollectorError::Unavailable);
    }
    let text = usage.get("result").and_then(Value::as_str).ok_or(CollectorError::Malformed)?;

    let mut snapshot = parse_claude_usage(text, now, account.profile_id());
    snapshot.plan_label = plan;
    Ok(snapshot)
}

/// `pro` -> `Pro`, `max` -> `Max`; empty is not a plan.
fn plan_label(plan: &str) -> Option<String> {
    let plan = plan.trim();
    let mut chars = plan.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect())
}
