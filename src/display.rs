//! View-model derivation: unknown values stay unknown, never zero.

use chrono::{DateTime, Utc};

use crate::countdown::{countdown_label, reset_display, ResetDisplay};
use crate::meter::fill_fraction;
use crate::snapshot::{BonusResets, Snapshot};

/// One window row: an empty track and no countdown when the usage is unknown.
#[derive(Debug, Clone, PartialEq)]
pub struct RowView {
    pub label: String,
    pub fill: Option<f64>,
    pub reset: Option<ResetDisplay>,
    /// Fill fractions of known scoped allowances on this window's clock; an
    /// unknown scoped percent is dropped, never drawn as 0.
    pub scoped: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccountView {
    Rows(Vec<RowView>),
    Unavailable,
}

/// "2 free resets · next expires in 20d 7h" for a reported bonus count.
///
/// `None` when the count is not reported: absent is never drawn as 0. A real
/// zero reads "0 free resets". The expiry is dropped when missing, already
/// past (stale data), or when there is nothing left to expire.
pub fn bonus_label(bonus: &BonusResets, now: DateTime<Utc>) -> Option<String> {
    let remaining = bonus.remaining?;
    let noun = if remaining == 1 { "free reset" } else { "free resets" };
    let expiry = bonus
        .expires_at
        .filter(|at| remaining > 0 && *at > now)
        .map(|at| format!(" · next expires in {}", countdown_label(at, now)));
    Some(format!("{remaining} {noun}{}", expiry.unwrap_or_default()))
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
            let scoped = w.scoped.iter().filter_map(|s| fill_fraction(s.used_percent)).collect();
            RowView { label: w.label.clone(), fill, reset, scoped }
        })
        .collect();
    if rows.iter().any(|r| r.fill.is_some() || !r.scoped.is_empty()) {
        AccountView::Rows(rows)
    } else {
        AccountView::Unavailable
    }
}
