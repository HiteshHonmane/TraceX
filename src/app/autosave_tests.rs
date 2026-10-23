use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

fn app() -> SessionRuntime {
    SessionRuntime::new()
}

#[test]
fn autosave_setting_adapters_preserve_values_when_pending_work_refuses_change() {
    let mut app = app();
    let original_path = app.auto_save_path.clone();
    app.auto_save.active = true;
    app.auto_save.pending = Some(AutosaveCommand::SetLimit(25));
    assert!(!app.set_auto_save_limit_mb(50));
    assert_eq!(app.auto_save_limit_mb, 25);
    assert!(!app.set_auto_save_path(PathBuf::from("must-not-open.log")));
    assert_eq!(app.auto_save_path, original_path);
    assert!(app.auto_save_pending());
    assert!(app.auto_save.worker.is_none());
    assert!(app.operation_error.is_some());
}

#[test]
fn disabled_autosave_settings_update_without_spawning_storage() {
    let mut app = app();
    let path = PathBuf::from("configured-only.log");
    app.auto_save_sequence = 4;
    assert!(app.set_auto_save_path(path.clone()));
    assert!(app.set_auto_save_limit_mb(50));
    assert_eq!(app.auto_save_path, Some(path));
    assert_eq!(app.auto_save_sequence, 0);
    assert_eq!(app.auto_save_limit_mb, 50);
    assert!(app.auto_save.worker.is_none());
    assert_eq!(app.unsaved_payload_count(), 0);
    assert!(!app.recovery_exporting());
}

#[test]
fn autosave_limit_rejects_invalid_values_without_mutation_or_worker_creation() {
    let mut app = app();
    for invalid in [0, 4097, u64::MAX] {
        assert!(!app.set_auto_save_limit_mb(invalid));
        assert_eq!(app.auto_save_limit_mb, 25);
        assert!(app.auto_save.worker.is_none());
        assert!(
            app.operation_error
                .as_deref()
                .unwrap()
                .contains("1 and 4096")
        );
    }
    assert!(app.set_auto_save_limit_mb(1));
    assert!(app.set_auto_save_limit_mb(4096));
    assert_eq!(app.auto_save_limit_mb, 4096);
    assert!(app.auto_save.worker.is_none());
}

#[test]
fn autosave_path_restart_refusal_restores_original_path_and_sequence() {
    let mut app = app();
    let original_path = app.auto_save_path.clone();
    app.auto_save_sequence = 7;
    app.auto_save_enabled = true;
    app.auto_save.recovery.push((
        WriteRequest {
            id: 0,
            payload: "unsaved".into(),
        },
        0,
    ));
    assert!(!app.set_auto_save_path(PathBuf::from("must-not-replace.log")));
    assert_eq!(app.auto_save_path, original_path);
    assert_eq!(app.auto_save_sequence, 7);
    assert_eq!(app.unsaved_payload_count(), 1);
    assert!(app.auto_save.worker.is_none());
}
fn directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("tracex-{name}-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    directory
}
fn until(app: &mut SessionRuntime, mut ready: impl FnMut(&SessionRuntime) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready(app) {
        assert!(
            Instant::now() < deadline,
            "autosave condition timed out: {:?}",
            app.auto_save_error
        );
        app.poll();
        std::thread::yield_now();
    }
}

