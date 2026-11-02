use std::fs;
use std::time::Duration;

use agent_meter::account::parse_refresh_interval;
use agent_meter::app::load_refresh_interval;

#[test]
fn the_interval_defaults_to_five_minutes_and_is_set_by_refresh_interval_secs() {
    assert_eq!(parse_refresh_interval("").unwrap(), Duration::from_secs(300));
    let accounts = "[accounts.a]\nprovider = \"claude\"\n";
    assert_eq!(parse_refresh_interval(accounts).unwrap(), Duration::from_secs(300));
    let text = format!("refresh_interval_secs = 120\n{accounts}");
    assert_eq!(parse_refresh_interval(&text).unwrap(), Duration::from_secs(120));
    // The key does not disturb account parsing.
    assert_eq!(agent_meter::account::parse_accounts(&text, std::path::Path::new("/home/u")).unwrap().len(), 1);
}

#[test]
fn an_interval_that_is_too_short_too_long_or_not_a_number_is_rejected() {
    for bad in ["0", "14", "86401", "-5", "\"soon\"", "1.5"] {
        let err = parse_refresh_interval(&format!("refresh_interval_secs = {bad}\n")).unwrap_err();
        assert!(err.to_string().contains("refresh_interval_secs"), "{bad}: {err}");
    }
    assert_eq!(parse_refresh_interval("refresh_interval_secs = 15").unwrap(), Duration::from_secs(15));
    assert_eq!(parse_refresh_interval("refresh_interval_secs = 86400").unwrap(), Duration::from_secs(86400));
}

#[test]
fn the_app_loads_the_interval_from_the_config_file() {
    let dir = std::env::temp_dir().join(format!("agent_meter_interval_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.toml");
    fs::write(&path, "refresh_interval_secs = 90\n").unwrap();
    assert_eq!(load_refresh_interval(&path).unwrap(), Duration::from_secs(90));
    fs::write(&path, "refresh_interval_secs = 3\n").unwrap();
    assert!(load_refresh_interval(&path).is_err());
    assert!(load_refresh_interval(&dir.join("missing.toml")).is_err());
    let _ = fs::remove_dir_all(&dir);
}
