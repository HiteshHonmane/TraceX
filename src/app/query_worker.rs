//! Latest-request query and width computation; no GUI or transport dependencies.
use crate::domain::log::LogEntry;
use crate::domain::query::{LogQuery, QueryScope};
use std::fmt;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

/// A stable history snapshot. Record text stays shared with the application.
pub(super) struct QueryRequest {
    pub generation: u64,
    pub query: LogQuery,
    pub records: Vec<Arc<LogEntry>>,
}
impl fmt::Debug for QueryRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryRequest")
            .field("generation", &self.generation)
            .field("query_empty", &self.query.is_empty())
            .field("record_count", &self.records.len())
            .finish()
    }
}

/// Indices address the submitted snapshot; only matching generations are usable.
#[derive(Debug)]
pub(super) struct QueryResult {
    pub generation: u64,
    pub rows: Vec<usize>,
    pub excluded: Vec<usize>,
    pub main_width: f32,
    pub excluded_width: f32,
    pub record_count: usize,
    pub query_empty: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueryStopReason {
    Requested,
    WorkerEndedUnexpectedly,
    StatePoisoned,
    RevisionExhausted,
}

#[derive(Debug)]
pub(super) struct SubmitError {
    pub request: QueryRequest,
    pub reason: QueryStopReason,
}

#[derive(Debug)]
pub(super) enum QueryPoll {
    Pending,
    Ready(QueryResult),
    Stopped(QueryStopReason),
}

#[derive(Default)]
struct QueryState {
    pending: Option<QueryRequest>,
    result: Option<QueryResult>,
    stop_requested: bool,
    stopped: Option<QueryStopReason>,
}
struct Shared {
    state: Mutex<QueryState>,
    wake: Condvar,
    revision: AtomicU64,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, QueryState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.stop_requested = true;
                state.stopped = Some(QueryStopReason::StatePoisoned);
                self.revision.store(0, Ordering::Release);
                self.wake.notify_one();
                state
            }
        }
    }
}

/// Reusable worker with one pending request and one replaceable result slot,
/// in addition to the currently executing request. Submission and polling never
/// wait for query work. Replaced snapshots are released outside the state lock.
/// Appends do not cancel work unless the caller explicitly submits a replacement.
pub(super) struct QueryWorker {
    shared: Arc<Shared>,
    thread: JoinHandle<()>,
}
impl QueryWorker {
    pub fn spawn() -> io::Result<Self> {
        Self::spawn_with_hook(|_| {})
    }

