//! Refresh gating: one in-flight collection per account, rate-limited after it ends.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshDecision {
    /// Caller may start a collection (and must call `finish` when it ends).
    Start,
    /// A collection for this account is already running; reuse its result.
    AlreadyInFlight,
    /// The last collection ended too recently.
    RateLimited { retry_after: Duration },
}

#[derive(Debug)]
pub struct RefreshGate {
    min_interval: Duration,
    in_flight: HashMap<String, ()>,
    last_finished: HashMap<String, Instant>,
}

impl RefreshGate {
    pub fn new(min_interval: Duration) -> Self {
        Self { min_interval, in_flight: HashMap::new(), last_finished: HashMap::new() }
    }

    pub fn request(&mut self, account: &str, now: Instant) -> RefreshDecision {
        if self.in_flight.contains_key(account) {
            return RefreshDecision::AlreadyInFlight;
        }
        if let Some(&done) = self.last_finished.get(account) {
            let elapsed = now.saturating_duration_since(done);
            if elapsed < self.min_interval {
                return RefreshDecision::RateLimited { retry_after: self.min_interval - elapsed };
            }
        }
        self.in_flight.insert(account.to_string(), ());
        RefreshDecision::Start
    }

    pub fn finish(&mut self, account: &str, now: Instant) {
        if self.in_flight.remove(account).is_some() {
            self.last_finished.insert(account.to_string(), now);
        }
    }
}
