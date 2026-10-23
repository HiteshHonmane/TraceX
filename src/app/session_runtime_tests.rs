//! Extraction characterization for independent runtime values, not Workspace delivery.
//! Registered below session_archive so controlled completion fixtures can access its
//! private handoff without adding a production test hook or relying on file timing.
use super::{ArchiveResult, SessionRuntime};
use crate::app::capture::CaptureInbox;
use crate::domain::{
    log::{LogEntry, LogLevel},
    query::QueryScope,
};
use std::sync::{Arc, Mutex, mpsc};

fn runtime() -> SessionRuntime {
    let mut runtime = SessionRuntime::new();
    runtime.auto_save_path = None;
    runtime
}

#[test]
fn real_runtimes_keep_capture_commands_and_investigation_state_independent() {
    let mut a = runtime();
    let mut b = runtime();
    a.selected_port = "A-device".into();
    b.selected_port = "B-device".into();
    a.command_input = "A-draft".into();
    b.command_input = "B-draft".into();
    a.command_history.push("A-old".into());
    b.command_history.push("B-old".into());
    a.search = "A-only".into();
    b.search = "B-only".into();
    b.show_info = false;
    b.send_newline = false;
    a.auto_scroll = false;

    let (a_feed, a_inbox) = CaptureInbox::test_channel();
    let (b_feed, b_inbox) = CaptureInbox::test_channel();
    a.capture = Some(a_inbox);
    b.capture = Some(b_inbox);
    a.connected = true;
    b.connected = true;
    a_feed.send("INFO A-only first".into()).unwrap();
    a_feed.send(String::new()).unwrap();
    b_feed.send("WARN B-only first".into()).unwrap();
    b_feed.send("WARN B-only café".into()).unwrap();
    a.poll();
    b.poll();

    assert_eq!(a.logs.len(), 2);
    assert_eq!(b.logs.len(), 2);
    assert_eq!((a.logs[0].id, b.logs[0].id), (0, 0));
    assert_eq!(a.logs[1].raw, "");
    assert!(a.logs[0].raw.contains("A-only"));
    assert!(b.logs.iter().all(|entry| entry.raw.contains("B-only")));
    assert_eq!(a.query_indices(QueryScope::Main), [0]);
    assert_eq!(b.query_indices(QueryScope::Main), [0, 1]);
    assert!(a.history_dirty && b.history_dirty);
    a.selected.insert(0);
    b.selected.insert(0);

    let (a_sender, a_receiver) = mpsc::sync_channel(1);
    let (b_sender, b_receiver) = mpsc::sync_channel(1);
    a.serial_tx = Some(a_sender);
    b.serial_tx = Some(b_sender);
    assert!(a.send_command("A-send"));
    assert!(!a.send_command("A-retry"), "A queue is deliberately full");
    assert_eq!(a.command_input, "A-retry");
    assert!(a.operation_error.as_deref().unwrap().contains("full"));
    assert_eq!(b.command_input, "B-draft");
    assert_eq!(b.command_history, ["B-old"]);
    assert!(b.operation_error.is_none());
    assert!(b.send_command("B-send"));
    assert_eq!(a_receiver.try_recv().unwrap(), b"A-send\r\n");
    assert_eq!(b_receiver.try_recv().unwrap(), b"B-send");
    assert!(a_receiver.try_recv().is_err());
    assert!(b_receiver.try_recv().is_err());

    let b_allocation = b.logs.as_ptr();
    a.search = "not present".into();
    a.clear();
    assert!(a.logs.is_empty());
    assert!(a.selected.is_empty());
    assert!(!a.history_dirty);
    assert_eq!(a.next_id, 2, "Clear must not reuse local record IDs");
    assert_eq!(b.logs.as_ptr(), b_allocation);
    assert_eq!(b.logs.len(), 2);
    assert!(b.history_dirty);
    assert!(b.selected.contains(&0));
    assert_eq!(b.search, "B-only");
    assert_eq!(b.query_indices(QueryScope::Main), [0, 1]);
    assert_eq!(b.command_input, "B-draft");
    assert_eq!(b.command_history, ["B-old", "B-send"]);
    assert_eq!(b.selected_port, "B-device");
    assert!(!b.show_info);
    assert!(b.auto_scroll);
}

