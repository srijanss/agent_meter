use std::io::{self, Write};

use agent_meter::terminal::{run_in_terminal, RawMode};

const LEAVE_ALT: &str = "\x1b[?1049l";

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
fn a_failed_mouse_off_write_does_not_stop_the_screen_and_raw_mode_restore() {
    // Writes 1-2 are entry (alternate screen, mouse capture). Write 3 is the
    // first restore command (mouse capture off) and fails.
    let mut out = FailsOnce { bytes: Vec::new(), calls: 0, fail_at: 3 };
    let mut raw = FakeRaw::default();

    let result = run_in_terminal(&mut out, &mut raw, |_| "body done");

    let error = result.expect_err("the cleanup failure is reported");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    let s = String::from_utf8(out.bytes.clone()).unwrap();
    assert!(s.contains(LEAVE_ALT), "the alternate screen is still left");
    assert!(!raw.enabled, "raw mode is still restored");
    assert_eq!(raw.disable_calls, 1);
}
