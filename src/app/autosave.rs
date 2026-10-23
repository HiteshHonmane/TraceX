use super::autosave_worker::{
    AutosaveCommand, AutosaveConfig, AutosaveEvent, AutosaveWorker, RecoveryRecord, SubmitError,
    WriteRequest,
};
use super::{TraceXApp, theme};
use eframe::egui::{self, RichText};
use std::sync::mpsc::TryRecvError;

// Initial queue budgets: at most 64 commands and 64 notifications, plus one
// caller-owned pending command. The transport queue remains separate legacy debt.
const COMMAND_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 64;

#[derive(Default)]
pub(super) struct AutosaveController {
    worker: Option<AutosaveWorker>,
    pending: Option<AutosaveCommand>,
    recovery: Vec<(WriteRequest, usize)>,
    failed: bool,
    pub(super) active: bool,
    pub(super) stopping: bool,
    last_saved_write: Option<(usize, usize)>,
    recovery_exporting: bool,
    finish_requested: bool,
    finish_observed: bool,
}

impl Drop for AutosaveController {
    fn drop(&mut self) {
        if let Some(worker) = &mut self.worker {
            // Never join or wait on the UI thread. Orderly application-exit
            // coordination uses request_auto_save_finish; Drop is best effort.
            let _ = worker.try_submit(AutosaveCommand::Finish);
        }
    }
}

impl TraceXApp {
    /// Queue Finish only after earlier pending work, without waiting for storage.
    pub(super) fn request_auto_save_finish(&mut self) {
        if self.auto_save.finish_requested || self.auto_save.pending.is_some() {
            return;
        }
        if self.auto_save.worker.is_none() {
            self.auto_save.finish_observed = true;
            return;
        }
        if self.submit_auto_save(AutosaveCommand::Finish) {
            self.auto_save.finish_requested = true;
            self.auto_save.active = false;
            self.auto_save.stopping = true;
        }
    }

    pub(super) fn auto_save_shutdown_complete(&self) -> bool {
        self.auto_save.worker.is_none()
            && self.auto_save.pending.is_none()
            && self.auto_save.recovery.is_empty()
            && !self.auto_save.failed
    }

    pub(super) fn auto_save_shutdown_failure(&self) -> Option<String> {
        (self.auto_save.failed || !self.auto_save.recovery.is_empty()).then(|| {
            self.auto_save_error.clone().unwrap_or_else(|| {
                "Autosave did not complete; unsaved payloads remain in memory.".to_owned()
            })
        })
    }

    fn submit_auto_save(&mut self, command: AutosaveCommand) -> bool {
        if self.auto_save.pending.is_some() {
            self.operation_error = Some(
                "Autosave is busy. Wait for queued data before changing its settings.".to_owned(),
            );
            return false;
        }
        if self.auto_save.worker.is_none() {
            match AutosaveWorker::spawn(COMMAND_CAPACITY, EVENT_CAPACITY) {
                Ok(worker) => {
                    self.auto_save.worker = Some(worker);
                    self.auto_save.finish_requested = false;
                    self.auto_save.finish_observed = false;
                }
                Err(error) => {
                    self.auto_save.failed = true;
                    self.auto_save_error =
                        Some(format!("Could not start autosave worker: {error}"));
                    if let AutosaveCommand::Write(request) = command {
                        self.auto_save.recovery.push((request, 0));
                    }
                    return false;
                }
            }
        }
        if let Some(worker) = &mut self.auto_save.worker {
            match worker.try_submit(command) {
                Ok(()) => {}
                Err(SubmitError::Full(command)) => self.auto_save.pending = Some(command),
                Err(SubmitError::Closed(command) | SubmitError::Finishing(command)) => {
                    if let AutosaveCommand::Write(request) = command {
                        self.auto_save.recovery.push((request, 0));
                    }
                    self.auto_save.failed = true;
                    self.auto_save_error = Some("Autosave worker ended. Pending data remains in memory; export unsaved payloads before closing.".to_owned());
                    return false;
                }
            }
        }
        true
    }

    pub(super) fn open_new_auto_save_file(&mut self) -> bool {
        if self.auto_save.finish_requested
            || self.auto_save.stopping
            || !self.auto_save.recovery.is_empty()
        {
            self.operation_error = Some(
                "Autosave has pending finalization or unsaved data. Export logs before restarting."
                    .to_owned(),
            );
            return false;
        }
        let config = AutosaveConfig {
            base_path: self
                .auto_save_path
                .clone()
                .unwrap_or_else(Self::default_log_path),
            limit_mb: self.auto_save_limit_mb,
            sequence: self.auto_save_sequence,
        };
        if self.submit_auto_save(AutosaveCommand::Start(config)) {
            self.auto_save.active = true;
            self.auto_save.failed = false;
            self.auto_save_error = None;
            true
        } else {
            false
        }
    }

