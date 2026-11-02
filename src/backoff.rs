//! When an account is next due for an automatic refresh: every `base`, doubling
//! with each consecutive failure up to a cap, and back to `base` on success.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Failures past this many stop lengthening the delay (64 times the base).
const MAX_DOUBLINGS: u32 = 6;

#[derive(Debug)]
pub struct Backoff {
    base: Duration,
    state: HashMap<String, (u32, Instant)>,
}

impl Backoff {
    pub fn new(base: Duration) -> Self {
        Self { base, state: HashMap::new() }
    }

    /// Notes how a collection that ended at `now` went.
    pub fn record(&mut self, account: &str, success: bool, now: Instant) {
        let failures = if success { 0 } else { self.failures(account).saturating_add(1) };
        self.state.insert(account.to_string(), (failures, now));
    }

    /// Whether the account's wait since its last collection is over.
    pub fn due(&self, account: &str, now: Instant) -> bool {
        match self.state.get(account) {
            None => true,
            Some(&(failures, last)) => {
                let delay = self.base * 2u32.pow(failures.min(MAX_DOUBLINGS));
                now.saturating_duration_since(last) >= delay
            }
        }
    }

    fn failures(&self, account: &str) -> u32 {
        self.state.get(account).map_or(0, |&(failures, _)| failures)
    }
}
