use super::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct MemoryState {
    files: Vec<Vec<u8>>,
    syncs: usize,
    fail_after: Option<usize>,
    sync_error: bool,
}
struct MemorySink {
    state: Arc<Mutex<MemoryState>>,
    index: usize,
}
impl Write for MemorySink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut state = self.state.lock().unwrap();
        let length = state.files[self.index].len();
        let count = match state.fail_after {
            Some(limit) if length >= limit => {
                return Err(io::Error::from(io::ErrorKind::StorageFull));
            }
            Some(limit) => bytes.len().min(limit - length),
            None => bytes.len(),
        };
        state.files[self.index].extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl SaveSink for MemorySink {
    fn sync_data(&mut self) -> io::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.syncs += 1;
        if state.sync_error {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        } else {
            Ok(())
        }
    }
}
fn factory(state: Arc<Mutex<MemoryState>>) -> FileFactory {
    Box::new(move |_, sequence| {
        let mut memory = state.lock().unwrap();
        let index = memory.files.len();
        memory.files.push(Vec::new());
        Ok(OpenedFile {
            sink: Box::new(MemorySink {
                state: Arc::clone(&state),
                index,
            }),
            path: PathBuf::from(format!("memory-{sequence}.log")),
            next_sequence: sequence + 1,
        })
    })
}
fn config() -> AutosaveConfig {
    AutosaveConfig {
        base_path: PathBuf::from("memory.log"),
        limit_mb: 1,
        sequence: 0,
    }
}
fn write(id: usize, payload: &str) -> AutosaveCommand {
    AutosaveCommand::Write(WriteRequest {
        id,
        payload: payload.to_owned(),
    })
}
fn event(worker: &AutosaveWorker) -> AutosaveEvent {
    worker.events.recv_timeout(Duration::from_secs(2)).unwrap()
}
fn finish(mut worker: AutosaveWorker) {
    worker.try_submit(AutosaveCommand::Finish).unwrap();
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Finished { failure: None }
    ));
    worker.thread.join().unwrap();
}

#[test]
fn starts_writes_in_order_and_stops_after_sync() {
    let state = Arc::new(Mutex::new(MemoryState::default()));
    let mut worker = AutosaveWorker::spawn_with_factory(4, 4, factory(Arc::clone(&state))).unwrap();
    worker.try_submit(AutosaveCommand::Start(config())).unwrap();
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Started {
            next_sequence: 1,
            ..
        }
    ));
    worker.try_submit(write(1, "first\n")).unwrap();
    worker.try_submit(write(2, "second\n")).unwrap();
    worker.try_submit(AutosaveCommand::Stop).unwrap();
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Written {
            id: 1,
            bytes: 6,
            current_bytes: 6
        }
    ));
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Written {
            id: 2,
            bytes: 7,
            current_bytes: 13
        }
    ));
    assert!(matches!(event(&worker), AutosaveEvent::Stopped));
    assert_eq!(state.lock().unwrap().files, [b"first\nsecond\n".to_vec()]);
    assert_eq!(state.lock().unwrap().syncs, 1);
    finish(worker);
}

#[test]
fn rotates_only_before_a_write_exceeds_nonempty_file_limit() {
    let state = Arc::new(Mutex::new(MemoryState::default()));
    let mut worker = AutosaveWorker::spawn_with_factory(4, 4, factory(Arc::clone(&state))).unwrap();
    worker.try_submit(AutosaveCommand::Start(config())).unwrap();
    let _ = event(&worker);
    worker
        .try_submit(write(1, &"x".repeat(1024 * 1024)))
        .unwrap();
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Written { id: 1, .. }
    ));
    worker.try_submit(write(2, "next")).unwrap();
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Started {
            next_sequence: 2,
            ..
        }
    ));
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Written {
            current_bytes: 4,
            ..
        }
    ));
    assert_eq!(
        state
            .lock()
            .unwrap()
            .files
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        [1024 * 1024, 4]
    );
    finish(worker);
}

