/// Maximum pending line payload before emitting a continuation record.
pub(crate) const MAX_FRAME_BYTES: usize = 64 * 1024;

/// Frames a byte stream independently of read boundaries and UI frameworks.
/// Invalid UTF-8 frames are represented as explicit, reversible hex records.
#[derive(Default)]
pub(crate) struct StreamFramer {
    pending: Vec<u8>,
}

impl StreamFramer {
    /// Emits LF-delimited records, including empty lines, with bounded buffering.
    pub(crate) fn push(&mut self, bytes: &[u8], mut emit: impl FnMut(String)) {
        for &byte in bytes {
            if byte == b'\n' {
                if self.pending.last() == Some(&b'\r') {
                    self.pending.pop();
                }
                emit(render_bytes(&self.pending));
                self.pending.clear();
                continue;
            }
            if self.pending.len() >= MAX_FRAME_BYTES
                && !(self.pending.len() == MAX_FRAME_BYTES && byte == b'\r')
            {
                // Keep an incomplete Unicode scalar (or CRLF's CR) with the
                // following bytes instead of turning valid text into hex.
                let end = match std::str::from_utf8(&self.pending) {
                    Err(error) if error.error_len().is_none() => error.valid_up_to(),
                    _ if self.pending.last() == Some(&b'\r') => self.pending.len() - 1,
                    _ => self.pending.len(),
                };
                emit(render_bytes(&self.pending[..end]));
                self.pending.drain(..end);
            }
            self.pending.push(byte);
        }
    }

    /// Emits the final unterminated payload once on shutdown or disconnection.
    pub(crate) fn finish(&mut self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        let record = render_bytes(&self.pending);
        self.pending.clear();
        Some(record)
    }
}

fn render_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            let mut text = String::with_capacity(28 + bytes.len() * 3);
            text.push_str("[TraceX invalid UTF-8 bytes:");
            for &byte in bytes {
                text.push(' ');
                text.push(HEX[(byte >> 4) as usize] as char);
                text.push(HEX[(byte & 0x0f) as usize] as char);
            }
            text.push(']');
            text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unicode_at_every_read_boundary() {
        let input = "hello é नमस्ते 🦀\n";
        for split in 0..=input.len() {
            let mut framer = StreamFramer::default();
            let mut records = Vec::new();
            framer.push(&input.as_bytes()[..split], |line| records.push(line));
            framer.push(&input.as_bytes()[split..], |line| records.push(line));
            assert_eq!(records, ["hello é नमस्ते 🦀"]);
            assert_eq!(framer.finish(), None);
        }
    }

    #[test]
    fn preserves_blank_lines_and_only_normalizes_crlf() {
        let mut framer = StreamFramer::default();
        let mut records = Vec::new();
        framer.push(b"\n\r", |line| records.push(line));
        framer.push(b"\ntext\r\r\n", |line| records.push(line));
        assert_eq!(records, ["", "", "text\r"]);
        assert_eq!(framer.finish(), None);
    }

    #[test]
    fn flushes_unterminated_payload_once_including_trailing_cr() {
        let mut framer = StreamFramer::default();
        framer.push(b"tail\r", |_| panic!("no completed line"));
        assert_eq!(framer.finish().as_deref(), Some("tail\r"));
        assert_eq!(framer.finish(), None);
    }

    #[test]
    fn preserves_all_malformed_frame_bytes_as_hex() {
        let mut framer = StreamFramer::default();
        let mut records = Vec::new();
        framer.push(b"a\xff\0\nvalid\n\xe2\x82", |line| records.push(line));
        assert_eq!(records, ["[TraceX invalid UTF-8 bytes: 61 FF 00]", "valid"]);
        assert_eq!(
            framer.finish().as_deref(),
            Some("[TraceX invalid UTF-8 bytes: E2 82]")
        );
    }

    #[test]
    fn bounds_unterminated_lines_without_splitting_unicode() {
        let mut framer = StreamFramer::default();
        let mut records = Vec::new();
        let input = format!("{}🦀tail", "a".repeat(MAX_FRAME_BYTES - 2));
        framer.push(input.as_bytes(), |line| records.push(line));
        assert!(framer.pending.len() <= MAX_FRAME_BYTES);
        records.extend(framer.finish());
        assert_eq!(records.concat(), input);
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn line_at_limit_does_not_add_an_extra_blank_record() {
        let mut framer = StreamFramer::default();
        let mut records = Vec::new();
        let input = format!("{}\r\n", "a".repeat(MAX_FRAME_BYTES));
        framer.push(input.as_bytes(), |line| records.push(line));
        assert_eq!(records, ["a".repeat(MAX_FRAME_BYTES)]);
        assert_eq!(framer.finish(), None);
    }

    #[test]
    fn long_malformed_input_preserves_every_byte_across_bounded_records() {
        let input = vec![0xff; MAX_FRAME_BYTES * 2 + 7];
        let mut framer = StreamFramer::default();
        let mut records = Vec::new();
        for chunk in input.chunks(4096) {
            framer.push(chunk, |line| records.push(line));
            assert!(framer.pending.len() <= MAX_FRAME_BYTES + 1);
        }
        records.extend(framer.finish());
        let reconstructed: Vec<u8> = records
            .iter()
            .flat_map(|record| {
                record
                    .strip_prefix("[TraceX invalid UTF-8 bytes:")
                    .unwrap()
                    .strip_suffix(']')
                    .unwrap()
                    .split_ascii_whitespace()
                    .map(|hex| u8::from_str_radix(hex, 16).unwrap())
            })
            .collect();
        assert_eq!(reconstructed, input);
        assert_eq!(records.len(), 3);
    }
}
