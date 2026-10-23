use super::*;
use crate::domain::log::LogLevel;
use crate::domain::query::SeveritySelection;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn query(text: &str) -> LogQuery {
    LogQuery::new(
        text,
        SeveritySelection {
            info: true,
            warn: false,
            error: true,
        },
    )
}
fn entry(id: usize, raw: &str, message: &str, level: LogLevel) -> Arc<LogEntry> {
    let mut entry = LogEntry::new(id, raw.to_owned(), "12:00:00.000");
    entry.message = message.to_owned();
    entry.level = level;
    Arc::new(entry)
}
fn request(generation: u64, text: &str, records: Vec<Arc<LogEntry>>) -> QueryRequest {
    QueryRequest {
        generation,
        query: query(text),
        records,
    }
}
fn ready(worker: &QueryWorker) -> QueryResult {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match worker.poll() {
            QueryPoll::Ready(result) => return result,
            QueryPoll::Stopped(reason) => panic!("worker stopped before result: {reason:?}"),
            QueryPoll::Pending => {}
        }
        assert!(
            Instant::now() < deadline,
            "query worker did not return a result"
        );
        thread::yield_now();
    }
}
fn stopped(worker: &QueryWorker, expected: QueryStopReason) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let QueryPoll::Stopped(reason) = worker.poll() {
            assert_eq!(reason, expected);
            if worker.is_finished() {
                return;
            }
        }
        assert!(Instant::now() < deadline, "query worker did not finish");
        thread::yield_now();
    }
}

#[test]
fn shared_domain_matching_preserves_severity_raw_message_and_unicode_widths() {
    let worker = QueryWorker::spawn().unwrap();
    let records = vec![
        entry(10, "RAW-NEEDLE", "body", LogLevel::Info),
        entry(20, "hidden", "needle", LogLevel::Warn),
        entry(30, "unrelated", "excluded 🦀", LogLevel::Error),
        entry(40, "unrelated", "NEEDLE", LogLevel::Debug),
    ];
    worker.submit(request(7, "  nEeDlE \t", records)).unwrap();
    let result = ready(&worker);
    assert_eq!(result.generation, 7);
    assert_eq!(result.rows, [0, 3]);
    assert_eq!(result.excluded, [2]);
    assert_eq!(result.record_count, 4);
    assert!(!result.query_empty);
    assert_eq!(result.main_width, 205.0 + 6.0 * 7.2);
    assert_eq!(
        result.excluded_width,
        205.0 + "excluded 🦀".chars().count() as f32 * 7.2
    );
    worker.stop();
    stopped(&worker, QueryStopReason::Requested);
}

#[test]
fn empty_query_keeps_main_severity_rows_and_marks_secondary_hidden() {
    let worker = QueryWorker::spawn().unwrap();
    worker
        .submit(request(
            1,
            " \n",
            vec![
                entry(1, "", "", LogLevel::Log),
                entry(2, "warn", "warn", LogLevel::Warn),
            ],
        ))
        .unwrap();
    let result = ready(&worker);
    assert_eq!(result.rows, [0]);
    assert!(result.excluded.is_empty());
    assert!(result.query_empty);
    assert_eq!(result.main_width, 205.0);
    assert_eq!(result.excluded_width, 0.0);
    worker.submit(request(2, "needle", Vec::new())).unwrap();
    let empty = ready(&worker);
    assert!(empty.rows.is_empty());
    assert!(empty.excluded.is_empty());
    assert_eq!(empty.main_width, 0.0);
    assert_eq!(empty.excluded_width, 0.0);
    worker.stop();
    stopped(&worker, QueryStopReason::Requested);
}

#[test]
fn bounded_pending_supersession_cancels_obsolete_work_and_releases_snapshot() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = QueryWorker::spawn_with_hook(move |generation| {
        if generation == 1 {
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        }
    })
    .unwrap();
    let shared_entry = entry(1, "needle", "needle", LogLevel::Info);
    worker
        .submit(request(1, "needle", vec![Arc::clone(&shared_entry)]))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    worker
        .submit(request(2, "needle", vec![Arc::clone(&shared_entry)]))
        .unwrap();
    worker
        .submit(request(3, "unmatched", vec![Arc::clone(&shared_entry)]))
        .unwrap();
    assert_eq!(
        Arc::strong_count(&shared_entry),
        3,
        "only caller, executing and latest pending snapshots remain"
    );
    {
        let state = worker.shared.lock();
        assert_eq!(state.pending.as_ref().unwrap().generation, 3);
        assert!(state.result.is_none());
    }
    release_tx.send(()).unwrap();
    let result = ready(&worker);
    assert_eq!(result.generation, 3);
    assert!(result.rows.is_empty());
    assert_eq!(result.excluded, [0]);
    assert_eq!(Arc::strong_count(&shared_entry), 1);
    assert!(matches!(worker.poll(), QueryPoll::Pending));
    worker.stop();
    stopped(&worker, QueryStopReason::Requested);
}

#[test]
fn replacement_discards_old_result_and_keeps_same_generation_requests_distinct() {
    let worker = QueryWorker::spawn().unwrap();
    let records = vec![entry(1, "needle", "needle", LogLevel::Info)];
    worker
        .submit(request(4, "needle", records.clone()))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while worker.shared.lock().result.is_none() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    worker.submit(request(4, "other", records)).unwrap();
    let result = ready(&worker);
    assert_eq!(result.generation, 4);
    assert!(result.rows.is_empty());
    assert_eq!(result.excluded, [0]);
    worker.stop();
    stopped(&worker, QueryStopReason::Requested);
}

#[test]
fn stop_cancels_pending_work_and_rejected_request_remains_caller_owned() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = QueryWorker::spawn_with_hook(move |_| {
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    })
    .unwrap();
    worker.submit(request(1, "first", Vec::new())).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    worker.submit(request(2, "second", Vec::new())).unwrap();
    worker.stop();
    assert!(matches!(
        worker.poll(),
        QueryPoll::Stopped(QueryStopReason::Requested)
    ));
    assert!(
        !worker.is_finished(),
        "blocked hook proves stop does not join the worker"
    );
    let rejected = worker
        .submit(request(3, "retained", Vec::new()))
        .unwrap_err();
    assert_eq!(rejected.request.generation, 3);
    assert_eq!(rejected.reason, QueryStopReason::Requested);
    assert!(worker.shared.lock().pending.is_none());
    release_tx.send(()).unwrap();
    stopped(&worker, QueryStopReason::Requested);
}

#[test]
fn unexpected_worker_exit_reports_terminal_status_instead_of_searching_forever() {
    let worker = QueryWorker::spawn_with_hook(|_| panic!("injected worker failure")).unwrap();
    worker.submit(request(1, "needle", Vec::new())).unwrap();
    stopped(&worker, QueryStopReason::WorkerEndedUnexpectedly);
    assert!(matches!(
        worker.poll(),
        QueryPoll::Stopped(QueryStopReason::WorkerEndedUnexpectedly)
    ));
    let rejected = worker
        .submit(request(2, "retained", Vec::new()))
        .unwrap_err();
    assert_eq!(rejected.request.generation, 2);
}
