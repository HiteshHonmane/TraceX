//! Disconnected save/open ownership. File I/O and segment parsing run off the UI.
use super::TraceXApp;
use crate::domain::{
    history_budget::HistoryBudget,
    log::SharedLog,
    query::{LogQuery, QueryScope},
    session_format::{SegmentReader, SegmentWriter},
};
use std::io::{BufWriter, Write};
use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
};

#[derive(Default)]
pub(super) struct SessionArchive {
    receiver: Option<mpsc::Receiver<ArchiveResult>>,
    // Shared handoff preserves moved history if spawning a worker fails.
    save_records: Option<Arc<Mutex<Option<Vec<SharedLog>>>>>,
    failure: Option<String>,
}
enum ArchiveResult {
    Saved {
        result: Result<(), String>,
        marks_clean: bool,
    },
    Opened(Result<Vec<SharedLog>, String>),
}
enum SaveKind {
    Session,
    Text(LogQuery),
}

fn export_text(path: PathBuf, records: &[SharedLog], query: &LogQuery) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|cause| format!("Could not create a new export file: {cause}"))?;
    {
        let mut output = BufWriter::new(&mut file);
        let mut first = true;
        for entry in records
            .iter()
            .filter(|entry| query.matches(entry, QueryScope::Main))
        {
            if !first {
                output.write_all(b"\n").map_err(|cause| cause.to_string())?;
            }
            first = false;
            write!(
                output,
                "[{}] [{}] {}",
                entry.timestamp,
                entry.level.label(),
                entry.message
            )
            .map_err(|cause| format!("Export write failed: {cause}"))?;
        }
        output
            .flush()
            .map_err(|cause| format!("Export flush failed: {cause}"))?;
    }
    file.sync_data()
        .map_err(|cause| format!("Export final sync failed: {cause}"))
}

fn save(path: PathBuf, records: &[SharedLog]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|cause| format!("Could not create a new session file: {cause}"))?;
    {
        let mut writer = SegmentWriter::new(&mut file).map_err(|cause| cause.to_string())?;
        for entry in records {
            writer.append(entry).map_err(|cause| cause.to_string())?;
        }
        writer.flush().map_err(|cause| cause.to_string())?;
    }
    file.sync_data()
        .map_err(|cause| format!("Session final sync failed: {cause}"))
}

fn open(path: PathBuf) -> Result<Vec<SharedLog>, String> {
    open_with_budget(path, HistoryBudget::default())
}

fn open_with_budget(path: PathBuf, mut budget: HistoryBudget) -> Result<Vec<SharedLog>, String> {
    let file = File::open(path).map_err(|cause| format!("Could not open session: {cause}"))?;
    let mut reader = SegmentReader::new(file).map_err(|cause| cause.to_string())?;
    let mut records = Vec::new();
    let mut previous_id = None;
    while let Some(entry) = reader.next_record().map_err(|cause| cause.to_string())? {
        if entry.id == usize::MAX || previous_id.is_some_and(|previous| entry.id <= previous) {
            return Err("Session IDs must increase and leave space for the next record".to_owned());
        }
        if !budget.can_retain_entry(&entry) {
            return Err("Session exceeds the preview's record/byte history budget; current logs were preserved".to_owned());
        }
        previous_id = Some(entry.id);
        budget.record_entry(&entry);
        records.push(Arc::new(entry));
    }
    // Read-only recovery inspection remains separate; only entirely valid files open.
    let _validated_bytes = reader.valid_prefix();
    Ok(records)
}

