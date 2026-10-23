//! Synthetic query CPU baseline, not GUI frame timing or UART throughput.
#[allow(dead_code)]
#[path = "../src/domain/mod.rs"]
mod domain;
#[allow(dead_code)]
#[path = "../src/app/query_worker.rs"]
mod query_worker;

use domain::{
    history_budget::HistoryBudget,
    log::{LogEntry, SharedLog},
    query::{LogQuery, QueryScope, SeveritySelection},
};
use query_worker::{QueryPoll, QueryRequest, QueryResult, QueryWorker};
use std::{
    hint::black_box,
    sync::Arc,
    time::{Duration, Instant},
};

fn query() -> LogQuery {
    LogQuery::new(
        " TiMeOuT ",
        SeveritySelection {
            info: true,
            warn: true,
            error: true,
        },
    )
}
fn fixture(count: usize) -> Result<Vec<SharedLog>, Box<dyn std::error::Error>> {
    let mut budget = HistoryBudget::default();
    let mut records = Vec::new();
    for index in 0..count {
        let raw = match index % 4 {
            0 => format!("\u{1b}[31m[12.000][MODEM_1] ERROR timeout sequence={index:07}\u{1b}[0m"),
            1 => format!("[12.001][SENSOR] INFO temperature=24 sequence={index:07}"),
            2 => format!("[12.002][RADIO] DEBUG connected sequence={index:07}"),
            _ => format!("plain device output sequence={index:07}"),
        };
        let entry = LogEntry::new(index, raw, "10:30:45.123");
        if !budget.can_retain_entry(&entry) {
            return Err("fixture exceeds provisional history budget".into());
        }
        budget.record_entry(&entry);
        records.push(Arc::new(entry));
    }
    Ok(records)
}
fn width(records: &[SharedLog], indices: &[usize]) -> f32 {
    indices.iter().fold(0.0f32, |maximum, &index| {
        maximum.max(205.0 + records[index].message.chars().count() as f32 * 7.2)
    })
}
fn old_scan(records: &[SharedLog]) -> (Vec<usize>, Vec<usize>, f32, f32) {
    let query = query();
    let rows = query.indices(records.iter().map(AsRef::as_ref), QueryScope::Main);
    let excluded = query.indices(records.iter().map(AsRef::as_ref), QueryScope::Exclude);
    let main_width = width(records, &rows);
    let excluded_width = width(records, &excluded);
    (rows, excluded, main_width, excluded_width)
}
fn wait_result(worker: &QueryWorker) -> Result<QueryResult, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match worker.poll() {
            QueryPoll::Ready(result) => return Ok(result),
            QueryPoll::Stopped(reason) => {
                return Err(format!("query worker stopped: {reason:?}").into());
            }
            QueryPoll::Pending => {}
        }
        if Instant::now() >= deadline {
            return Err("query timed out".into());
        }
        std::thread::yield_now();
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "--help")
    {
        println!(
            "Usage: query_frame_baseline [COUNT]\nDefault/max: 100000. Three runs. Synthetic CPU timings; no GUI or UART."
        );
        return Ok(());
    }
    if arguments.len() > 1 {
        return Err("provide at most one event count".into());
    }
    let count = match arguments.first() {
        Some(value) => value.parse::<usize>()?,
        None => 100_000,
    };
    if count == 0 || count > 100_000 {
        return Err("count must be 1..=100000".into());
    }
    let records = fixture(count)?;
    let raw_bytes: usize = records.iter().map(|entry| entry.raw.len()).sum();
    let worker = QueryWorker::spawn()?;
    println!(
        "events,run,raw_bytes,old_query_width_ms,snapshot_prepare_ms,submit_ms,end_to_end_ms,cached_access_ns,matches,excluded,main_width,excluded_width"
    );
    for run in 1..=3 {
        let started = Instant::now();
        let expected = old_scan(black_box(&records));
        let old_ms = started.elapsed().as_secs_f64() * 1000.0;
        let end_to_end = Instant::now();
        let started = Instant::now();
        let request = QueryRequest {
            generation: run,
            query: query(),
            records: records.to_vec(),
        };
        let prepare_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        worker
            .submit(request)
            .map_err(|error| format!("submit: {:?}", error.reason))?;
        let submit_ms = started.elapsed().as_secs_f64() * 1000.0;
        let result = Arc::new(wait_result(&worker)?);
        let end_to_end_ms = end_to_end.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(result.rows, expected.0);
        assert_eq!(result.excluded, expected.1);
        assert_eq!(result.main_width, expected.2);
        assert_eq!(result.excluded_width, expected.3);
        assert_eq!(result.record_count, count);
        assert_eq!(result.generation, run);
        assert!(!result.query_empty);
        let started = Instant::now();
        for _ in 0..100_000 {
            let snapshot = Arc::clone(black_box(&result));
            black_box((
                snapshot.rows.len(),
                snapshot.excluded.len(),
                snapshot.main_width,
                snapshot.excluded_width,
            ));
        }
        let cached_ns = started.elapsed().as_secs_f64() * 1e9 / 100_000.0;
        println!(
            "{count},{run},{raw_bytes},{old_ms:.3},{prepare_ms:.3},{submit_ms:.3},{end_to_end_ms:.3},{cached_ns:.3},{},{},{:.3},{:.3}",
            result.rows.len(),
            result.excluded.len(),
            result.main_width,
            result.excluded_width
        );
    }
    worker.stop();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !worker.is_finished() {
        if Instant::now() >= deadline {
            return Err("worker stop timed out".into());
        }
        std::thread::yield_now();
    }
    Ok(())
}
