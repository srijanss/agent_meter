use std::io::{self, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};

use agent_meter::terminal::{run_in_terminal, RawMode};

const LEAVE_ALT: &str = "\x1b[?1049l";
const MOUSE_OFF: &str = "\x1b[?1000l";

#[derive(Default)]
struct FakeRaw {
    enabled: bool,
    disable_calls: u32,
    fail_disable: bool,
}

impl RawMode for FakeRaw {
    fn enable(&mut self) -> io::Result<()> {
        self.enabled = true;
        Ok(())
    }
    fn disable(&mut self) -> io::Result<()> {
        self.disable_calls += 1;
        if self.fail_disable {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "tcsetattr"));
        }
        self.enabled = false;
        Ok(())
    }
}

/// Fails every write after the first `ok_writes` calls.
struct FailsAfter {
    bytes: Vec<u8>,
    calls: usize,
    ok_writes: usize,
}

impl Write for FailsAfter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.calls > self.ok_writes {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "terminal gone"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_raw_mode_disable_failure_is_returned_after_the_screen_is_restored() {
    let mut out: Vec<u8> = Vec::new();
    let mut raw = FakeRaw { fail_disable: true, ..FakeRaw::default() };

    let result = run_in_terminal(&mut out, &mut raw, |_| "body done");

    let error = result.expect_err("the cleanup failure is reported");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains(MOUSE_OFF) && s.contains(LEAVE_ALT), "the screen was still restored");
    assert_eq!(raw.disable_calls, 1);
}

#[test]
fn the_original_panic_wins_when_cleanup_also_fails() {
    // Entry (2 writes) works; every restore write then fails, and so does raw mode.
    let mut out = FailsAfter { bytes: Vec::new(), calls: 0, ok_writes: 2 };
    let mut raw = FakeRaw { fail_disable: true, ..FakeRaw::default() };

    let caught = catch_unwind(AssertUnwindSafe(|| {
        let _ = run_in_terminal(&mut out, &mut raw, |_| -> () { panic!("body panic") });
    }));

    let payload = caught.expect_err("the body's panic propagates");
    assert_eq!(payload.downcast_ref::<&str>().copied(), Some("body panic"));
    assert_eq!(raw.disable_calls, 1, "raw mode was still attempted despite the failed writes");
}

#[test]
fn every_cleanup_step_failing_still_reports_the_first_error_once() {
    let mut out = FailsAfter { bytes: Vec::new(), calls: 0, ok_writes: 2 };
    let mut raw = FakeRaw { fail_disable: true, ..FakeRaw::default() };

    let result = run_in_terminal(&mut out, &mut raw, |_| ());

    let error = result.expect_err("cleanup failed");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe, "the first failing step's error wins");
    assert_eq!(raw.disable_calls, 1);
}
