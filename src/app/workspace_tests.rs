//! Real Workspace routing/lifetime regressions using controlled workers, no UART.
use super::*;
use crate::app::{capture::CaptureInbox, query_worker::QueryWorker};
use crate::domain::history_budget::HistoryBudget;
use crate::domain::{log::LogEntry, session_format::SegmentWriter};
use std::{
    cell::Cell,
    fs::OpenOptions,
    io::{self, Read, Write},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

fn pair() -> (Workspace, SessionId, SessionId) {
    let mut workspace = Workspace::new();
    let a = workspace.active_session_id();
    let b = workspace.create_session().unwrap();
    for runtime in &mut workspace.sessions {
        runtime.auto_save_path = None;
    }
    (workspace, a, b)
}

fn spin_until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "controlled worker did not finish"
        );
        thread::yield_now();
    }
}

fn poll_until(workspace: &mut Workspace, now: Instant, ready: impl Fn(&Workspace) -> bool) {
    spin_until(|| {
        workspace.poll(now, &[]);
        ready(workspace)
    });
}

fn seed_query(workspace: &mut Workspace, id: SessionId, marker: &str, needle_first: bool) {
    let runtime = workspace.session_mut(id).unwrap();
    for has_needle in [needle_first, !needle_first] {
        runtime.record_log_line(format!(
            "INFO {marker} {}",
            if has_needle { "needle" } else { "other" }
        ));
    }
    runtime.search = "needle".into();
}

struct QueryGate {
    entered: mpsc::Receiver<u64>,
    release: Option<mpsc::SyncSender<()>>,
}
impl QueryGate {
    fn install(workspace: &mut Workspace, id: SessionId) -> Self {
        let (entered_tx, entered) = mpsc::sync_channel(1);
        let (release, release_rx) = mpsc::sync_channel(1);
        let mut first = true;
        let worker = QueryWorker::spawn_test_hook(move |generation| {
            if first {
                first = false;
                entered_tx.send(generation).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        })
        .unwrap();
        workspace
            .session_mut(id)
            .unwrap()
            .query_view
            .install_test_worker(worker);
        Self {
            entered,
            release: Some(release),
        }
    }
    fn wait_entered(&self) {
        self.entered.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    fn release(&mut self) {
        if let Some(release) = self.release.take() {
            release.send(()).unwrap();
        }
    }
}
impl Drop for QueryGate {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.try_send(());
        }
    }
}

// Reader waits on a bounded channel timeout; it never opens a physical endpoint.
struct SilentStream {
    tick: mpsc::Receiver<()>,
    written: Arc<Mutex<Vec<u8>>>,
}
impl Read for SilentStream {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        let _ = self.tick.recv_timeout(Duration::from_millis(2));
        Err(io::Error::from(io::ErrorKind::TimedOut))
    }
}
impl Write for SilentStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.written.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn silent_stream() -> (SilentStream, mpsc::Sender<()>, Arc<Mutex<Vec<u8>>>) {
    let (keepalive, tick) = mpsc::channel();
    let written = Arc::new(Mutex::new(Vec::new()));
    (
        SilentStream {
            tick,
            written: Arc::clone(&written),
        },
        keepalive,
        written,
    )
}

struct ArchiveFile(PathBuf);
impl ArchiveFile {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "TraceX-workspace-open-{}-{:?}",
            std::process::id(),
            thread::current().id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("owner-A.tracex");
        {
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            let mut writer = SegmentWriter::new(file).unwrap();
            for (id, raw) in ["INFO A-new other", "INFO A-new needle"]
                .into_iter()
                .enumerate()
            {
                writer
                    .append(&LogEntry::new(id, raw.into(), "original file time"))
                    .unwrap();
            }
            writer.flush().unwrap();
        }
        Self(path)
    }
}
impl Drop for ArchiveFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        if let Some(directory) = self.0.parent() {
            let _ = std::fs::remove_dir(directory);
        }
    }
}

