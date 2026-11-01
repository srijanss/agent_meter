//! Tooltip text for a usage window row. Pure: derived from a snapshot and a clock.

use chrono::{DateTime, Duration, Utc};

use crate::countdown::{reset_display, ResetDisplay};
use crate::freshness::{freshness, Freshness};
use crate::meter::fill_fraction;
use crate::snapshot::{Snapshot, Source};

/// Lines describing window `index` of `snapshot`, or `None` when there is no such window.
///
/// Unknown usage reads "unknown", never 0%. A stale snapshot adds how long ago
/// it was collected.
pub fn window_tooltip(
    snapshot: &Snapshot,
    index: usize,
    now: DateTime<Utc>,
    stale_after: Duration,
) -> Option<Vec<String>> {
    let window = snapshot.windows.get(index)?;
    // Usage and reset are independent: either may be known without the other.
    let usage = match percent(window.used_percent) {
        Some(used) => format!("{} {used}% used", window.label),
        None => format!("{} usage unknown", window.label),
    };
    let reset = match window.resets_at.map(|at| reset_display(at, now)) {
        Some(ResetDisplay::Remaining(left)) => format!(", resets in {left}"),
        Some(ResetDisplay::Stale) => ", reset time passed".to_string(),
        None => String::new(),
    };
    let headline = format!("{usage}{reset}");
    let mut lines = vec![headline, format!("Source: {}", source_name(window.source))];
    if let Freshness::Stale { age } = freshness(snapshot, now, stale_after) {
        lines.push(format!("Last updated {age} ago"));
    }
    for scoped in &window.scoped {
        lines.push(match percent(scoped.used_percent) {
            Some(used) => format!("{} {used}% used", scoped.label),
            None => format!("{} usage unknown", scoped.label),
        });
    }
    Some(lines)
}

fn percent(used: Option<f64>) -> Option<u32> {
    fill_fraction(used).map(|f| (f * 100.0).round() as u32)
}

fn source_name(source: Source) -> &'static str {
    match source {
        Source::ProviderReported => "provider_reported",
        Source::LocalEstimate => "local_estimate",
        Source::Unknown => "unknown",
    }
}
