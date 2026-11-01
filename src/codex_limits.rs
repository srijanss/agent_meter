//! Maps a Codex app-server `account/rateLimits/read` result into a snapshot.
//!
//! The input is untrusted JSON from another process: every field is read
//! defensively and anything missing or malformed becomes unknown, never a guess.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::snapshot::{BonusResets, Health, Snapshot, Source, Window};

/// Window length that Codex reports for its weekly limit, in minutes.
const WEEKLY_MINS: i64 = 10080;
/// Window length that Codex reports for its session limit, in minutes.
const SESSION_MINS: i64 = 300;

/// Builds a snapshot from the `result` of `account/rateLimits/read`.
///
/// Only the backward-compatible single-bucket view (`rateLimits`) is used.
/// A window the response does not contain is not listed at all.
pub fn parse_codex_rate_limits(result: &Value, collected_at: DateTime<Utc>, profile_id: &str) -> Snapshot {
    let limits = result.get("rateLimits");

    let windows: Vec<Window> = ["primary", "secondary"]
        .into_iter()
        .filter_map(|slot| {
            let window = limits?.get(slot).filter(|w| w.is_object())?;
            Some(parse_window(slot, window))
        })
        .collect();

    let known = windows.iter().any(|w| w.source == Source::ProviderReported);
    Snapshot {
        schema_version: 1,
        provider: "codex".to_string(),
        profile_id: profile_id.to_string(),
        plan_label: limits.and_then(|l| l.get("planType")).and_then(Value::as_str).and_then(plan_label),
        collected_at,
        source: if known { Source::ProviderReported } else { Source::Unknown },
        health: if known { Health::Ok } else { Health::Unsupported },
        windows,
        bonus_resets: parse_bonus_resets(result.get("rateLimitResetCredits")),
    }
}

/// Free reset credits from `rateLimitResetCredits`.
///
/// Absent or unusable data stays `None` (not reported), never 0. The backend's
/// `availableCount` is the total when valid, because the detail list may be
/// capped or missing; otherwise the `available` rows are counted. The expiry is
/// the soonest among the available rows that expire at all.
fn parse_bonus_resets(credits: Option<&Value>) -> Option<BonusResets> {
    let credits = credits.filter(|v| v.is_object())?;
    let rows = credits.get("credits").and_then(Value::as_array);
    let available: Vec<&Value> = rows
        .map(|rows| {
            rows.iter()
                .filter(|row| row.get("status").and_then(Value::as_str) == Some("available"))
                .collect()
        })
        .unwrap_or_default();

    let reported = credits
        .get("availableCount")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok());
    // Counting rows is only exact when every row has a readable status; a list
    // with unreadable rows gives a lower bound at best, so it is unknown.
    let counted = rows
        .filter(|rows| rows.iter().all(|row| row.get("status").and_then(Value::as_str).is_some()))
        .and_then(|_| u32::try_from(available.len()).ok());
    let remaining = reported.or(counted)?;

    let expires_at = available
        .iter()
        .filter_map(|row| row.get("expiresAt").and_then(Value::as_i64))
        .filter_map(|secs| DateTime::from_timestamp(secs, 0))
        .min();
    Some(BonusResets { remaining: Some(remaining), expires_at, source: Source::ProviderReported })
}

fn parse_window(slot: &str, window: &Value) -> Window {
    let duration = window.get("windowDurationMins").and_then(Value::as_i64).filter(|m| *m > 0);
    let (id, label) = match duration {
        Some(SESSION_MINS) => ("session".to_string(), "Session".to_string()),
        Some(WEEKLY_MINS) => ("weekly".to_string(), "Weekly".to_string()),
        Some(mins) if mins % 60 == 0 => (format!("{}h", mins / 60), format!("{}h window", mins / 60)),
        Some(mins) => (format!("{mins}m"), format!("{mins}m window")),
        None => (slot.to_string(), capitalize(slot)),
    };

    let used_percent = window
        .get("usedPercent")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0);
    // Without a usable percentage the window is unknown, and so is its reset.
    let resets_at = used_percent.and_then(|_| {
        let secs = window.get("resetsAt").and_then(Value::as_i64)?;
        DateTime::from_timestamp(secs, 0)
    });

    Window {
        id,
        label,
        used_percent,
        resets_at,
        source: if used_percent.is_some() { Source::ProviderReported } else { Source::Unknown },
        scoped: Vec::new(),
    }
}

/// `promax` -> `Promax`, `self_serve_business_prolite` -> `Self Serve Business Prolite`.
fn plan_label(plan: &str) -> Option<String> {
    let plan = plan.trim();
    if plan.is_empty() || plan.eq_ignore_ascii_case("unknown") {
        return None;
    }
    Some(plan.split('_').filter(|w| !w.is_empty()).map(capitalize).collect::<Vec<_>>().join(" "))
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect(),
        None => String::new(),
    }
}