#[test]
fn capacity_and_exhausted_id_rejections_preserve_all_existing_owners() {
    let mut workspace = Workspace::new();
    let a = workspace.active_session_id();
    let (_feed, inbox) = CaptureInbox::test_channel();
    {
        let owner = workspace.session_mut(a).unwrap();
        owner.record_log_line("A evidence before opening another session".into());
        owner.command_input = "A draft".into();
        owner.selected.insert(0);
        owner.capture = Some(inbox);
    }
    let original_rows = workspace.session(a).unwrap().logs.as_ptr();
    let b = workspace.create_session().unwrap();
    assert_eq!(workspace.session(a).unwrap().logs.as_ptr(), original_rows);
    assert_eq!(
        workspace.session(a).unwrap().logs[0].raw,
        "A evidence before opening another session"
    );
    assert!(workspace.session(a).unwrap().capture.is_some());
    assert!(workspace.session(a).unwrap().selected.contains(&0));
    workspace.session_mut(b).unwrap().command_input = "B draft".into();
    let next = workspace.next_session_id;
    assert_eq!(
        workspace.create_session(),
        Err(WorkspaceError::SessionLimit)
    );
    assert_eq!(workspace.next_session_id, next);
    assert_eq!(workspace.active_session_id(), b);
    assert_eq!(workspace.sessions().len(), 2);
    assert_eq!(workspace.session(a).unwrap().command_input, "A draft");
    assert_eq!(workspace.session(b).unwrap().command_input, "B draft");

    let mut exhausted = Workspace::new();
    let original = exhausted.active_session_id();
    exhausted.next_session_id = None;
    {
        let owner = exhausted.session_mut(original).unwrap();
        owner.command_input = "keep me".into();
        owner.record_log_line("preserved saved evidence".into());
        owner.history_dirty = false;
    }
    assert_eq!(
        exhausted.create_session(),
        Err(WorkspaceError::SessionIdExhausted)
    );
    assert_eq!(exhausted.sessions().len(), 1);
    assert_eq!(exhausted.active_session_id(), original);
    let now = Instant::now();
    let obsolete_close = exhausted.request_close_session(original, now).unwrap();
    exhausted.poll(now, &[]);
    exhausted.poll(now, &[]);
    assert_eq!(
        exhausted.sessions().len(),
        1,
        "reserve replacement before removal"
    );
    assert_eq!(
        exhausted.session(original).unwrap().command_input,
        "keep me"
    );
    assert!(!exhausted.session(original).unwrap().shutdown.active());
    assert!(exhausted.close_target(original).is_none());
    assert!(exhausted.session(original).unwrap().can_save_session());
    assert_eq!(
        exhausted.close_without_saving_session(obsolete_close),
        Err(WorkspaceError::StaleCloseTarget)
    );
    assert_eq!(
        exhausted.session(original).unwrap().logs[0].raw,
        "preserved saved evidence"
    );
    assert!(exhausted.operation_error.is_some());

    workspace.next_close_request = Some(u64::MAX);
    assert_eq!(
        workspace.request_window_close(now),
        Err(WorkspaceError::CloseRequestExhausted)
    );
    assert!(!workspace.window_closing());
    assert_eq!(workspace.next_close_request, Some(u64::MAX));
    assert!(workspace.close_requests.is_empty());
    assert!(workspace.sessions().iter().all(|s| !s.shutdown.active()));
}

#[test]
fn inactive_capture_backpressure_does_not_stop_another_owners_progress() {
    let (mut workspace, a, b) = pair();
    let (a_feed, a_inbox) = CaptureInbox::test_channel();
    let (b_feed, b_inbox) = CaptureInbox::test_channel();
    workspace.session_mut(a).unwrap().capture = Some(a_inbox);
    workspace.session_mut(a).unwrap().history = HistoryBudget::new(0, usize::MAX);
    workspace.session_mut(b).unwrap().capture = Some(b_inbox);
    workspace.session_mut(b).unwrap().command_input = "B draft".into();
    workspace.session_mut(b).unwrap().search = "B-only".into();
    a_feed.send("A-only retained tail".into()).unwrap();
    b_feed.send("INFO B-only first".into()).unwrap();
    workspace.select_session(b).unwrap();
    let now = Instant::now();
    assert_eq!(workspace.poll(now, &[]), 1);
    assert!(workspace.session(a).unwrap().history_limit_reached);
    assert!(workspace.session(a).unwrap().capture_error.is_some());
    assert_eq!(
        workspace
            .session_mut(a)
            .unwrap()
            .capture
            .as_mut()
            .unwrap()
            .peek_record()
            .unwrap(),
        "A-only retained tail"
    );
    b_feed.send("WARN B-only second".into()).unwrap();
    assert_eq!(workspace.poll(now, &[]), 1);
    let owner = workspace.session(b).unwrap();
    assert_eq!(owner.logs.len(), 2);
    assert!(owner.logs.iter().all(|row| row.raw.contains("B-only")));
    assert!(owner.history_dirty);
    assert_eq!(owner.command_input, "B draft");
    assert_eq!(owner.search, "B-only");
}

