//! The versioned, normalized usage snapshot (agents_plugin_spec.md §4).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Where a value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    ProviderReported,
    LocalEstimate,
    Unknown,
}

/// Whether the collector could report usage for this account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Ok,
    Unsupported,
}

/// One limit window, such as the session or weekly allowance.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Window {
    pub id: String,
    pub label: String,
    /// Share of the allowance consumed, not remaining.
    pub used_percent: Option<f64>,
    pub resets_at: Option<DateTime<Utc>>,
    pub source: Source,
    /// Model-scoped allowances on this window's clock (e.g. a per-model weekly limit).
    #[serde(default)]
    pub scoped: Vec<ScopedAllowance>,
}

/// A model-scoped allowance sharing its window's reset clock.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ScopedAllowance {
    pub label: String,
    pub used_percent: Option<f64>,
}

/// Bonus/free reset entitlements, separate from window resets.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BonusResets {
    pub remaining: Option<u32>,
    pub expires_at: Option<DateTime<Utc>>,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub provider: String,
    pub profile_id: String,
    pub plan_label: Option<String>,
    pub collected_at: DateTime<Utc>,
    pub source: Source,
    pub health: Health,
    pub windows: Vec<Window>,
    pub bonus_resets: Option<BonusResets>,
}

/// The only schema version this build understands.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Parses a snapshot, rejecting any schema version other than the supported
/// one (and, through `Source`, any unknown source) rather than guessing.
/// Largest collector payload accepted; a real snapshot is a few KiB.
pub const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;

fn invalid(message: &str) -> serde_json::Error {
    <serde_json::Error as serde::de::Error>::custom(message)
}

/// A used share must be a finite number from 0 to 100.
fn valid_percent(percent: Option<f64>) -> bool {
    percent.is_none_or(|p| p.is_finite() && (0.0..=100.0).contains(&p))
}

/// Parses a snapshot from collector output, which is untrusted: an oversized
/// payload, an unsupported schema version or an unknown source, a wrong type,
/// a timestamp without a timezone, or a percentage outside 0..=100 is an error,
/// never a guessed value.
pub fn parse_snapshot(json: &str) -> Result<Snapshot, serde_json::Error> {
    if json.len() > MAX_SNAPSHOT_BYTES {
        return Err(invalid("snapshot is too large"));
    }
    let snapshot: Snapshot = serde_json::from_str(json)?;
    if snapshot.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(invalid("unsupported schema_version"));
    }
    let percents_valid = snapshot.windows.iter().all(|window| {
        valid_percent(window.used_percent)
            && window.scoped.iter().all(|scoped| valid_percent(scoped.used_percent))
    });
    if !percents_valid {
        return Err(invalid("used_percent must be from 0 to 100"));
    }
    Ok(snapshot)
}
