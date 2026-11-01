//! The Herdr popup: a pure render of account snapshots into a ratatui frame.

use chrono::{DateTime, Utc};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::account::{Account, Provider};
use crate::countdown::ResetDisplay;
use crate::display::{account_view, bonus_label, AccountView};
use crate::snapshot::Snapshot;

/// Border and button accent.
const ACCENT: Color = Color::Rgb(232, 163, 23);
const BUTTON_BG: Color = Color::Rgb(58, 46, 20);

/// One configured account and its latest snapshot, if any.
#[derive(Debug, Clone)]
pub struct AccountEntry {
    pub account: Account,
    pub snapshot: Option<Snapshot>,
}

pub fn render_popup(frame: &mut Frame, entries: &[AccountEntry], now: DateTime<Utc>) {
    let block = Block::bordered().border_style(Style::new().fg(ACCENT));
    frame.render_widget(block, frame.area());

    let [header, divider, body] = regions(frame.area());
    render_header(frame, header);
    render_divider(frame, divider);
    render_body(frame, body, &body_rows(entries, now));
}

/// Header, divider and body areas inside the popup border. Shared by drawing
/// and hit-testing so clicks always land where things are drawn.
fn regions(area: Rect) -> [Rect; 3] {
    let inner = Block::bordered().inner(area);
    Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)])
        .areas(inner)
}

/// Width of the header's button column: ` ↻ ` + gap + ` × `.
const BUTTONS_WIDTH: u16 = 7;

/// Splits the header into its title and, when all of it fits, the button column.
fn header_parts(header: Rect) -> (Rect, Option<Rect>) {
    if header.height == 0 || header.width < BUTTONS_WIDTH {
        return (header, None);
    }
    let [title, buttons] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(BUTTONS_WIDTH)]).areas(header);
    (title, Some(buttons))
}

/// The clickable close (×) button of a popup drawn in `area`; empty when the
/// header is too narrow to show it.
pub fn close_button(area: Rect) -> Rect {
    match header_parts(regions(area)[0]) {
        (_, Some(buttons)) => Rect::new(buttons.x + 4, buttons.y, 3, 1),
        (_, None) => Rect::default(),
    }
}

/// One line of the popup body.
enum BodyRow {
    /// Left text and right-aligned text on one line.
    Split { left: Line<'static>, right: Line<'static> },
    /// A usage window: title, meter, and time left.
    Window { title: String, fill: Option<f64>, scoped: Vec<f64>, time_left: String },
    Blank,
    Rule,
}

/// Columns of a window row: title | meter | time left.
const TITLE_WIDTH: u16 = 9;
const TIME_WIDTH: u16 = 7;
/// Meters at or above this fill use the urgent color.
const URGENT_FILL: f64 = 0.9;

fn body_rows(entries: &[AccountEntry], now: DateTime<Utc>) -> Vec<BodyRow> {
    let mut rows = Vec::new();
    for (provider, mark, title) in [
        (Provider::Claude, "✻", "Claude Code"),
        (Provider::Codex, "◍", "Codex"),
    ] {
        let accounts: Vec<&AccountEntry> =
            entries.iter().filter(|e| e.account.provider == provider).collect();
        if accounts.is_empty() {
            continue;
        }
        if !rows.is_empty() {
            rows.push(BodyRow::Rule);
        }
        let lone_plan = match accounts.as_slice() {
            [only] => plan_label(only),
            _ => None,
        };
        rows.push(BodyRow::Split {
            left: Line::from(vec![
                Span::styled(format!("{mark} "), Style::new().fg(ACCENT)),
                Span::styled(title, Style::new().add_modifier(Modifier::BOLD)),
            ]),
            right: dim(lone_plan.unwrap_or_default()),
        });
        if let [only] = accounts.as_slice() {
            rows.extend(window_rows(only, now));
        } else {
            for (index, entry) in accounts.iter().enumerate() {
                if index > 0 {
                    rows.push(BodyRow::Blank);
                }
                rows.push(account_row(entry));
                rows.extend(window_rows(entry, now));
            }
        }
    }
    rows
}