#[test]
fn frame_budget_is_fair_for_two_backlogs_and_reuses_an_idle_owners_share() {
    for both_backlogged in [true, false] {
        let (mut workspace, a, b) = pair();
        let (a_feed, a_inbox) = CaptureInbox::test_channel();
        let (b_feed, b_inbox) = CaptureInbox::test_channel();
        workspace.session_mut(a).unwrap().capture = Some(a_inbox);
        workspace.session_mut(b).unwrap().capture = Some(b_inbox);
        a_feed
            .send_batch((0..300).map(|i| format!("A-only {i}")).collect())
            .unwrap();
        let held = if both_backlogged {
            b_feed
                .send_batch((0..300).map(|i| format!("B-only {i}")).collect())
                .unwrap();
            None
        } else {
            Some(
                workspace
                    .session_mut(b)
                    .unwrap()
                    .hold_session_save_for_test(),
            )
        };
        let now = Instant::now();
        assert_eq!(workspace.poll(now, &[]), 256);
        if both_backlogged {
            assert_eq!(workspace.session(a).unwrap().logs.len(), 128);
            assert_eq!(workspace.session(b).unwrap().logs.len(), 128);
        } else {
            assert_eq!(workspace.session(a).unwrap().logs.len(), 256);
            assert!(workspace.session(b).unwrap().logs.is_empty());
            assert!(workspace.session(b).unwrap().session_archive_busy());
        }
        if let Some(held) = held {
            held.complete(Ok(())).unwrap();
        }
        poll_until(&mut workspace, now, |w| {
            w.session(a).unwrap().logs.len() == 300
                && (!both_backlogged || w.session(b).unwrap().logs.len() == 300)
        });
        for (index, row) in workspace.session(a).unwrap().logs.iter().enumerate() {
            assert_eq!(row.raw, format!("A-only {index}"));
        }
    }
}

#[test]
fn inactive_archive_completion_and_pending_tail_stay_with_the_original_owner() {
    for success in [true, false] {
        let (mut workspace, a, b) = pair();
        {
            let owner = workspace.session_mut(a).unwrap();
            owner.history = HistoryBudget::new(2, usize::MAX);
            owner.record_log_line("INFO A-only first".into());
            owner.record_log_line("ERROR A-only second".into());
            owner.logs.reserve(1);
            owner.search = "nonmatching display filter".into();
            owner.show_error = false;
        }
        let original_allocation = workspace.session(a).unwrap().logs.as_ptr();
        let original_entry = Arc::clone(&workspace.session(a).unwrap().logs[0]);
        let held = workspace
            .session_mut(a)
            .unwrap()
            .hold_session_save_for_test();
        let (a_feed, a_inbox) = CaptureInbox::test_channel();
        let (b_feed, b_inbox) = CaptureInbox::test_channel();
        a_feed.send("INFO A-only pending tail".into()).unwrap();
        b_feed
            .send_batch((0..300).map(|i| format!("INFO B-only {i}")).collect())
            .unwrap();
        workspace.session_mut(a).unwrap().capture = Some(a_inbox);
        workspace.session_mut(b).unwrap().capture = Some(b_inbox);
        workspace.session_mut(b).unwrap().command_input = "B draft".into();
        workspace.session_mut(a).unwrap().operation_error = None;
        assert_eq!(
            workspace.clear_session(a),
            Err(WorkspaceError::OperationRejected)
        );
        assert!(
            workspace
                .session(a)
                .unwrap()
                .operation_error
                .as_deref()
                .is_some_and(|error| error.to_ascii_lowercase().contains("clear")),
            "busy Clear must report its refusal"
        );
        workspace.select_session(b).unwrap();
        let now = Instant::now();
        assert_eq!(workspace.poll(now, &[]), 256);
        assert!(workspace.session(a).unwrap().logs.is_empty());
        assert!(workspace.session(a).unwrap().session_archive_busy());
        assert_eq!(workspace.session(b).unwrap().logs.len(), 256);
        held.complete(if success {
            Ok(())
        } else {
            Err("A archive failed".into())
        })
        .unwrap();
        workspace.poll(now, &[]);
        let owner = workspace.session(a).unwrap();
        assert_eq!(owner.logs.as_ptr(), original_allocation);
        assert!(Arc::ptr_eq(&owner.logs[0], &original_entry));
        assert_eq!(owner.logs.len(), 2);
        assert_eq!(owner.history_dirty, !success);
        assert!(owner.history_limit_reached);
        assert_eq!(owner.session_archive_shutdown_failure().is_some(), !success);
        assert_eq!(workspace.session(b).unwrap().logs.len(), 300);
        assert!(workspace.session(b).unwrap().history_dirty);
        assert_eq!(workspace.session(b).unwrap().command_input, "B draft");
        assert!(workspace.session(b).unwrap().operation_error.is_none());
        assert_eq!(
            workspace
                .session_mut(a)
                .unwrap()
                .capture
                .as_mut()
                .unwrap()
                .peek_record()
                .unwrap(),
            "INFO A-only pending tail"
        );
        if success {
            workspace.clear_session(a).unwrap();
            workspace.poll(now, &[]);
            assert_eq!(
                workspace.session(a).unwrap().logs[0].raw,
                "INFO A-only pending tail"
            );
            assert!(workspace.session(a).unwrap().history_dirty);
        }
    }
}

