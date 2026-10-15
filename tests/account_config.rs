use agent_meter::account::{parse_accounts, Provider};
use std::path::{Path, PathBuf};

const HOME: &str = "/home/u";

#[test]
fn parses_accounts_with_default_and_explicit_homes() {
    let toml = r#"
[accounts.claude-primary]
provider = "claude"

[accounts.claude-work]
provider = "claude"
home = "/data/claude-work"

[accounts.codex-main]
provider = "codex"
"#;
    let accounts = parse_accounts(toml, Path::new(HOME)).expect("valid config");

    assert_eq!(accounts.len(), 3);
    let primary = accounts.iter().find(|a| a.name == "claude-primary").unwrap();
    assert_eq!(primary.provider, Provider::Claude);
    assert_eq!(primary.profile_id(), "claude-primary");
    assert_eq!(primary.home, PathBuf::from("/home/u/.claude"));
    assert_eq!(primary.home_env(), ("CLAUDE_CONFIG_DIR", PathBuf::from("/home/u/.claude")));

    let work = accounts.iter().find(|a| a.name == "claude-work").unwrap();
    assert_eq!(work.home, PathBuf::from("/data/claude-work"));

    let codex = accounts.iter().find(|a| a.name == "codex-main").unwrap();
    assert_eq!(codex.provider, Provider::Codex);
    assert_eq!(codex.home_env(), ("CODEX_HOME", PathBuf::from("/home/u/.codex")));
}

#[test]
fn rejects_bad_providers_blank_and_control_values_and_credentials() {
    let cases = [
        ("unknown provider", "[accounts.a]\nprovider = \"gemini\"\n"),
        ("missing provider", "[accounts.a]\nhome = \"/x\"\n"),
        ("blank home", "[accounts.a]\nprovider = \"claude\"\nhome = \"  \"\n"),
        ("control char home", "[accounts.a]\nprovider = \"claude\"\nhome = \"/x\\u0007y\"\n"),
        ("blank name", "[accounts.\" \"]\nprovider = \"claude\"\n"),
        ("credential field", "[accounts.a]\nprovider = \"claude\"\ntoken = \"abc\"\n"),
        ("not toml", "[[[ nope"),
    ];
    for (name, toml) in cases {
        assert!(parse_accounts(toml, Path::new(HOME)).is_err(), "{name} should be rejected");
    }
}

#[test]
fn errors_never_echo_config_values() {
    let toml = "[accounts.a]\nprovider = \"claude\"\ntoken = \"sk-supersecret\"\n";
    let err = parse_accounts(toml, Path::new(HOME)).unwrap_err();
    assert!(!err.to_string().contains("supersecret"), "{err}");
}

#[test]
fn empty_config_has_no_accounts() {
    assert!(parse_accounts("", Path::new(HOME)).unwrap().is_empty());
}