fn window_rows(entry: &AccountEntry, now: DateTime<Utc>) -> Vec<BodyRow> {
    // No snapshot yet, or nothing measured: say so rather than show a bare heading.
    let unavailable = || {
        vec![BodyRow::Split { left: dim("Usage unavailable".to_string()), right: Line::default() }]
    };
    let Some(snapshot) = &entry.snapshot else { return unavailable() };
    let mut body: Vec<BodyRow> = match account_view(snapshot, now) {
        AccountView::Rows(rows) => rows
            .into_iter()
            .map(|row| BodyRow::Window {
                title: row.label,
                fill: row.fill,
                scoped: row.scoped,
                time_left: match row.reset {
                    Some(ResetDisplay::Remaining(left)) => left,
                    Some(ResetDisplay::Stale) | None => String::new(),
                },
            })
            .collect(),
        AccountView::Unavailable => unavailable(),
    };
    // Free resets get a line only when reported; absent is never drawn as 0.
    if let Some(label) = snapshot.bonus_resets.as_ref().and_then(|bonus| bonus_label(bonus, now)) {
        body.push(BodyRow::Split { left: dim(label), right: Line::default() });
    }
    body
}

fn plan_label(entry: &AccountEntry) -> Option<String> {
    entry.snapshot.as_ref().and_then(|s| s.plan_label.clone())
}

fn account_row(entry: &AccountEntry) -> BodyRow {
    let name = match plan_label(entry) {
        Some(plan) => format!("{} · {plan}", entry.account.name),
        None => entry.account.name.clone(),
    };
    let right = if entry.account.active {
        Line::from(Span::styled("ACTIVE", Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)))
    } else {
        dim("Use".to_string())
    };
    BodyRow::Split { left: Line::from(Span::styled(name, Style::new().add_modifier(Modifier::BOLD))), right }
}

fn dim(text: String) -> Line<'static> {
    Line::from(Span::styled(text, Style::new().fg(Color::Gray)))
}

fn render_body(frame: &mut Frame, area: Rect, rows: &[BodyRow]) {
    for (offset, row) in rows.iter().enumerate().take(area.height as usize) {
        let line = Rect::new(area.x, area.y + offset as u16, area.width, 1);
        match row {
            BodyRow::Split { left, right } => {
                let width = right.width() as u16;
                let [left_area, right_area] =
                    Layout::horizontal([Constraint::Min(0), Constraint::Length(width)]).areas(line);
                frame.render_widget(Paragraph::new(left.clone()), left_area);
                frame.render_widget(Paragraph::new(right.clone()), right_area);
            }
            BodyRow::Window { title, fill, scoped, time_left } => {
                let [title_area, _, meter_area, _, time_area] = Layout::horizontal([
                    Constraint::Length(TITLE_WIDTH),
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Length(TIME_WIDTH),
                ])
                .areas(line);
                frame.render_widget(Paragraph::new(title.as_str()), title_area);
                frame.render_widget(Paragraph::new(meter_line(*fill, scoped, meter_area.width)), meter_area);
                frame.render_widget(
                    Paragraph::new(time_left.as_str())
                        .alignment(Alignment::Right)
                        .style(Style::new().fg(Color::Gray)),
                    time_area,
                );
            }
            BodyRow::Blank => {}
            BodyRow::Rule => render_divider(frame, line),
        }
    }
}

/// A meter track of `width` cells, filled in proportion to `fill`, with a tick
/// at each scoped allowance's used percent.
fn meter_line(fill: Option<f64>, scoped: &[f64], width: u16) -> Line<'static> {
    let width = width as usize;
    let filled = fill.map_or(0, |f| (f * width as f64).round() as usize).min(width);
    let color = if fill.is_some_and(|f| f >= URGENT_FILL) { Color::Red } else { Color::White };
    let mut cells: Vec<Span<'static>> = (0..width)
        .map(|i| {
            let track = if i < filled { color } else { Color::DarkGray };
            Span::styled("━", Style::new().fg(track))
        })
        .collect();
    for fraction in scoped {
        if width > 0 {
            let at = ((fraction * width as f64).floor() as usize).min(width - 1);
            cells[at] = Span::styled("┃", Style::new().fg(ACCENT));
        }
    }
    Line::from(cells)
}

fn render_header(frame: &mut Frame, area: Rect) {
    let (title, buttons) = header_parts(area);
    let mark = Span::styled("[•_•] ", Style::new().fg(Color::White));
    let name = Span::styled("Agents", Style::new().add_modifier(Modifier::BOLD));
    frame.render_widget(Paragraph::new(Line::from(vec![mark, name])), title);
    let Some(buttons) = buttons else { return };

    let button = Style::new().fg(ACCENT).bg(BUTTON_BG);
    let line = Line::from(vec![
        Span::styled(" ↻ ", button),
        Span::raw(" "),
        Span::styled(" × ", button),
    ]);
    frame.render_widget(Paragraph::new(line), buttons);
}

fn render_divider(frame: &mut Frame, area: Rect) {
    let rule = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(rule).style(Style::new().fg(Color::DarkGray)),
        area,
    );
}
