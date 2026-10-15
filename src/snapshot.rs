//! The versioned, normalized usage snapshot (agents_plugin_spec.md §4).

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Where a value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    ProviderReported,
    LocalEstimate,
    Unknown,
}

/// Whether the collector could report usage for this account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Ok,
    Unsupported,
}

/// One limit window, such as the session or weekly allowance.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Window {
    pub id: String,
    pub label: String,
    /// Share of the allowance consumed, not remaining.
    pub used_percent: Option<f64>,
    pub resets_at: Option<DateTime<Utc>>,
    pub source: Source,
}

/// Bonus/free reset entitlements, separate from window resets.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BonusResets {
    pub remaining: Option<u32>,
    pub expires_at: Option<DateTime<Utc>>,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

pub fn parse_snapshot(json: &str) -> Result<Snapshot, serde_json::Error> {
    serde_json::from_str(json)
}