#[test]
fn partial_write_failure_returns_original_payload_and_confirmed_prefix() {
    let state = Arc::new(Mutex::new(MemoryState {
        fail_after: Some(2),
        ..MemoryState::default()
    }));
    let mut worker = AutosaveWorker::spawn_with_factory(4, 4, factory(state)).unwrap();
    worker.try_submit(AutosaveCommand::Start(config())).unwrap();
    let _ = event(&worker);
    worker.try_submit(write(8, "abcd")).unwrap();
    match event(&worker) {
        AutosaveEvent::Failed {
            command: AutosaveCommand::Write(request),
            failure,
        } => {
            assert_eq!(request.id, 8);
            assert_eq!(request.payload, "abcd");
            assert_eq!(failure.operation, AutosaveOperation::Write);
            assert_eq!(failure.written_prefix_bytes, 2);
            assert_eq!(failure.error.kind(), io::ErrorKind::StorageFull);
        }
        other => panic!("unexpected event: {other:?}"),
    }
    worker.try_submit(write(9, "more")).unwrap();
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Failed {
            command: AutosaveCommand::Write(WriteRequest { id: 9, .. }),
            ..
        }
    ));
    finish(worker);
}

#[test]
fn queue_saturation_returns_command_without_blocking_or_losing_payload() {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let state = Arc::new(Mutex::new(MemoryState::default()));
    let mut open = factory(Arc::clone(&state));
    let mut worker = AutosaveWorker::spawn_with_factory(
        1,
        2,
        Box::new(move |path, sequence| {
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            open(path, sequence)
        }),
    )
    .unwrap();
    worker.try_submit(AutosaveCommand::Start(config())).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.try_submit(write(1, "accepted")).unwrap();
    let returned = match worker.try_submit(write(2, "retain me")) {
        Err(SubmitError::Full(command)) => command,
        other => panic!("unexpected submission: {other:?}"),
    };
    release_tx.send(()).unwrap();
    let _ = event(&worker);
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Written { id: 1, .. }
    ));
    worker.try_submit(returned).unwrap();
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Written { id: 2, .. }
    ));
    assert_eq!(state.lock().unwrap().files[0], b"acceptedretain me");
    finish(worker);
}

#[test]
fn create_and_final_sync_failures_are_observable_and_finish_is_terminal() {
    let mut failed = AutosaveWorker::spawn_with_factory(
        2,
        2,
        Box::new(|_, _| Err(io::Error::from(io::ErrorKind::PermissionDenied))),
    )
    .unwrap();
    failed.try_submit(AutosaveCommand::Start(config())).unwrap();
    assert!(matches!(
        event(&failed),
        AutosaveEvent::Failed {
            command: AutosaveCommand::Start(_),
            failure: AutosaveFailure {
                operation: AutosaveOperation::Create,
                ..
            }
        }
    ));
    finish(failed);
    let state = Arc::new(Mutex::new(MemoryState {
        sync_error: true,
        ..MemoryState::default()
    }));
    let mut worker = AutosaveWorker::spawn_with_factory(2, 2, factory(state)).unwrap();
    worker.try_submit(AutosaveCommand::Start(config())).unwrap();
    let _ = event(&worker);
    worker.try_submit(AutosaveCommand::Finish).unwrap();
    assert!(matches!(
        worker.try_submit(write(3, "retained")),
        Err(SubmitError::Finishing(AutosaveCommand::Write(_)))
    ));
    assert!(matches!(
        event(&worker),
        AutosaveEvent::Finished {
            failure: Some(AutosaveFailure {
                operation: AutosaveOperation::Sync,
                ..
            })
        }
    ));
    worker.thread.join().unwrap();
}

#[test]
fn failed_start_or_stop_sync_faults_old_session_and_rejects_later_writes() {
    for command in [AutosaveCommand::Start(config()), AutosaveCommand::Stop] {
        let state = Arc::new(Mutex::new(MemoryState::default()));
        let mut worker =
            AutosaveWorker::spawn_with_factory(4, 4, factory(Arc::clone(&state))).unwrap();
        worker.try_submit(AutosaveCommand::Start(config())).unwrap();
        let _ = event(&worker);
        worker.try_submit(write(1, "old destination")).unwrap();
        let _ = event(&worker);
        state.lock().unwrap().sync_error = true;
        worker.try_submit(command).unwrap();
        worker
            .try_submit(write(2, "must not reach old destination"))
            .unwrap();
        assert!(matches!(
            event(&worker),
            AutosaveEvent::Failed {
                failure: AutosaveFailure {
                    operation: AutosaveOperation::Sync,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            event(&worker),
            AutosaveEvent::Failed {
                command: AutosaveCommand::Write(WriteRequest { id: 2, .. }),
                ..
            }
        ));
        assert_eq!(state.lock().unwrap().files, [b"old destination".to_vec()]);
        state.lock().unwrap().sync_error = false;
        finish(worker);
    }
}
