use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::models::{parse_model_profiles, quota_for, ModelProfile, Quota};
use agent_meter::popup::AccountEntry;

fn entry(name: &str, provider: Provider, active: bool) -> AccountEntry {
    AccountEntry {
        account: Account { name: name.into(), provider, home: PathBuf::from("/x"), active },
        snapshot: None,
    }
}

fn profile(harness: &str, provider: &str) -> ModelProfile {
    parse_model_profiles(&format!(
        "[models.p]\nharness = \"{harness}\"\nmodel = \"m\"\nprovider = \"{provider}\"\n"
    ))
    .unwrap()
    .remove(0)
}

fn source_name<'a>(quota: &Quota<'a>) -> Option<&'a str> {
    match quota {
        Quota::Account(entry) => Some(entry.account.name.as_str()),
        Quota::Unknown => None,
    }
}

#[test]
fn a_profiles_quota_is_its_providers_active_account_whatever_the_harness() {
    let entries = [
        entry("claude-old", Provider::Claude, false),
        entry("claude-main", Provider::Claude, true),
        entry("codex-old", Provider::Codex, false),
        entry("codex-main", Provider::Codex, true),
    ];
    for harness in ["claude", "pi"] {
        assert_eq!(source_name(&quota_for(&profile(harness, "anthropic"), &entries)), Some("claude-main"));
        assert_eq!(source_name(&quota_for(&profile(harness, "openai"), &entries)), Some("codex-main"));
    }
}

#[test]
fn without_an_active_account_the_quota_is_unknown_and_says_so() {
    let none_active = [entry("c", Provider::Claude, false), entry("x", Provider::Codex, false)];
    let only_codex = [entry("x", Provider::Codex, true)];
    for entries in [&none_active[..], &only_codex[..], &[]] {
        let quota = quota_for(&profile("claude", "anthropic"), entries);
        assert!(matches!(quota, Quota::Unknown));
        assert_eq!(quota.label(), "Quota unknown");
    }
    let known = [entry("c", Provider::Claude, true)];
    assert_eq!(quota_for(&profile("pi", "anthropic"), &known).label(), "c");
}

#[test]
fn two_profiles_on_one_provider_share_one_account_not_two() {
    let entries = [entry("c", Provider::Claude, true)];
    let (a, b) = (profile("claude", "anthropic"), profile("pi", "anthropic"));
    let (qa, qb) = (quota_for(&a, &entries), quota_for(&b, &entries));
    match (qa, qb) {
        (Quota::Account(x), Quota::Account(y)) => assert!(std::ptr::eq(x, y)),
        _ => panic!("both resolve to the same account entry"),
    }
}
