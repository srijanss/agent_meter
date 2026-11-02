//! Optional local usage history: a bounded, append-style log of the usage
//! figures each snapshot reported. It keeps no credentials, plan, path or other
//! account detail, and each figure keeps its `source` so a local estimate is
//! never mistaken for subscription-wide usage.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
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
        fs::read_to_string(&self.path)
            .map(|text| text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect())
            .unwrap_or_default()
    }

    /// Adds a snapshot's figures, dropping the oldest records beyond the bound.
    /// The file is replaced whole, so a failure never leaves a half-written one.
    pub fn record(&self, snapshot: &Snapshot) -> io::Result<()> {
        let mut records = self.load();
        records.push(HistoryRecord::from_snapshot(snapshot));
        let excess = records.len().saturating_sub(self.max_records);
        let mut text = String::new();
        for record in &records[excess..] {
            text.push_str(&serde_json::to_string(record).map_err(io::Error::other)?);
            text.push('\n');
        }
        let temp = self.path.with_extension("tmp");
        let mut file = OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, &self.path)
    }
}
