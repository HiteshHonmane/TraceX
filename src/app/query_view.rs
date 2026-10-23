//! Application-owned query generations and immutable view snapshots.
use super::TraceXApp;
use super::query_worker::{QueryPoll, QueryRequest, QueryResult, QueryWorker};
use crate::domain::{log::SharedLog, query::LogQuery};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct QueryView {
    worker: Option<QueryWorker>,
    key: Option<LogQuery>,
    generation: u64,
    requested_len: usize,
    pending: bool,
    snapshot: Option<Arc<QueryResult>>,
    failure: Option<String>,
}

impl QueryView {
    /// A clear, archive handoff or history replacement invalidates index identity.
    pub(super) fn invalidate(&mut self) {
        self.key = None;
        self.snapshot = None;
    }

    fn next_generation(&mut self) -> bool {
        match self.generation.checked_add(1) {
            Some(next) => {
                self.generation = next;
                true
            }
            None => {
                self.failure = Some(
                    "Search identifiers exhausted. Save your session and restart TraceX.".into(),
                );
                self.pending = false;
                false
            }
        }
    }

    fn refresh(&mut self, records: &[SharedLog], query: LogQuery) {
        let changed = self.key.as_ref() != Some(&query) || records.len() < self.requested_len;
        if changed {
            self.snapshot = None;
            self.pending = false;
            self.failure = None;
            self.requested_len = 0;
            self.key = Some(query.clone());
            if !self.next_generation() {
                return;
            }
            // A new query is an explicit opportunity to retry a stopped worker.
            if self.worker.as_ref().is_some_and(QueryWorker::is_finished) {
                self.worker = None;
            }
        }
        // A failed/exhausted generation cannot accept a late result. A changed
        // query above explicitly clears retryable failures before reaching here.
        if self.failure.is_some() {
            return;
        }
        if let Some(worker) = &self.worker {
            match worker.poll() {
                QueryPoll::Ready(result)
                    if result.generation == self.generation
                        && result.record_count <= records.len() =>
                {
                    self.snapshot = Some(Arc::new(result));
                    self.pending = false;
                }
                QueryPoll::Stopped(reason) => {
                    self.pending = false;
                    self.failure = Some(format!(
                        "Background search stopped: {reason:?}. Change the query to retry; captured logs are retained."
                    ));
                }
                _ => {}
            }
        }
        if self.failure.is_some()
            || self.pending
            || self
                .snapshot
                .as_ref()
                .is_some_and(|view| view.record_count == records.len())
        {
            return;
        }
        if records.is_empty() && self.worker.is_none() {
            self.snapshot = Some(Arc::new(QueryResult {
                generation: self.generation,
                rows: Vec::new(),
                excluded: Vec::new(),
                main_width: 0.0,
                excluded_width: 0.0,
                record_count: 0,
                query_empty: query.is_empty(),
            }));
            return;
        }
        if self.worker.is_none() {
            match QueryWorker::spawn() {
                Ok(worker) => self.worker = Some(worker),
                Err(cause) => {
                    self.failure = Some(format!(
                        "Could not start background search: {cause}. Captured logs are retained."
                    ));
                    return;
                }
            }
        }
        if !self.next_generation() {
            return;
        }
        let request = QueryRequest {
            generation: self.generation,
            query,
            records: records.to_vec(),
        };
        if let Some(worker) = &self.worker {
            match worker.submit(request) {
                Ok(()) => {
                    self.pending = true;
                    self.requested_len = records.len();
                }
                Err(error) => {
                    self.failure = Some(format!(
                        "Could not submit background search: {:?}. Captured logs are retained.",
                        error.reason
                    ));
                    // The source history still owns these records; release only
                    // the rejected query's shared handles, never source logs.
                    drop(error.request);
                }
            }
        }
    }
}

impl TraceXApp {
    pub(super) fn refresh_query_view(&mut self) {
        if self.session_archive_busy() {
            self.query_view.invalidate();
            return;
        }
        let query = self.log_query();
        self.query_view.refresh(&self.logs, query);
    }

    pub(super) fn query_snapshot(&self) -> Option<Arc<QueryResult>> {
        self.query_view.snapshot.clone()
    }

    pub(super) fn query_pending(&self) -> bool {
        self.query_view.pending
    }

    pub(super) fn query_failure(&self) -> Option<&str> {
        self.query_view.failure.as_deref()
    }

    pub(super) fn copy_query_rows(&self, ctx: &eframe::egui::Context, indices: &[usize]) {
        ctx.copy_text(super::log_format::format_log_entries(
            indices
                .iter()
                .filter_map(|&index| self.logs.get(index).map(AsRef::as_ref)),
        ));
    }
}

#[cfg(test)]
#[path = "query_view_tests.rs"]
mod tests;
