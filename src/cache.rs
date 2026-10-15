//! Per-account snapshot cache: a failed refresh never discards the last good data.

use crate::snapshot::Snapshot;

/// A cached snapshot, flagged stale (with the collector error) after a failed refresh.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedSnapshot {
    snapshot: Snapshot,
    error: Option<String>,
}

impl CachedSnapshot {
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn is_stale(&self) -> bool {
        self.error.is_some()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[derive(Debug, Default)]
pub struct SnapshotCache {
    current: Option<CachedSnapshot>,
    last_error: Option<String>,
}

impl SnapshotCache {
    pub fn record(&mut self, result: Result<Snapshot, String>) {
        match result {
            Ok(snapshot) => {
                self.current = Some(CachedSnapshot { snapshot, error: None });
                self.last_error = None;
            }
            Err(error) => {
                if let Some(cached) = self.current.as_mut() {
                    cached.error = Some(error.clone());
                }
                self.last_error = Some(error);
            }
        }
    }

    pub fn current(&self) -> Option<&CachedSnapshot> {
        self.current.as_ref()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}
