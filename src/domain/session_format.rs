//! Bounded, checksummed session segment codec. See SESSION-FORMAT.md.
use super::log::{LogEntry, LogLevel};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

const HEADER: &[u8; 12] = b"TRACEXSG\x01\0\0\0";
pub(crate) const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Offset identifies the last complete validated prefix; inputs are never repaired.
#[derive(Debug)]
pub(crate) struct FormatError {
    pub(crate) valid_prefix: u64,
    pub(crate) problem: Problem,
}

#[derive(Debug)]
pub(crate) enum Problem {
    Io(io::Error),
    Header,
    Truncated,
    Length,
    Checksum,
    Payload(String),
    IdRange,
    Poisoned,
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Session error after byte {}: ",
            self.valid_prefix
        )?;
        match &self.problem {
            Problem::Io(error) => write!(formatter, "{error}"),
            Problem::Payload(error) => write!(formatter, "invalid record: {error}"),
            other => write!(formatter, "{other:?}"),
        }
    }
}
impl std::error::Error for FormatError {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRecord {
    id: u64,
    timestamp: String,
    level: LogLevel,
    #[serde(deserialize_with = "required_tag")]
    tag: Option<String>,
    message: String,
    raw: String,
}

fn required_tag<'de, D: serde::Deserializer<'de>>(input: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(input)
}

fn error(valid_prefix: u64, problem: Problem) -> FormatError {
    FormatError {
        valid_prefix,
        problem,
    }
}

fn checksum(length: &[u8; 4], payload: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in length.iter().chain(payload) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

struct BoundedPayload {
    bytes: Vec<u8>,
    overflow: bool,
}
impl Write for BoundedPayload {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_PAYLOAD_BYTES.saturating_sub(self.bytes.len()) {
            self.overflow = true;
            return Err(io::Error::other("Session payload exceeds format limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Creates a segment on a fresh caller-owned stream. Caller owns file creation/sync.
pub(crate) struct SegmentWriter<W: Write> {
    sink: W,
    offset: u64,
    poisoned: bool,
}
impl<W: Write> SegmentWriter<W> {
    pub(crate) fn new(mut sink: W) -> Result<Self, FormatError> {
        sink.write_all(HEADER)
            .map_err(|cause| error(0, Problem::Io(cause)))?;
        Ok(Self {
            sink,
            offset: HEADER.len() as u64,
            poisoned: false,
        })
    }

    /// Returns after all frame bytes are accepted, not after durable media sync.
    pub(crate) fn append(&mut self, entry: &LogEntry) -> Result<(), FormatError> {
        if self.poisoned {
            return Err(error(self.offset, Problem::Poisoned));
        }
        // Borrow serialization fields to avoid copying the original capture.
        #[derive(Serialize)]
        struct BorrowedRecord<'a> {
            id: u64,
            timestamp: &'a str,
            level: LogLevel,
            tag: Option<&'a str>,
            message: &'a str,
            raw: &'a str,
        }
        let id = u64::try_from(entry.id).map_err(|_| error(self.offset, Problem::IdRange))?;
        let record = BorrowedRecord {
            id,
            timestamp: &entry.timestamp,
            level: entry.level,
            tag: entry.tag.as_deref(),
            message: &entry.message,
            raw: &entry.raw,
        };
        let mut payload = BoundedPayload {
            bytes: Vec::new(),
            overflow: false,
        };
        if let Err(cause) = serde_json::to_writer(&mut payload, &record) {
            return Err(error(
                self.offset,
                if payload.overflow {
                    Problem::Length
                } else {
                    Problem::Payload(cause.to_string())
                },
            ));
        }
        let length = (payload.bytes.len() as u32).to_le_bytes();
        let crc = checksum(&length, &payload.bytes).to_le_bytes();
        if let Err(cause) = self
            .sink
            .write_all(&length)
            .and_then(|()| self.sink.write_all(&crc))
            .and_then(|()| self.sink.write_all(&payload.bytes))
        {
            self.poisoned = true;
            return Err(error(self.offset, Problem::Io(cause)));
        }
        self.offset += 8 + payload.bytes.len() as u64;
        Ok(())
    }

    /// Flushes the sink. This is not a durability checkpoint.
    pub(crate) fn flush(&mut self) -> Result<(), FormatError> {
        if self.poisoned {
            return Err(error(self.offset, Problem::Poisoned));
        }
        self.sink.flush().map_err(|cause| {
            self.poisoned = true;
            error(self.offset, Problem::Io(cause))
        })
    }
}

/// Streams one validated record at a time and stops permanently on any error.
#[derive(Debug)]
pub(crate) struct SegmentReader<R: Read> {
    source: R,
    offset: u64,
    ended: bool,
}
impl<R: Read> SegmentReader<R> {
    pub(crate) fn new(mut source: R) -> Result<Self, FormatError> {
        let mut header = [0; 12];
        source
            .read_exact(&mut header)
            .map_err(|cause| error(0, read_problem(cause)))?;
        if &header != HEADER {
            return Err(error(0, Problem::Header));
        }
        Ok(Self {
            source,
            offset: 12,
            ended: false,
        })
    }
    pub(crate) fn valid_prefix(&self) -> u64 {
        self.offset
    }

    pub(crate) fn next_record(&mut self) -> Result<Option<LogEntry>, FormatError> {
        if self.ended {
            return Ok(None);
        }
        match self.read_record() {
            Ok(Some(record)) => Ok(Some(record)),
            outcome => {
                self.ended = true;
                outcome
            }
        }
    }
    fn read_record(&mut self) -> Result<Option<LogEntry>, FormatError> {
        let mut frame = [0u8; 8];
        loop {
            match self.source.read(&mut frame[..1]) {
                Ok(0) => return Ok(None),
                Ok(_) => break,
                Err(cause) if cause.kind() == io::ErrorKind::Interrupted => continue,
                Err(cause) => return Err(error(self.offset, Problem::Io(cause))),
            }
        }
        self.source
            .read_exact(&mut frame[1..])
            .map_err(|cause| error(self.offset, read_problem(cause)))?;
        let length_bytes = [frame[0], frame[1], frame[2], frame[3]];
        let length = u32::from_le_bytes(length_bytes) as usize;
        if length == 0 || length > MAX_PAYLOAD_BYTES {
            return Err(error(self.offset, Problem::Length));
        }
        let mut payload = vec![0; length];
        self.source
            .read_exact(&mut payload)
            .map_err(|cause| error(self.offset, read_problem(cause)))?;
        let crc = u32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
        if crc != checksum(&length_bytes, &payload) {
            return Err(error(self.offset, Problem::Checksum));
        }
        let record: WireRecord = serde_json::from_slice(&payload)
            .map_err(|cause| error(self.offset, Problem::Payload(cause.to_string())))?;
        let id = usize::try_from(record.id).map_err(|_| error(self.offset, Problem::IdRange))?;
        self.offset += 8 + length as u64;
        Ok(Some(LogEntry {
            id,
            timestamp: record.timestamp,
            level: record.level,
            tag: record.tag,
            message: record.message,
            raw: record.raw,
        }))
    }
}
fn read_problem(cause: io::Error) -> Problem {
    if cause.kind() == io::ErrorKind::UnexpectedEof {
        Problem::Truncated
    } else {
        Problem::Io(cause)
    }
}

#[cfg(test)]
#[path = "session_format_tests.rs"]
mod tests;
