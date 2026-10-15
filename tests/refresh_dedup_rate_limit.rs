use agent_meter::refresh::{RefreshDecision, RefreshGate};
use std::time::{Duration, Instant};

#[test]
fn concurrent_requests_dedupe_and_finished_ones_are_rate_limited() {
    let t0 = Instant::now();
    let mut gate = RefreshGate::new(Duration::from_secs(10));

    assert_eq!(gate.request("a", t0), RefreshDecision::Start);
    assert_eq!(gate.request("a", t0), RefreshDecision::AlreadyInFlight);
    // Another account is independent.
    assert_eq!(gate.request("b", t0), RefreshDecision::Start);

    gate.finish("a", t0 + Duration::from_secs(1));
    assert_eq!(
        gate.request("a", t0 + Duration::from_secs(4)),
        RefreshDecision::RateLimited { retry_after: Duration::from_secs(7) }
    );
    assert_eq!(gate.request("a", t0 + Duration::from_secs(11)), RefreshDecision::Start);
}
