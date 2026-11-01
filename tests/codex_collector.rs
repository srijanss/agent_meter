use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use agent_meter::account::{Account, Provider};
use agent_meter::codex_collector::collect_codex;
use agent_meter::failure::CollectorError;
use agent_meter::runner::CommandSpec;
use agent_meter::session::{LineSession, SessionStarter};
use agent_meter::snapshot::Source;
use chrono::{TimeZone, Utc};

#[derive(Default)]
struct State {
    started: Vec<CommandSpec>,
    sent: Vec<String>,
    replies: VecDeque<Result<String, CollectorError>>,
}

struct Script(Rc<RefCell<State>>);
struct Session(Rc<RefCell<State>>);

impl SessionStarter for Script {
    fn start(&self, spec: &CommandSpec) -> Result<Box<dyn LineSession>, CollectorError> {
        self.0.borrow_mut().started.push(spec.clone());
        Ok(Box::new(Session(self.0.clone())))
    }
}

impl LineSession for Session {
    fn send(&mut self, line: &str) -> Result<(), CollectorError> {
        self.0.borrow_mut().sent.push(line.to_string());
        Ok(())
    }
    fn recv(&mut self, _timeout: Duration) -> Result<String, CollectorError> {
        self.0.borrow_mut().replies.pop_front().unwrap_or(Err(CollectorError::Unavailable))
    }
}

fn line(text: &str) -> Result<String, CollectorError> {
    Ok(text.to_string())
}

const INIT_OK: &str = r#"{"id":1,"result":{"userAgent":"x","codexHome":"/cfg/me","platformFamily":"unix","platformOs":"linux"}}"#;
const LIMITS_OK: &str = r#"{"id":2,"result":{"rateLimits":{"primary":{"usedPercent":22,"windowDurationMins":300,"resetsAt":1793000000},"secondary":{"usedPercent":17,"windowDurationMins":10080,"resetsAt":1793500000},"planType":"plus"},"accountId":"acct-secret-1"}}"#;

fn account() -> Account {
    Account { name: "Personal".into(), provider: Provider::Codex, home: PathBuf::from("/cfg/personal"), active: false }
}

fn collect(
    replies: Vec<Result<String, CollectorError>>,
    account: &Account,
) -> (Result<agent_meter::snapshot::Snapshot, CollectorError>, Rc<RefCell<State>>) {
    let state = Rc::new(RefCell::new(State { replies: replies.into(), ..State::default() }));
    let now = Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();
    let result = collect_codex(&Script(state.clone()), account, now);
    (result, state)
}

#[test]
fn a_signed_in_account_yields_a_provider_reported_snapshot() {
    // Notifications between replies are skipped.
    let replies = vec![
        line(r#"{"method":"configWarning","params":{}}"#),
        line(INIT_OK),
        line(r#"{"method":"account/updated","params":{}}"#),
        line(LIMITS_OK),
    ];
    let (result, state) = collect(replies, &account());
    let snapshot = result.unwrap();
    assert_eq!(snapshot.provider, "codex");
    assert_eq!(snapshot.profile_id, "Personal");
    assert_eq!(snapshot.plan_label.as_deref(), Some("Plus"));
    assert_eq!(snapshot.source, Source::ProviderReported);
    assert_eq!(snapshot.windows.len(), 2);
    assert!(!format!("{snapshot:?}").contains("acct-secret-1"), "account id leaked");

    let state = state.borrow();
    assert_eq!(state.started.len(), 1);
    let spec = &state.started[0];
    assert_eq!(spec.program(), "codex");
    assert_eq!(spec.args(), ["-s", "read-only", "-a", "on-request", "app-server"]);
    assert_eq!(spec.environment(), [("CODEX_HOME".to_string(), "/cfg/personal".to_string())]);

    // initialize, then initialized, then the read: in that order, as JSON lines.
    let sent: Vec<serde_json::Value> = state.sent.iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(sent.len(), 3);
    assert_eq!((sent[0]["id"].as_i64(), sent[0]["method"].as_str()), (Some(1), Some("initialize")));
    assert_eq!((sent[1].get("id"), sent[1]["method"].as_str()), (None, Some("initialized")));
    assert_eq!((sent[2]["id"].as_i64(), sent[2]["method"].as_str()), (Some(2), Some("account/rateLimits/read")));
}

#[test]
fn an_auth_error_is_sign_in_required_and_other_errors_are_unavailable() {
    let auth = r#"{"id":2,"error":{"code":-32600,"message":"codex account authentication required to read rate limits"}}"#;
    assert_eq!(collect(vec![line(INIT_OK), line(auth)], &account()).0, Err(CollectorError::SignInRequired));

    let other = r#"{"id":2,"error":{"code":-32603,"message":"backend exploded"}}"#;
    assert_eq!(collect(vec![line(INIT_OK), line(other)], &account()).0, Err(CollectorError::Unavailable));

    let init_err = r#"{"id":1,"error":{"code":-32600,"message":"nope"}}"#;
    let (result, state) = collect(vec![line(init_err)], &account());
    assert_eq!(result, Err(CollectorError::Unavailable));
    assert_eq!(state.borrow().sent.len(), 1, "no read after a failed initialize");
}

#[test]
fn an_exited_or_unreadable_server_is_unavailable_or_malformed() {
    // The server exited (or timed out) mid-conversation.
    assert_eq!(collect(vec![line(INIT_OK)], &account()).0, Err(CollectorError::Unavailable));
    assert_eq!(collect(vec![], &account()).0, Err(CollectorError::Unavailable));
    // A result that is not an object is malformed, never a guess.
    let bad = r#"{"id":2,"result":"nope"}"#;
    assert_eq!(collect(vec![line(INIT_OK), line(bad)], &account()).0, Err(CollectorError::Malformed));
    // A flood of unrelated lines does not wait forever.
    let noise: Vec<_> = (0..500).map(|_| line("not json")).collect();
    assert_eq!(collect(noise, &account()).0, Err(CollectorError::Unavailable));
}

#[test]
fn only_a_usable_codex_account_starts_a_process() {
    let claude = Account { provider: Provider::Claude, ..account() };
    let (result, state) = collect(vec![line(INIT_OK), line(LIMITS_OK)], &claude);
    assert_eq!(result, Err(CollectorError::Internal));
    assert!(state.borrow().started.is_empty());

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let home = PathBuf::from(std::ffi::OsString::from_vec(b"/cfg/\xff".to_vec()));
        let odd = Account { home, ..account() };
        let (result, state) = collect(vec![line(INIT_OK), line(LIMITS_OK)], &odd);
        assert_eq!(result, Err(CollectorError::Internal));
        assert!(state.borrow().started.is_empty());
    }
}