#[test]
fn endpoint_claim_and_same_endpoint_reconnect_validate_the_original_command_target() {
    let (mut workspace, a, b) = pair();
    workspace.session_mut(a).unwrap().selected_port = "TEST-ENDPOINT".into();
    workspace.session_mut(b).unwrap().selected_port = "test-endpoint".into();
    workspace.session_mut(b).unwrap().command_input = "B untouched".into();
    let (old_stream, _old_keepalive, old_bytes) = silent_stream();
    workspace
        .connect_using(a, |runtime| runtime.start_capture(old_stream))
        .unwrap();
    let old_target = workspace.command_target(a).unwrap();
    let opens = Cell::new(0);
    assert_eq!(
        workspace.connect_using(b, |_| {
            opens.set(opens.get() + 1);
            false
        }),
        Err(WorkspaceError::EndpointInUse(a))
    );
    assert_eq!(opens.get(), 0, "reject before opening any transport");
    workspace.disconnect(a).unwrap();
    assert_eq!(
        workspace.connect_using(b, |_| {
            opens.set(opens.get() + 1);
            false
        }),
        Err(WorkspaceError::EndpointInUse(a)),
        "stopped inbox still owns endpoint until drained"
    );
    assert_eq!(opens.get(), 0);
    let now = Instant::now();
    poll_until(&mut workspace, now, |w| {
        w.session(a).unwrap().capture.is_none()
    });
    let (new_stream, _new_keepalive, new_bytes) = silent_stream();
    workspace
        .connect_using(a, |runtime| runtime.start_capture(new_stream))
        .unwrap();
    let new_target = workspace.command_target(a).unwrap();
    assert_ne!(new_target.connection, old_target.connection);
    workspace.select_session(b).unwrap();
    assert_eq!(
        workspace.submit_command(old_target, "stale"),
        Err(WorkspaceError::StaleCommandTarget)
    );
    assert_eq!(workspace.session(a).unwrap().command_input, "stale");
    assert_eq!(workspace.session(b).unwrap().command_input, "B untouched");
    assert!(old_bytes.lock().unwrap().is_empty());
    assert!(new_bytes.lock().unwrap().is_empty());
    workspace.submit_command(new_target, "A-first").unwrap();
    workspace.submit_command(new_target, "A-second").unwrap();
    poll_until(&mut workspace, now, |_| {
        new_bytes.lock().unwrap().len() == b"A-first\r\nA-second\r\n".len()
    });
    assert_eq!(*new_bytes.lock().unwrap(), b"A-first\r\nA-second\r\n");
    assert_eq!(workspace.session(b).unwrap().command_input, "B untouched");
    workspace.disconnect(a).unwrap();
    poll_until(&mut workspace, now, |w| {
        w.session(a).unwrap().capture.is_none()
    });
    workspace.session_mut(a).unwrap().next_connection_id = None;
    assert_eq!(
        workspace.connect_using(a, |_| {
            opens.set(opens.get() + 1);
            false
        }),
        Err(WorkspaceError::OperationRejected)
    );
    assert_eq!(opens.get(), 0);
    assert!(
        workspace
            .session(a)
            .unwrap()
            .operation_error
            .as_deref()
            .unwrap()
            .contains("identifiers")
    );
}

