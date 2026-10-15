//! Failure isolation: one account's collector failing never affects another's.

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::snapshot::Snapshot;

/// Why an account could not be measured. Carries no provider payload, so it is
/// safe to log and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectorError {
    /// CLI missing or provider unreachable.
    Unavailable,
    SignInRequired,
    RateLimited,
    Malformed,
    /// The collector itself panicked.
    Internal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccountOutcome {
    Snapshot(Snapshot),
    Failed(CollectorError),
}

pub type Collect = Box<dyn FnOnce() -> Result<Snapshot, CollectorError>>;

/// Runs each account's collector independently, preserving order.
pub fn collect_all(accounts: Vec<(String, Collect)>) -> Vec<(String, AccountOutcome)> {
    accounts
        .into_iter()
        .map(|(id, collect)| {
            let outcome = match catch_unwind(AssertUnwindSafe(collect)) {
                Ok(Ok(snapshot)) => AccountOutcome::Snapshot(snapshot),
                Ok(Err(error)) => AccountOutcome::Failed(error),
                Err(payload) => {
                    // The payload is arbitrary collector-owned data; its destructor
                    // may panic too, which must not escape this boundary.
                    if let Err(nested) = catch_unwind(AssertUnwindSafe(move || drop(payload))) {
                        // Dropping it could panic again; leaking is the safe choice.
                        std::mem::forget(nested);
                    }
                    AccountOutcome::Failed(CollectorError::Internal)
                }
            };
            (id, outcome)
        })
        .collect()
}
