//! Turns terminal events into popup actions. Pure: no I/O, so it is testable
//! without a terminal.

use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};

use chrono::{DateTime, Utc};

use crate::popup::{close_button, page_size, refresh_button, use_links_viewed, AccountEntry};

/// What the popup should do in response to an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Exit the popup so Herdr dismisses it.
    Close,
    /// Refresh every account's usage.
    Refresh,
    /// Mark the named account as the one in use for its provider.
    Use(String),
    /// Move the popup's content by this many rows (negative is up).
    Scroll(isize),
}

/// Rows a mouse wheel step moves the content.
const WHEEL_ROWS: isize = 3;

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
    action_scrolled(event, area, entries, now, 0)
}

/// Like `action_in` for a popup whose content is scrolled down by `scroll`
/// rows: Use links are those visible at that offset, and the wheel, arrow and
/// page keys ask to scroll.
pub fn action_scrolled(
    event: &Event,
    area: Rect,
    entries: &[AccountEntry],
    now: DateTime<Utc>,
    scroll: usize,
) -> Option<Action> {
    action_viewed(event, area, entries, now, scroll, false)
}

/// Like `action_scrolled` for a popup that may show a notice, which covers the
/// bottom body row: a Use link under it cannot be chosen.
pub fn action_viewed(
    event: &Event,
    area: Rect,
    entries: &[AccountEntry],
    now: DateTime<Utc>,
    scroll: usize,
    has_notice: bool,
) -> Option<Action> {
    if let Some(action) = action_for(event, area) {
        return Some(action);
    }
    let page = page_size(area) as isize;
    match event {
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollDown => {
            return Some(Action::Scroll(WHEEL_ROWS));
        }
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollUp => {
            return Some(Action::Scroll(-WHEEL_ROWS));
        }
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Down => return Some(Action::Scroll(1)),
            KeyCode::Up => return Some(Action::Scroll(-1)),
            KeyCode::PageDown => return Some(Action::Scroll(page)),
            KeyCode::PageUp => return Some(Action::Scroll(-page)),
            _ => {}
        },
        _ => {}
    }
    let links = use_links_viewed(area, entries, now, scroll, has_notice);
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
