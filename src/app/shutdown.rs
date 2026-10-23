use std::time::{Duration, Instant};

// A bounded decision deadline, not a guarantee that an unresponsive device or
// filesystem can be forcibly finalized. The UI remains responsive throughout.
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Default, Debug, PartialEq, Eq)]
enum Phase {
    #[default]
    Open,
    Draining,
    Finishing,
    Decision(String),
    UnsavedSession,
    Ready,
}

#[derive(Default)]
pub(super) struct ShutdownCoordinator {
    phase: Phase,
    started: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShutdownAction {
    Wait,
    FinishAutosave,
    Close,
}

pub(super) struct ShutdownSnapshot {
    pub capture_drained: bool,
    pub autosave_complete: bool,
    pub archive_complete: bool,
    pub unsaved_history: bool,
    pub failure: Option<String>,
}

impl ShutdownCoordinator {
    pub(super) fn request(&mut self, now: Instant) -> bool {
        if self.active() {
            return false;
        }
        self.phase = Phase::Draining;
        self.started = Some(now);
        true
    }

    pub(super) fn active(&self) -> bool {
        self.phase != Phase::Open
    }

    pub(super) fn allows_close(&self) -> bool {
        self.phase == Phase::Ready
    }

    pub(super) fn decision(&self) -> Option<&str> {
        match &self.phase {
            Phase::Decision(reason) => Some(reason),
            _ => None,
        }
    }

    pub(super) fn keep_open(&mut self) {
        self.phase = Phase::Open;
        self.started = None;
    }

    pub(super) fn unsaved_session_confirmation(&self) -> bool {
        self.phase == Phase::UnsavedSession
    }

    pub(super) fn close_without_saving_session(&mut self) {
        if self.unsaved_session_confirmation() {
            self.phase = Phase::Ready;
        }
    }

    pub(super) fn force_close(&mut self) {
        self.phase = Phase::Ready;
    }

    pub(super) fn advance(&mut self, now: Instant, snapshot: ShutdownSnapshot) -> ShutdownAction {
        if self.phase == Phase::Ready {
            return ShutdownAction::Close;
        }
        if matches!(self.phase, Phase::Decision(_) | Phase::UnsavedSession) {
            if let Some(reason) = snapshot.failure {
                self.phase = Phase::Decision(reason);
            }
            return ShutdownAction::Wait;
        }
        if !matches!(self.phase, Phase::Draining | Phase::Finishing) {
            return ShutdownAction::Wait;
        }
        if let Some(reason) = snapshot.failure {
            self.phase = Phase::Decision(reason);
            return ShutdownAction::Wait;
        }
        if self
            .started
            .is_some_and(|started| now.saturating_duration_since(started) >= SHUTDOWN_DEADLINE)
        {
            self.phase = Phase::Decision("Shutdown is incomplete: capture or storage has not finished within 10 seconds. Pending logs may remain unsaved.".to_owned());
            return ShutdownAction::Wait;
        }
        match self.phase {
            Phase::Draining if snapshot.capture_drained => {
                self.phase = Phase::Finishing;
                ShutdownAction::FinishAutosave
            }
            Phase::Finishing if snapshot.autosave_complete && snapshot.archive_complete => {
                if snapshot.unsaved_history {
                    self.phase = Phase::UnsavedSession;
                    ShutdownAction::Wait
                } else {
                    self.phase = Phase::Ready;
                    ShutdownAction::Close
                }
            }
            _ => ShutdownAction::Wait,
        }
    }
}

impl super::TraceXApp {
    pub(super) fn request_shutdown(&mut self, now: Instant) {
        if self.shutdown.request(now) {
            self.disconnect();
        }
    }

