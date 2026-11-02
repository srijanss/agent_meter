//! The popup's application state: accounts and their latest data, gated
//! refreshes on worker threads, and the actions chosen in the popup. The
//! terminal loop in `main.rs` only draws this and feeds it events, so all of it
//! is testable without a terminal.

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use crossterm::event::{Event, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use crate::account::{mark_active, parse_accounts, Account, Provider};
use crate::claude_collector::collect_claude;
use crate::codex_collector::collect_codex;
use crate::collection::Collections;
use crate::failure::{AccountOutcome, CollectorError};
use crate::input::{action_viewed, Action};
use crate::popup::{max_scroll, AccountEntry};
use crate::refresh::{accounts_to_refresh, RefreshGate};
use crate::runner::SystemRunner;
use crate::snapshot::Snapshot;
use crate::system_session::SystemSessionStarter;

/// A problem worth telling the user about. Never carries config values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError(String);

impl AppError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AppError {}

/// Shown when choosing an account fails; the cause stays out of the popup.
const SWITCH_FAILED: &str = "Could not switch account; check the config file";

pub const USAGE: &str = "usage: herdr-agents [--config PATH]";

/// Largest config file read; a config is a few lines, so anything bigger is wrong.
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

pub fn default_config_path(user_home: &Path) -> PathBuf {
    user_home.join(".config").join("agent-meter").join("config.toml")
}

/// The config path named on the command line, if any.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<PathBuf>, AppError> {
    let mut args = args.into_iter();
    match (args.next().as_deref(), args.next(), args.next()) {
        (None, _, _) => Ok(None),
        (Some("--config"), Some(path), None) if !path.is_empty() => Ok(Some(PathBuf::from(path))),
        _ => Err(AppError::new(USAGE)),
    }
}

pub fn load_accounts(path: &Path, user_home: &Path) -> Result<Vec<Account>, AppError> {
    let cannot_read = || AppError::new(format!("cannot read config at {}", path.display()));
    let mut text = String::new();
    File::open(path)
        .and_then(|file| file.take(MAX_CONFIG_BYTES + 1).read_to_string(&mut text))
        .map_err(|_| cannot_read())?;
    if text.len() as u64 > MAX_CONFIG_BYTES {
        return Err(AppError::new("config is too large"));
    }
    let accounts = parse_accounts(&text, user_home).map_err(|e| AppError::new(e.to_string()))?;
    if accounts.is_empty() {
        return Err(AppError::new("config has no accounts; add an [accounts.<name>] table"));
    }
    Ok(accounts)
}

pub type Job = Box<dyn FnOnce() -> Result<Snapshot, CollectorError> + Send>;
/// Builds the collection job for an account; tests script it.
pub type JobFactory = Box<dyn Fn(&Account) -> Job>;

/// Jobs that run the real `claude` and `codex` CLIs.
pub fn system_jobs() -> JobFactory {
    Box::new(|account: &Account| {
        let account = account.clone();
        match account.provider {
            Provider::Claude => Box::new(move || collect_claude(&SystemRunner, &account, Utc::now())),
            Provider::Codex => {
                Box::new(move || collect_codex(&SystemSessionStarter, &account, Utc::now()))
            }
        }
    })
}

pub struct App {
    config_path: PathBuf,
    user_home: PathBuf,
    entries: Vec<AccountEntry>,
    gate: RefreshGate,
    collections: Collections,
    jobs: JobFactory,
    hover: Option<Position>,
    notice: Option<String>,
    scroll: usize,
    quit: bool,
}

impl App {
    /// Loads the accounts; every one is listed at once, without data until its
    /// first collection finishes.
    pub fn new(
        config_path: PathBuf,
        user_home: &Path,
        jobs: JobFactory,
        min_refresh_interval: Duration,
    ) -> Result<Self, AppError> {
        let entries = load_accounts(&config_path, user_home)?
            .into_iter()
            .map(|account| AccountEntry { account, snapshot: None })
            .collect();
        Ok(Self {
            config_path,
            user_home: user_home.to_path_buf(),
            entries,
            gate: RefreshGate::new(min_refresh_interval),
            collections: Collections::new(),
            jobs,
            hover: None,
            notice: None,
            scroll: 0,
            quit: false,
        })
    }

    pub fn entries(&self) -> &[AccountEntry] {
        &self.entries
    }

    pub fn refreshing(&self) -> bool {
        self.gate.any_in_flight()
    }

    /// Where the pointer last was inside the popup, if it still is.
    pub fn hover(&self) -> Option<Position> {
        self.hover
    }

    /// A message about the last action that failed, shown until the next
    /// action. Fixed text: it never carries config values or paths.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Rows the content is scrolled down by; draw with this offset.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Starts a collection for each account that is neither running nor too
    /// recently finished. Returns at once; results arrive through `poll`.
    pub fn refresh(&mut self, now: Instant) {
        let names: Vec<String> = self.entries.iter().map(|e| e.account.name.clone()).collect();
        for name in accounts_to_refresh(&mut self.gate, &names, now) {
            if let Some(entry) = self.entries.iter().find(|e| e.account.name == name) {
                let job = (self.jobs)(&entry.account);
                self.collections.spawn(name, job);
            }
        }
    }

    /// Takes in finished collections. A failure leaves the account's last good
    /// data in place rather than clearing it.
    pub fn poll(&mut self, now: Instant) {
        for (name, outcome) in self.collections.poll() {
            self.gate.finish(&name, now);
            if let (AccountOutcome::Snapshot(snapshot), Some(entry)) =
                (outcome, self.entries.iter_mut().find(|e| e.account.name == name))
            {
                entry.snapshot = Some(snapshot);
            }
        }
    }

    /// Applies the action for `event`. Errors are from choosing an account
    /// (the config could not be rewritten or read back); the popup stays usable.
    pub fn handle(
        &mut self,
        event: &Event,
        area: Rect,
        now_utc: DateTime<Utc>,
        now: Instant,
    ) -> Result<(), AppError> {
        if let Event::Mouse(MouseEvent { kind: MouseEventKind::Moved, column, row, .. }) = event {
            let at = Position::new(*column, *row);
            self.hover = area.contains(at).then_some(at);
        }
        let has_notice = self.notice.is_some();
        let action = action_viewed(event, area, &self.entries, now_utc, self.scroll, has_notice);
        if action.is_some() {
            self.notice = None;
        }
        match action {
            Some(Action::Scroll(rows)) => {
                let max = max_scroll(area, &self.entries, now_utc, false);
                self.scroll = self.scroll.saturating_add_signed(rows).min(max);
            }
            Some(Action::Close) => self.quit = true,
            Some(Action::Refresh) => self.refresh(now),
            Some(Action::Use(name)) => {
                if let Err(error) = self.use_account(&name) {
                    self.notice = Some(SWITCH_FAILED.to_string());
                    return Err(error);
                }
            }
            None => {}
        }
        Ok(())
    }

    fn use_account(&mut self, name: &str) -> Result<(), AppError> {
        mark_active(&self.config_path, name).map_err(|e| AppError::new(e.to_string()))?;
        // Load first: if the config cannot be read back, nothing changes.
        let accounts = load_accounts(&self.config_path, &self.user_home)?;
        let mut snapshots: HashMap<String, Option<Snapshot>> = self
            .entries
            .drain(..)
            .map(|entry| (entry.account.name, entry.snapshot))
            .collect();
        self.entries = accounts
            .into_iter()
            .map(|account| {
                let snapshot = snapshots.remove(&account.name).flatten();
                AccountEntry { account, snapshot }
            })
            .collect();
        Ok(())
    }
}
