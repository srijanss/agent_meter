//! Model profiles from this project's own TOML: names and model ids only,
//! never credentials. Read for Orq's later use.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::account::{is_clean, ConfigError, Provider};
use crate::popup::AccountEntry;

/// The coding harness a profile runs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Harness {
    Claude,
    Pi,
}

/// The model provider a profile's model comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProvider {
    Anthropic,
    Openai,
}

/// One `[models.<name>]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProfile {
    pub name: String,
    pub harness: Harness,
    pub model: String,
    pub thinking: Option<String>,
    pub provider: ModelProvider,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfile {
    harness: Harness,
    model: String,
    thinking: Option<String>,
    provider: ModelProvider,
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(default)]
    models: BTreeMap<String, RawProfile>,
}

/// Reads the `[models.<name>]` tables, sorted by name. Other tables are ignored.
/// Errors never echo config values, which could be secrets.
pub fn parse_model_profiles(toml_text: &str) -> Result<Vec<ModelProfile>, ConfigError> {
    let raw: RawConfig = toml::from_str(toml_text).map_err(|_| {
        ConfigError::new("config is not valid TOML or has invalid model profiles")
    })?;
    raw.models
        .into_iter()
        .map(|(name, p)| {
            let clean = is_clean(&name)
                && is_clean(&p.model)
                && p.thinking.as_deref().is_none_or(is_clean);
            if !clean {
                return Err(ConfigError::new("model profile has a blank or control-character value"));
            }
            Ok(ModelProfile { name, harness: p.harness, model: p.model, thinking: p.thinking, provider: p.provider })
        })
        .collect()
}

/// Where a profile's usage comes from.
#[derive(Debug, Clone, Copy)]
pub enum Quota<'a> {
    /// The provider's active account; its snapshot is the profile's usage.
    Account(&'a AccountEntry),
    /// No active account for the provider.
    Unknown,
}

impl Quota<'_> {
    pub fn label(&self) -> &str {
        match self {
            Quota::Account(entry) => &entry.account.name,
            Quota::Unknown => "Quota unknown",
        }
    }
}

/// A profile's usage is its provider's active account, whatever the harness:
/// the harness is never a quota source, so profiles never double-count.
pub fn quota_for<'a>(profile: &ModelProfile, entries: &'a [AccountEntry]) -> Quota<'a> {
    let provider = match profile.provider {
        ModelProvider::Anthropic => Provider::Claude,
        ModelProvider::Openai => Provider::Codex,
    };
    entries
        .iter()
        .find(|entry| entry.account.provider == provider && entry.account.active)
        .map_or(Quota::Unknown, Quota::Account)
}