#[test]
fn enabling_autosave_creates_file_before_any_log_and_stop_is_ordered() {
    let mut app = app();
    let root = directory("autosave-enable");
    app.auto_save_path = Some(root.join("capture.log"));
    app.auto_save_enabled = true;
    app.apply_auto_save_enabled_change();
    assert!(app.auto_save.active);
    until(&mut app, |app| app.auto_save_current_path.is_some());
    let path = app.auto_save_current_path.clone().unwrap();
    assert!(path.exists());
    assert_eq!(fs::read(&path).unwrap(), b"");
    app.record_log_line("first".to_owned());
    app.record_log_line("second".to_owned());
    app.auto_save_enabled = false;
    app.apply_auto_save_enabled_change();
    assert!(app.auto_save.stopping);
    until(&mut app, |app| !app.auto_save.stopping);
    let saved = fs::read_to_string(&path).unwrap();
    let lines: Vec<_> = saved.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("first"));
    assert!(lines[1].ends_with("second"));
    assert_eq!(app.auto_save.last_saved_write.unwrap().0, 1);
    assert_eq!(app.auto_save_bytes_written, saved.len() as u64);
    fs::remove_file(path).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn saturated_queue_retains_one_write_and_stops_ingest_until_manual_tail_drains() {
    let mut app = app();
    let root = directory("autosave-backpressure");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    app.auto_save.worker = Some(AutosaveWorker::spawn_gated(1, 2, entered_tx, release_rx).unwrap());
    app.auto_save_path = Some(root.join("capture.log"));
    app.auto_save_enabled = true;
    app.apply_auto_save_enabled_change();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let (sender, inbox) = crate::app::capture::CaptureInbox::test_channel();
    app.capture = Some(inbox);
    app.connected = true;
    for line in ["first", "second", "final tail"] {
        sender.send(line.to_owned()).unwrap();
    }
    app.poll();
    assert_eq!(app.logs.len(), 2);
    assert!(!app.can_ingest_log());
    assert!(
        matches!(&app.auto_save.pending, Some(AutosaveCommand::Write(WriteRequest { id: 1, payload })) if payload.ends_with("second\n"))
    );
    app.auto_save_enabled = false;
    app.apply_auto_save_enabled_change();
    assert!(
        app.auto_save_enabled,
        "toggle must not overtake pending write"
    );
    assert!(app.auto_save.pending.is_some());
    app.disconnect();
    drop(sender);
    release_tx.send(()).unwrap();
    until(&mut app, |app| {
        app.capture.is_none() && !app.auto_save.stopping
    });
    assert_eq!(app.logs.len(), 3);
    assert!(app.auto_save.pending.is_none());
    let path = app.auto_save_current_path.clone().unwrap();
    let saved = fs::read_to_string(&path).unwrap();
    let lines: Vec<_> = saved.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("first"));
    assert!(lines[1].ends_with("second"));
    assert!(lines[2].ends_with("final tail"));
    fs::remove_file(path).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn failed_start_preserves_visible_payload_and_recovery_export_never_overwrites() {
    let mut app = app();
    let root = directory("autosave-recovery");
    let blocker = root.join("not-a-directory");
    fs::write(&blocker, "keep this file").unwrap();
    app.auto_save_path = Some(blocker.join("capture.log"));
    app.auto_save_enabled = true;
    app.apply_auto_save_enabled_change();
    app.record_log_line("retained payload".to_owned());
    until(&mut app, |app| {
        app.auto_save.failed && app.auto_save.recovery.len() == 1
    });
    assert_eq!(app.logs[0].raw, "retained payload");
    assert!(!app.can_ingest_log());
    assert!(app.auto_save_error.is_some());
    let exported = root.join("recovery.json");
    fs::write(&exported, "existing recovery").unwrap();
    app.export_recovery_to(exported.clone());
    until(&mut app, |app| !app.auto_save.recovery_exporting);
    assert_eq!(app.auto_save.recovery.len(), 1);
    assert_eq!(fs::read_to_string(&exported).unwrap(), "existing recovery");
    fs::remove_file(&exported).unwrap();
    app.export_recovery_to(exported.clone());
    until(&mut app, |app| !app.auto_save.recovery_exporting);
    assert!(app.auto_save.recovery.is_empty());
    let recovery: serde_json::Value =
        serde_json::from_slice(&fs::read(&exported).unwrap()).unwrap();
    assert!(
        recovery["records"][0]["request"]["payload"]
            .as_str()
            .unwrap()
            .ends_with("retained payload\n")
    );
    assert_eq!(recovery["records"][0]["written_prefix_bytes"], 0);
    assert!(recovery["notice"].as_str().unwrap().contains("duplicate"));
    app.auto_save_enabled = false;
    app.apply_auto_save_enabled_change();
    assert!(app.can_ingest_log());
    fs::remove_file(blocker).unwrap();
    fs::remove_file(exported).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn failed_pending_write_stays_in_fifo_until_earlier_failures_are_returned() {
    let mut app = app();
    let root = directory("autosave-failure-order");
    let blocker = root.join("blocked");
    fs::write(&blocker, "not a directory").unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    app.auto_save.worker = Some(AutosaveWorker::spawn_gated(1, 1, entered_tx, release_rx).unwrap());
    app.auto_save_path = Some(blocker.join("capture.log"));
    app.auto_save_enabled = true;
    app.apply_auto_save_enabled_change();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let (sender, inbox) = crate::app::capture::CaptureInbox::test_channel();
    app.capture = Some(inbox);
    sender.send("earlier".to_owned()).unwrap();
    sender.send("later".to_owned()).unwrap();
    app.poll();
    assert!(matches!(
        &app.auto_save.pending,
        Some(AutosaveCommand::Write(WriteRequest { id: 1, .. }))
    ));
    // Model an observed storage failure while earlier accepted requests remain
    // queued. The pending request must not bypass those requests into recovery.
    app.auto_save.failed = true;
    app.poll_auto_save();
    assert!(app.auto_save.recovery.is_empty());
    assert!(app.auto_save.pending.is_some());
    release_tx.send(()).unwrap();
    until(&mut app, |app| app.auto_save.recovery.len() == 2);
    assert_eq!(
        app.auto_save
            .recovery
            .iter()
            .map(|(request, _)| request.id)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(app.auto_save.pending.is_none());
    fs::remove_file(blocker).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn stalled_storage_causes_bounded_capture_stop_then_saves_every_received_record() {
    use std::collections::VecDeque;
    use std::io::{self, Read, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct ReadBatches {
        chunks: VecDeque<Vec<u8>>,
        reads: Arc<AtomicUsize>,
    }
    impl Read for ReadBatches {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let bytes = self.chunks.pop_front().unwrap();
            output[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
    }
    impl Write for ReadBatches {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut app = app();
    let root = directory("capture-storage-stall");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    app.auto_save.worker = Some(AutosaveWorker::spawn_gated(1, 1, entered_tx, release_rx).unwrap());
    app.auto_save_path = Some(root.join("capture.log"));
    app.auto_save_enabled = true;
    app.apply_auto_save_enabled_change();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let reads = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(Mutex::new(false));
    let budget = crate::app::capture::CaptureBudget {
        queued_batches: 1,
        ..crate::app::capture::CaptureBudget::default()
    };
    let (inbox, commands) = crate::app::capture::start_reader(
        ReadBatches {
            chunks: VecDeque::from([
                b"first\n".to_vec(),
                b"second\nfinal tail".to_vec(),
                b"must not read\n".to_vec(),
            ]),
            reads: Arc::clone(&reads),
        },
        budget,
        Arc::clone(&stop),
        0,
    )
    .unwrap();
    app.capture = Some(inbox);
    app.serial_tx = Some(commands);
    app.stop_flag = Some(stop);
    app.connected = true;
    app.last_connected_port = Some("fake".to_owned());
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let inbox = app.capture.as_mut().unwrap();
        inbox.poll_status();
        if inbox.terminal_reason().is_some() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert!(app.capture.as_ref().unwrap().usage().bytes <= budget.queued_bytes);
    app.poll();
    assert!(app.auto_save.pending.is_some());
    assert_eq!(app.logs.len(), 2);
    assert!(app.last_connected_port.is_none());
    assert!(
        app.capture_error
            .as_deref()
            .unwrap()
            .contains("could not keep up")
    );
    app.disconnect();
    assert_eq!(
        app.capture.as_ref().unwrap().terminal_reason(),
        Some(&crate::app::capture::CaptureStopReason::Saturated)
    );
    release_tx.send(()).unwrap();
    until(&mut app, |app| {
        app.capture.is_none() && !app.auto_save.stopping
    });
    assert_eq!(
        app.logs
            .iter()
            .map(|entry| entry.raw.as_str())
            .collect::<Vec<_>>(),
        ["first", "second", "final tail"]
    );
    let path = app.auto_save_current_path.clone().unwrap();
    let saved = fs::read_to_string(&path).unwrap();
    let lines: Vec<_> = saved.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("first"));
    assert!(lines[1].ends_with("second"));
    assert!(lines[2].ends_with("final tail"));
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    fs::remove_file(path).unwrap();
    fs::remove_dir(root).unwrap();
}
