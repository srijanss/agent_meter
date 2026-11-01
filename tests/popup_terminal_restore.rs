use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};

use agent_meter::terminal::{run_in_terminal, RawMode};

const ENTER_ALT: &str = "\x1b[?1049h";
const LEAVE_ALT: &str = "\x1b[?1049l";
const MOUSE_ON: &str = "\x1b[?1000h";
const MOUSE_OFF: &str = "\x1b[?1000l";

#[derive(Default)]
struct FakeRaw {
    enabled: bool,
    enable_calls: u32,
    disable_calls: u32,
    fail_enable: bool,
}

impl RawMode for FakeRaw {
    fn enable(&mut self) -> io::Result<()> {
        self.enable_calls += 1;
        if self.fail_enable {
            return Err(io::Error::new(io::ErrorKind::Other, "no tty"));
        }
        self.enabled = true;
        Ok(())
    }
    fn disable(&mut self) -> io::Result<()> {
        self.disable_calls += 1;
        self.enabled = false;
        Ok(())
    }
}

fn text(out: &[u8]) -> String {
    String::from_utf8(out.to_vec()).expect("terminal output is utf-8")
}

fn assert_restored(out: &[u8], raw: &FakeRaw) {
    let s = text(out);
    let mouse_off = s.rfind(MOUSE_OFF).expect("mouse capture disabled");
    let leave = s.rfind(LEAVE_ALT).expect("alternate screen left");
    assert!(mouse_off < leave, "mouse capture is disabled before leaving the alternate screen");
    assert!(!raw.enabled, "raw mode is off after restore");
    assert_eq!(raw.disable_calls, 1, "raw mode is disabled exactly once");
}

#[test]
fn enters_alternate_screen_with_mouse_capture_and_raw_mode() {
    let mut out: Vec<u8> = Vec::new();
    let mut raw = FakeRaw::default();
    let mut seen_enabled = false;
    let mut seen_prefix = String::new();
    run_in_terminal(&mut out, &mut raw, |w: &mut Vec<u8>| {
        seen_prefix = text(w);
    })
    .expect("runs");
    seen_enabled |= raw.enable_calls == 1;
    assert!(seen_enabled);
    assert!(seen_prefix.contains(ENTER_ALT), "alternate screen entered before the body runs");
    assert!(seen_prefix.contains(MOUSE_ON), "mouse capture on before the body runs");
}

#[test]
fn restores_on_normal_exit_and_returns_the_body_value() {
    let mut out: Vec<u8> = Vec::new();
    let mut raw = FakeRaw::default();
    let value = run_in_terminal(&mut out, &mut raw, |_| 42).expect("runs");
    assert_eq!(value, 42);
    assert_restored(&out, &raw);
}

#[test]
fn restores_when_the_body_returns_an_error() {
    let mut out: Vec<u8> = Vec::new();
    let mut raw = FakeRaw::default();
    let result: io::Result<Result<(), &str>> =
        run_in_terminal(&mut out, &mut raw, |_| Err("render failed"));
    assert_eq!(result.expect("terminal itself was fine"), Err("render failed"));
    assert_restored(&out, &raw);
}

#[test]
fn restores_on_panic_and_propagates_it() {
    let mut out: Vec<u8> = Vec::new();
    let mut raw = FakeRaw::default();
    let caught = catch_unwind(AssertUnwindSafe(|| {
        let _ = run_in_terminal(&mut out, &mut raw, |_| -> () { panic!("boom") });
    }));
    let payload = caught.expect_err("the panic still propagates to the caller");
    assert_eq!(payload.downcast_ref::<&str>().copied(), Some("boom"));
    assert_restored(&out, &raw);
}

#[test]
fn failure_to_enter_raw_mode_is_an_error_and_leaves_nothing_changed() {
    let mut out: Vec<u8> = Vec::new();
    let mut raw = FakeRaw { fail_enable: true, ..FakeRaw::default() };
    let mut ran = false;
    let result = run_in_terminal(&mut out, &mut raw, |_| ran = true);
    assert!(result.is_err());
    assert!(!ran, "the body never runs on a terminal that could not be set up");
    assert!(!raw.enabled);
    assert!(!text(&out).contains(ENTER_ALT), "no alternate screen without raw mode");
}
