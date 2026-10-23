//! Creates a clearly synthetic native UI review archive using the production codec.
//! This is deterministic review data, not captured device output or a UI test.
#[allow(dead_code)]
#[path = "../src/domain/mod.rs"]
mod domain;

use domain::{
    log::{LogEntry, LogLevel},
    query::{LogQuery, QueryScope, SeveritySelection},
    session_format::{SegmentReader, SegmentWriter},
};
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
    path::PathBuf,
};

fn record(id: usize, level: LogLevel, message: impl Into<String>) -> LogEntry {
    let message = message.into();
    LogEntry {
        id,
        timestamp: format!("12:00:{id:02}.000"),
        level,
        tag: Some("SYNTHETIC_QA".into()),
        raw: if message.is_empty() {
            String::new()
        } else {
            format!("[SYNTHETIC_QA][{}] {message}", level.label())
        },
        message,
    }
}

fn fixture() -> Vec<LogEntry> {
    use LogLevel::{Debug, Error, Info, Log, Verbose, Warn};
    vec![
        record(
            0,
            Log,
            "SYNTHETIC QA SESSION: generated fixture; no device connected.",
        ),
        record(
            1,
            Info,
            "Synthetic boot complete; firmware=qa-fixture; sequence=001",
        ),
        record(
            2,
            Log,
            "Plain synthetic device output with no severity decoration.",
        ),
        record(
            3,
            Warn,
            "Synthetic timeout waiting for sensor response; retry=1",
        ),
        record(
            4,
            Error,
            "Synthetic timeout exhausted; channel=fixture-only",
        ),
        record(5, Debug, "Synthetic debug counter=42; state=READY"),
        record(
            6,
            Verbose,
            "Synthetic verbose trace: read -> parse -> retain",
        ),
        record(7, Info, "Synthetic Unicode: café; 23°C; μs; 中文; नमस्ते; 🦀"),
        record(8, Log, ""),
        record(
            9,
            Info,
            format!(
                "Synthetic long line: {} END_OF_LONG_LINE",
                "0123456789abcdef ".repeat(32)
            ),
        ),
        record(
            10,
            Warn,
            "Synthetic supply warning: measured=3.18V; threshold=3.20V",
        ),
        record(
            11,
            Error,
            "Synthetic checksum failure; expected=ABCD; actual=1234",
        ),
        record(
            12,
            Log,
            "Synthetic punctuation: \"quoted\", [brackets], {braces}, path=C:\\qa\\fixture",
        ),
        record(
            13,
            Info,
            "Synthetic timeout recovered; no command was sent.",
        ),
        record(
            14,
            Info,
            "Synthetic selection marker: select this row and an offscreen row.",
        ),
        record(15, Log, "Synthetic spacing: left\tmiddle   right"),
        record(
            16,
            Warn,
            "Synthetic review reminder: queue accepted is not device acknowledgement.",
        ),
        record(
            17,
            Info,
            "SYNTHETIC END: 18 records; search marker matches=3 (IDs 3, 4, 13).",
        ),
    ]
}

fn encode_and_verify(records: &[LogEntry]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    {
        let mut writer = SegmentWriter::new(&mut bytes)?;
        for record in records {
            writer.append(record)?;
        }
        writer.flush()?;
    }
    let mut reader = SegmentReader::new(Cursor::new(&bytes))?;
    for expected in records {
        let actual = reader.next_record()?.ok_or("roundtrip ended early")?;
        if actual.id != expected.id
            || actual.timestamp != expected.timestamp
            || actual.level != expected.level
            || actual.tag != expected.tag
            || actual.message != expected.message
            || actual.raw != expected.raw
        {
            return Err(format!("codec roundtrip changed fixture record {}", expected.id).into());
        }
    }
    if reader.next_record()?.is_some() {
        return Err("codec roundtrip added a record".into());
    }
    Ok(bytes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "--help")
    {
        println!(
            "Usage: ui_review_fixture [NEW_OUTPUT.tracex]\nDefault: .tooling/design-review/visual-fixture.tracex\nSynthetic data only. Refuses to overwrite an existing file."
        );
        return Ok(());
    }
    if arguments.len() > 1 {
        return Err("provide at most one new output path".into());
    }
    let path = arguments.first().map_or_else(
        || PathBuf::from(".tooling/design-review/visual-fixture.tracex"),
        PathBuf::from,
    );
    let records = fixture();
    let query = LogQuery::new(
        "timeout",
        SeveritySelection {
            info: true,
            warn: true,
            error: true,
        },
    );
    let matched_ids: Vec<_> = query
        .indices(records.iter(), QueryScope::Main)
        .into_iter()
        .map(|index| records[index].id)
        .collect();
    if matched_ids != [3, 4, 13] {
        return Err(format!("unexpected fixture query result: {matched_ids:?}").into());
    }
    let bytes = encode_and_verify(&records)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    output.write_all(&bytes)?;
    output.sync_data()?;
    println!(
        "Created synthetic QA archive: {} ({} records, {} bytes).",
        path.display(),
        records.len(),
        bytes.len()
    );
    println!(
        "Includes normal/info/warn/error/debug/verbose, blank, long and Unicode rows. Not hardware evidence."
    );
    println!("Search timeout: IDs 3, 4, 13.");
    Ok(())
}
