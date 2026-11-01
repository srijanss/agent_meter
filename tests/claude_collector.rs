use std::cell::RefCell;
use std::path::PathBuf;

use agent_meter::account::{Account, Provider};
use agent_meter::claude_collector::collect_claude;
use agent_meter::failure::CollectorError;
use agent_meter::runner::{CommandRunner, CommandSpec, Limits, Output};
use agent_meter::snapshot::{Snapshot, Source};
use chrono::{TimeZone, Utc};

const USAGE_TEXT: &str = "You are currently using your subscription\\n\\nCurrent session: 22% used · resets Nov 2, 12:40am (Australia/Sydney)\\nCurrent week (all models): 17% used · resets Nov 6, 10pm (Australia/Sydney)\\n\\nWhat's contributing to your usage?\\n";

struct Fake {
    auth: Result<Output, CollectorError>,
    usage: Result<Output, CollectorError>,
    calls: RefCell<Vec<CommandSpec>>,
}

impl CommandRunner for Fake {
    fn run(&self, spec: &CommandSpec, _limits: &Limits) -> Result<Output, CollectorError> {
        self.calls.borrow_mut().push(spec.clone());
        if spec.args().first().map(String::as_str) == Some("auth") {
            self.auth.clone()
        } else {
            self.usage.clone()
        }
    }
}

fn ok(stdout: &str) -> Result<Output, CollectorError> {
    Ok(Output { stdout: stdout.to_string(), success: true })
}

fn logged_in() -> Result<Output, CollectorError> {
    ok(r#"{"loggedIn":true,"authMethod":"claude.ai","email":"me@example.com","orgId":"0b0c1d2e-0000-4000-8000-000000000000","orgName":"Acme","subscriptionType":"pro"}"#)
}

fn usage_ok() -> Result<Output, CollectorError> {
    ok(&format!(r#"{{"type":"result","subtype":"success","is_error":false,"result":"{USAGE_TEXT}"}}"#))
}

fn account() -> Account {
    Account { name: "Work".into(), provider: Provider::Claude, home: PathBuf::from("/cfg/work"), active: false }
}

fn run(
    auth: Result<Output, CollectorError>,
    usage: Result<Output, CollectorError>,
) -> (Result<Snapshot, CollectorError>, Vec<CommandSpec>) {
    let fake = Fake { auth, usage, calls: RefCell::new(Vec::new()) };
    let now = Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();
    let result = collect_claude(&fake, &account(), now);
    (result, fake.calls.into_inner())
}

#[test]
fn a_signed_in_account_yields_a_provider_reported_snapshot() {
    let (result, calls) = run(logged_in(), usage_ok());
    let snapshot = result.unwrap();
    assert_eq!(snapshot.plan_label.as_deref(), Some("Pro"));
    assert_eq!(snapshot.profile_id, "Work");
    assert_eq!(snapshot.source, Source::ProviderReported);
    assert_eq!(snapshot.windows.len(), 2);
    assert_eq!(snapshot.windows[0].used_percent, Some(22.0));
    assert_eq!(snapshot.windows[1].used_percent, Some(17.0));

    // Auth gates collection; both calls point the CLI at this account.
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].args(), ["auth", "status", "--json"]);
    assert_eq!(calls[1].args(), ["-p", "/usage", "--output-format", "json"]);
    for call in &calls {
        assert_eq!(call.program(), "claude");
        assert_eq!(call.environment(), [("CLAUDE_CONFIG_DIR".to_string(), "/cfg/work".to_string())]);
    }
    // Identity fields are discarded.
    let dump = format!("{snapshot:?}");
    for secret in ["me@example.com", "Acme", "0b0c1d2e"] {
        assert!(!dump.contains(secret), "{secret} leaked");
    }
}

#[test]
fn signed_out_stops_before_usage_is_asked() {
    let (result, calls) = run(ok(r#"{"loggedIn":false}"#), usage_ok());
    assert_eq!(result, Err(CollectorError::SignInRequired));
    assert_eq!(calls.len(), 1, "no usage call for a signed-out account");

    // A non-zero exit that still says it is signed out.
    let failed = Ok(Output { stdout: r#"{"loggedIn":false}"#.into(), success: false });
    assert_eq!(run(failed, usage_ok()).0, Err(CollectorError::SignInRequired));
}

#[test]
fn a_failed_process_is_not_trusted_even_with_plausible_output() {
    let failed = |out: Result<Output, CollectorError>| {
        let output = out.unwrap();
        Ok(Output { success: false, ..output })
    };
    // Auth exited non-zero but printed loggedIn:true: no usage call is made.
    let (result, calls) = run(failed(logged_in()), usage_ok());
    assert_eq!(result, Err(CollectorError::Unavailable));
    assert_eq!(calls.len(), 1);
    // Usage exited non-zero but printed a result: not accepted as a snapshot.
    assert_eq!(run(logged_in(), failed(usage_ok())).0, Err(CollectorError::Unavailable));
}

#[test]
fn a_non_claude_account_never_launches_the_cli() {
    let fake = Fake { auth: logged_in(), usage: usage_ok(), calls: RefCell::new(Vec::new()) };
    let codex = Account { provider: Provider::Codex, ..account() };
    let now = Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();
    assert_eq!(collect_claude(&fake, &codex, now), Err(CollectorError::Internal));
    assert!(fake.calls.borrow().is_empty());
}

#[cfg(unix)]
#[test]
fn a_non_utf8_config_dir_is_refused_not_rewritten() {
    use std::os::unix::ffi::OsStringExt;
    let home = PathBuf::from(std::ffi::OsString::from_vec(b"/cfg/\xff".to_vec()));
    let odd = Account { home, ..account() };
    let fake = Fake { auth: logged_in(), usage: usage_ok(), calls: RefCell::new(Vec::new()) };
    let now = Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();
    assert_eq!(collect_claude(&fake, &odd, now), Err(CollectorError::Internal));
    assert!(fake.calls.borrow().is_empty(), "the CLI must not run against a different directory");
}

#[test]
fn failures_map_to_plain_errors() {
    assert_eq!(run(Err(CollectorError::Unavailable), usage_ok()).0, Err(CollectorError::Unavailable));
    assert_eq!(run(ok("not json"), usage_ok()).0, Err(CollectorError::Malformed));
    assert_eq!(run(ok(r#"{"loggedIn":"yes"}"#), usage_ok()).0, Err(CollectorError::Malformed));
    assert_eq!(run(logged_in(), Err(CollectorError::Unavailable)).0, Err(CollectorError::Unavailable));
    assert_eq!(run(logged_in(), ok("garbage")).0, Err(CollectorError::Malformed));
    assert_eq!(run(logged_in(), ok(r#"{"is_error":false}"#)).0, Err(CollectorError::Malformed));
    assert_eq!(
        run(logged_in(), ok(r#"{"is_error":true,"result":"boom"}"#)).0,
        Err(CollectorError::Unavailable)
    );
}
