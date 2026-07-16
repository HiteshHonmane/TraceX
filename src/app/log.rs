use super::theme;
use chrono::Local;
use eframe::egui::Color32;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(super) fn strip_ansi(input: &str) -> String {
    let re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(input, "").to_string()
}

fn parse_log_level(text: &str) -> LogLevel {
    let upper = text.to_uppercase();
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
    let re = Regex::new(r"\]\[([A-Za-z][A-Za-z0-9_]*)\]\s").ok()?;
    let mut last = None;
    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let t = m.as_str();
            if !t.contains('.') && t.parse::<u32>().is_err() {
                last = Some(t.to_string());
            }
        }
    }
    last
}

fn extract_message(text: &str) -> String {
    if let Some(pos) = text.rfind(']') {
        let msg = text[pos + 1..].trim();
        if !msg.is_empty() {
            return msg.to_string();
        }
    }
    text.to_string()
}

// ═══════════════════════════════════════════════════════════════════════════════
// THEME
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub(super) enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Verbose,
    Log,
}

impl LogLevel {
    pub(super) fn color(&self) -> Color32 {
        match self {
            Self::Error => theme::ERROR,
            Self::Warn => theme::WARNING,
            Self::Info => theme::INFO,
            Self::Debug => theme::DEBUG,
            Self::Verbose => theme::VERBOSE,
            Self::Log => theme::TEXT_DIM,
        }
    }
    pub(super) fn label(&self) -> &'static str {
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

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct LogEntry {
    pub(super) id: usize,
    pub(super) timestamp: String,
    pub(super) level: LogLevel,
    pub(super) tag: Option<String>,
    pub(super) message: String,
    pub(super) raw: String,
}

impl LogEntry {
    pub(super) fn new(id: usize, raw: String) -> Self {
        let clean = strip_ansi(&raw);
        Self {
            id,
            timestamp: Local::now().format("%H:%M:%S.%3f").to_string(),
            level: parse_log_level(&clean),
            tag: extract_tag(&clean),
            message: extract_message(&clean),
            raw,
        }
    }

    pub(super) fn format_line(&self) -> String {
        format_log_line(&self.timestamp, self.level, &self.message)
    }
}

pub(super) fn format_log_line(timestamp: &str, level: LogLevel, message: &str) -> String {
    format!("[{}] [{}] {}", timestamp, level.label(), message)
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

pub(super) fn selected_log_text(logs: &[LogEntry], selected: &HashSet<usize>) -> Option<String> {
    (selected.len() > 1)
        .then(|| format_log_entries(logs.iter().filter(|entry| selected.contains(&entry.id))))
}
