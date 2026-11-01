//! Parser for the `result` text of `claude -p /usage --output-format json`.
//!
//! Anything it cannot read becomes unknown (`None`), never a guess.

use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

use crate::snapshot::{Health, ScopedAllowance, Snapshot, Source, Window};

/// Years searched for the next valid occurrence of a month/day (covers Feb 29).
const YEAR_SEARCH: i32 = 8;

/// One parsed `Current …:` line.
struct Entry {
    used_percent: Option<f64>,
    resets_at: Option<DateTime<Utc>>,
}

impl Entry {
    fn recognized(&self) -> bool {
        self.used_percent.is_some() || self.resets_at.is_some()
    }
}

/// Builds a snapshot with the Session and Weekly windows from `/usage` text.
///
/// `collected_at` anchors the year of each reset ("Nov 6" has no year: it is
/// the next such date after collection). The plan label is not in this text,
/// so it is left unset for the caller.
pub fn parse_claude_usage(text: &str, collected_at: DateTime<Utc>, profile_id: &str) -> Snapshot {
    let mut session = None;
    let mut weekly = None;
    let mut scoped: Vec<(String, Entry)> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("What's contributing") {
            break; // the local breakdown below is not subscription usage
        }
        let Some(rest) = line.strip_prefix("Current ") else { continue };
        if let Some(rest) = rest.strip_prefix("session:") {
            session.get_or_insert_with(|| parse_entry(rest, collected_at));
        } else if let Some(rest) = rest.strip_prefix("week (all models):") {
            weekly.get_or_insert_with(|| parse_entry(rest, collected_at));
        } else if let Some(rest) = rest.strip_prefix("week (") {
            if let Some((label, rest)) = rest.split_once("):") {
                scoped.push((label.trim().to_string(), parse_entry(rest, collected_at)));
            }
        }
    }

    let mut windows = vec![
        window("session", "Session", session),
        window("weekly", "Weekly", weekly),
    ];
    // A model line shares the weekly clock only if it resets at the same instant.
    if let Some(reset) = windows[1].resets_at {
        windows[1].scoped = scoped
            .into_iter()
            .filter(|(_, entry)| entry.resets_at == Some(reset))
            .map(|(label, entry)| ScopedAllowance { label, used_percent: entry.used_percent })
            .collect();
    }

    let known = windows.iter().any(|w| w.source == Source::ProviderReported);
    Snapshot {
        schema_version: 1,
        provider: "claude".to_string(),
        profile_id: profile_id.to_string(),
        plan_label: None,
        collected_at,
        source: if known { Source::ProviderReported } else { Source::Unknown },
        health: if known { Health::Ok } else { Health::Unsupported },
        windows,
        bonus_resets: None,
    }
}

fn window(id: &str, label: &str, entry: Option<Entry>) -> Window {
    let (used_percent, resets_at, source) = match entry {
        Some(entry) if entry.recognized() => {
            (entry.used_percent, entry.resets_at, Source::ProviderReported)
        }
        _ => (None, None, Source::Unknown),
    };
    Window {
        id: id.to_string(),
        label: label.to_string(),
        used_percent,
        resets_at,
        source,
        scoped: Vec::new(),
    }
}

/// Parses ` 38% used · resets Nov 1, 7:40pm (Australia/Sydney)`.
fn parse_entry(rest: &str, collected_at: DateTime<Utc>) -> Entry {
    let (used, resets) = match rest.split_once('·') {
        Some((used, resets)) => (used, Some(resets)),
        None => (rest, None),
    };
    Entry {
        used_percent: parse_used(used),
        resets_at: resets.and_then(|r| parse_reset(r.trim(), collected_at)),
    }
}

fn parse_used(text: &str) -> Option<f64> {
    let value: f64 = text.trim().strip_suffix("used")?.trim().strip_suffix('%')?.trim().parse().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

/// Parses `resets Nov 1, 7:40pm (Australia/Sydney)` into the next such instant
/// after `collected_at`.
fn parse_reset(text: &str, collected_at: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let text = text.strip_prefix("resets ")?;
    let open = text.rfind('(')?;
    let zone: Tz = text[open + 1..].strip_suffix(')')?.trim().parse().ok()?;
    let (date, time) = text[..open].trim().split_once(", ")?;

    let (month, day) = date.split_once(' ')?;
    let month = month_number(month)?;
    let day: u32 = day.trim().parse().ok()?;
    let (hour, minute) = parse_clock(time.trim())?;

    let first_year = collected_at.with_timezone(&zone).year();
    for year in first_year..=first_year + YEAR_SEARCH {
        let Some(naive) = NaiveDate::from_ymd_opt(year, month, day).and_then(|d| d.and_hms_opt(hour, minute, 0))
        else {
            continue; // e.g. Feb 29 in a non-leap year
        };
        let instant = match zone.from_local_datetime(&naive) {
            LocalResult::Single(at) => at.with_timezone(&Utc),
            // Happens twice and the text does not say which: unknown if either
            // could still be ahead, otherwise this year's is already over.
            LocalResult::Ambiguous(first, second) => {
                if first.max(second).with_timezone(&Utc) > collected_at {
                    return None;
                }
                continue;
            }
            // Skipped by a DST jump. Offsets are under a day, so a wall-clock
            // time a full day before collection is certainly past.
            LocalResult::None => {
                if naive.and_utc() + Duration::days(1) > collected_at {
                    return None;
                }
                continue;
            }
        };
        if instant > collected_at {
            return Some(instant);
        }
    }
    None
}

fn month_number(name: &str) -> Option<u32> {
    const MONTHS: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let name = name.get(..3)?;
    MONTHS.iter().position(|m| m.eq_ignore_ascii_case(name)).map(|i| i as u32 + 1)
}

/// Parses `7:40pm`, `10pm`, `12am` into a 24-hour (hour, minute).
fn parse_clock(text: &str) -> Option<(u32, u32)> {
    let lower = text.to_ascii_lowercase();
    let (clock, pm) = match (lower.strip_suffix("am"), lower.strip_suffix("pm")) {
        (Some(clock), _) => (clock.to_string(), false),
        (_, Some(clock)) => (clock.to_string(), true),
        _ => return None,
    };
    let (hour, minute) = match clock.trim().split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (clock.trim().parse::<u32>().ok()?, 0),
    };
    if !(1..=12).contains(&hour) || minute > 59 {
        return None;
    }
    Some((hour % 12 + if pm { 12 } else { 0 }, minute))
}
