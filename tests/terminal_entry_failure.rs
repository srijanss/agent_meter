use std::io::{self, Write};

use agent_meter::terminal::{run_in_terminal, RawMode};

struct BrokenPipe;

impl Write for BrokenPipe {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "gone"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "gone"))
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
fn entry_write_failure_rolls_back_raw_mode_and_skips_the_body() {
    let mut out = BrokenPipe;
    let mut raw = FakeRaw::default();
    let mut ran = false;
    let result = run_in_terminal(&mut out, &mut raw, |_| ran = true);
    assert_eq!(result.expect_err("setup failed").kind(), io::ErrorKind::BrokenPipe);
    assert!(!ran);
    assert!(!raw.enabled, "raw mode rolled back");
    assert_eq!(raw.disable_calls, 1);
}
