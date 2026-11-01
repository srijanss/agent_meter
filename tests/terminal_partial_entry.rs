use std::io::{self, Write};

use agent_meter::terminal::{run_in_terminal, RawMode};

const ENTER_ALT: &str = "\x1b[?1049h";
const LEAVE_ALT: &str = "\x1b[?1049l";
const MOUSE_OFF: &str = "\x1b[?1000l";

/// Accepts writes until the `fail_at`-th call, fails that one call, then
/// accepts everything again.
struct FailsOnce {
    bytes: Vec<u8>,
    calls: usize,
    fail_at: usize,
}

impl Write for FailsOnce {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FakeRaw {
    enabled: bool,
    disable_calls: u32,
}

impl RawMode for FakeRaw {
    fn enable(&mut self) -> io::Result<()> {
        self.enabled = true;
        Ok(())
    }
    fn disable(&mut self) -> io::Result<()> {
        self.enabled = false;
        self.disable_calls += 1;
        Ok(())
    }
}

#[test]
fn partial_entry_failure_leaves_the_alternate_screen_and_reports_the_original_error() {
    // Write 1 enters the alternate screen; write 2 (mouse capture) fails.
    let mut out = FailsOnce { bytes: Vec::new(), calls: 0, fail_at: 2 };
    let mut raw = FakeRaw::default();
    let mut ran = false;

    let result = run_in_terminal(&mut out, &mut raw, |_| ran = true);

    let error = result.expect_err("setup failed");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe, "the original setup error is reported");
    assert!(!ran, "the body never runs after a failed setup");
    let s = String::from_utf8(out.bytes.clone()).unwrap();
    assert!(s.contains(ENTER_ALT), "the alternate screen had been entered");
    assert!(s.contains(MOUSE_OFF), "mouse capture is turned off during rollback");
    assert!(s.contains(LEAVE_ALT), "the alternate screen is left during rollback");
    assert!(!raw.enabled, "raw mode is rolled back");
    assert_eq!(raw.disable_calls, 1);
}
