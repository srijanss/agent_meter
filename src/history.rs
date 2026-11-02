//! Optional local usage history: a bounded, append-style log of the usage
//! figures each snapshot reported. It keeps no credentials, plan, path or other
//! account detail, and each figure keeps its `source` so a local estimate is
//! never mistaken for subscription-wide usage.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc, Weekday};
use serde::{Deserialize, Serialize};

use crate::snapshot::{Snapshot, Source};

/// One window's usage at the time of a snapshot.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WindowRecord {
    pub id: String,
    pub used_percent: Option<f64>,
    /// A window can differ from its snapshot: an estimate stays an estimate.
    pub source: Source,
}

/// One snapshot's usage figures.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HistoryRecord {
    pub account: String,
    pub provider: String,
    pub recorded_at: DateTime<Utc>,
    pub source: Source,
    pub windows: Vec<WindowRecord>,
}

impl HistoryRecord {
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        HistoryRecord {
            account: snapshot.profile_id.clone(),
            provider: snapshot.provider.clone(),
            recorded_at: snapshot.collected_at,
            source: snapshot.source,
            windows: snapshot
                .windows
                .iter()
                .map(|w| WindowRecord {
                    id: w.id.clone(),
                    used_percent: w.used_percent,
                    source: w.source,
                })
                .collect(),
        }
    }
}

/// A JSON-lines file holding at most `max_records` of the newest records.
pub struct HistoryStore {
    path: PathBuf,
    max_records: usize,
}

impl HistoryStore {
    pub fn new(path: PathBuf, max_records: usize) -> Self {
        HistoryStore { path, max_records }
    }

    /// Every readable record, oldest first. A missing file is an empty history
    /// and a line that cannot be read is skipped.
    pub fn load(&self) -> Vec<HistoryRecord> {
        self.read().unwrap_or_default()
    }

    /// Like `load`, but only a missing file is an empty history: any other
    /// failure to read is reported so it can never be mistaken for one.
    fn read(&self) -> io::Result<Vec<HistoryRecord>> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(bytes
                .split(|&b| b == b'\n')
                .filter_map(|line| serde_json::from_slice(line).ok())
                .collect()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Adds a snapshot's figures, dropping the oldest records beyond the bound.
    /// An exclusive lock covers the whole read-modify-replace so concurrent
    /// recorders cannot lose each other's records, and the file is replaced
    /// whole, so a failure never leaves a half-written one.
    pub fn record(&self, snapshot: &Snapshot) -> io::Result<()> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(self.sibling("lock"))?;
        lock.lock()?;

        let mut records = self.read()?;
        records.push(HistoryRecord::from_snapshot(snapshot));
        let excess = records.len().saturating_sub(self.max_records);
        let mut text = String::new();
        for record in &records[excess..] {
            text.push_str(&serde_json::to_string(record).map_err(io::Error::other)?);
            text.push('\n');
        }

        // A fresh, never-before-existing file: nothing planted at a guessable
        // name is followed or inherited, and only a file this call created is
        // ever removed.
        let (mut file, temp) = loop {
            let temp = self.sibling(&format!("{}.{}.tmp", std::process::id(), next_nonce()));
            match OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp) {
                Ok(file) => break (file, temp),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        };
        let written = (|| {
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temp, &self.path)
        })();
        if written.is_err() {
            let _ = fs::remove_file(&temp);
        }
        written
    }

    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(".");
        name.push(suffix);
        self.path.with_file_name(name)
    }
}

fn next_nonce() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    nanos ^ NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The weekday with the most usage, judged in `tz` (the viewer's local zone).
/// A day's usage is the highest figure any record reported that day, and a
/// weekday's usage is the sum over its days. `None` when no record reported
/// any usage, so there is nothing honest to name. Ties go to the earlier
/// weekday, Monday first.
pub fn busiest_day<Z: TimeZone>(records: &[HistoryRecord], tz: &Z) -> Option<Weekday> {
    let mut peaks: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for record in records {
        let day = record.recorded_at.with_timezone(tz).date_naive();
        let peak = record.windows.iter().filter_map(|w| w.used_percent).fold(0.0_f64, f64::max);
        let slot = peaks.entry(day).or_insert(0.0);
        *slot = slot.max(peak);
    }
    let mut totals = [0.0_f64; 7];
    for (day, peak) in peaks {
        totals[day.weekday().num_days_from_monday() as usize] += peak;
    }
    let (index, total) = totals
        .iter()
        .enumerate()
        .fold((0, 0.0), |best, (i, &t)| if t > best.1 { (i, t) } else { best });
    (total > 0.0).then(|| Weekday::try_from(index as u8).expect("index is below 7"))
}
