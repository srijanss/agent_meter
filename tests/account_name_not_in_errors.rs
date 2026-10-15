use agent_meter::account::parse_accounts;
use std::path::Path;

#[test]
fn invalid_home_error_does_not_echo_the_account_name() {
    let toml = "[accounts.\"sk-sentinel-secret\"]\nprovider = \"claude\"\nhome = \"  \"\n";
    let err = parse_accounts(toml, Path::new("/home/u")).unwrap_err();

    assert!(!err.to_string().contains("sentinel"), "{err}");
    assert!(!format!("{err:?}").contains("sentinel"), "{err:?}");
}
