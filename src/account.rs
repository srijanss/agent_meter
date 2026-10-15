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
    mark_active_inner(path, name, None, None)
}

/// Test seam: runs `hook` in the window just before the lock file is opened.
#[doc(hidden)]
pub fn mark_active_racing(path: &Path, name: &str, hook: &dyn Fn()) -> Result<(), ConfigError> {
    mark_active_inner(path, name, None, Some(hook))
}

/// A step of the atomic write that can be made to fail on purpose.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteStage {
    PermissionCopy,
    Write,
    Sync,
    Rename,
}

/// Test seam: `mark_active` with an I/O failure injected at `stage`.
#[doc(hidden)]
pub fn mark_active_failing_at(
    path: &Path,
    name: &str,
    stage: WriteStage,
) -> Result<(), ConfigError> {
    mark_active_inner(path, name, Some(stage), None)
}

/// Opens (creating if needed) the lock file without ever following a symlink.
/// Creation is exclusive, so a new lock is told apart from an existing one at
/// open time; permissions are changed only on the opened, verified regular file.
fn open_lock(
    lock_path: &Path,
    config: &Path,
    before_open: Option<&dyn Fn()>,
) -> Result<std::fs::File, ConfigError> {
    let fail = || ConfigError("cannot lock config".into());
    // The lock gets the config's access, never broader, so every user who may
    // rewrite a shared config can also take its lock.
    let config_permissions = std::fs::metadata(config)
        .map_err(|_| ConfigError("cannot read config".into()))?
        .permissions();
    if let Some(hook) = before_open {
        hook();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        #[cfg(target_os = "linux")]
        const O_NOFOLLOW: i32 = if cfg!(any(target_arch = "aarch64", target_arch = "arm")) {
            0o100000
        } else {
            0o400000
        };
        #[cfg(not(target_os = "linux"))]
        const O_NOFOLLOW: i32 = 0x100;
        #[cfg(target_os = "linux")]
        const O_NONBLOCK: i32 = 0o4000;
        #[cfg(not(target_os = "linux"))]
        const O_NONBLOCK: i32 = 0x4;
        let mut create = std::fs::OpenOptions::new();
        create.read(true).write(true).create_new(true).mode(0o600).custom_flags(O_NOFOLLOW);
        let (file, created) = match create.open(lock_path) {
            Ok(file) => (file, true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // O_NONBLOCK so a planted FIFO cannot hang the open.
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(O_NOFOLLOW | O_NONBLOCK)
                    .open(lock_path)
                    .map_err(|_| fail())?;
                (file, false)
            }
            Err(_) => return Err(fail()),
        };
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let meta = file.metadata().map_err(|_| fail())?;
        // A lock with other links is someone else's file in disguise: O_NOFOLLOW
        // does not stop hard links, and changing its mode would change theirs.
        if !meta.file_type().is_file() || meta.nlink() != 1 {
            return Err(fail());
        }
        let wanted = config_permissions.mode() & 0o7777;
        if meta.permissions().mode() & 0o7777 != wanted {
            let result = file.set_permissions(config_permissions);
            // An existing lock owned by someone else cannot be changed; it still works.
            if created {
                result.map_err(|_| fail())?;
            }
        }
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        let _ = config_permissions;
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|_| fail())
    }
}

fn mark_active_inner(
    path: &Path,
    name: &str,
    fault: Option<WriteStage>,
    before_lock_open: Option<&dyn Fn()>,
) -> Result<(), ConfigError> {
    let fail = |msg: &str| ConfigError(msg.into());
    // Serializes the whole read-modify-write across threads and processes. The
    // OS drops the lock when the handle closes (or the process dies), so a
    // crash never leaves a stale lock behind.
    let lock_name = path.file_name().ok_or_else(|| fail("config path has no file name"))?;
    let lock_path = path.with_file_name(format!(".{}.lock", lock_name.to_string_lossy()));
    let lock = open_lock(&lock_path, path, before_lock_open)?;
    lock.lock().map_err(|_| fail("cannot lock config"))?;
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
    let mut changed = false;
    for (key, account) in accounts.iter_mut() {
        let Some(table) = account.as_table_mut() else { continue };
        if table.get("provider").and_then(toml::Value::as_str) != Some(provider.as_str()) {
            continue;
        }
        if key == name {
            if table.get("active").and_then(toml::Value::as_bool) != Some(true) {
                table.insert("active".into(), toml::Value::Boolean(true));
                changed = true;
            }
        } else if table.remove("active").is_some() {
            changed = true;
        }
    }
    if !changed {
        // Already the sole active account: nothing to write.
        return Ok(());
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
        let inject = |stage: WriteStage| {
            if fault == Some(stage) {
                Err(std::io::Error::other("injected fault"))
            } else {
                Ok(())
            }
        };
        inject(WriteStage::PermissionCopy)?;
        file.set_permissions(std::fs::metadata(path)?.permissions())?;
        inject(WriteStage::Write)?;
        file.write_all(out.as_bytes())?;
        inject(WriteStage::Sync)?;
        file.sync_all()?;
        inject(WriteStage::Rename)?;
        std::fs::rename(&tmp, path)
    };
    write().map_err(|_| {
        // Only reached once this call created `tmp`, so it is ours to remove.
        let _ = std::fs::remove_file(&tmp);
        fail("cannot write config")
    })
}
