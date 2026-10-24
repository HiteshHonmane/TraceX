use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct FakeStream {
    chunks: VecDeque<Vec<u8>>,
    reads: Arc<AtomicUsize>,
    released: Arc<AtomicBool>,
    gate: Option<(mpsc::Sender<()>, Receiver<()>)>,
}
impl Read for FakeStream {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some((entered, release)) = self.gate.take() {
            entered.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        let Some(bytes) = self.chunks.pop_front() else {
            return Ok(0);
        };
        assert!(bytes.len() <= output.len());
        output[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}
impl Write for FakeStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Drop for FakeStream {
    fn drop(&mut self) {
        self.released.store(true, Ordering::SeqCst);
    }
}
fn fake(chunks: Vec<Vec<u8>>) -> (FakeStream, Arc<AtomicUsize>, Arc<AtomicBool>) {
    let reads = Arc::new(AtomicUsize::new(0));
    let released = Arc::new(AtomicBool::new(false));
    (
        FakeStream {
            chunks: chunks.into(),
            reads: Arc::clone(&reads),
            released: Arc::clone(&released),
            gate: None,
        },
        reads,
        released,
    )
}
fn saturated_budget() -> CaptureBudget {
    CaptureBudget {
        queued_batches: 1,
        ..CaptureBudget::default()
    }
}
fn wait_terminal(inbox: &mut CaptureInbox) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        inbox.poll_status();
        if inbox.terminal_reason().is_some() {
            return;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
}
fn drain(inbox: &mut CaptureInbox) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut records = Vec::new();
    while !inbox.is_drained() {
        if let Some(record) = inbox.pop_record() {
            records.push(record);
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    records
}

#[test]
fn saturation_stops_before_third_read_and_preserves_queued_rejected_batch_and_tail() {
    let (stream, reads, released) = fake(vec![
        b"first\n".to_vec(),
        b"second\n\nfinal tail".to_vec(),
        b"must not read\n".to_vec(),
    ]);
    let (mut inbox, _commands) =
        start_reader(stream, saturated_budget(), Arc::new(Mutex::new(false)), 10).unwrap();
    wait_terminal(&mut inbox);
    assert_eq!(inbox.terminal_reason(), Some(&CaptureStopReason::Saturated));
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(inbox.next_batch_id(), 13);
    assert_eq!(inbox.rejected_batches(), 1);
    assert_eq!(drain(&mut inbox), ["first", "second", "", "final tail"]);
    assert_eq!(inbox.processed_records(), 4);
    assert_eq!(inbox.usage().bytes, 0);
}

#[test]
fn entire_blank_line_read_is_retained_and_expansion_accounting_is_bounded() {
    let budget = saturated_budget();
    let (stream, reads, _) = fake(vec![
        b"first\n".to_vec(),
        vec![b'\n'; READ_BYTES],
        b"not read".to_vec(),
    ]);
    let (mut inbox, _commands) =
        start_reader(stream, budget, Arc::new(Mutex::new(false)), 0).unwrap();
    wait_terminal(&mut inbox);
    let rejected = &inbox.terminal.as_ref().unwrap().retained[0];
    assert_eq!(rejected.records.len(), READ_BYTES);
    assert!(rejected.charge.bytes <= budget.maximum_batch_bytes());
    assert!(inbox.usage().bytes <= budget.queued_bytes);
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    let records = drain(&mut inbox);
    assert_eq!(records.len(), READ_BYTES + 1);
    assert_eq!(records[0], "first");
    assert!(records[1..].iter().all(String::is_empty));
}

#[test]
fn maximum_partial_frame_invalid_byte_and_final_tail_survive_saturation() {
    let mut chunks = vec![b"first\n".to_vec()];
    let payload = vec![b'a'; MAX_FRAME_BYTES - 2];
    chunks.extend(payload.chunks(READ_BYTES).map(<[u8]>::to_vec));
    chunks.push(b"\xff\nlast-tail".to_vec());
    chunks.push(b"not read".to_vec());
    let expected_reads = chunks.len() - 1;
    let (stream, reads, _) = fake(chunks);
    let budget = saturated_budget();
    let (mut inbox, _commands) =
        start_reader(stream, budget, Arc::new(Mutex::new(false)), 0).unwrap();
    wait_terminal(&mut inbox);
    assert_eq!(reads.load(Ordering::SeqCst), expected_reads);
    for batch in &inbox.terminal.as_ref().unwrap().retained {
        assert!(batch.charge.bytes <= budget.maximum_batch_bytes());
    }
    let records = drain(&mut inbox);
    assert_eq!(records[0], "first");
    assert_eq!(records[2], "last-tail");
    let hex = records[1]
        .strip_prefix("[TraceX invalid UTF-8 bytes:")
        .unwrap()
        .strip_suffix(']')
        .unwrap();
    let original: Vec<_> = hex
        .split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect();
    assert_eq!(original[..original.len() - 1], payload);
    assert_eq!(original.last(), Some(&0xff));
}

#[test]
fn split_unicode_and_eof_tail_keep_order_and_meaningful_terminal_reason() {
    let (stream, _, released) = fake(vec![
        b"unicode \xf0\x9f".to_vec(),
        b"\xa6\x80\nlast".to_vec(),
    ]);
    let (mut inbox, _commands) = start_reader(
        stream,
        CaptureBudget::default(),
        Arc::new(Mutex::new(false)),
        50,
    )
    .unwrap();
    wait_terminal(&mut inbox);
    assert!(matches!(
        inbox.terminal_reason(),
        Some(CaptureStopReason::ReadFailed {
            kind: io::ErrorKind::UnexpectedEof,
            ..
        })
    ));
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(drain(&mut inbox), ["unicode 🦀", "last"]);
    assert_eq!(inbox.next_batch_id(), 52);
}

#[test]
fn dropped_consumer_does_not_block_port_release_or_independent_terminal_owner() {
    let (mut stream, _, released) = fake(vec![b"retained\n".to_vec()]);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    stream.gate = Some((entered_tx, release_rx));
    let (inbox, _commands) =
        start_reader(stream, saturated_budget(), Arc::new(Mutex::new(false)), 0).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let slot = Arc::clone(&inbox.slot);
    drop(inbox);
    release_tx.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(terminal) = lock(&slot).as_ref() {
            assert_eq!(terminal.reason, CaptureStopReason::ConsumerGone);
            assert_eq!(terminal.retained[0].records[0], "retained");
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert!(released.load(Ordering::SeqCst));
}

#[test]
fn invalid_capture_budgets_are_rejected_before_starting_reader() {
    for budget in [
        CaptureBudget {
            read_bytes: 0,
            ..CaptureBudget::default()
        },
        CaptureBudget {
            queued_bytes: 1,
            ..CaptureBudget::default()
        },
        CaptureBudget {
            queued_records: 1,
            ..CaptureBudget::default()
        },
        CaptureBudget {
            queued_batches: usize::MAX,
            ..CaptureBudget::default()
        },
    ] {
        assert_eq!(
            budget.validate().unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
}

#[test]
fn worker_sender_loss_without_terminal_drains_records_and_reports_unknown_continuity() {
    let (sender, batches) = mpsc::sync_channel(1);
    let batch = CaptureBatch::new(42, VecDeque::from(["accepted".to_owned()]));
    let usage = Arc::new(Mutex::new(batch.charge));
    sender.send(batch).unwrap();
    drop(sender);
    let mut inbox = CaptureInbox {
        worker: None,
        batches,
        usage,
        slot: Arc::new(Mutex::new(None)),
        terminal: None,
        current: None,
        current_reserved: false,
        channel_closed: false,
        processed_records: 0,
        next_sequence: 42,
    };
    assert_eq!(drain(&mut inbox), ["accepted"]);
    assert_eq!(
        inbox.terminal_reason(),
        Some(&CaptureStopReason::WorkerEndedUnexpectedly)
    );
    assert_eq!(inbox.received_bytes(), None);
    assert_eq!(inbox.next_batch_id(), 43);
    assert!(inbox.is_drained());
}

#[test]
fn command_partial_failure_keeps_final_received_tail_and_prefix_detail() {
    struct CommandFailureStream {
        command: SyncSender<Vec<u8>>,
        first_read: bool,
        written: usize,
    }
    impl Read for CommandFailureStream {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            assert!(!self.first_read);
            self.first_read = true;
            output[..4].copy_from_slice(b"tail");
            self.command.try_send(b"PING".to_vec()).unwrap();
            Ok(4)
        }
    }
    impl Write for CommandFailureStream {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            if self.written == 0 {
                self.written = 2;
                Ok(2)
            } else {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let (command, command_rx) = mpsc::sync_channel(1);
    let (sender, batches) = mpsc::sync_channel(1);
    let usage = Arc::new(Mutex::new(CaptureUsage::default()));
    let slot = Arc::new(Mutex::new(None));
    read_stream(
        CommandFailureStream {
            command,
            first_read: false,
            written: 0,
        },
        saturated_budget(),
        Arc::new(Mutex::new(false)),
        command_rx,
        ReaderOutput {
            batches: sender,
            usage: Arc::clone(&usage),
            slot: Arc::clone(&slot),
        },
        0,
    );
    let mut inbox = CaptureInbox {
        worker: None,
        batches,
        usage,
        slot,
        terminal: None,
        current: None,
        current_reserved: false,
        channel_closed: false,
        processed_records: 0,
        next_sequence: 0,
    };
    wait_terminal(&mut inbox);
    assert!(matches!(
        inbox.terminal_reason(),
        Some(CaptureStopReason::CommandFailed {
            written_prefix: 2,
            kind: io::ErrorKind::BrokenPipe,
            ..
        })
    ));
    assert_eq!(drain(&mut inbox), ["tail"]);
}

#[test]
fn unexpected_worker_unwind_retains_ownership_until_transport_cleanup_finishes() {
    struct PanickingStream {
        cleanup_started: mpsc::Sender<()>,
        release_cleanup: Receiver<()>,
    }
    impl Read for PanickingStream {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("injected transport implementation failure");
        }
    }
    impl Write for PanickingStream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Drop for PanickingStream {
        fn drop(&mut self) {
            // Never panic during unwind; disconnect also releases this gate.
            let _ = self.cleanup_started.send(());
            let _ = self.release_cleanup.recv_timeout(Duration::from_secs(3));
        }
    }
    let (cleanup_started, cleanup_wait) = mpsc::channel();
    let (release_cleanup, cleanup_gate) = mpsc::channel();
    let (mut inbox, _commands) = start_reader(
        PanickingStream {
            cleanup_started,
            release_cleanup: cleanup_gate,
        },
        CaptureBudget::default(),
        Arc::new(Mutex::new(false)),
        0,
    )
    .unwrap();
    cleanup_wait.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(inbox.pop_record(), None);
    assert_eq!(
        inbox.terminal_reason(),
        Some(&CaptureStopReason::WorkerEndedUnexpectedly)
    );
    let released_before_cleanup = inbox.is_drained();
    // Release before asserting so the deliberate failure of the old code never
    // strands a worker in a destructor or depends on the watchdog expiring.
    release_cleanup.send(()).unwrap();
    assert!(drain(&mut inbox).is_empty());
    assert!(
        !released_before_cleanup,
        "sender closure must not release a transport still unwinding"
    );
}
