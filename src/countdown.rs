//! Human-readable time until a window resets, derived from absolute timestamps.

use chrono::{DateTime, Utc};

/// Time remaining until `resets_at` as of `now`: "Nd Nh", "Nh Nm" or "Nm".
///
/// Anything under a minute reads "1m" so an imminent reset never shows "0m".
pub fn countdown_label(resets_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let minutes = (resets_at - now).num_minutes().max(1);
    let (days, hours, mins) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

/// What to show for a window's reset time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResetDisplay {
    /// Time left until the reset, e.g. "2h 0m".
    Remaining(String),
    /// The reset time has already passed: the data needs a refresh, so no
    /// (negative) countdown is shown.
    Stale,
}

pub fn reset_display(resets_at: DateTime<Utc>, now: DateTime<Utc>) -> ResetDisplay {
    if resets_at <= now {
        ResetDisplay::Stale
    } else {
        ResetDisplay::Remaining(countdown_label(resets_at, now))
    }
}
