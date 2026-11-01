//! Turns a collector outcome into what the popup lists: a failed account stays
//! listed, with no snapshot, rather than being hidden or shown as zero usage.

use crate::account::Account;
use crate::failure::{AccountOutcome, CollectorError};
use crate::popup::AccountEntry;

pub fn entry_for(account: Account, outcome: AccountOutcome) -> AccountEntry {
    let snapshot = match outcome {
        AccountOutcome::Snapshot(snapshot) => Some(snapshot),
        AccountOutcome::Failed(_) => None,
    };
    AccountEntry { account, snapshot }
}

/// Plain-language cause for the "Usage unavailable" hover. Fixed text only: it
/// never carries provider output.
pub fn unavailable_reason(error: CollectorError) -> &'static str {
    match error {
        CollectorError::Unavailable => "CLI not found",
        CollectorError::SignInRequired => "Sign-in required",
        CollectorError::RateLimited => "Rate limited",
        CollectorError::Malformed => "Unreadable response",
        CollectorError::Internal => "Collector error",
    }
}
