//! Maps a Codex app-server `account/rateLimits/read` result into a snapshot.
//!
//! The input is untrusted JSON from another process: every field is read
//! defensively and anything missing or malformed becomes unknown, never a guess.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::snapshot::{Health, Snapshot, Source, Window};

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
        bonus_resets: None,
    }
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
