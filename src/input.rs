//! Turns terminal events into popup actions. Pure: no I/O, so it is testable
//! without a terminal.

use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};

use chrono::{DateTime, Utc};

use crate::popup::{close_button, refresh_button, use_links, AccountEntry};

/// What the popup should do in response to an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Exit the popup so Herdr dismisses it.
    Close,
    /// Refresh every account's usage.
    Refresh,
    /// Mark the named account as the one in use for its provider.
    Use(String),
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

/// Like `action_for`, but also resolves the account Use links drawn for
/// `entries`: a left click on a link, or its number key (1 is the topmost).
pub fn action_in(
    event: &Event,
    area: Rect,
    entries: &[AccountEntry],
    now: DateTime<Utc>,
) -> Option<Action> {
    if let Some(action) = action_for(event, area) {
        return Some(action);
    }
    let links = use_links(area, entries, now);
    match event {
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => links
            .into_iter()
            .find(|link| link.area.contains(Position::new(mouse.column, mouse.row)))
            .map(|link| Action::Use(link.account)),
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            let KeyCode::Char(digit @ '1'..='9') = key.code else { return None };
            let number = digit.to_digit(10)? as usize;
            links.into_iter().nth(number - 1).map(|link| Action::Use(link.account))
        }
        _ => None,
    }
}
