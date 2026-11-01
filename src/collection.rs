//! Runs collections on worker threads so the popup's event loop never waits on
//! a collector process. The UI submits jobs with `spawn` and drains finished
//! ones with `poll`; neither blocks, however long a collection hangs.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::failure::{collect_all, AccountOutcome, Collect, CollectorError};
use crate::snapshot::Snapshot;

pub struct Collections {
    results: Sender<(String, AccountOutcome)>,
    finished: Receiver<(String, AccountOutcome)>,
}

impl Collections {
    pub fn new() -> Self {
        let (results, finished) = mpsc::channel();
        Self { results, finished }
    }

    /// Starts `job` for `account` on its own thread and returns at once. A panic
    /// in the job is reported as `CollectorError::Internal`; a thread that cannot
    /// be started reports `Unavailable`.
    pub fn spawn<F>(&self, account: String, job: F)
    where
        F: FnOnce() -> Result<Snapshot, CollectorError> + Send + 'static,
    {
        self.spawn_with(
            |work| thread::Builder::new().name("collection".into()).spawn(work).map(drop),
            account,
            job,
        );
    }

    /// Like `spawn`, but `start` decides how the work is run. If `start` fails the
    /// job is dropped unrun and the account reports exactly one `Unavailable`.
    pub fn spawn_with<S, F>(&self, start: S, account: String, job: F)
    where
        S: FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>,
        F: FnOnce() -> Result<Snapshot, CollectorError> + Send + 'static,
    {
        let results = self.results.clone();
        let name = account.clone();
        let work = Box::new(move || {
            let collect: Collect = Box::new(job);
            // `collect_all` isolates panics, including ones with hostile payloads.
            for outcome in collect_all(vec![(account, collect)]) {
                let _ = results.send(outcome);
            }
        });
        if start(work).is_err() {
            let _ = self.results.send((name, AccountOutcome::Failed(CollectorError::Unavailable)));
        }
    }

    /// Finished collections so far, without waiting for any that are still running.
    pub fn poll(&self) -> Vec<(String, AccountOutcome)> {
        self.finished.try_iter().collect()
    }
}

impl Default for Collections {
    fn default() -> Self {
        Self::new()
    }
}
