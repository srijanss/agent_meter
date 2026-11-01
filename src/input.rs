//! Turns terminal events into popup actions. Pure: no I/O, so it is testable
//! without a terminal.

use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};

use crate::popup::{close_button, refresh_button};

/// What the popup should do in response to an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Exit the popup so Herdr dismisses it.
    Close,
    /// Refresh every account's usage.
    Refresh,
}

/// The action for `event` on a popup drawn in `area`, if any.
///
/// Only key presses and left-button presses count: releases and repeats of the
/// same gesture must not fire a second time.
pub fn action_for(event: &Event, area: Rect) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc => {
            Some(Action::Close)
        }
        Event::Key(key) if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('r') => {
            Some(Action::Refresh)
        }
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
            let at = Position::new(mouse.column, mouse.row);
            if close_button(area).contains(at) {
                Some(Action::Close)
            } else if refresh_button(area).contains(at) {
                Some(Action::Refresh)
            } else {
                None
            }
        }
        _ => None,
    }
}