    pub(super) fn advance_shutdown(&mut self, now: Instant) -> ShutdownAction {
        let mut failures = Vec::new();
        if let Some(reason) = &self.capture_error {
            failures.push(reason.clone());
        }
        if self.history_limit_reached && self.capture.is_some() {
            failures.push("Capture drain is incomplete because retained history reached its limit. Keep open to save/export and clear history before retrying close.".to_owned());
        }
        failures.extend(self.auto_save_shutdown_failure());
        failures.extend(self.session_archive_shutdown_failure());
        let action = self.shutdown.advance(
            now,
            ShutdownSnapshot {
                capture_drained: self.capture.is_none(),
                autosave_complete: self.auto_save_shutdown_complete(),
                archive_complete: !self.session_archive_busy(),
                unsaved_history: self.history_dirty && !self.logs.is_empty(),
                failure: (!failures.is_empty()).then(|| failures.join("\n\n")),
            },
        );
        if matches!(self.shutdown.phase, Phase::Finishing) {
            // Retry only until an earlier pending write makes room for Finish.
            // The controller ensures the terminal command is submitted once.
            self.request_auto_save_finish();
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(capture_drained: bool, autosave_complete: bool) -> ShutdownSnapshot {
        ShutdownSnapshot {
            capture_drained,
            autosave_complete,
            archive_complete: true,
            unsaved_history: false,
            failure: None,
        }
    }

    #[test]
    fn close_waits_for_capture_drain_then_storage_completion() {
        let now = Instant::now();
        let mut shutdown = ShutdownCoordinator::default();
        assert!(shutdown.request(now));
        assert!(!shutdown.request(now + Duration::from_secs(1)));
        assert_eq!(
            shutdown.advance(now, snapshot(false, true)),
            ShutdownAction::Wait
        );
        assert_eq!(
            shutdown.advance(now, snapshot(true, false)),
            ShutdownAction::FinishAutosave
        );
        assert_eq!(
            shutdown.advance(now, snapshot(true, false)),
            ShutdownAction::Wait
        );
        assert_eq!(
            shutdown.advance(now, snapshot(true, true)),
            ShutdownAction::Close
        );
        assert!(shutdown.allows_close());
    }

    #[test]
    fn deadline_requires_deliberate_decision_and_keep_open_resets_request() {
        let now = Instant::now();
        let mut shutdown = ShutdownCoordinator::default();
        shutdown.request(now);
        assert_eq!(
            shutdown.advance(now + SHUTDOWN_DEADLINE, snapshot(false, false)),
            ShutdownAction::Wait
        );
        assert!(shutdown.decision().is_some());
        assert!(!shutdown.allows_close());
        // Late success cannot silently dismiss the explicit decision.
        assert_eq!(
            shutdown.advance(now + SHUTDOWN_DEADLINE, snapshot(true, true)),
            ShutdownAction::Wait
        );
        shutdown.keep_open();
        assert!(!shutdown.active());
        assert!(shutdown.request(now + SHUTDOWN_DEADLINE));
    }

    #[test]
    fn save_failure_requires_explicit_force_close() {
        let now = Instant::now();
        let mut shutdown = ShutdownCoordinator::default();
        shutdown.request(now);
        let mut state = snapshot(true, true);
        state.failure = Some("Disk write failed; retained payloads are unsaved.".to_owned());
        assert_eq!(shutdown.advance(now, state), ShutdownAction::Wait);
        assert!(shutdown.decision().unwrap().contains("unsaved"));
        assert!(!shutdown.allows_close());
        shutdown.force_close();
        assert_eq!(
            shutdown.advance(now, snapshot(false, false)),
            ShutdownAction::Close
        );
    }

    #[test]
    fn text_autosave_finishes_before_unsaved_session_confirmation() {
        use std::sync::{Arc, Mutex};
        let mut app = super::super::TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())));
        let root =
            std::env::temp_dir().join(format!("tracex-orderly-close-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        app.auto_save_path = Some(root.join("capture.log"));
        app.auto_save_enabled = true;
        app.apply_auto_save_enabled_change();
        app.record_log_line("saved before close".to_owned());
        let stop = Arc::new(Mutex::new(false));
        app.stop_flag = Some(Arc::clone(&stop));
        app.last_connected_port = Some("test-device".to_owned());
        let now = Instant::now();
        app.request_shutdown(now);
        assert!(*stop.lock().unwrap());
        assert!(app.last_connected_port.is_none());
        assert!(!app.shutdown.allows_close());
        let test_deadline = now + Duration::from_secs(3);
        loop {
            assert!(
                Instant::now() < test_deadline,
                "close did not finish: {:?}",
                app.shutdown.decision()
            );
            app.poll_auto_save();
            let action = app.advance_shutdown(Instant::now());
            assert_ne!(action, ShutdownAction::Close);
            if app.shutdown.unsaved_session_confirmation() {
                break;
            }
            std::thread::yield_now();
        }
        assert!(app.auto_save_shutdown_complete());
        assert!(!app.shutdown.allows_close());
        app.shutdown.close_without_saving_session();
        assert!(app.shutdown.allows_close());
        let path = app.auto_save_current_path.take().unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("saved before close")
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn close_during_session_save_waits_for_original_history_to_return() {
        use std::sync::{Arc, Mutex};
        let mut app = super::super::TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())));
        let root =
            std::env::temp_dir().join(format!("tracex-close-session-save-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("session.tracex");
        app.record_log_line("original retained evidence".to_owned());
        app.save_session_to(path.clone());
        assert!(app.session_archive_busy());
        assert!(app.logs.is_empty(), "ownership is with the save worker");
        let now = Instant::now();
        app.request_shutdown(now);
        assert_eq!(app.advance_shutdown(now), ShutdownAction::FinishAutosave);
        assert_eq!(app.advance_shutdown(now), ShutdownAction::Wait);
        assert!(!app.shutdown.allows_close());
        let test_deadline = now + Duration::from_secs(3);
        loop {
            assert!(Instant::now() < test_deadline);
            app.poll();
            if app.advance_shutdown(Instant::now()) == ShutdownAction::Close {
                break;
            }
            std::thread::yield_now();
        }
        assert!(!app.session_archive_busy());
        assert!(!app.history_dirty);
        assert_eq!(app.logs.len(), 1);
        assert_eq!(app.logs[0].raw, "original retained evidence");
        app.shutdown.keep_open();
        app.record_log_line("new capture after save".to_owned());
        assert!(
            app.history_dirty,
            "more capture makes the saved snapshot stale"
        );
        assert!(path.exists());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn dirty_history_without_autosave_requires_explicit_session_discard() {
        use std::sync::{Arc, Mutex};
        let mut app = super::super::TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())));
        assert!(!app.history_dirty);
        app.record_log_line("only retained in memory".to_owned());
        assert!(app.history_dirty);
        assert!(!app.auto_save_enabled);
        let now = Instant::now();
        app.request_shutdown(now);
        assert_eq!(app.advance_shutdown(now), ShutdownAction::FinishAutosave);
        assert_eq!(app.advance_shutdown(now), ShutdownAction::Wait);
        assert!(app.shutdown.unsaved_session_confirmation());
        assert!(!app.shutdown.allows_close());
        app.shutdown.keep_open();
        assert_eq!(app.logs[0].raw, "only retained in memory");
        app.clear();
        assert!(!app.history_dirty);
        app.request_shutdown(now);
        app.advance_shutdown(now);
        assert_eq!(app.advance_shutdown(now), ShutdownAction::Close);
    }

    #[test]
    fn completed_autosave_still_waits_for_archive_worker() {
        let now = Instant::now();
        let mut shutdown = ShutdownCoordinator::default();
        shutdown.request(now);
        shutdown.advance(now, snapshot(true, false));
        let mut state = snapshot(true, true);
        state.archive_complete = false;
        assert_eq!(shutdown.advance(now, state), ShutdownAction::Wait);
        assert!(!shutdown.allows_close());
        assert_eq!(
            shutdown.advance(now, snapshot(true, true)),
            ShutdownAction::Close
        );
    }
}