#[test]
fn queue_refusal_retains_origin_draft_and_never_uses_the_active_sender() {
    let (mut workspace, a, b) = pair();
    let (a_tx, a_rx) = mpsc::sync_channel(1);
    let (b_tx, b_rx) = mpsc::sync_channel(1);
    for (id, sender) in [(a, a_tx), (b, b_tx)] {
        let runtime = workspace.session_mut(id).unwrap();
        runtime.connected = true;
        runtime.connection_id = Some(ConnectionId(7));
        runtime.serial_tx = Some(sender);
    }
    workspace.session_mut(b).unwrap().command_input = "B draft".into();
    workspace.session_mut(b).unwrap().auto_clear_command = false;
    workspace.session_mut(b).unwrap().send_newline = false;
    let a_target = workspace.command_target(a).unwrap();
    let b_target = workspace.command_target(b).unwrap();
    workspace.select_session(b).unwrap();
    workspace.submit_command(a_target, "A-first").unwrap();
    assert_eq!(
        workspace.submit_command(a_target, "A-retry"),
        Err(WorkspaceError::OperationRejected)
    );
    assert_eq!(workspace.session(a).unwrap().command_input, "A-retry");
    assert!(
        workspace
            .session(a)
            .unwrap()
            .operation_error
            .as_deref()
            .unwrap()
            .contains("full")
    );
    assert_eq!(workspace.session(b).unwrap().command_input, "B draft");
    assert!(workspace.session(b).unwrap().command_history.is_empty());
    workspace.submit_command(b_target, "B-only").unwrap();
    assert_eq!(a_rx.try_recv().unwrap(), b"A-first\r\n");
    assert_eq!(b_rx.try_recv().unwrap(), b"B-only");
    workspace.submit_command(a_target, "A-retry").unwrap();
    assert_eq!(a_rx.try_recv().unwrap(), b"A-retry\r\n");
    assert!(workspace.session(a).unwrap().command_input.is_empty());
    assert_eq!(workspace.session(b).unwrap().command_input, "B draft");
    assert!(b_rx.try_recv().is_err());
}

#[test]
fn equal_generation_query_completions_are_applied_to_the_inactive_origin_only() {
    let (mut workspace, a, b) = pair();
    seed_query(&mut workspace, a, "A", true);
    seed_query(&mut workspace, b, "B", false);
    workspace.session_mut(a).unwrap().selected.insert(0);
    workspace.session_mut(b).unwrap().selected.insert(1);
    let now = Instant::now();
    workspace.select_session(a).unwrap();
    workspace.poll(now, &[]);
    spin_until(|| {
        workspace
            .session(a)
            .unwrap()
            .query_view
            .test_result_waiting()
    });
    workspace.select_session(b).unwrap();
    workspace.poll(now, &[]);
    let a_view = workspace.session(a).unwrap().query_snapshot().unwrap();
    assert_eq!(a_view.rows, [0]);
    poll_until(&mut workspace, now, |w| {
        w.session(b).unwrap().query_snapshot().is_some()
    });
    let b_view = workspace.session(b).unwrap().query_snapshot().unwrap();
    assert_eq!(a_view.generation, b_view.generation);
    assert_eq!(a_view.record_count, b_view.record_count);
    assert_eq!(b_view.rows, [1]);
    assert_eq!(workspace.active_session_id(), b);
    workspace.clear_session(b).unwrap();
    workspace.poll(now, &[]);
    assert!(Arc::ptr_eq(
        &a_view,
        &workspace.session(a).unwrap().query_snapshot().unwrap()
    ));
    assert!(workspace.session(a).unwrap().selected.contains(&0));
    assert!(workspace.session(b).unwrap().selected.is_empty());
}