impl TraceXApp {
    pub(super) fn session_archive_busy(&self) -> bool {
        self.session_archive.receiver.is_some()
    }
    pub(super) fn session_archive_shutdown_failure(&self) -> Option<String> {
        self.session_archive.failure.clone()
    }
    fn archive_ready(&mut self) -> bool {
        if self.shutdown.active()
            || self.connected
            || self.capture.is_some()
            || self.auto_save.stopping
            || self.session_archive_busy()
        {
            self.operation_error = Some(
                "Disconnect and wait for capture/storage/session work to finish first.".to_owned(),
            );
            return false;
        }
        true
    }
    pub(super) fn save_session_to(&mut self, path: PathBuf) {
        if !self.archive_ready() {
            return;
        }
        self.start_owned_save(path, SaveKind::Session);
    }
    fn text_export_ready(&mut self) -> bool {
        if self.shutdown.active() || self.connected || self.session_archive_busy() {
            self.operation_error = Some("Disconnect and wait for the current file operation before exporting retained logs.".into());
            return false;
        }
        true
    }
    pub(super) fn export_logs_to(&mut self, path: PathBuf) {
        if !self.text_export_ready() {
            return;
        }
        self.start_owned_save(path, SaveKind::Text(self.log_query()));
    }
    pub(super) fn choose_export_logs(&mut self) {
        if !self.text_export_ready() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log", &["log", "txt"])
            .set_file_name("export.log")
            .save_file()
        {
            self.export_logs_to(path);
        }
    }
    fn start_owned_save(&mut self, path: PathBuf, kind: SaveKind) {
        let marks_clean = matches!(&kind, SaveKind::Session);
        let records = Arc::new(Mutex::new(Some(std::mem::take(&mut self.logs))));
        let worker_records = Arc::clone(&records);
        let (sender, receiver) = mpsc::sync_channel(1);
        let spawned = thread::Builder::new()
            .name("tracex-session-save".into())
            .spawn(move || {
                // Borrow under the handoff lock only on the worker. UI never waits on it
                // until completion has been received.
                let outcome = match worker_records.lock() {
                    Ok(guard) => match guard.as_ref() {
                        Some(records) => match kind {
                            SaveKind::Session => save(path, records),
                            SaveKind::Text(query) => export_text(path, records, &query),
                        },
                        None => Err("Session save lost its record ownership".to_owned()),
                    },
                    Err(_) => Err("Session save ownership lock was poisoned".to_owned()),
                };
                let _ = sender.send(ArchiveResult::Saved {
                    result: outcome,
                    marks_clean,
                });
            });
        match spawned {
            Ok(_) => {
                self.session_archive.save_records = Some(records);
                self.session_archive.receiver = Some(receiver);
                self.operation_error = Some(
                    "Saving retained records… capture ingestion waits until the file operation completes.".into(),
                );
            }
            Err(cause) => {
                self.logs = take_records(records);
                self.archive_failed(format!("Could not start session save: {cause}"));
            }
        }
    }
    pub(super) fn open_session_from(&mut self, path: PathBuf) {
        if !self.archive_ready() {
            return;
        }
        if !self.logs.is_empty() {
            self.operation_error = Some("Save or export the current records, then deliberately Clear before opening another session.".to_owned());
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("tracex-session-open".into())
            .spawn(move || {
                let _ = sender.send(ArchiveResult::Opened(open(path)));
            }) {
            Ok(_) => {
                self.session_archive.receiver = Some(receiver);
                self.operation_error = Some(
                    "Opening session… validating the complete file before restoring records."
                        .into(),
                );
            }
            Err(cause) => self.archive_failed(format!("Could not start session open: {cause}")),
        }
    }
    fn archive_failed(&mut self, message: String) {
        self.operation_error = Some(message.clone());
        self.session_archive.failure = Some(message);
    }
    pub(super) fn poll_session_archive(&mut self) {
        let outcome = match self
            .session_archive
            .receiver
            .as_ref()
            .map(mpsc::Receiver::try_recv)
        {
            Some(Ok(outcome)) => outcome,
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                if let Some(records) = self.session_archive.save_records.take() {
                    self.logs = take_records(records);
                }
                self.session_archive.receiver = None;
                self.rebuild_history_accounting();
                self.archive_failed(
                    "Session worker ended unexpectedly; original records retained.".into(),
                );
                return;
            }
            _ => return,
        };
        self.session_archive.receiver = None;
        let result = match outcome {
            ArchiveResult::Saved {
                result,
                marks_clean,
            } => {
                if let Some(records) = self.session_archive.save_records.take() {
                    self.logs = take_records(records);
                }
                if result.is_ok() && marks_clean {
                    self.history_dirty = false;
                }
                result
            }
            ArchiveResult::Opened(result) => result.map(|records| {
                self.next_id = records.last().map_or(0, |entry| entry.id + 1);
                self.logs = records;
                self.selected.clear();
                self.scroll_to_log = None;
                self.history_dirty = false;
            }),
        };
        self.rebuild_history_accounting();
        match result {
            Ok(()) => {
                self.session_archive.failure = None;
                self.operation_error = None;
            }
            Err(message) => self.archive_failed(message),
        }
    }
    pub(super) fn choose_save_session(&mut self) {
        if !self.archive_ready() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("TraceX session", &["tracex"])
            .set_file_name("TraceX-session.tracex")
            .save_file()
        {
            self.save_session_to(path);
        }
    }
    pub(super) fn choose_open_session(&mut self) {
        if !self.archive_ready() {
            return;
        }
        if !self.logs.is_empty() {
            self.operation_error = Some("Save or export the current records, then deliberately Clear before opening another session.".to_owned());
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("TraceX session", &["tracex"])
            .pick_file()
        {
            self.open_session_from(path);
        }
    }
}

fn take_records(records: Arc<Mutex<Option<Vec<SharedLog>>>>) -> Vec<SharedLog> {
    match records.lock() {
        Ok(mut guard) => guard.take().unwrap_or_default(),
        Err(poisoned) => poisoned.into_inner().take().unwrap_or_default(),
    }
}

#[cfg(test)]
#[path = "session_archive_tests.rs"]
mod tests;