    pub(super) fn stop_auto_save(&mut self) -> bool {
        if !self.auto_save.active || self.auto_save.stopping {
            return true;
        }
        if self.submit_auto_save(AutosaveCommand::Stop) {
            self.auto_save.active = false;
            self.auto_save.stopping = true;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_auto_save_enabled_change(&mut self) {
        if self.auto_save_enabled {
            if !self.auto_save.active && !self.open_new_auto_save_file() {
                self.auto_save_enabled = false;
            }
        } else if !self.stop_auto_save() {
            self.auto_save_enabled = true;
        }
    }

    pub(super) fn append_auto_save_line(&mut self, line: &str) {
        if !self.auto_save_enabled {
            return;
        }
        let request = WriteRequest {
            id: self.next_id.saturating_sub(1),
            payload: format!("{line}\n"),
        };
        // can_ingest_log reserves the single pending slot before removing a log.
        // This also covers records supplied directly by other application callers.
        if self.auto_save.pending.is_some() || self.auto_save.failed {
            self.auto_save.recovery.push((request, 0));
            self.auto_save.failed = true;
            self.auto_save_error = Some("Autosave data is retained in memory after backpressure or failure. Export logs before closing.".to_owned());
            return;
        }
        if !self.submit_auto_save(AutosaveCommand::Write(request)) {
            // Worker creation failure occurs before this command can be accepted.
            // In normal capture Start has already created the worker.
            self.auto_save.failed = true;
        }
    }

    pub(super) fn can_ingest_log(&self) -> bool {
        self.auto_save.pending.is_none() && (!self.auto_save_enabled || !self.auto_save.failed)
    }

    pub(super) fn poll_auto_save(&mut self) {
        let mut disconnected = false;
        loop {
            let result = self
                .auto_save
                .worker
                .as_ref()
                .map(AutosaveWorker::try_event);
            match result {
                Some(Ok(AutosaveEvent::Started {
                    path,
                    next_sequence,
                })) => {
                    self.auto_save_current_path = Some(path);
                    self.auto_save_sequence = next_sequence;
                    self.auto_save_bytes_written = 0;
                }
                Some(Ok(AutosaveEvent::Written {
                    id,
                    bytes,
                    current_bytes,
                })) => {
                    self.auto_save.last_saved_write = Some((id, bytes));
                    self.auto_save_bytes_written = current_bytes;
                }
                Some(Ok(AutosaveEvent::Stopped)) => self.auto_save.stopping = false,
                Some(Ok(AutosaveEvent::LimitUpdated)) => {}
                Some(Ok(AutosaveEvent::RecoveryExported { path, count })) => {
                    self.auto_save.recovery_exporting = false;
                    let exported = count.min(self.auto_save.recovery.len());
                    self.auto_save.recovery.drain(..exported);
                    self.auto_save_error = Some(format!(
                        "Recovery saved to {}. Disable and re-enable autosave to restart; {} unsaved payloads remain.",
                        path.display(),
                        self.auto_save.recovery.len()
                    ));
                }
                Some(Ok(AutosaveEvent::Failed { command, failure })) => {
                    self.auto_save.failed = true;
                    self.auto_save_error = Some(format!(
                        "Autosave {:?} failed: {}. Confirmed partial bytes: {}. Unsaved payloads remain in memory; export unsaved payloads before closing. Disable autosave to continue capture.",
                        failure.operation, failure.error, failure.written_prefix_bytes
                    ));
                    match command {
                        AutosaveCommand::Write(request) => self
                            .auto_save
                            .recovery
                            .push((request, failure.written_prefix_bytes)),
                        AutosaveCommand::Stop => {
                            self.auto_save.stopping = false;
                            self.auto_save.active = false;
                        }
                        AutosaveCommand::Start(_) => self.auto_save.active = false,
                        AutosaveCommand::ExportRecovery { .. } => {
                            self.auto_save.recovery_exporting = false
                        }
                        _ => {}
                    }
                }
                Some(Ok(AutosaveEvent::Finished { failure })) => {
                    self.auto_save.finish_observed = true;
                    self.auto_save.active = false;
                    self.auto_save.stopping = self.auto_save.finish_requested;
                    if let Some(failure) = failure {
                        self.auto_save.failed = true;
                        self.auto_save_error =
                            Some(format!("Autosave finalization failed: {}", failure.error));
                    }
                }
                Some(Err(TryRecvError::Disconnected)) => {
                    disconnected = true;
                    break;
                }
                Some(Err(TryRecvError::Empty)) | None => break,
            }
        }
        if disconnected
            && self
                .auto_save
                .worker
                .as_ref()
                .is_some_and(AutosaveWorker::is_finished)
        {
            if !self.auto_save.finish_observed {
                self.auto_save.failed = true;
            }
            self.auto_save.active = false;
            self.auto_save.stopping = self.auto_save.finish_requested;
            if !self.auto_save.finish_observed && self.auto_save_error.is_none() {
                self.auto_save_error = Some(
                    "Autosave worker stopped unexpectedly. Export logs before closing.".to_owned(),
                );
            }
        }
        if let Some(command) = self.auto_save.pending.take() {
            // Keep failed pending writes in the same FIFO as earlier accepted
            // writes. The faulted worker returns them as ordered failures.
            if let Some(worker) = &mut self.auto_save.worker {
                match worker.try_submit(command) {
                    Ok(()) => {}
                    Err(SubmitError::Full(command)) => self.auto_save.pending = Some(command),
                    Err(SubmitError::Closed(command) | SubmitError::Finishing(command)) => {
                        if let AutosaveCommand::Write(request) = command {
                            self.auto_save.recovery.push((request, 0));
                        }
                        self.auto_save.failed = true;
                        self.auto_save.stopping = false;
                        self.auto_save_error = Some("Autosave worker ended before queued work completed. Unsaved payloads remain in memory; export unsaved payloads before closing.".to_owned());
                    }
                }
            } else {
                self.auto_save.pending = Some(command);
                self.auto_save.failed = true;
                self.auto_save_error = Some(
                    "Autosave worker is unavailable; pending data remains in memory.".to_owned(),
                );
            }
        }
        // Finished is acknowledged only after observing both the terminal event
        // and thread completion. Dropping a completed JoinHandle never waits.
        if self.auto_save.finish_observed
            && self
                .auto_save
                .worker
                .as_ref()
                .is_some_and(AutosaveWorker::is_finished)
        {
            self.auto_save.worker = None;
            self.auto_save.finish_requested = false;
            self.auto_save.stopping = false;
        }
    }

    pub(super) fn render_auto_save_status(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = &self.auto_save_error {
            ui.label(
                RichText::new(error)
                    .monospace()
                    .size(8.5)
                    .color(theme::ERROR),
            );
        } else if self.auto_save.pending.is_some() {
            ui.label(
                RichText::new("Autosave queue full; capture waits for storage.")
                    .monospace()
                    .size(8.5)
                    .color(theme::WARNING),
            );
        } else if self.auto_save.stopping {
            ui.label(
                RichText::new("Finalizing saved logs...")
                    .monospace()
                    .size(8.5)
                    .color(theme::TEXT_MUTED),
            );
        } else if let Some(path) = &self.auto_save_current_path {
            let label = if self.auto_save_enabled && self.auto_save.active {
                "Saving to"
            } else {
                "Last file"
            };
            ui.label(
                RichText::new(format!("{label}: {}", path.display()))
                    .monospace()
                    .size(8.5)
                    .color(theme::TEXT_MUTED),
            );
        } else if self.auto_save.active {
            ui.label(
                RichText::new("Opening log file...")
                    .monospace()
                    .size(8.5)
                    .color(theme::TEXT_MUTED),
            );
        }
        self.render_recovery_export(ui);
    }

    fn render_recovery_export(&mut self, ui: &mut egui::Ui) {
        if self.auto_save.recovery.is_empty() {
            return;
        }
        ui.label(format!(
            "{} unsaved payloads retained",
            self.auto_save.recovery.len()
        ));
        if ui
            .add_enabled(
                !self.auto_save.recovery_exporting,
                egui::Button::new("Export unsaved payloads"),
            )
            .clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Recovery JSON", &["json"])
                .set_file_name("TraceX-recovery.json")
                .save_file()
        {
            self.export_recovery_to(path);
        }
    }

    fn export_recovery_to(&mut self, path: std::path::PathBuf) {
        if self.auto_save.recovery_exporting {
            return;
        }
        let records = self
            .auto_save
            .recovery
            .iter()
            .map(|(request, prefix)| RecoveryRecord {
                request: request.clone(),
                written_prefix_bytes: *prefix,
            })
            .collect();
        if self.submit_auto_save(AutosaveCommand::ExportRecovery { path, records }) {
            self.auto_save.recovery_exporting = true;
        }
    }

    pub(super) fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("LOG FILES")
                .size(10.0)
                .color(theme::ACCENT)
                .strong(),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("File limit");
            let previous = self.auto_save_limit_mb;
            if ui
                .add(
                    egui::DragValue::new(&mut self.auto_save_limit_mb)
                        .range(1..=4096)
                        .suffix(" MB"),
                )
                .changed()
                && self.auto_save.active
                && !self.submit_auto_save(AutosaveCommand::SetLimit(self.auto_save_limit_mb))
            {
                self.auto_save_limit_mb = previous;
            }
        });
        ui.add_space(4.0);
        if ui.button("Choose save location").clicked() {
            self.choose_save_path();
        }
    }

    fn choose_save_path(&mut self) {
        if self.auto_save.pending.is_some() || self.auto_save.stopping {
            self.operation_error = Some(
                "Wait for autosave to finish queued work before changing its location.".to_owned(),
            );
            return;
        }
        let default_name = self
            .auto_save_path
            .as_ref()
            .and_then(|p| p.file_name().and_then(|n| n.to_str()))
            .unwrap_or("TraceX.log");
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log", &["log", "txt"])
            .set_file_name(default_name)
            .save_file()
        {
            let previous_path = self.auto_save_path.replace(path);
            let previous_sequence = self.auto_save_sequence;
            self.auto_save_sequence = 0;
            if self.auto_save_enabled && !self.open_new_auto_save_file() {
                self.auto_save_path = previous_path;
                self.auto_save_sequence = previous_sequence;
            }
        }
    }
}