#[test]
fn held_archive_returns_to_its_runtime_while_another_runtime_keeps_polling() {
    for save_succeeds in [true, false] {
        let mut a = runtime();
        let mut b = runtime();
        a.logs.push(Arc::new(LogEntry {
            id: 19,
            timestamp: "A-original-time".into(),
            level: LogLevel::Error,
            tag: Some("A-tag".into()),
            message: "A-parsed".into(),
            raw: "A-only original raw 🦀".into(),
        }));
        a.next_id = 20;
        a.history_dirty = true;
        a.rebuild_history_accounting();
        let original_allocation = a.logs.as_ptr();
        let original_entry = Arc::clone(&a.logs[0]);
        let original_raw_allocation = original_entry.raw.as_ptr();
        b.command_input = "B-draft".into();
        b.search = "B-only".into();

        let (a_feed, a_inbox) = CaptureInbox::test_channel();
        let (b_feed, b_inbox) = CaptureInbox::test_channel();
        a_feed.send("INFO A-only pending tail".into()).unwrap();
        a.capture = Some(a_inbox);
        b.capture = Some(b_inbox);
        b.connected = true;

        // Model the worker-owned handoff, holding completion explicitly. Real
        // filesystem/spawn/format behavior remains covered by archive tests.
        let handed_off = Arc::new(Mutex::new(Some(std::mem::take(&mut a.logs))));
        let (complete, receiver) = mpsc::sync_channel(1);
        a.session_archive.save_records = Some(handed_off);
        a.session_archive.receiver = Some(receiver);
        for marker in ["WARN B-only first", "WARN B-only second"] {
            b_feed.send(marker.into()).unwrap();
            a.poll();
            b.poll();
            assert!(a.session_archive_busy());
            assert!(a.logs.is_empty());
            assert_eq!(
                a.capture.as_mut().unwrap().peek_record().unwrap(),
                "INFO A-only pending tail"
            );
        }
        assert_eq!(b.logs.len(), 2);
        assert!(b.logs.iter().all(|entry| entry.raw.contains("B-only")));
        b.selected.insert(0);
        let b_allocation = b.logs.as_ptr();

        complete
            .send(ArchiveResult::Saved {
                result: if save_succeeds {
                    Ok(())
                } else {
                    Err("injected A archive failure".into())
                },
                marks_clean: true,
            })
            .unwrap();
        a.poll_session_archive();
        assert!(!a.session_archive_busy());
        assert_eq!(a.logs.as_ptr(), original_allocation);
        assert!(Arc::ptr_eq(&a.logs[0], &original_entry));
        assert_eq!(a.logs[0].raw.as_ptr(), original_raw_allocation);
        assert_eq!(a.logs[0].timestamp, "A-original-time");
        assert_eq!(a.logs[0].tag.as_deref(), Some("A-tag"));
        assert_eq!(a.history_dirty, !save_succeeds);
        assert_eq!(
            a.session_archive_shutdown_failure().is_some(),
            !save_succeeds
        );

        a.poll();
        assert_eq!(a.logs.len(), 2);
        assert_eq!(a.logs[1].id, 20);
        assert_eq!(a.logs[1].raw, "INFO A-only pending tail");
        assert!(
            a.history_dirty,
            "admitted tail is outside the saved snapshot"
        );
        assert_eq!(b.logs.as_ptr(), b_allocation);
        assert_eq!(b.logs.len(), 2);
        assert!(b.logs.iter().all(|entry| entry.raw.contains("B-only")));
        assert!(b.history_dirty);
        assert!(b.selected.contains(&0));
        assert_eq!(b.search, "B-only");
        assert_eq!(b.command_input, "B-draft");
        assert!(b.operation_error.is_none());
    }
}
