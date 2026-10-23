//! One application-owned capture, history and investigation runtime.
//! No window, widget, context or port-discovery ownership belongs here.
use super::autosave::AutosaveController;
use super::{capture, query_view, session_archive, shutdown};
use super::{format_log_entries, format_log_line, register_submitted_command};
use crate::domain::history_budget::HistoryBudget;
use crate::domain::log::{LogEntry, SharedLog};
use crate::domain::query::{LogQuery, QueryScope, SeveritySelection};
use chrono::Local;
use directories::UserDirs;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The sole live session owner during the first extraction increment.
pub(super) struct SessionRuntime {
    pub(super) selected_port: String,
    pub(super) baud_rate: u32,

    // Connection
    pub(super) connected: bool,
    pub(super) serial_tx: Option<SyncSender<Vec<u8>>>,
    pub(super) capture: Option<capture::CaptureInbox>,
    pub(super) next_capture_batch: u64,
    pub(super) capture_error: Option<String>,
    pub(super) stop_flag: Option<Arc<Mutex<bool>>>,
    pub(super) last_connected_port: Option<String>,

    // Logs
    pub(super) logs: Vec<SharedLog>,
    pub(super) query_view: query_view::QueryView,
    pub(super) next_id: usize,
    pub(super) history: HistoryBudget,
    pub(super) history_dirty: bool,
    pub(super) history_limit_reached: bool,
    pub(super) shutdown: shutdown::ShutdownCoordinator,
    pub(super) session_archive: session_archive::SessionArchive,

    // Command
    pub(super) command_input: String,
    pub(super) command_history: Vec<String>,
    pub(super) send_newline: bool,
    pub(super) auto_clear_command: bool,

    // Filter
    pub(super) search: String,
    pub(super) selected: HashSet<usize>,
    pub(super) filter_panel_open: bool,
    pub(super) filter_exclude: bool,
    pub(super) filter_results_height: f32,
    pub(super) scroll_to_log: Option<usize>,
    pub(super) scroll_jump_settle_frames: u8,

    // Auto-save
    pub(super) auto_save_enabled: bool,
    pub(super) auto_save_path: Option<PathBuf>,
    pub(super) auto_save_current_path: Option<PathBuf>,
    pub(super) auto_save: AutosaveController,
    pub(super) auto_save_limit_mb: u64,
    pub(super) auto_save_bytes_written: u64,
    pub(super) auto_save_sequence: u32,
    pub(super) auto_save_error: Option<String>,
    pub(super) operation_error: Option<String>,

    // UI
    pub(super) auto_scroll: bool,
    pub(super) live_scroll_frames: u8,
    pub(super) show_info: bool,
    pub(super) show_warn: bool,
    pub(super) show_error: bool,

    // Stats
    pub(super) rate: f32,
    pub(super) rate_count: usize,
    pub(super) rate_timer: Instant,

    // Auto-reconnect
    pub(super) reconnect_cooldown: Instant,
}

impl SessionRuntime {
    pub(super) fn new() -> Self {
        Self {
            selected_port: String::new(),
            baud_rate: 115200,

            connected: false,
            serial_tx: None,
            capture: None,
            next_capture_batch: 0,
            capture_error: None,
            stop_flag: None,
            last_connected_port: None,

            logs: Vec::new(),
            query_view: query_view::QueryView::default(),
            next_id: 0,
            history: HistoryBudget::default(),
            history_dirty: false,
            history_limit_reached: false,
            shutdown: shutdown::ShutdownCoordinator::default(),
            session_archive: session_archive::SessionArchive::default(),

            command_input: String::new(),
            command_history: Vec::new(),
            send_newline: true,
            auto_clear_command: true,

            search: String::new(),
            selected: HashSet::new(),
            filter_panel_open: false,
            filter_exclude: false,
            filter_results_height: 180.0,
            scroll_to_log: None,
            scroll_jump_settle_frames: 0,

            auto_save_enabled: false,
            auto_save_path: Some(Self::default_log_path()),
            auto_save_current_path: None,
            auto_save: AutosaveController::default(),
            auto_save_limit_mb: 25,
            auto_save_bytes_written: 0,
            auto_save_sequence: 0,
            auto_save_error: None,
            operation_error: None,

            auto_scroll: true,
            live_scroll_frames: 0,
            show_info: true,
            show_warn: true,
            show_error: true,

            rate: 0.0,
            rate_count: 0,
            rate_timer: Instant::now(),

            reconnect_cooldown: Instant::now(),
        }
    }

