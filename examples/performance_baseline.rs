//! Deterministic domain microbenchmark; run with `cargo run --release --example performance_baseline`.
#[path = "../src/domain/log.rs"]
mod log;
#[path = "../src/domain/query.rs"]
mod query;

use log::{LogEntry, LogLevel};
use query::{LogQuery, QueryScope, SeveritySelection};
use std::{hint::black_box, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--help") {
        println!(
            "Usage: performance_baseline [EVENT_COUNT ...]\nDefault: 10000 100000 1000000. Maximum per fixture: 1000000. Five scan/format repetitions; three fixture runs."
        );
        return Ok(());
    }
    let counts = if arguments.is_empty() {
        vec![10_000, 100_000, 1_000_000]
    } else {
        arguments
            .iter()
            .map(|value| value.parse::<usize>())
            .collect::<Result<Vec<_>, _>>()?
    };
    if counts.iter().any(|count| *count == 0 || *count > 1_000_000) {
        return Err("event counts must be between 1 and 1000000".into());
    }
    println!(
        "events,run,raw_bytes,parse_ms,scan_ms,format_ms,matches,formatted_bytes,checksum,clone_query_ms,index_query_ms"
    );
    for count in counts {
        for run in 1..=3 {
            measure(count, run);
        }
    }
    Ok(())
}

fn measure(count: usize, run: usize) {
    // Generation is excluded. Each fourth line contains an ANSI-colored ERROR.
    let raw: Vec<String> = (0..count)
        .map(|index| match index % 4 {
            0 => format!("\u{1b}[31m[12.000][MODEM_1] ERROR timeout sequence={index:07}\u{1b}[0m"),
            1 => format!("[12.001][SENSOR] INFO temperature=24 sequence={index:07}"),
            2 => format!("[12.002][RADIO] DEBUG connected sequence={index:07}"),
            _ => format!("plain device output sequence={index:07}"),
        })
        .collect();
    let raw_bytes: usize = raw.iter().map(String::len).sum();
    let start = Instant::now();
    let entries: Vec<LogEntry> = raw
        .into_iter()
        .enumerate()
        .map(|(index, line)| LogEntry::new(index, line, "10:30:45.123"))
        .collect();
    let parse_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let mut matches = 0;
    for _ in 0..5 {
        matches = black_box(&entries)
            .iter()
            .filter(|entry| {
                entry.level == LogLevel::Error && entry.message.contains(black_box("timeout"))
            })
            .count();
        black_box(matches);
    }
    let scan_ms = start.elapsed().as_secs_f64() * 200.0;
    let start = Instant::now();
    let mut formatted_bytes = 0;
    let mut checksum = 0;
    for _ in 0..5 {
        formatted_bytes = 0;
        checksum = 0;
        for entry in black_box(&entries) {
            // Explicit synthetic text-export shape, not the application's export path.
            let text = format!(
                "{} [{}] {} {}",
                entry.timestamp,
                entry.level.label(),
                entry.tag.as_deref().unwrap_or(""),
                entry.message
            );
            formatted_bytes += black_box(text).len();
            checksum ^= entry.id ^ entry.raw.len();
        }
        black_box((formatted_bytes, checksum));
    }
    let format_ms = start.elapsed().as_secs_f64() * 200.0;
    let (clone_query_ms, index_query_ms) = compare_queries(&entries);
    println!(
        "{count},{run},{raw_bytes},{parse_ms:.3},{scan_ms:.3},{format_ms:.3},{matches},{formatted_bytes},{checksum},{clone_query_ms:.3},{index_query_ms:.3}"
    );
}

fn compare_queries(entries: &[LogEntry]) -> (f64, f64) {
    let query = LogQuery::new(
        " TiMeOuT ",
        SeveritySelection {
            info: true,
            warn: true,
            error: true,
        },
    );
    let start = Instant::now();
    for _ in 0..3 {
        let rows: Vec<LogEntry> = black_box(entries)
            .iter()
            .filter(|entry| {
                let text = black_box(" TiMeOuT ").trim().to_lowercase();
                entry.raw.to_lowercase().contains(&text)
                    || entry.message.to_lowercase().contains(&text)
            })
            .cloned()
            .collect();
        black_box(rows);
    }
    let clone_ms = start.elapsed().as_secs_f64() * 1000.0 / 3.0;
    let start = Instant::now();
    for _ in 0..3 {
        black_box(query.indices(black_box(entries), QueryScope::Main));
    }
    let index_ms = start.elapsed().as_secs_f64() * 1000.0 / 3.0;
    // Check all scopes outside timing, including equal selected record identities.
    let indices = query.indices(entries, QueryScope::Include);
    let expected: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            (entry.raw.to_lowercase().contains("timeout")
                || entry.message.to_lowercase().contains("timeout"))
            .then_some(index)
        })
        .collect();
    assert_eq!(indices, expected);
    assert_eq!(
        query.indices(entries, QueryScope::Exclude).len() + indices.len(),
        entries.len()
    );
    (clone_ms, index_ms)
}
