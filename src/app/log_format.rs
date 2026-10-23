use crate::domain::log::{LogEntry, LogLevel};
#[cfg(test)]
use std::collections::HashSet;

pub(super) fn format_log_line(timestamp: &str, level: LogLevel, message: &str) -> String {
    format!("[{timestamp}] [{}] {message}", level.label())
}

pub(super) fn format_log_rows<'a>(
    rows: impl IntoIterator<Item = (&'a str, LogLevel, &'a str)>,
) -> String {
    rows.into_iter()
        .map(|(timestamp, level, message)| format_log_line(timestamp, level, message))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn format_log_entries<'a>(entries: impl IntoIterator<Item = &'a LogEntry>) -> String {
    format_log_rows(entries.into_iter().map(|entry| {
        (
            entry.timestamp.as_str(),
            entry.level,
            entry.message.as_str(),
        )
    }))
}

#[cfg(test)]
pub(super) fn selected_log_text(logs: &[LogEntry], selected: &HashSet<usize>) -> Option<String> {
    (selected.len() > 1)
        .then(|| format_log_entries(logs.iter().filter(|entry| selected.contains(&entry.id))))
}