    pub(super) fn default_log_path() -> PathBuf {
        if let Some(user_dirs) = UserDirs::new()
            && let Some(doc_dir) = user_dirs.document_dir()
        {
            return doc_dir.join("TraceX.log");
        }
        // Fallback to current directory if Documents folder not found
        PathBuf::from("TraceX.log")
    }

    pub(super) fn can_retain_line_parts(&self, length: usize, capacity: usize) -> bool {
        !self.history_limit_reached && self.history.can_retain_line_parts(length, capacity)
    }

    pub(super) fn rebuild_history_accounting(&mut self) {
        self.history.rebuild(&self.logs);
        self.history_limit_reached = false;
        self.query_view.invalidate();
    }

    pub(super) fn log_query(&self) -> LogQuery {
        LogQuery::new(
            &self.search,
            SeveritySelection {
                info: self.show_info,
                warn: self.show_warn,
                error: self.show_error,
            },
        )
    }

    pub(super) fn query_indices(&self, scope: QueryScope) -> Vec<usize> {
        self.log_query()
            .indices(self.logs.iter().map(AsRef::as_ref), scope)
    }

    pub(super) fn record_log_line(&mut self, line: String) {
        let timestamp = Local::now().format("%H:%M:%S.%3f").to_string();
        let entry = LogEntry::new(self.next_id, line, timestamp);
        self.next_id += 1;
        self.rate_count += 1;
        self.append_auto_save_line(&format_log_line(
            &entry.timestamp,
            entry.level,
            &entry.message,
        ));

        self.history.record_entry(&entry);
        self.logs.push(Arc::new(entry));
        self.history_dirty = true;
    }

    pub(super) fn send_command(&mut self, command: &str) -> bool {
        let command = command.trim();
        if command.is_empty() {
            return false;
        }
        let size = command
            .len()
            .saturating_add(if self.send_newline { 2 } else { 0 });
        if size > capture::MAX_COMMAND_BYTES {
            self.operation_error = Some(format!(
                "Command exceeds the {}-byte limit; edit it before retrying.",
                capture::MAX_COMMAND_BYTES
            ));
            self.command_input = command.to_owned();
            return false;
        }
        let Some(command) = register_submitted_command(&mut self.command_history, command) else {
            return false;
        };
        let mut data = command.as_bytes().to_vec();
        if self.send_newline {
            data.extend_from_slice(b"\r\n");
        }
        let error = match self.serial_tx.as_ref() {
            Some(sender) if self.connected => match sender.try_send(data) {
                Ok(()) => {
                    self.operation_error = None;
                    return true;
                }
                Err(TrySendError::Full(_)) => {
                    "Command queue is full; the command is retained for retry."
                }
                Err(TrySendError::Disconnected(_)) => {
                    "Could not send command because the connection ended."
                }
            },
            _ => "Connect to a device before sending commands.",
        };
        self.command_input = command;
        self.operation_error = Some(error.to_owned());
        false
    }

    pub(super) fn filtered(&self) -> Vec<&LogEntry> {
        self.query_indices(QueryScope::Main)
            .into_iter()
            .map(|index| self.logs[index].as_ref())
            .collect()
    }

    pub(super) fn export_text(&self) -> String {
        format_log_entries(self.filtered())
    }

    pub(super) fn clear(&mut self) {
        if self.session_archive_busy() {
            self.operation_error =
                Some("Wait for the session operation before clearing logs.".to_owned());
            return;
        }
        self.logs.clear();
        self.query_view.invalidate();
        self.history_dirty = false;
        self.selected.clear();
        self.history.reset();
        self.history_limit_reached = false;
    }

    /// Format selected records, or the current filtered history when none are selected.
    pub(super) fn copy_selected_text(&self) -> String {
        if self.selected.is_empty() {
            self.export_text()
        } else {
            format_log_entries(
                self.logs
                    .iter()
                    .filter(|entry| self.selected.contains(&entry.id))
                    .map(AsRef::as_ref),
            )
        }
    }

    /// Format all retained records; clipboard ownership stays in the native UI.
    pub(super) fn copy_all_text(&self) -> String {
        format_log_entries(self.logs.iter().map(AsRef::as_ref))
    }
}
