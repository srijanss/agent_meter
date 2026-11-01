//! Terminal setup and guaranteed restore for the Herdr popup.
//!
//! The popup runs on the alternate screen with mouse capture and raw mode on.
//! [`run_in_terminal`] always undoes all three — on normal return, on an error
//! value, and on panic — so Herdr gets its origin pane back cleanly.

use std::io::{self, Write};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

/// Raw-mode switch, abstracted so the lifecycle can be tested without a TTY.
pub trait RawMode {
    fn enable(&mut self) -> io::Result<()>;
    fn disable(&mut self) -> io::Result<()>;
}

/// Raw mode on the process's real terminal.
pub struct CrosstermRawMode;

impl RawMode for CrosstermRawMode {
    fn enable(&mut self) -> io::Result<()> {
        enable_raw_mode()
    }
    fn disable(&mut self) -> io::Result<()> {
        disable_raw_mode()
    }
}

/// Enter raw mode, the alternate screen and mouse capture, run `body`, and
/// restore the terminal however `body` ends. A panic in `body` is restored
/// from, then re-raised unchanged.
pub fn run_in_terminal<W, R, T>(
    out: &mut W,
    raw: &mut R,
    body: impl FnOnce(&mut W) -> T,
) -> io::Result<T>
where
    W: Write,
    R: RawMode,
{
    raw.enable()?;
    if let Err(error) = execute!(out, EnterAlternateScreen, EnableMouseCapture) {
        // Entry may have got part-way (e.g. the alternate screen is on but
        // mouse capture failed), so undo everything and report the original error.
        let _ = restore(out, raw);
        return Err(error);
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| body(out)));
    let restored = restore(out, raw);
    match outcome {
        Ok(value) => restored.map(|()| value),
        Err(payload) => resume_unwind(payload),
    }
}

fn restore<W: Write, R: RawMode>(out: &mut W, raw: &mut R) -> io::Result<()> {
    // Every step is attempted even if an earlier one failed, so one bad write
    // cannot leave the screen switched or raw mode on. The first error wins.
    let mouse = execute!(out, DisableMouseCapture);
    let screen = execute!(out, LeaveAlternateScreen);
    let mode = raw.disable();
    mouse.and(screen).and(mode)
}
