use super::*;
use crate::domain::log::LogEntry;
use crate::domain::query::SeveritySelection;
use std::time::{Duration, Instant};

fn query(text: &str) -> LogQuery {
    LogQuery::new(
        text,
        SeveritySelection {
            info: true,
            warn: true,
            error: true,
        },
    )
}
fn records(lines: &[&str]) -> Vec<SharedLog> {
    lines
        .iter()
        .enumerate()
        .map(|(id, raw)| Arc::new(LogEntry::new(id, (*raw).into(), "t")))
        .collect()
}
fn complete(view: &mut QueryView, records: &[SharedLog], query: LogQuery) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        view.refresh(records, query.clone());
        assert!(view.failure.is_none(), "{:?}", view.failure);
        if !view.pending && view.snapshot.is_some() {
            return;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn unchanged_frames_reuse_results_without_resubmitting_or_copying_text() {
    let records = records(&["INFO first", "ERROR needle", "other"]);
    let mut view = QueryView::default();
    complete(&mut view, &records, query("needle"));
    let snapshot = view.snapshot.clone().unwrap();
    let generation = view.generation;
    for _ in 0..100 {
        view.refresh(&records, query("needle"));
        assert!(Arc::ptr_eq(&snapshot, view.snapshot.as_ref().unwrap()));
    }
    assert_eq!(view.generation, generation);
    assert_eq!(snapshot.rows, [1]);
    assert_eq!(snapshot.excluded, [0, 2]);
}

#[test]
fn new_query_hides_previous_results_before_the_worker_completes() {
    let records = records(&["first", "second"]);
    let mut view = QueryView::default();
    complete(&mut view, &records, query("first"));
    assert_eq!(view.snapshot.as_ref().unwrap().rows, [0]);
    view.refresh(&records, query("second"));
    assert!(view.snapshot.is_none());
    assert!(view.pending);
    complete(&mut view, &records, query("second"));
    assert_eq!(view.snapshot.as_ref().unwrap().rows, [1]);
}

#[test]
fn appending_records_keeps_valid_prefix_and_eventually_includes_new_rows() {
    let mut records = records(&["first needle"]);
    let mut view = QueryView::default();
    complete(&mut view, &records, query("needle"));
    let prefix = view.snapshot.clone().unwrap();
    records.push(Arc::new(LogEntry::new(1, "second needle".into(), "t")));
    view.refresh(&records, query("needle"));
    assert!(Arc::ptr_eq(&prefix, view.snapshot.as_ref().unwrap()));
    assert!(view.pending);
    complete(&mut view, &records, query("needle"));
    assert_eq!(view.snapshot.as_ref().unwrap().rows, [0, 1]);
    assert_eq!(view.snapshot.as_ref().unwrap().record_count, 2);
}

#[test]
fn clear_and_equal_length_replacement_cannot_reuse_old_indices() {
    let mut app = TraceXApp::with_ports(Arc::new(std::sync::Mutex::new(Vec::new())));
    app.record_log_line("old needle".into());
    complete(&mut app.query_view, &app.logs, query("needle"));
    app.clear();
    assert!(app.query_snapshot().is_none());
    app.record_log_line("replacement".into());
    app.search = "needle".into();
    app.refresh_query_view();
    assert!(app.query_snapshot().is_none());
    complete(&mut app.query_view, &app.logs, query("needle"));
    assert!(app.query_snapshot().unwrap().rows.is_empty());
    assert_eq!(app.logs[0].raw, "replacement");
}

#[test]
fn initial_empty_history_requires_no_worker_and_has_no_secondary_matches() {
    let mut view = QueryView::default();
    view.refresh(&[], query(" "));
    assert!(view.worker.is_none());
    assert!(!view.pending);
    assert!(view.snapshot.as_ref().unwrap().query_empty);
    assert!(view.snapshot.as_ref().unwrap().rows.is_empty());
}

#[test]
fn failed_worker_is_visible_and_generation_exhaustion_never_wraps() {
    let records = records(&["one"]);
    let mut view = QueryView::default();
    complete(&mut view, &records, query(""));
    view.worker.as_ref().unwrap().stop();
    view.refresh(&records, query(""));
    assert!(view.failure.as_deref().unwrap().contains("stopped"));
    assert!(!view.pending);
    view.generation = u64::MAX;
    view.refresh(&records, query("changed"));
    assert_eq!(view.generation, u64::MAX);
    assert!(view.failure.as_deref().unwrap().contains("exhausted"));
    assert!(view.snapshot.is_none());
}

#[test]
fn exhausted_generation_cannot_accept_an_old_result_for_a_changed_query() {
    let records = records(&["old match"]);
    let worker = QueryWorker::spawn().unwrap();
    worker
        .submit(QueryRequest {
            generation: u64::MAX,
            query: query("old"),
            records: records.clone(),
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !worker.result_waiting() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let mut view = QueryView {
        worker: Some(worker),
        generation: u64::MAX,
        key: Some(query("old")),
        requested_len: records.len(),
        pending: true,
        ..Default::default()
    };
    view.refresh(&records, query("new"));
    view.refresh(&records, query("new"));
    assert!(view.failure.as_deref().unwrap().contains("exhausted"));
    assert!(view.snapshot.is_none());
}
