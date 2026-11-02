use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_meter::app::{JobFactory, AppError};
use agent_meter::failure::CollectorError;
use agent_meter::snapshot::parse_snapshot;
use agent_meter::usage_cli::{parse_command, usage_json, usage_report, Command};
use agent_meter::popup::AccountEntry;
use agent_meter::account::{Account, Provider};
use chrono::{TimeZone, Utc};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn snapshot_json(name: &str) -> String {
    format!(
        r#"{{"schema_version":1,"provider":"claude","profile_id":"{name}","plan_label":"Pro",
        "collected_at":"2026-10-14T00:00:00Z","source":"provider_reported","health":"ok",
        "windows":[{{"id":"s","label":"Session","used_percent":42.0,"resets_at":"2026-10-14T05:00:00Z",
        "source":"provider_reported"}}],"bonus_resets":null}}"#
    )
}

/// `work` reports a snapshot; every other account fails.
fn jobs() -> JobFactory {
    Box::new(|account| {
        let name = account.name.clone();
        Box::new(move || {
            if name == "work" {
                Ok(parse_snapshot(&snapshot_json(&name)).unwrap())
            } else {
                Err(CollectorError::Unavailable)
            }
        })
    })
}

fn config_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_meter_usage_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn usage_json_is_a_distinct_read_only_command_with_an_optional_config() {
    assert_eq!(parse_command(args(&[])).unwrap(), Command::Popup(None));
    assert_eq!(
        parse_command(args(&["--config", "/c.toml"])).unwrap(),
        Command::Popup(Some(PathBuf::from("/c.toml")))
    );
    assert_eq!(parse_command(args(&["usage", "--json"])).unwrap(), Command::UsageJson(None));
    assert_eq!(
        parse_command(args(&["usage", "--json", "--config", "/c.toml"])).unwrap(),
        Command::UsageJson(Some(PathBuf::from("/c.toml")))
    );
    for bad in [&["usage"][..], &["usage", "--config", "/c"], &["usage", "--json", "x"], &["usage", "--json", "--config"], &["--json"]] {
        let err: AppError = parse_command(args(bad)).unwrap_err();
        assert!(err.to_string().contains("usage"), "{bad:?}");
    }
}

#[test]
fn usage_report_lists_every_account_with_its_snapshot_freshness_and_no_secrets() {
    let dir = config_dir("report");
    let path = dir.join("config.toml");
    fs::write(
        &path,
        "[accounts.work]\nprovider = \"claude\"\nhome = \"/secret/work-home\"\nactive = true\n\
         [accounts.broken]\nprovider = \"codex\"\nhome = \"/secret/broken-home\"\n",
    )
    .unwrap();

    let text = usage_report(&path, Path::new("/home/u"), jobs(), Duration::from_secs(30)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();

    assert_eq!(value["schema_version"], 1);
    assert!(value["generated_at"].as_str().unwrap().ends_with('Z'));
    let accounts = value["accounts"].as_array().unwrap();
    assert_eq!(accounts.len(), 2);

    let work = accounts.iter().find(|a| a["name"] == "work").unwrap();
    assert_eq!(work["provider"], "claude");
    assert_eq!(work["active"], true);
    assert_eq!(work["snapshot"]["schema_version"], 1);
    assert_eq!(work["snapshot"]["source"], "provider_reported");
    assert_eq!(work["snapshot"]["collected_at"], "2026-10-14T00:00:00Z");
    assert_eq!(work["snapshot"]["windows"][0]["used_percent"], 42.0);
    assert!(work["age_seconds"].is_i64(), "freshness is reported");

    let broken = accounts.iter().find(|a| a["name"] == "broken").unwrap();
    assert!(broken["snapshot"].is_null());
    assert!(broken["age_seconds"].is_null());

    // No config paths or other account internals leak into the output.
    assert!(!text.contains("/secret"), "{text}");
    assert!(!text.contains("home"), "{text}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn freshness_is_the_whole_seconds_since_collection_and_never_negative() {
    let entry = AccountEntry {
        account: Account {
            name: "work".into(),
            provider: Provider::Claude,
            home: PathBuf::from("/x"),
            active: false,
        },
        snapshot: Some(parse_snapshot(&snapshot_json("work")).unwrap()),
    };
    let age = |now| {
        let text = usage_json(std::slice::from_ref(&entry), now);
        serde_json::from_str::<serde_json::Value>(&text).unwrap()["accounts"][0]["age_seconds"].clone()
    };
    // Collected at 2026-10-14T00:00:00Z.
    assert_eq!(age(Utc.with_ymd_and_hms(2026, 10, 14, 0, 1, 30).unwrap()), 90);
    assert_eq!(age(Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()), 0);
    assert_eq!(age(Utc.with_ymd_and_hms(2026, 10, 13, 23, 0, 0).unwrap()), 0, "clock skew clamps to zero");
}
