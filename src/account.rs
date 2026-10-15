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
    /// The account marked in use for its provider (a config mark only).
    pub active: bool,
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
    active: Option<bool>,
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
                // The name is config-supplied and may itself be sensitive.
                return Err(ConfigError(
                    "an account home is blank or has control characters".into(),
                ));
            }
            None => user_home.join(match account.provider {
                Provider::Claude => ".claude",
                Provider::Codex => ".codex",
            }),
        };
        accounts.push(Account {
            name,
            provider: account.provider,
            home,
            active: account.active.unwrap_or(false),
        });
    }
    Ok(accounts)
}

/// Marks `name` active for its provider in the config file at `path`, clearing
/// the mark from that provider's other accounts. Writes only this config file,
/// atomically (temp file + rename); never touches credentials or CLI logins.
pub fn mark_active(path: &Path, name: &str) -> Result<(), ConfigError> {
    let fail = |msg: &str| ConfigError(msg.into());
    let text = std::fs::read_to_string(path).map_err(|_| fail("cannot read config"))?;
    let mut doc: toml::Table = text.parse().map_err(|_| fail("config is not valid TOML"))?;
    let accounts = doc
        .get_mut("accounts")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| fail("config has no accounts"))?;
    let provider = accounts
        .get(name)
        .and_then(|a| a.get("provider"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| fail("no such account"))?
        .to_string();
    for (key, account) in accounts.iter_mut() {
        let Some(table) = account.as_table_mut() else { continue };
        if table.get("provider").and_then(toml::Value::as_str) != Some(provider.as_str()) {
            continue;
        }
        if key == name {
            table.insert("active".into(), toml::Value::Boolean(true));
        } else {
            table.remove("active");
        }
    }
    let out = toml::to_string(&doc).map_err(|_| fail("cannot serialize config"))?;
    let file_name = path.file_name().ok_or_else(|| fail("config path has no file name"))?;
    // Created exclusively under a name unique to this call, so a planted file
    // or symlink is skipped (never written through, never removed).
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        // Owner-only until the original's permissions are copied over.
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut created = None;
    for _ in 0..16 {
        let tmp = path.with_file_name(format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        match options.open(&tmp) {
            Ok(file) => {
                created = Some((tmp, file));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(fail("cannot write config")),
        }
    }
    let (tmp, mut file) = created.ok_or_else(|| fail("cannot write config"))?;
    let mut write = || -> std::io::Result<()> {
        use std::io::Write;
        file.set_permissions(std::fs::metadata(path)?.permissions())?;
        file.write_all(out.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    };
    write().map_err(|_| {
        // Only reached once this call created `tmp`, so it is ours to remove.
        let _ = std::fs::remove_file(&tmp);
        fail("cannot write config")
    })
}