#[test]
fn inactive_open_completion_replaces_only_its_owner_and_rejects_old_query_indices() {
    let (mut workspace, a, b) = pair();
    let file = ArchiveFile::new();
    seed_query(&mut workspace, a, "A-old", true);
    let now = Instant::now();
    workspace.select_session(a).unwrap();
    workspace.poll(now, &[]);
    spin_until(|| {
        workspace
            .session(a)
            .unwrap()
            .query_view
            .test_result_waiting()
    });
    workspace.select_session(b).unwrap();
    workspace.clear_session(a).unwrap();
    let (feed, inbox) = CaptureInbox::test_channel();
    feed.send("WARN B-only other".into()).unwrap();
    feed.send("WARN B-only needle".into()).unwrap();
    workspace.session_mut(b).unwrap().capture = Some(inbox);
    workspace.session_mut(b).unwrap().command_input = "B draft".into();
    workspace.session_mut(b).unwrap().search = "needle".into();
    workspace.open_session_from(a, file.0.clone()).unwrap();
    poll_until(&mut workspace, now, |w| {
        !w.session(a).unwrap().session_archive_busy()
    });
    let owner = workspace.session(a).unwrap();
    assert_eq!(owner.logs.len(), 2);
    assert!(
        owner
            .logs
            .iter()
            .all(|row| row.raw.contains("A-new") && row.timestamp == "original file time")
    );
    assert_eq!(owner.next_id, 2);
    assert_eq!(owner.source_path.as_ref(), Some(&file.0));
    assert!(!owner.history_dirty);
    assert!(owner.query_snapshot().is_none());
    assert_eq!(workspace.session(b).unwrap().logs.len(), 2);
    assert!(workspace.session(b).unwrap().history_dirty);
    assert_eq!(workspace.session(b).unwrap().command_input, "B draft");
    workspace.select_session(a).unwrap();
    poll_until(&mut workspace, now, |w| {
        w.session(a).unwrap().query_snapshot().is_some()
    });
    assert_eq!(
        workspace.session(a).unwrap().query_snapshot().unwrap().rows,
        [1]
    );
}

#[test]
fn inactive_clear_equal_length_replacement_never_installs_an_old_query_result() {
    let (mut workspace, a, b) = pair();
    seed_query(&mut workspace, a, "A-old", true);
    seed_query(&mut workspace, b, "B", false);
    let now = Instant::now();
    workspace.select_session(a).unwrap();
    workspace.poll(now, &[]);
    spin_until(|| {
        workspace
            .session(a)
            .unwrap()
            .query_view
            .test_result_waiting()
    });
    workspace.select_session(b).unwrap();
    workspace.clear_session(a).unwrap();
    seed_query(&mut workspace, a, "A-new", false);
    workspace.poll(now, &[]);
    assert!(workspace.session(a).unwrap().query_snapshot().is_none());
    poll_until(&mut workspace, now, |w| {
        w.session(b).unwrap().query_snapshot().is_some()
    });
    assert!(workspace.session(a).unwrap().query_snapshot().is_none());
    assert_eq!(
        workspace.session(b).unwrap().query_snapshot().unwrap().rows,
        [1]
    );
    workspace.select_session(a).unwrap();
    poll_until(&mut workspace, now, |w| {
        w.session(a).unwrap().query_snapshot().is_some()
    });
    assert_eq!(
        workspace.session(a).unwrap().query_snapshot().unwrap().rows,
        [1]
    );
    assert_eq!(
        workspace.session(b).unwrap().query_snapshot().unwrap().rows,
        [1]
    );
    assert!(
        workspace
            .session(a)
            .unwrap()
            .logs
            .iter()
            .all(|row| row.raw.contains("A-new"))
    );
}

