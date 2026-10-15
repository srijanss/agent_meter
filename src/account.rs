//! Accounts from this project's own TOML config: names and paths only, never credentials.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Claude,
    Codex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub provider: Provider,
    /// The CLI's config directory (explicit, or `~/.claude` / `~/.codex`).
    pub home: PathBuf,
}

impl Account {
    /// The account name is the opaque profile id used in snapshots.
    pub fn profile_id(&self) -> &str {
        &self.name
    }

    /// Environment variable and value that point the provider CLI at this account.
    pub fn home_env(&self) -> (&'static str, PathBuf) {
        let var = match self.provider {
            Provider::Claude => "CLAUDE_CONFIG_DIR",
            Provider::Codex => "CODEX_HOME",
        };
        (var, self.home.clone())
    }
}

/// Config problem. Never echoes config values, which could be secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAccount {
    provider: Provider,
    home: Option<String>,
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(default)]
    accounts: BTreeMap<String, RawAccount>,
}

fn is_clean(s: &str) -> bool {
    !s.trim().is_empty() && !s.chars().any(char::is_control)
}

/// Parses `[accounts.<name>]` tables. `user_home` supplies the default dirs.
pub fn parse_accounts(toml_text: &str, user_home: &Path) -> Result<Vec<Account>, ConfigError> {
    // The toml error text quotes source lines, so it is deliberately dropped.
    let raw: RawConfig = toml::from_str(toml_text).map_err(|_| {
        ConfigError("config is not valid TOML or has unsupported account fields".into())
    })?;
    let mut accounts = Vec::with_capacity(raw.accounts.len());
    for (name, account) in raw.accounts {
        if !is_clean(&name) {
            return Err(ConfigError("account name is blank or has control characters".into()));
        }
        let home = match account.home {
            Some(home) if is_clean(&home) => PathBuf::from(home),
            Some(_) => {
                return Err(ConfigError(format!(
                    "account `{name}` home is blank or has control characters"
                )))
            }
            None => user_home.join(match account.provider {
                Provider::Claude => ".claude",
                Provider::Codex => ".codex",
            }),
        };
        accounts.push(Account { name, provider: account.provider, home });
    }
    Ok(accounts)
}
