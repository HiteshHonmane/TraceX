use super::*;
use crate::domain::log::{LogEntry, LogLevel, SharedLog};
use std::time::{Duration, Instant};

fn directory() -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "TraceX-session-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}
fn records() -> Vec<SharedLog> {
    vec![Arc::new(LogEntry {
        id: 19,
        timestamp: "original time".into(),
        level: LogLevel::Error,
        tag: None,
        message: "custom message".into(),
        raw: "raw 🦀".into(),
    })]
}
fn app() -> TraceXApp {
    TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())))
}
fn complete(app: &mut TraceXApp) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.session_archive_busy() {
        app.poll_session_archive();
        assert!(
            Instant::now() < deadline,
            "archive worker completion timeout"
        );
        std::thread::yield_now();
    }
}
#[test]
fn save_and_open_roundtrip_restore_original_records_and_next_id() {
    let directory = directory();
    let path = directory.join("roundtrip.tracex");
    let mut app = app();
    app.logs = records();
    app.history_dirty = true;
    app.rebuild_history_accounting();
    let original = app.logs.as_ptr();
    let shared_record = Arc::clone(&app.logs[0]);
    let raw_allocation = shared_record.raw.as_ptr();
    app.save_session_to(path.clone());
    assert!(app.session_archive_busy());
    assert!(app.logs.is_empty());
    complete(&mut app);
    assert_eq!(app.logs.as_ptr(), original);
    assert!(Arc::ptr_eq(&app.logs[0], &shared_record));
    assert_eq!(app.logs[0].raw.as_ptr(), raw_allocation);
    assert!(app.session_archive_shutdown_failure().is_none());
    assert!(!app.history_dirty);
    app.logs.clear();
    app.open_session_from(path.clone());
    complete(&mut app);
    assert_eq!(app.logs[0].raw, "raw 🦀");
    assert_eq!(app.logs[0].timestamp, "original time");
    assert_eq!(app.next_id, 20);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
#[test]
fn existing_target_is_never_overwritten_and_original_history_returns() {
    let directory = directory();
    let path = directory.join("exists.tracex");
    std::fs::write(&path, "keep me").unwrap();
    let mut app = app();
    app.logs = records();
    app.history_dirty = true;
    let pointer = app.logs.as_ptr();
    let shared_record = Arc::clone(&app.logs[0]);
    app.save_session_to(path.clone());
    complete(&mut app);
    assert_eq!(app.logs.as_ptr(), pointer);
    assert!(Arc::ptr_eq(&app.logs[0], &shared_record));
    assert!(app.session_archive_shutdown_failure().is_some());
    assert!(app.history_dirty);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me");
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
#[test]
fn corrupted_or_truncated_open_preserves_current_history_and_selection() {
    let directory = directory();
    let path = directory.join("bad.tracex");
    save(path.clone(), &records()).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.pop();
    std::fs::write(&path, bytes).unwrap();
    let mut app = app();
    app.logs = records();
    app.selected.insert(19);
    app.open_session_from(path.clone());
    assert!(!app.session_archive_busy());
    assert_eq!(app.logs[0].id, 19);
    assert!(app.selected.contains(&19));
    assert!(app.operation_error.as_deref().unwrap().contains("Clear"));
    app.logs.clear();
    app.open_session_from(path.clone());
    complete(&mut app);
    assert!(app.logs.is_empty());
    assert!(app.session_archive_shutdown_failure().is_some());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
#[test]
fn rejects_nonmonotonic_ids_without_replacing_history() {
    let directory = directory();
    let path = directory.join("ids.tracex");
    let mut repeated = records();
    repeated.extend(records());
    save(path.clone(), &repeated).unwrap();
    assert!(open(path.clone()).unwrap_err().contains("IDs"));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn open_enforces_record_and_byte_budgets() {
    let directory = directory();
    let path = directory.join("budget.tracex");
    save(path.clone(), &records()).unwrap();
    assert!(
        open_with_budget(path.clone(), HistoryBudget::new(0, usize::MAX))
            .unwrap_err()
            .contains("budget")
    );
    assert!(
        open_with_budget(path.clone(), HistoryBudget::new(10, 0))
            .unwrap_err()
            .contains("budget")
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn connected_or_busy_actions_leave_existing_history_untouched() {
    let mut app = app();
    app.logs = records();
    app.connected = true;
    app.save_session_to(PathBuf::from("must-not-be-created.tracex"));
    assert!(!app.session_archive_busy());
    assert_eq!(app.logs[0].id, 19);
    app.open_session_from(PathBuf::from("does-not-exist.tracex"));
    assert!(!app.session_archive_busy());
    assert_eq!(app.logs[0].id, 19);
}

#[test]
fn asynchronous_text_export_uses_shared_query_and_returns_original_allocation() {
    let directory = directory();
    let path = directory.join("query.log");
    let mut app = app();
    app.logs = vec![
        LogEntry {
            id: 1,
            timestamp: "one".into(),
            level: LogLevel::Info,
            tag: None,
            message: "parsed body".into(),
            raw: "raw NEEDLE".into(),
        },
        LogEntry {
            id: 2,
            timestamp: "two".into(),
            level: LogLevel::Error,
            tag: None,
            message: "needle message".into(),
            raw: "other".into(),
        },
        LogEntry {
            id: 3,
            timestamp: "three".into(),
            level: LogLevel::Warn,
            tag: None,
            message: "needle hidden".into(),
            raw: "needle".into(),
        },
    ]
    .into_iter()
    .map(Arc::new)
    .collect();
    app.search = "  NeEdLe \n".into();
    app.history_dirty = true;
    app.show_warn = false;
    let pointer = app.logs.as_ptr();
    app.export_logs_to(path.clone());
    assert!(app.session_archive_busy());
    complete(&mut app);
    assert_eq!(app.logs.as_ptr(), pointer);
    assert!(app.history_dirty);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[one] [INF] parsed body\n[two] [ERR] needle message"
    );
    app.export_logs_to(path.clone());
    complete(&mut app);
    assert!(app.session_archive_shutdown_failure().is_some());
    assert_eq!(app.logs.as_ptr(), pointer);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[one] [INF] parsed body\n[two] [ERR] needle message"
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn stopped_capture_pending_data_waits_during_export_then_drains() {
    let directory = directory();
    let path = directory.join("before-drain.log");
    let mut app = app();
    app.logs = records();
    app.next_id = 20;
    app.rebuild_history_accounting();
    let (feed, inbox) = super::super::capture::CaptureInbox::test_channel();
    feed.send("INFO pending tail".into()).unwrap();
    drop(feed);
    app.capture = Some(inbox);
    app.connected = false;
    // A held completion channel proves ingestion gating without depending on
    // filesystem timing or how quickly the actual worker finishes.
    let (_held_sender, held_receiver) = mpsc::sync_channel(1);
    app.session_archive.receiver = Some(held_receiver);
    app.poll();
    assert_eq!(app.logs.len(), 1);
    assert_eq!(
        app.capture.as_mut().unwrap().peek_record(),
        Some(&"INFO pending tail".to_owned())
    );
    app.session_archive.receiver = None;
    app.export_logs_to(path.clone());
    complete(&mut app);
    app.poll();
    assert_eq!(app.logs.len(), 2);
    assert!(app.logs[1].raw.contains("pending tail"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[original time] [ERR] custom message"
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

fn capacity_stopped_app() -> TraceXApp {
    let mut app = app();
    let mut original = records();
    Arc::make_mut(&mut original[0]).tag = Some("original_tag".into());
    app.logs = original;
    app.next_id = 20;
    app.history = HistoryBudget::new(1, usize::MAX);
    app.rebuild_history_accounting();
    app.history_dirty = true;
    app.search = "nonmatching investigation filter".into();
    app.show_error = false;
    let (feed, inbox) = super::super::capture::CaptureInbox::test_channel();
    feed.send("pending original tail".into()).unwrap();
    drop(feed);
    app.capture = Some(inbox);
    app.connected = true;
    app.poll();
    assert!(!app.connected);
    assert!(app.history_limit_reached);
    app
}

#[test]
fn capacity_stop_session_snapshot_preserves_all_fields_and_leaves_pending_tail_unsaved() {
    let directory = directory();
    let path = directory.join("retained.tracex");
    let mut app = capacity_stopped_app();
    let original = Arc::clone(&app.logs[0]);
    app.save_session_to(path.clone());
    assert!(app.session_archive_busy());
    assert_eq!(
        app.capture.as_mut().unwrap().peek_record().unwrap(),
        "pending original tail"
    );
    complete(&mut app);
    assert!(Arc::ptr_eq(&app.logs[0], &original));
    assert!(!app.history_dirty, "only retained history was saved");
    let saved = open(path.clone()).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, original.id);
    assert_eq!(saved[0].timestamp, original.timestamp);
    assert_eq!(saved[0].level, original.level);
    assert_eq!(saved[0].tag, original.tag);
    assert_eq!(saved[0].message, original.message);
    assert_eq!(saved[0].raw, original.raw);
    app.poll();
    assert!(app.history_limit_reached);
    assert_eq!(app.logs.len(), 1);
    assert_eq!(
        app.capture.as_mut().unwrap().peek_record().unwrap(),
        "pending original tail"
    );
    app.clear();
    app.poll();
    assert!(app.capture.is_none());
    assert_eq!(app.logs.len(), 1);
    assert_eq!(app.logs[0].id, 20);
    assert_eq!(app.logs[0].raw, "pending original tail");
    assert!(
        app.history_dirty,
        "pending tail was outside the saved snapshot"
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn failed_capacity_snapshot_preserves_dirty_history_pending_tail_and_existing_file() {
    let directory = directory();
    let path = directory.join("existing.tracex");
    std::fs::write(&path, b"original file").unwrap();
    let mut app = capacity_stopped_app();
    let original = Arc::clone(&app.logs[0]);
    app.save_session_to(path.clone());
    assert!(app.session_archive_busy());
    complete(&mut app);
    assert!(app.session_archive_shutdown_failure().is_some());
    assert!(app.history_dirty);
    assert!(Arc::ptr_eq(&app.logs[0], &original));
    app.poll();
    assert_eq!(
        app.capture.as_mut().unwrap().peek_record().unwrap(),
        "pending original tail"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"original file");
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn session_capabilities_distinguish_stopped_pending_save_from_drained_empty_open() {
    let mut app = app();
    assert!(app.can_save_session());
    assert!(app.can_open_session());
    let (feed, inbox) = super::super::capture::CaptureInbox::test_channel();
    feed.send("unread evidence".into()).unwrap();
    drop(feed);
    app.capture = Some(inbox);
    assert!(app.can_save_session());
    assert!(!app.can_open_session());
    app.open_session_from(PathBuf::from("must-not-replace-pending.tracex"));
    assert!(!app.session_archive_busy());
    assert_eq!(
        app.capture.as_mut().unwrap().peek_record().unwrap(),
        "unread evidence"
    );
    app.poll();
    assert!(app.capture.is_none());
    assert!(
        !app.can_open_session(),
        "retained evidence requires deliberate Clear"
    );
    app.clear();
    assert!(app.can_open_session());
    app.connected = true;
    assert!(!app.can_save_session());
    assert!(!app.can_open_session());
    app.connected = false;
    app.auto_save.stopping = true;
    assert!(!app.can_save_session());
    assert!(!app.can_open_session());
    app.auto_save.stopping = false;
    app.request_shutdown(Instant::now());
    assert!(!app.can_save_session());
    assert!(!app.can_open_session());
}