#[test]
fn pending_inactive_capture_has_named_close_failure_and_global_force_is_explicit() {
    let (mut workspace, a, b) = pair();
    let (a_feed, a_inbox) = CaptureInbox::test_channel();
    let (b_feed, b_inbox) = CaptureInbox::test_channel();
    let stopped = Arc::new(Mutex::new(false));
    {
        let owner = workspace.session_mut(a).unwrap();
        owner.capture = Some(a_inbox);
        owner.connected = true;
        owner.stop_flag = Some(Arc::clone(&stopped));
    }
    workspace.session_mut(b).unwrap().capture = Some(b_inbox);
    a_feed.send("A-only tail".into()).unwrap();
    b_feed.send("B-only first".into()).unwrap();
    let now = Instant::now();
    let target = workspace.request_close_session(a, now).unwrap();
    workspace.poll(now, &[]);
    assert!(*stopped.lock().unwrap());
    assert!(workspace.session(a).unwrap().capture.is_some());
    assert_eq!(
        workspace.close_without_saving_session(target),
        Err(WorkspaceError::OperationRejected)
    );
    b_feed.send("B-only second".into()).unwrap();
    workspace.poll(now + Duration::from_secs(11), &[]);
    let prompt = workspace.close_prompt().unwrap();
    assert_eq!(prompt.target, target);
    assert_eq!(prompt.title, workspace.session(a).unwrap().title);
    assert!(matches!(prompt.kind, ClosePromptKind::Incomplete(_)));
    assert_eq!(workspace.session(b).unwrap().logs.len(), 2);
    workspace.keep_session_open(target).unwrap();
    assert!(!workspace.session(a).unwrap().shutdown.active());
    assert_eq!(workspace.session(a).unwrap().logs[0].raw, "A-only tail");
    workspace
        .request_window_close(now + Duration::from_secs(12))
        .unwrap();
    workspace.poll(now + Duration::from_secs(23), &[]);
    assert!(!workspace.window_allows_close());
    let force_target = workspace.close_prompt().unwrap().target;
    assert_eq!(force_target.session, a);
    workspace.force_close_window(force_target).unwrap();
    assert!(workspace.window_allows_close());
    assert_eq!(
        workspace.sessions().len(),
        2,
        "force permission is not owner removal"
    );
    assert!(workspace.session(a).unwrap().history_dirty);
    assert!(workspace.session(b).unwrap().history_dirty);
    workspace.keep_window_open();
    assert!(!workspace.window_allows_close());
    assert_eq!(
        workspace.force_close_window(force_target),
        Err(WorkspaceError::StaleCloseTarget)
    );
    drop(a_feed);
    drop(b_feed);
    poll_until(&mut workspace, now, |w| {
        w.sessions().iter().all(|s| s.capture.is_none())
    });
    assert!(workspace.sessions().iter().all(|s| !s.shutdown.active()));
}

#[test]
fn dirty_inactive_close_decisions_stay_named_and_stale_decisions_cannot_discard() {
    let (mut workspace, a, b) = pair();
    workspace
        .session_mut(a)
        .unwrap()
        .record_log_line("A-only unsaved".into());
    workspace
        .session_mut(b)
        .unwrap()
        .record_log_line("B-only unsaved".into());
    workspace.session_mut(b).unwrap().command_input = "B draft".into();
    let now = Instant::now();
    let first = workspace.request_close_session(a, now).unwrap();
    poll_until(&mut workspace, now, |w| {
        w.session(a)
            .unwrap()
            .shutdown
            .unsaved_session_confirmation()
    });
    let prompt = workspace.close_prompt().unwrap();
    assert_eq!(prompt.target, first);
    assert_eq!(prompt.title, workspace.session(a).unwrap().title);
    assert_eq!(prompt.kind, ClosePromptKind::UnsavedSession);
    assert!(!prompt.window);
    assert_eq!(workspace.active_session_id(), b);
    workspace.keep_session_open(first).unwrap();
    assert_eq!(workspace.session(a).unwrap().logs[0].raw, "A-only unsaved");
    let second = workspace.request_close_session(a, now).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        workspace.close_without_saving_session(first),
        Err(WorkspaceError::StaleCloseTarget)
    );
    poll_until(&mut workspace, now, |w| {
        w.session(a)
            .unwrap()
            .shutdown
            .unsaved_session_confirmation()
    });
    workspace.close_without_saving_session(second).unwrap();
    poll_until(&mut workspace, now, |w| w.session(a).is_none());
    assert_eq!(
        workspace.submit_command(
            CommandTarget {
                session: a,
                connection: ConnectionId(1)
            },
            "obsolete A command"
        ),
        Err(WorkspaceError::StaleCommandTarget)
    );
    assert_eq!(workspace.session(b).unwrap().logs[0].raw, "B-only unsaved");
    assert!(workspace.session(b).unwrap().history_dirty);
    assert_eq!(workspace.session(b).unwrap().command_input, "B draft");
    assert!(!workspace.window_allows_close());
}

#[test]
fn query_workers_finish_before_owner_removal_or_capacity_reuse() {
    let (mut workspace, _, b) = pair();
    // Reuse the spare owner slot repeatedly; a held old worker must count.
    let mut a = workspace.sessions()[0].id;
    for cycle in 0..3 {
        seed_query(&mut workspace, a, "A", true);
        workspace.session_mut(a).unwrap().history_dirty = false;
        let mut gate = QueryGate::install(&mut workspace, a);
        let now = Instant::now();
        workspace.select_session(a).unwrap();
        workspace.poll(now, &[]);
        gate.wait_entered();
        workspace.select_session(b).unwrap();
        workspace.request_close_session(a, now).unwrap();
        workspace.poll(now, &[]);
        workspace.poll(now, &[]);
        assert!(
            workspace.session(a).is_some(),
            "held worker cannot be orphaned"
        );
        assert_eq!(
            workspace.create_session(),
            Err(WorkspaceError::SessionLimit)
        );
        gate.release();
        poll_until(&mut workspace, now, |w| w.session(a).is_none());
        let previous = a;
        a = workspace.create_session().unwrap();
        assert_ne!(a, previous);
        assert_eq!(workspace.sessions().len(), 2, "cycle {cycle}");
    }
}

