//! `herdr-agents`: the Herdr popup showing Claude Code and Codex usage.

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use agent_meter::app::{default_config_path, parse_args, system_jobs, App};
use agent_meter::popup::render_popup_with;
use agent_meter::terminal::{run_in_terminal, CrosstermRawMode};
use chrono::Utc;
use crossterm::event;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// Keeps a reappearing countdown or in-progress indicator current while idle.
const TICK: Duration = Duration::from_millis(200);
/// A manual refresh may re-run an account this long after its last one ended.
const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

fn main() -> ExitCode {
    // Herdr owns stdout, so every message goes to stderr.
    match start() {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("herdr-agents: {message}");
            ExitCode::from(code)
        }
    }
}

/// Exit code 2 is a problem with how it was started (arguments, config);
/// 1 is a problem running the popup.
fn start() -> Result<(), (u8, String)> {
    let usage = |e: agent_meter::app::AppError| (2, e.to_string());
    let config = parse_args(std::env::args().skip(1)).map_err(usage)?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or((2, "HOME is not set".to_string()))?;
    let config = config.unwrap_or_else(|| default_config_path(&home));
    let mut app = App::new(config, &home, system_jobs(), MIN_REFRESH_INTERVAL).map_err(usage)?;

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err((1, "must run in a terminal, as a Herdr popup".to_string()));
    }
    let mut out = io::stdout();
    run_in_terminal(&mut out, &mut CrosstermRawMode, |out| event_loop(&mut app, out))
        .and_then(|result| result)
        .map_err(|e| (1, format!("terminal error: {e}")))
}

fn event_loop<W: Write>(app: &mut App, out: &mut W) -> io::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;
    app.refresh(Instant::now());
    while !app.should_quit() {
        app.poll(Instant::now());
        terminal.draw(|frame| {
            render_popup_with(frame, app.entries(), Utc::now(), app.refreshing());
        })?;
        if event::poll(TICK)? {
            let event = event::read()?;
            let size = terminal.size()?;
            let area = Rect::new(0, 0, size.width, size.height);
            // Choosing an account can fail to rewrite the config; the popup
            // keeps running with what it has.
            let _ = app.handle(&event, area, Utc::now(), Instant::now());
        }
    }
    Ok(())
}
