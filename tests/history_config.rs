use std::path::{Path, PathBuf};

use agent_meter::history::{parse_history_config, DEFAULT_MAX_RECORDS};

const HOME: &str = "/home/u";

fn parse(text: &str) -> Result<Option<agent_meter::history::HistoryConfig>, agent_meter::account::ConfigError> {
    parse_history_config(text, Path::new(HOME))
}

#[test]
fn history_is_off_unless_the_history_table_is_present() {
    assert_eq!(parse("").unwrap(), None);
    assert_eq!(parse("refresh_interval_secs = 60\n[accounts.a]\nprovider = \"claude\"\n").unwrap(), None);
}

#[test]
fn a_history_table_sets_the_file_and_a_bounded_record_count() {
    let config = parse("[history]\npath = \"/var/lib/meter/h.jsonl\"\n").unwrap().unwrap();
    assert_eq!(config.path, PathBuf::from("/var/lib/meter/h.jsonl"));
    assert_eq!(config.max_records, DEFAULT_MAX_RECORDS);

    let config = parse("[history]\npath = \"~/h.jsonl\"\nmax_records = 50\n").unwrap().unwrap();
    assert_eq!(config.path, PathBuf::from("/home/u/h.jsonl"), "~/ is the user's home");
    assert_eq!(config.max_records, 50);
}

#[test]
fn a_bad_history_table_is_rejected_without_echoing_its_values() {
    for bad in [
        "[history]\n",
        "[history]\npath = \"\"\n",
        "[history]\npath = \"relative/h.jsonl\"\n",
        "[history]\npath = \"/a/\\u0007b\"\n",
        "[history]\npath = 5\n",
        "[history]\npath = \"/a/h\"\nmax_records = 0\n",
        "[history]\npath = \"/a/h\"\nmax_records = 1000001\n",
        "[history]\npath = \"/a/h\"\nmax_records = \"many\"\n",
        "[history]\npath = \"/a/h\"\ntoken = \"sk-secret-123\"\n",
        "history = \"/a/h\"\n",
    ] {
        let err = parse(bad).unwrap_err().to_string();
        assert!(err.contains("history"), "{bad}: {err}");
        assert!(!err.contains("sk-secret-123") && !err.contains("relative/h"), "{err}");
    }
}

#[test]
fn the_history_table_does_not_disturb_the_other_config_readers() {
    let text = "[history]\npath = \"/a/h\"\n[accounts.a]\nprovider = \"claude\"\n";
    assert_eq!(agent_meter::account::parse_accounts(text, Path::new(HOME)).unwrap().len(), 1);
    assert!(agent_meter::models::parse_model_profiles(text).unwrap().is_empty());
}
