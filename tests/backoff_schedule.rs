use std::time::{Duration, Instant};

use agent_meter::backoff::Backoff;

const BASE: Duration = Duration::from_secs(60);

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn the_delay_doubles_per_consecutive_failure_is_capped_and_resets_on_success() {
    let t0 = Instant::now();
    let mut backoff = Backoff::new(BASE);
    assert!(backoff.due("a", t0), "an account never refreshed is due");

    backoff.record("a", false, t0);
    assert!(!backoff.due("a", t0 + secs(119)));
    assert!(backoff.due("a", t0 + secs(120)));
    assert!(backoff.due("b", t0), "other accounts are unaffected");

    backoff.record("a", false, t0 + secs(120));
    assert!(!backoff.due("a", t0 + secs(120 + 239)));
    assert!(backoff.due("a", t0 + secs(120 + 240)));

    // Many failures stop growing at 64 times the base.
    let mut at = t0;
    for _ in 0..30 {
        backoff.record("a", false, at);
        at += secs(60 * 64);
    }
    let last = at - secs(60 * 64);
    assert!(!backoff.due("a", last + secs(60 * 64 - 1)));
    assert!(backoff.due("a", last + secs(60 * 64)));

    backoff.record("a", true, last);
    assert!(!backoff.due("a", last + secs(59)));
    assert!(backoff.due("a", last + secs(60)));
}
