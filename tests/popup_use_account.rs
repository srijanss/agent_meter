use std::fs;
use std::path::{Path, PathBuf};

use agent_meter::account::{mark_active, parse_accounts};
use agent_meter::input::{action_in, Action};
use agent_meter::popup::{render_popup, use_links, AccountEntry};
use chrono::{DateTime, TimeZone, Utc};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;

const AREA: Rect = Rect { x: 0, y: 0, width: 48, height: 20 };

const CONFIG: &str = r#"
[accounts.claude-primary]
provider = "claude"
active = true

[accounts.claude-work]
provider = "claude"
home = "/data/claude-work"

[accounts.codex-main]
provider = "codex"
active = true
"#;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
}

fn entries(config: &str) -> Vec<AccountEntry> {
    parse_accounts(config, Path::new("/home/u"))
        .unwrap()
        .into_iter()
        .map(|account| AccountEntry { account, snapshot: None })
        .collect()
}

fn render(entries: &[AccountEntry]) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    terminal.draw(|frame| render_popup(frame, entries, now())).unwrap();
    terminal.backend().buffer().clone()
}

/// Column and row where `word` starts, found by reading the rendered rows.
fn find_word(buf: &Buffer, word: &str) -> Option<(u16, u16)> {
    (0..buf.area.height).find_map(|y| {
        let row: Vec<&str> = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
        (0..row.len())
            .find(|&x| row[x..].concat().starts_with(word))
            .map(|x| (x as u16, y))
    })
}

fn click(button: MouseButton, (column, row): (u16, u16)) -> Event {
    Event::Mouse(MouseEvent { kind: MouseEventKind::Down(button), column, row, modifiers: KeyModifiers::NONE })
}

fn key(code: KeyCode, kind: KeyEventKind) -> Event {
    let mut event = KeyEvent::new(code, KeyModifiers::NONE);
    event.kind = kind;
    Event::Key(event)
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_use_link_and_digit_keys_choose_a_non_active_account_and_nothing_else_does() {
    let entries = entries(CONFIG);
    let buf = render(&entries);
    let at = find_word(&buf, "Use").expect("claude-work shows a Use link");
    let act = |event: Event| action_in(&event, AREA, &entries, now());

    let links = use_links(AREA, &entries, now());
    assert_eq!(links.len(), 1, "only the non-active account of a multi-account provider has one");
    assert_eq!(links[0].account, "claude-work");
    assert!(links[0].area.contains(at.into()));

    // Every cell of the drawn link works; its neighbours and other buttons do not.
    for dx in 0..3 {
        assert_eq!(act(click(MouseButton::Left, (at.0 + dx, at.1))), Some(Action::Use("claude-work".into())));
    }
    assert_eq!(act(click(MouseButton::Left, (at.0 - 1, at.1))), None);
    assert_eq!(act(click(MouseButton::Left, (at.0, at.1 + 1))), None);
    assert_eq!(act(click(MouseButton::Right, at)), None);
    let active = find_word(&buf, "ACTIVE").expect("the active account is labelled");
    assert_eq!(act(click(MouseButton::Left, active)), None, "ACTIVE is not a link");

    // The keyboard equivalent is the number of the Use link, counted top to bottom.
    assert_eq!(act(key(KeyCode::Char('1'), KeyEventKind::Press)), Some(Action::Use("claude-work".into())));
    assert_eq!(act(key(KeyCode::Char('2'), KeyEventKind::Press)), None);
    assert_eq!(act(key(KeyCode::Char('0'), KeyEventKind::Press)), None);
    assert_eq!(act(key(KeyCode::Char('1'), KeyEventKind::Release)), None);

    // The older actions still work through the same entry point.
    assert_eq!(act(key(KeyCode::Esc, KeyEventKind::Press)), Some(Action::Close));
    assert_eq!(act(key(KeyCode::Char('r'), KeyEventKind::Press)), Some(Action::Refresh));
}

#[test]
fn using_an_account_moves_the_active_label_and_touches_only_the_config() {
    let dir = scratch("use_account");
    let path = dir.join("config.toml");
    fs::write(&path, CONFIG).unwrap();

    let before = render(&entries(CONFIG));
    let use_at = find_word(&before, "Use").unwrap();
    let active_at = find_word(&before, "ACTIVE").unwrap();

    // The click's action names the account; applying it marks it active in the config.
    let Some(Action::Use(name)) = action_in(
        &click(MouseButton::Left, use_at),
        AREA,
        &entries(CONFIG),
        now(),
    ) else {
        panic!("the click did not choose an account")
    };
    mark_active(&path, &name).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    let after = render(&entries(&text));
    let new_active = find_word(&after, "ACTIVE");
    let new_use = find_word(&after, "Use").unwrap();
    // claude's two accounts swapped rows; codex's lone account is unchanged.
    assert_eq!(new_use.1, active_at.1, "the old active account now offers Use");
    assert_eq!(new_active.map(|p| p.1), Some(use_at.1), "ACTIVE moved to the clicked row");

    // Only the config and its own lock file appear: no credentials, caches or logs.
    let mut files: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, vec![".config.toml.lock", "config.toml"], "nothing but the config is written");
}
