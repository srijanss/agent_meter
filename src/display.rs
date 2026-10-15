//! View-model derivation: unknown values stay unknown, never zero.

use chrono::{DateTime, Utc};

use crate::countdown::{reset_display, ResetDisplay};
use crate::meter::fill_fraction;
use crate::snapshot::Snapshot;

/// One window row: an empty track and no countdown when the usage is unknown.
#[derive(Debug, Clone, PartialEq)]
pub struct RowView {
    pub label: String,
    pub fill: Option<f64>,
    pub reset: Option<ResetDisplay>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccountView {
    Rows(Vec<RowView>),
    Unavailable,
}

pub fn account_view(snapshot: &Snapshot, now: DateTime<Utc>) -> AccountView {
    let rows: Vec<RowView> = snapshot
        .windows
        .iter()
        .map(|w| {
            let fill = fill_fraction(w.used_percent);
            let reset = fill
                .and(w.resets_at)
                .map(|at| reset_display(at, now));
            RowView { label: w.label.clone(), fill, reset }
        })
        .collect();
    if rows.iter().any(|r| r.fill.is_some()) {
        AccountView::Rows(rows)
    } else {
        AccountView::Unavailable
    }
}