#[test]
fn keep_open_before_query_worker_finishes_recovers_without_changing_query() {
    let (mut workspace, a, b) = pair();
    seed_query(&mut workspace, a, "A", true);
    workspace.session_mut(a).unwrap().history_dirty = false;
    let mut gate = QueryGate::install(&mut workspace, a);
    let now = Instant::now();
    workspace.select_session(a).unwrap();
    workspace.poll(now, &[]);
    gate.wait_entered();
    workspace.select_session(b).unwrap();
    let target = workspace.request_close_session(a, now).unwrap();
    workspace.poll(now, &[]);
    workspace.keep_session_open(target).unwrap();
    workspace.select_session(a).unwrap();
    workspace.poll(now, &[]); // Still gated: do not submit into the stopping worker.
    assert!(workspace.session(a).unwrap().query_failure().is_none());
    gate.release();
    poll_until(&mut workspace, now, |w| {
        w.session(a).unwrap().query_snapshot().is_some()
    });
    let owner = workspace.session(a).unwrap();
    assert_eq!(owner.search, "needle");
    assert_eq!(owner.query_snapshot().unwrap().rows, [0]);
    assert!(owner.query_failure().is_none());
    assert!(!owner.shutdown.active());
}

#[test]
fn window_keep_open_resets_ready_owners_and_superseded_tab_close_requests() {
    for (supersede_tab, archive_failure) in [(false, false), (true, false), (false, true)] {
        let (mut workspace, a, b) = pair();
        workspace
            .session_mut(b)
            .unwrap()
            .record_log_line("B-only unsaved".into());
        workspace.select_session(a).unwrap();
        let held = archive_failure.then(|| {
            workspace
                .session_mut(b)
                .unwrap()
                .hold_session_save_for_test()
        });
        let now = Instant::now();
        let old = supersede_tab.then(|| workspace.request_close_session(a, now).unwrap());
        workspace.request_window_close(now).unwrap();
        assert_eq!(
            workspace.create_session(),
            Err(WorkspaceError::WindowClosing)
        );
        poll_until(&mut workspace, now, |w| {
            w.session(a).unwrap().shutdown.allows_close()
        });
        if let Some(held) = held {
            assert!(workspace.session(b).unwrap().session_archive_busy());
            assert!(!workspace.window_allows_close());
            held.complete(Err("inactive B archive failed".into()))
                .unwrap();
            poll_until(&mut workspace, now, |w| {
                w.session(b).unwrap().shutdown.decision().is_some()
            });
            assert!(matches!(
                workspace.close_prompt().unwrap().kind,
                ClosePromptKind::Incomplete(ref reason)
                    if reason.contains("inactive B archive failed")
            ));
        } else {
            poll_until(&mut workspace, now, |w| {
                w.session(b)
                    .unwrap()
                    .shutdown
                    .unsaved_session_confirmation()
            });
        }
        assert_eq!(workspace.active_session_id(), a);
        assert!(!workspace.window_allows_close());
        assert_eq!(workspace.close_prompt().unwrap().target.session, b);
        if let Some(old) = old {
            assert_eq!(
                workspace.close_without_saving_session(old),
                Err(WorkspaceError::StaleCloseTarget)
            );
        }
        workspace.keep_window_open();
        assert!(!workspace.window_closing());
        assert!(workspace.close_requests.is_empty());
        assert!(workspace.sessions().iter().all(|s| !s.shutdown.active()));
        workspace.poll(now, &[]);
        assert!(workspace.session(a).is_some());
        assert_eq!(workspace.session(b).unwrap().logs[0].raw, "B-only unsaved");
        if archive_failure {
            assert!(
                workspace
                    .session(b)
                    .unwrap()
                    .session_archive_shutdown_failure()
                    .unwrap()
                    .contains("inactive B archive failed")
            );
        }
    }
}