    fn spawn_with_hook(mut before_compute: impl FnMut(u64) + Send + 'static) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(QueryState::default()),
            wake: Condvar::new(),
            revision: AtomicU64::new(0),
        });
        let worker_shared = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("tracex-query".to_owned())
            .spawn(move || {
                run_worker(&worker_shared, &mut before_compute);
            })?;
        Ok(Self { shared, thread })
    }

    /// Replaces an obsolete pending request and result. Rejected requests remain
    /// caller-owned. Snapshot replacement may release Arc references on the caller,
    /// but it never drops them while holding the shared-state lock.
    pub fn submit(&self, request: QueryRequest) -> Result<(), SubmitError> {
        let mut state = self.shared.lock();
        if state.stop_requested || state.stopped.is_some() || self.thread.is_finished() {
            let reason = state.stopped.unwrap_or(if state.stop_requested {
                QueryStopReason::Requested
            } else {
                QueryStopReason::WorkerEndedUnexpectedly
            });
            return Err(SubmitError { request, reason });
        }
        let Some(revision) = self.shared.revision.load(Ordering::Acquire).checked_add(1) else {
            state.stop_requested = true;
            state.stopped = Some(QueryStopReason::RevisionExhausted);
            self.shared.revision.store(0, Ordering::Release);
            drop(state);
            self.shared.wake.notify_one();
            return Err(SubmitError {
                request,
                reason: QueryStopReason::RevisionExhausted,
            });
        };
        let previous_request = state.pending.replace(request);
        let previous_result = state.result.take();
        self.shared.revision.store(revision, Ordering::Release);
        drop(state);
        self.shared.wake.notify_one();
        drop(previous_request);
        drop(previous_result);
        Ok(())
    }

    /// Terminal reasons persist across polls, including unexpected thread exit.
    pub fn poll(&self) -> QueryPoll {
        let mut state = self.shared.lock();
        if let Some(reason) = state.stopped {
            return QueryPoll::Stopped(reason);
        }
        // Cancellation prevents any later publication immediately. Actual thread
        // release is checked separately through is_finished(), never by joining.
        if state.stop_requested {
            return QueryPoll::Stopped(QueryStopReason::Requested);
        }
        if self.thread.is_finished() {
            state.stop_requested = true;
            state.stopped = Some(QueryStopReason::WorkerEndedUnexpectedly);
            return QueryPoll::Stopped(QueryStopReason::WorkerEndedUnexpectedly);
        }
        if let Some(result) = state.result.take() {
            return QueryPoll::Ready(result);
        }
        QueryPoll::Pending
    }

    /// Nonblocking cancellation. Poll until Stopped/is_finished; never join on UI.
    pub fn stop(&self) {
        let mut state = self.shared.lock();
        state.stop_requested = true;
        self.shared.revision.store(0, Ordering::Release);
        let pending = state.pending.take();
        let result = state.result.take();
        drop(state);
        self.shared.wake.notify_one();
        drop(pending);
        drop(result);
    }

    pub fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }

    #[cfg(test)]
    pub(super) fn result_waiting(&self) -> bool {
        self.shared.lock().result.is_some()
    }
}
impl Drop for QueryWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_worker(shared: &Shared, before_compute: &mut impl FnMut(u64)) {
    loop {
        let mut state = shared.lock();
        while state.pending.is_none() && !state.stop_requested {
            state = match shared.wake.wait(state) {
                Ok(state) => state,
                Err(poisoned) => {
                    let mut state = poisoned.into_inner();
                    state.stop_requested = true;
                    state.stopped = Some(QueryStopReason::StatePoisoned);
                    shared.revision.store(0, Ordering::Release);
                    return;
                }
            };
        }
        if state.stop_requested {
            if state.stopped.is_none() {
                state.stopped = Some(QueryStopReason::Requested);
            }
            return;
        }
        let Some(request) = state.pending.take() else {
            continue;
        };
        let revision = shared.revision.load(Ordering::Acquire);
        drop(state);
        before_compute(request.generation);
        let result = compute(request, &shared.revision, revision);
        let mut state = shared.lock();
        let previous =
            if !state.stop_requested && shared.revision.load(Ordering::Acquire) == revision {
                result.and_then(|result| state.result.replace(result))
            } else {
                None
            };
        drop(state);
        drop(previous);
    }
}

fn compute(
    request: QueryRequest,
    current_revision: &AtomicU64,
    revision: u64,
) -> Option<QueryResult> {
    let mut result = QueryResult {
        generation: request.generation,
        rows: Vec::new(),
        excluded: Vec::new(),
        main_width: 0.0,
        excluded_width: 0.0,
        record_count: request.records.len(),
        query_empty: request.query.is_empty(),
    };
    for (index, entry) in request.records.iter().enumerate() {
        if current_revision.load(Ordering::Acquire) != revision {
            return None;
        }
        let scope = if result.query_empty {
            QueryScope::Main
        } else {
            QueryScope::Include
        };
        if request.query.matches(entry, scope) {
            result.rows.push(index);
            result.main_width = result
                .main_width
                .max(205.0 + entry.message.chars().count() as f32 * 7.2);
        } else if !result.query_empty && request.query.matches(entry, QueryScope::Exclude) {
            result.excluded.push(index);
            result.excluded_width = result
                .excluded_width
                .max(205.0 + entry.message.chars().count() as f32 * 7.2);
        }
    }
    if current_revision.load(Ordering::Acquire) != revision {
        return None;
    }
    Some(result)
}

#[cfg(test)]
#[path = "query_worker_tests.rs"]
mod tests;