#[cfg(test)]
#[path = "autosave_tests.rs"]
mod tests;

#[cfg(test)]
mod shutdown_tests {
    use super::*;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant};

    #[test]
    fn finish_waits_for_pending_payload_and_acknowledged_worker_exit() {
        let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())));
        let root = std::env::temp_dir().join(format!("tracex-finish-order-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        app.auto_save.worker =
            Some(AutosaveWorker::spawn_gated(1, 2, entered_tx, release_rx).unwrap());
        app.auto_save_path = Some(root.join("capture.log"));
        app.auto_save_enabled = true;
        app.apply_auto_save_enabled_change();
        entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        app.next_id = 1;
        app.append_auto_save_line("first");
        app.next_id = 2;
        app.append_auto_save_line("second");
        assert!(app.auto_save.pending.is_some());
        app.request_auto_save_finish();
        assert!(
            !app.auto_save.finish_requested,
            "Finish cannot overtake pending data"
        );
        assert!(!app.auto_save_shutdown_complete());
        release_tx.send(()).unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        while !app.auto_save_shutdown_complete() {
            assert!(
                Instant::now() < deadline,
                "worker completion timed out: {:?}",
                app.auto_save_error
            );
            app.poll_auto_save();
            app.request_auto_save_finish();
            std::thread::yield_now();
        }
        assert!(app.auto_save.finish_observed);
        assert!(app.auto_save.worker.is_none());
        assert!(app.auto_save_shutdown_failure().is_none());
        let path = app.auto_save_current_path.take().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\nsecond\n");
        app.request_shutdown(Instant::now());
        app.shutdown.keep_open();
        assert!(
            app.open_new_auto_save_file(),
            "Keep open permits a new worker after finalization"
        );
        app.next_id = 3;
        app.append_auto_save_line("after keep open");
        let deadline = Instant::now() + Duration::from_secs(3);
        while !app.auto_save_shutdown_complete() {
            assert!(Instant::now() < deadline);
            app.poll_auto_save();
            app.request_auto_save_finish();
            std::thread::yield_now();
        }
        let restarted_path = app.auto_save_current_path.take().unwrap();
        assert_ne!(restarted_path, path);
        assert_eq!(
            std::fs::read_to_string(&restarted_path).unwrap(),
            "after keep open\n"
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(restarted_path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn failed_create_is_not_reported_complete_after_finish() {
        let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())));
        let root =
            std::env::temp_dir().join(format!("tracex-finish-failure-{}", std::process::id()));
        std::fs::write(&root, b"a file cannot be a log directory").unwrap();
        app.auto_save_path = Some(root.join("capture.log"));
        app.auto_save_enabled = true;
        app.apply_auto_save_enabled_change();
        app.request_auto_save_finish();
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.auto_save.worker.is_some() {
            assert!(Instant::now() < deadline);
            app.poll_auto_save();
            std::thread::yield_now();
        }
        assert!(app.auto_save.finish_observed);
        assert!(!app.auto_save_shutdown_complete());
        assert!(app.auto_save_shutdown_failure().is_some());
        std::fs::remove_file(root).unwrap();
    }
}
