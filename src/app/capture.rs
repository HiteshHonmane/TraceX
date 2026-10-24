//! Bounded read batches with an independent terminal payload slot.
use crate::domain::stream_framing::{MAX_FRAME_BYTES, StreamFramer};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::mem::size_of;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

pub(super) const MAX_COMMAND_BYTES: usize = 4096;
const READ_BYTES: usize = 4096;

/// Initial conservative engineering budgets; these are not a throughput SLA.
#[derive(Clone, Copy, Debug)]
pub(super) struct CaptureBudget {
    pub queued_batches: usize,
    pub queued_bytes: usize,
    pub queued_records: usize,
    pub read_bytes: usize,
    pub command_capacity: usize,
    pub max_command_bytes: usize,
}
impl Default for CaptureBudget {
    fn default() -> Self {
        Self {
            queued_batches: 32,
            queued_bytes: 4 * 1024 * 1024,
            queued_records: 32768,
            read_bytes: READ_BYTES,
            command_capacity: 16,
            max_command_bytes: MAX_COMMAND_BYTES,
        }
    }
}
impl CaptureBudget {
    /// Includes conservative rendered hex, blank-record and vector capacity slack.
    pub fn maximum_batch_bytes(self) -> usize {
        3 * (MAX_FRAME_BYTES + 1 + self.read_bytes)
            + (28 + 2 * size_of::<String>()) * (self.read_bytes + 1)
            + size_of::<CaptureBatch>()
    }
    pub fn validate(self) -> io::Result<Self> {
        if self.read_bytes == 0
            || self.read_bytes > READ_BYTES
            || self.queued_batches == 0
            || self.queued_batches > 1024
            || self.command_capacity > 1024
            || self.command_capacity == 0
            || self.max_command_bytes == 0
            || self.max_command_bytes > MAX_COMMAND_BYTES
            || self.queued_records < self.read_bytes + 1
            || self.queued_bytes < self.maximum_batch_bytes()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Capture budget cannot hold a maximum expanded read batch",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CaptureStopReason {
    ManualStop,
    CommandsClosed,
    Saturated,
    ConsumerGone,
    ReadFailed {
        kind: io::ErrorKind,
        detail: String,
    },
    CommandFailed {
        kind: io::ErrorKind,
        detail: String,
        written_prefix: usize,
    },
    CommandTooLarge,
    SequenceExhausted,
    WorkerEndedUnexpectedly,
}
impl CaptureStopReason {
    pub fn message(&self) -> String {
        match self {
            Self::Saturated => "Capture stopped because processing could not keep up. Received data is retained pending processing; additional device output may be missing.".to_owned(),
            Self::ReadFailed { detail, .. } => format!("Capture stopped after a read failure: {detail}. Received data is retained; continuity is unknown."),
            Self::CommandFailed { detail, written_prefix, .. } => format!("Capture stopped after a command write failure: {detail}. Confirmed command prefix: {written_prefix} bytes. Received data is retained."),
            Self::CommandTooLarge => "Capture stopped after an oversized command was rejected.".to_owned(),
            Self::ConsumerGone => "Capture stopped because its consumer closed.".to_owned(),
            Self::SequenceExhausted => "Capture stopped because batch identifiers were exhausted.".to_owned(),
            Self::WorkerEndedUnexpectedly => "Capture worker ended unexpectedly. Queued records remain retained, but an unfinished read or frame may be missing; continuity is unknown.".to_owned(),
            Self::ManualStop | Self::CommandsClosed => "Capture stopped.".to_owned(),
        }
    }
    pub fn requires_attention(&self) -> bool {
        !matches!(self, Self::ManualStop | Self::CommandsClosed)
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub(super) struct CaptureUsage {
    pub batches: usize,
    pub bytes: usize,
    pub records: usize,
}
#[derive(Debug)]
struct CaptureBatch {
    sequence: u64,
    records: VecDeque<String>,
    charge: CaptureUsage,
}
impl CaptureBatch {
    fn new(sequence: u64, records: VecDeque<String>) -> Self {
        let bytes = size_of::<Self>()
            + records.capacity() * size_of::<String>()
            + records.iter().map(String::capacity).sum::<usize>();
        let charge = CaptureUsage {
            batches: 1,
            bytes,
            records: records.len(),
        };
        Self {
            sequence,
            records,
            charge,
        }
    }
}
#[derive(Debug)]
struct CaptureTerminal {
    reason: CaptureStopReason,
    retained: VecDeque<CaptureBatch>,
    next_sequence: u64,
    received_bytes: Option<u64>,
    rejected_batches: usize,
}
type TerminalSlot = Arc<Mutex<Option<CaptureTerminal>>>;
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Owns queued, partially consumed and terminal records until explicitly popped.
/// Terminal status may be observed early, but terminal records follow queued data.
pub(super) struct CaptureInbox {
    // Sender closure during unwind can precede slow transport destruction.
    // Keep resource ownership until the worker has actually exited; never join
    // it on the UI thread. None is used only by workerless test fixtures.
    worker: Option<thread::JoinHandle<()>>,
    batches: Receiver<CaptureBatch>,
    usage: Arc<Mutex<CaptureUsage>>,
    slot: TerminalSlot,
    terminal: Option<CaptureTerminal>,
    current: Option<CaptureBatch>,
    current_reserved: bool,
    channel_closed: bool,
    processed_records: u64,
    next_sequence: u64,
}
impl CaptureInbox {
    pub fn poll_status(&mut self) {
        if self.terminal.is_none() {
            self.terminal = lock(&self.slot).take();
        }
    }
    pub fn terminal_reason(&self) -> Option<&CaptureStopReason> {
        self.terminal.as_ref().map(|terminal| &terminal.reason)
    }
    pub fn next_batch_id(&self) -> u64 {
        self.terminal
            .as_ref()
            .map_or(self.next_sequence, |terminal| {
                terminal.next_sequence.max(self.next_sequence)
            })
    }
    pub fn usage(&self) -> CaptureUsage {
        *lock(&self.usage)
    }
    pub fn received_bytes(&self) -> Option<u64> {
        self.terminal
            .as_ref()
            .and_then(|terminal| terminal.received_bytes)
    }
    pub fn rejected_batches(&self) -> usize {
        self.terminal
            .as_ref()
            .map_or(0, |terminal| terminal.rejected_batches)
    }
    pub fn processed_records(&self) -> u64 {
        self.processed_records
    }
    pub fn current_batch_id(&self) -> Option<u64> {
        self.current.as_ref().map(|batch| batch.sequence)
    }
    pub fn is_drained(&self) -> bool {
        self.worker
            .as_ref()
            .is_none_or(thread::JoinHandle::is_finished)
            && self.channel_closed
            && self.current.is_none()
            && self
                .terminal
                .as_ref()
                .is_some_and(|terminal| terminal.retained.is_empty())
    }
    fn fill(&mut self) {
        self.poll_status();
        if self.current.is_some() {
            return;
        }
        if !self.channel_closed {
            match self.batches.try_recv() {
                Ok(batch) => {
                    self.next_sequence = self.next_sequence.max(batch.sequence.saturating_add(1));
                    self.current = Some(batch);
                    self.current_reserved = true;
                    return;
                }
                Err(TryRecvError::Disconnected) => {
                    self.channel_closed = true;
                    self.poll_status();
                    if self.terminal.is_none() {
                        self.terminal = Some(CaptureTerminal {
                            reason: CaptureStopReason::WorkerEndedUnexpectedly,
                            retained: VecDeque::new(),
                            next_sequence: self.next_sequence,
                            received_bytes: None,
                            rejected_batches: 0,
                        });
                    }
                }
                Err(TryRecvError::Empty) => return,
            }
        }
        if let Some(terminal) = &mut self.terminal {
            self.current = terminal.retained.pop_front();
            self.current_reserved = false;
        }
    }
    pub fn peek_record(&mut self) -> Option<&String> {
        self.fill();
        self.current.as_ref()?.records.front()
    }
    pub fn pop_record(&mut self) -> Option<String> {
        self.fill();
        let batch = self.current.as_mut()?;
        let record = batch.records.pop_front()?;
        self.processed_records = self.processed_records.saturating_add(1);
        if batch.records.is_empty() {
            if self.current_reserved {
                let mut usage = lock(&self.usage);
                usage.batches = usage.batches.saturating_sub(batch.charge.batches);
                usage.bytes = usage.bytes.saturating_sub(batch.charge.bytes);
                usage.records = usage.records.saturating_sub(batch.charge.records);
            }
            self.current = None;
        }
        Some(record)
    }
}

pub(super) fn start_reader(
    port: impl Read + Write + Send + 'static,
    budget: CaptureBudget,
    stop: Arc<Mutex<bool>>,
    first_sequence: u64,
) -> io::Result<(CaptureInbox, SyncSender<Vec<u8>>)> {
    let budget = budget.validate()?;
    if first_sequence >= u64::MAX - 1 {
        return Err(io::Error::other("Capture batch identifiers exhausted"));
    }
    let (sender, batches) = mpsc::sync_channel(budget.queued_batches);
    let (commands, command_rx) = mpsc::sync_channel(budget.command_capacity);
    let usage = Arc::new(Mutex::new(CaptureUsage::default()));
    let slot = Arc::new(Mutex::new(None));
    let mut inbox = CaptureInbox {
        worker: None,
        batches,
        usage: Arc::clone(&usage),
        slot: Arc::clone(&slot),
        terminal: None,
        current: None,
        current_reserved: false,
        channel_closed: false,
        processed_records: 0,
        next_sequence: first_sequence,
    };
    inbox.worker = Some(
        thread::Builder::new()
            .name("tracex-capture".to_owned())
            .spawn(move || {
                read_stream(
                    port,
                    budget,
                    stop,
                    command_rx,
                    ReaderOutput {
                        batches: sender,
                        usage,
                        slot,
                    },
                    first_sequence,
                );
            })?,
    );
    Ok((inbox, commands))
}

struct ReaderOutput {
    batches: SyncSender<CaptureBatch>,
    usage: Arc<Mutex<CaptureUsage>>,
    slot: TerminalSlot,
}

fn read_stream(
    mut port: impl Read + Write,
    budget: CaptureBudget,
    stop: Arc<Mutex<bool>>,
    commands: Receiver<Vec<u8>>,
    output: ReaderOutput,
    mut sequence: u64,
) {
    let ReaderOutput {
        batches,
        usage,
        slot,
    } = output;
    let mut framer = StreamFramer::default();
    let mut retained = VecDeque::new();
    let mut received_bytes = 0u64;
    let mut rejected_batches = 0;
    let mut buffer = [0u8; READ_BYTES];
    let reason = loop {
        if *lock(&stop) {
            break CaptureStopReason::ManualStop;
        }
        if sequence >= u64::MAX - 1 {
            break CaptureStopReason::SequenceExhausted;
        }
        match commands.try_recv() {
            Ok(command) => {
                if command.len() > budget.max_command_bytes {
                    break CaptureStopReason::CommandTooLarge;
                }
                let mut written = 0;
                let error = loop {
                    if written == command.len() {
                        break None;
                    }
                    match port.write(&command[written..]) {
                        Ok(0) => break Some(io::Error::from(io::ErrorKind::WriteZero)),
                        Ok(count) => written += count,
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => break Some(error),
                    }
                };
                if let Some(error) = error {
                    break CaptureStopReason::CommandFailed {
                        kind: error.kind(),
                        detail: error.to_string(),
                        written_prefix: written,
                    };
                }
            }
            Err(TryRecvError::Disconnected) => break CaptureStopReason::CommandsClosed,
            Err(TryRecvError::Empty) => {}
        }
        match port.read(&mut buffer[..budget.read_bytes]) {
            Ok(0) => {
                break CaptureStopReason::ReadFailed {
                    kind: io::ErrorKind::UnexpectedEof,
                    detail: "transport returned end of stream".to_owned(),
                };
            }
            Ok(count) => {
                received_bytes = received_bytes.saturating_add(count as u64);
                let mut records = VecDeque::new();
                framer.push(&buffer[..count], |record| records.push_back(record));
                if records.is_empty() {
                    continue;
                }
                let batch = CaptureBatch::new(sequence, records);
                sequence += 1;
                let mut queued = lock(&usage);
                if queued.batches >= budget.queued_batches
                    || queued.bytes.saturating_add(batch.charge.bytes) > budget.queued_bytes
                    || queued.records.saturating_add(batch.charge.records) > budget.queued_records
                {
                    drop(queued);
                    retained.push_back(batch);
                    rejected_batches = 1;
                    break CaptureStopReason::Saturated;
                }
                let charge = batch.charge;
                match batches.try_send(batch) {
                    Ok(()) => {
                        queued.batches += 1;
                        queued.bytes += charge.bytes;
                        queued.records += charge.records;
                    }
                    Err(TrySendError::Full(batch)) => {
                        retained.push_back(batch);
                        rejected_batches = 1;
                        break CaptureStopReason::Saturated;
                    }
                    Err(TrySendError::Disconnected(batch)) => {
                        retained.push_back(batch);
                        break CaptureStopReason::ConsumerGone;
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => {
                break CaptureStopReason::ReadFailed {
                    kind: error.kind(),
                    detail: error.to_string(),
                };
            }
        }
    };
    if let Some(tail) = framer.finish() {
        retained.push_back(CaptureBatch::new(sequence, VecDeque::from([tail])));
        sequence += 1;
    }
    drop(port);
    *lock(&slot) = Some(CaptureTerminal {
        reason,
        retained,
        next_sequence: sequence,
        received_bytes: Some(received_bytes),
        rejected_batches,
    });
    // Data sender closure follows terminal publication and transport release.
}

#[cfg(test)]
pub(super) struct TestFeed {
    sender: SyncSender<CaptureBatch>,
    usage: Arc<Mutex<CaptureUsage>>,
    slot: TerminalSlot,
    sequence: std::cell::Cell<u64>,
    publish_terminal: bool,
}
#[cfg(test)]
impl TestFeed {
    pub fn close_unexpectedly(mut self) {
        self.publish_terminal = false;
    }
    pub fn send(&self, record: String) -> Result<(), String> {
        self.send_batch(vec![record])
    }
    pub fn send_batch(&self, records: Vec<String>) -> Result<(), String> {
        if records.is_empty() {
            return Err("Test capture batch must contain a record".into());
        }
        let sequence = self.sequence.get();
        let batch = CaptureBatch::new(sequence, VecDeque::from(records));
        let charge = batch.charge;
        let mut usage = lock(&self.usage);
        self.sender
            .try_send(batch)
            .map_err(|error| error.to_string())?;
        usage.batches += 1;
        usage.bytes += charge.bytes;
        usage.records += charge.records;
        self.sequence.set(sequence + 1);
        Ok(())
    }
}
#[cfg(test)]
impl Drop for TestFeed {
    fn drop(&mut self) {
        if !self.publish_terminal {
            return;
        }
        *lock(&self.slot) = Some(CaptureTerminal {
            reason: CaptureStopReason::ManualStop,
            retained: VecDeque::new(),
            next_sequence: self.sequence.get(),
            received_bytes: Some(0),
            rejected_batches: 0,
        });
    }
}
#[cfg(test)]
impl CaptureInbox {
    pub fn test_channel() -> (TestFeed, Self) {
        let (sender, batches) = mpsc::sync_channel(32);
        let usage = Arc::new(Mutex::new(CaptureUsage::default()));
        let slot = Arc::new(Mutex::new(None));
        let feed = TestFeed {
            sender,
            usage: Arc::clone(&usage),
            slot: Arc::clone(&slot),
            sequence: std::cell::Cell::new(0),
            publish_terminal: true,
        };
        let inbox = Self {
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
        (feed, inbox)
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
