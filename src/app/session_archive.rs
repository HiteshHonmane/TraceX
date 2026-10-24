//! Disconnected save/open ownership. File I/O and segment parsing run off the UI.
use super::session::SessionRuntime;
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
    opening_path: Option<PathBuf>,
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

impl SessionRuntime {
    pub(super) fn session_archive_busy(&self) -> bool {
        self.session_archive.receiver.is_some()
    }
    pub(super) fn session_archive_shutdown_failure(&self) -> Option<String> {
        self.session_archive.failure.clone()
    }
    /// A stopped capture may retain unread records: Save snapshots only history,
    /// leaving that inbox owned and pausing ingestion through the archive handoff.
    pub(super) fn can_save_session(&self) -> bool {
        !self.shutdown.active()
            && !self.connected
            && !self.auto_save.stopping
            && !self.session_archive_busy()
    }
    /// Open cannot replace history or detach a capture that still owns records.
    pub(super) fn can_open_session(&self) -> bool {
        self.can_save_session() && self.capture.is_none() && self.logs.is_empty()
    }
    pub(super) fn save_session_ready(&mut self) -> bool {
        if !self.can_save_session() {
            self.operation_error = Some(
                "Disconnect and wait for storage/session work before saving retained records."
                    .to_owned(),
            );
            return false;
        }
        true
    }
    pub(super) fn open_session_ready(&mut self) -> bool {
        if self.can_open_session() {
            return true;
        }
        self.operation_error = Some(if self.can_save_session() && self.capture.is_none() {
            "Save or export the current records, then deliberately Clear before opening another session.".to_owned()
        } else {
            "Disconnect and wait for capture/storage/session work to finish first.".to_owned()
        });
        false
    }
    pub(super) fn save_session_to(&mut self, path: PathBuf) {
        if !self.save_session_ready() {
            return;
        }
        self.start_owned_save(path, SaveKind::Session);
    }
    pub(super) fn text_export_ready(&mut self) -> bool {
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
        if !self.open_session_ready() {
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let opening_path = path.clone();
        match thread::Builder::new()
            .name("tracex-session-open".into())
            .spawn(move || {
                let _ = sender.send(ArchiveResult::Opened(open(path)));
            }) {
            Ok(_) => {
                self.session_archive.opening_path = Some(opening_path);
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
                self.session_archive.opening_path = None;
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
                self.source_path = self.session_archive.opening_path.take();
                self.next_id = records.last().map_or(0, |entry| entry.id + 1);
                self.logs = records;
                self.selected.clear();
                self.scroll_to_log = None;
                self.history_dirty = false;
            }),
        };
        self.session_archive.opening_path = None;
        self.rebuild_history_accounting();
        match result {
            Ok(()) => {
                self.session_archive.failure = None;
                self.operation_error = None;
            }
            Err(message) => self.archive_failed(message),
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

#[cfg(test)]
#[path = "session_runtime_tests.rs"]
mod runtime_owner_tests;

/// Controlled completion handoff for workspace tests; no timing or disk thread.
#[cfg(test)]
pub(super) struct HeldArchiveSave(mpsc::SyncSender<ArchiveResult>);
#[cfg(test)]
impl HeldArchiveSave {
    pub(super) fn complete(self, result: Result<(), String>) -> Result<(), String> {
        self.0
            .send(ArchiveResult::Saved {
                result,
                marks_clean: true,
            })
            .map_err(|cause| cause.to_string())
    }
}
#[cfg(test)]
impl SessionRuntime {
    pub(super) fn hold_session_save_for_test(&mut self) -> HeldArchiveSave {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.session_archive.save_records =
            Some(Arc::new(Mutex::new(Some(std::mem::take(&mut self.logs)))));
        self.session_archive.receiver = Some(receiver);
        HeldArchiveSave(sender)
    }
}
