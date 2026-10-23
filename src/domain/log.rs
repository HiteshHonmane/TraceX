use serde::{Deserialize, Serialize};

/// A normalized severity assigned to one parsed device log entry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub(crate) enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Verbose,
    Log,
}

impl LogLevel {
    /// Returns the stable short label used by text exports and copy operations.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Error => "ERR",
            Self::Warn => "WRN",
            Self::Info => "INF",
            Self::Debug => "DBG",
            Self::Verbose => "VRB",
            Self::Log => "LOG",
        }
    }
}

/// A structured log record independent of transport, storage, and UI frameworks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct LogEntry {
    pub(crate) id: usize,
    pub(crate) timestamp: String,
    pub(crate) level: LogLevel,
    pub(crate) tag: Option<String>,
    pub(crate) message: String,
    pub(crate) raw: String,
}

impl LogEntry {
    /// Parses a raw device line using a timestamp supplied by the application.
    ///
    /// Supplying the timestamp keeps clock access outside the domain and makes
    /// parsing deterministic in tests.
    pub(crate) fn new(id: usize, raw: String, timestamp: impl Into<String>) -> Self {
        let clean = strip_ansi(&raw);
        Self {
            id,
            timestamp: timestamp.into(),
            level: parse_log_level(&clean),
            tag: extract_tag(&clean),
            message: extract_message(&clean),
            raw,
        }
    }
}

/// Removes ANSI SGR color sequences without compiling a regular expression for
/// every received line. Incomplete or unsupported escape sequences are retained.
pub(crate) fn strip_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut segment_start = 0;
    let mut index = 0;

    while index + 1 < bytes.len() {
        if bytes[index] != 0x1b || bytes[index + 1] != b'[' {
            index += 1;
            continue;
        }

        let mut sequence_end = index + 2;
        while sequence_end < bytes.len()
            && (bytes[sequence_end].is_ascii_digit() || bytes[sequence_end] == b';')
        {
            sequence_end += 1;
        }

        if sequence_end < bytes.len() && bytes[sequence_end] == b'm' {
            output.push_str(&input[segment_start..index]);
            index = sequence_end + 1;
            segment_start = index;
        } else {
            index += 1;
        }
    }

    output.push_str(&input[segment_start..]);
    output
}

fn parse_log_level(text: &str) -> LogLevel {
    let upper = text.to_ascii_uppercase();
    if upper.contains("ERROR") {
        LogLevel::Error
    } else if upper.contains("WARN") {
        LogLevel::Warn
    } else if upper.contains("INFO") {
        LogLevel::Info
    } else if upper.contains("DEBUG") {
        LogLevel::Debug
    } else if upper.contains("VERBOSE") || upper.contains("TRACE") {
        LogLevel::Verbose
    } else {
        LogLevel::Log
    }
}

fn extract_tag(text: &str) -> Option<String> {
    let mut last = None;

    for (marker_start, _) in text.match_indices("][") {
        let candidate_start = marker_start + 2;
        let candidate = &text[candidate_start..];
        if !candidate
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        {
            continue;
        }
        // Stop at the first invalid tag byte, rather than repeatedly searching
        // the remaining line for a distant "] ". Adjacent malformed markers
        // therefore cannot turn one bounded input record into quadratic work.
        let end = candidate
            .bytes()
            .position(|byte| !byte.is_ascii_alphanumeric() && byte != b'_');
        if let Some(end) = end
            && candidate.as_bytes()[end..].starts_with(b"] ")
        {
            last = Some(&candidate[..end]);
        }
    }

    last.map(str::to_owned)
}

fn extract_message(text: &str) -> String {
    if let Some(position) = text.rfind(']') {
        let message = text[position + 1..].trim();
        if !message.is_empty() {
            return message.to_owned();
        }
    }
    text.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_supported_ansi_sgr_sequences() {
        assert_eq!(
            strip_ansi("\u{1b}[31mERROR\u{1b}[0m device failed"),
            "ERROR device failed"
        );
    }

    #[test]
    fn preserves_incomplete_or_unsupported_escape_sequences() {
        assert_eq!(strip_ansi("value \u{1b}[31"), "value \u{1b}[31");
        assert_eq!(strip_ansi("value \u{1b}[2K"), "value \u{1b}[2K");
    }

    #[test]
    fn parses_level_tag_and_message_without_ui_dependencies() {
        let entry = LogEntry::new(
            7,
            "[12.000][NETWORK][MODEM_1] WARN reconnecting".to_owned(),
            "10:30:45.123",
        );

        assert_eq!(entry.id, 7);
        assert_eq!(entry.timestamp, "10:30:45.123");
        assert_eq!(entry.level, LogLevel::Warn);
        assert_eq!(entry.tag.as_deref(), Some("MODEM_1"));
        assert_eq!(entry.message, "WARN reconnecting");
    }

    #[test]
    fn falls_back_to_unstructured_log_data() {
        let entry = LogEntry::new(1, "plain device output".to_owned(), "00:00:00.000");

        assert_eq!(entry.level, LogLevel::Log);
        assert_eq!(entry.tag, None);
        assert_eq!(entry.message, "plain device output");
    }

    #[test]
    fn tag_parser_rejects_malformed_markers_and_keeps_last_valid_ascii_tag() {
        for malformed in [
            "][ABC",
            "][ABC]",
            "][9ABC] ",
            "][é] ",
            "][A-B] ",
            "][ABC]X] ",
        ] {
            assert_eq!(extract_tag(malformed), None, "{malformed}");
        }
        assert_eq!(
            extract_tag("[t][FIRST] [t][LAST_2] message"),
            Some("LAST_2".into())
        );
        assert_eq!(
            extract_tag("[t][FIRST] [t][invalid-thing] message"),
            Some("FIRST".into())
        );
        let malformed = format!("{}[FINAL] message", "][A".repeat(16_000));
        assert_eq!(extract_tag(&malformed), None);
        let valid_tail = format!("{}][FINAL] message", "][A".repeat(16_000));
        assert_eq!(extract_tag(&valid_tail), Some("FINAL".into()));
    }
}
