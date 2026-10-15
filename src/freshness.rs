//! Snapshot age: stale flagging for the UI and JSON output.

use chrono::{DateTime, Duration, Utc};

use crate::countdown::countdown_label;
use crate::snapshot::Snapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    /// Older than the threshold; `age` is formatted like a countdown ("11m", "1d 2h").
    Stale { age: String },
}

pub fn freshness(snapshot: &Snapshot, now: DateTime<Utc>, threshold: Duration) -> Freshness {
    if now - snapshot.collected_at > threshold {
        Freshness::Stale { age: countdown_label(now, snapshot.collected_at) }
    } else {
        Freshness::Fresh
    }
}
