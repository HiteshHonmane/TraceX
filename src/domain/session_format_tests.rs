use super::*;
use std::io::Cursor;

fn entry() -> LogEntry {
    LogEntry {
        id: 42,
        timestamp: "2026-10-23T12:00:00+05:30".into(),
        level: LogLevel::Warn,
        tag: Some("custom".into()),
        message: "derived 🦀\n".into(),
        raw: "\u{1b}[31mraw\0 नमस्ते".into(),
    }
}
fn segment() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut writer = SegmentWriter::new(&mut bytes).unwrap();
    writer.append(&entry()).unwrap();
    writer.flush().unwrap();
    bytes
}
#[test]
fn preserves_every_supplied_field_without_reparsing() {
    let bytes = segment();
    let mut reader = SegmentReader::new(Cursor::new(&bytes)).unwrap();
    let restored = reader.next_record().unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(entry()).unwrap()
    );
    assert_eq!(reader.valid_prefix(), bytes.len() as u64);
    assert!(reader.next_record().unwrap().is_none());
}
#[test]
fn every_truncation_is_explicit_except_complete_boundaries() {
    let bytes = segment();
    for cut in 0..bytes.len() {
        let constructor = SegmentReader::new(Cursor::new(&bytes[..cut]));
        if cut < 12 {
            assert!(matches!(
                constructor.unwrap_err().problem,
                Problem::Truncated
            ));
            continue;
        }
        let mut reader = constructor.unwrap();
        if cut == 12 {
            assert!(reader.next_record().unwrap().is_none());
        } else {
            let failure = reader.next_record().unwrap_err();
            assert_eq!(failure.valid_prefix, 12);
            assert!(matches!(failure.problem, Problem::Truncated));
            assert!(reader.next_record().unwrap().is_none());
        }
    }
}
#[test]
fn rejects_magic_version_and_reserved_bits() {
    for index in [0, 8, 10] {
        let mut bytes = segment();
        bytes[index] ^= 1;
        assert!(matches!(
            SegmentReader::new(Cursor::new(bytes)).unwrap_err().problem,
            Problem::Header
        ));
    }
}
#[test]
fn length_is_checked_before_reading_or_allocating_payload() {
    for length in [0u32, MAX_PAYLOAD_BYTES as u32 + 1, u32::MAX] {
        let mut bytes = HEADER.to_vec();
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        let mut reader = SegmentReader::new(Cursor::new(bytes)).unwrap();
        assert!(matches!(
            reader.next_record().unwrap_err().problem,
            Problem::Length
        ));
    }
}
#[test]
fn corrupt_second_frame_preserves_first_frame_boundary() {
    let mut bytes = segment();
    let first_end = bytes.len();
    let second = segment();
    bytes.extend_from_slice(&second[12..]);
    bytes[first_end + 8] ^= 1;
    let mut reader = SegmentReader::new(Cursor::new(bytes)).unwrap();
    reader.next_record().unwrap();
    let failure = reader.next_record().unwrap_err();
    assert_eq!(failure.valid_prefix, first_end as u64);
    assert!(matches!(failure.problem, Problem::Checksum));
}
#[test]
fn rejects_valid_checksum_invalid_schema_including_missing_tag() {
    for payload in [
        b"{}".as_slice(),
        br#"{"id":1,"timestamp":"t","level":"Info","message":"m","raw":"r"}"#,
    ] {
        let mut bytes = HEADER.to_vec();
        let length = (payload.len() as u32).to_le_bytes();
        bytes.extend_from_slice(&length);
        bytes.extend_from_slice(&checksum(&length, payload).to_le_bytes());
        bytes.extend_from_slice(payload);
        let mut reader = SegmentReader::new(Cursor::new(bytes)).unwrap();
        assert!(matches!(
            reader.next_record().unwrap_err().problem,
            Problem::Payload(_)
        ));
    }
}
#[test]
fn serialization_overflow_does_not_write_a_frame_or_poison_writer() {
    let mut bytes = Vec::new();
    {
        let mut writer = SegmentWriter::new(&mut bytes).unwrap();
        let mut oversized = entry();
        oversized.raw = "x".repeat(MAX_PAYLOAD_BYTES);
        assert!(matches!(
            writer.append(&oversized).unwrap_err().problem,
            Problem::Length
        ));
        writer.append(&entry()).unwrap();
    }
    assert_eq!(bytes, segment());
}
struct Failing {
    remaining: usize,
    flush_fails: bool,
}
impl Write for Failing {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("injected"));
        }
        let count = self.remaining.min(bytes.len());
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.flush_fails {
            Err(io::Error::other("flush"))
        } else {
            Ok(())
        }
    }
}
#[test]
fn io_failures_poison_writer_and_initial_header_failure_is_returned() {
    assert!(
        SegmentWriter::new(Failing {
            remaining: 5,
            flush_fails: false
        })
        .is_err()
    );
    let mut writer = SegmentWriter::new(Failing {
        remaining: 15,
        flush_fails: false,
    })
    .unwrap();
    assert!(matches!(
        writer.append(&entry()).unwrap_err().problem,
        Problem::Io(_)
    ));
    assert!(matches!(
        writer.append(&entry()).unwrap_err().problem,
        Problem::Poisoned
    ));
    let mut writer = SegmentWriter::new(Failing {
        remaining: 1000,
        flush_fails: true,
    })
    .unwrap();
    assert!(writer.flush().is_err());
    assert!(matches!(
        writer.flush().unwrap_err().problem,
        Problem::Poisoned
    ));
}
#[test]
fn crc_matches_independently_known_ieee_vector() {
    assert_eq!(checksum(b"1234", b"56789"), 0xcbf4_3926);
}

#[test]
fn read_io_error_is_distinct_from_clean_eof_and_stops_reader() {
    struct BrokenRead {
        header: Cursor<Vec<u8>>,
    }
    impl Read for BrokenRead {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.header.position() < 12 {
                self.header.read(output)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected read error",
                ))
            }
        }
    }
    let mut reader = SegmentReader::new(BrokenRead {
        header: Cursor::new(HEADER.to_vec()),
    })
    .unwrap();
    let failure = reader.next_record().unwrap_err();
    assert_eq!(failure.valid_prefix, 12);
    assert!(matches!(failure.problem, Problem::Io(_)));
    assert!(reader.next_record().unwrap().is_none());
}
