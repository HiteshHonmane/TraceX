//! Ordered, bounded autosave commands. File operations run only on the worker.
use super::storage::{create_timestamped_log_file, should_rotate_log};
use chrono::Local;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};

/// Configuration takes effect in queue order and creates a fresh output file.
#[derive(Clone, Debug)]
pub(super) struct AutosaveConfig {
    pub base_path: PathBuf,
    pub limit_mb: u64,
    pub sequence: u32,
}

/// One complete text payload, including its desired newline.
#[derive(Clone, Debug, serde::Serialize)]
pub(super) struct WriteRequest {
    pub id: usize,
    pub payload: String,
}

/// Recovery preserves full payloads and explicitly records possible duplicates.
#[derive(Debug, serde::Serialize)]
pub(super) struct RecoveryRecord {
    pub request: WriteRequest,
    pub written_prefix_bytes: usize,
}

/// Commands are applied in submission order. Finish rejects subsequent commands.
#[derive(Debug)]
pub(super) enum AutosaveCommand {
    Start(AutosaveConfig),
    Write(WriteRequest),
    SetLimit(u64),
    ExportRecovery {
        path: PathBuf,
        records: Vec<RecoveryRecord>,
    },
    Stop,
    Finish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AutosaveOperation {
    Create,
    Write,
    Sync,
    RecoveryExport,
}

/// A failed write returns its whole original request and a confirmed prefix count.
/// Retrying the complete payload can duplicate that prefix; recovery is explicit.
#[derive(Debug)]
pub(super) struct AutosaveFailure {
    pub operation: AutosaveOperation,
    pub error: io::Error,
    pub written_prefix_bytes: usize,
}

#[derive(Debug)]
pub(super) enum AutosaveEvent {
    Started {
        path: PathBuf,
        next_sequence: u32,
    },
    Written {
        id: usize,
        bytes: usize,
        current_bytes: u64,
    },
    Stopped,
    LimitUpdated,
    RecoveryExported {
        path: PathBuf,
        count: usize,
    },
    Failed {
        command: AutosaveCommand,
        failure: AutosaveFailure,
    },
    Finished {
        failure: Option<AutosaveFailure>,
    },
}

/// A refused command is returned intact for caller-owned pending retention.
#[derive(Debug)]
pub(super) enum SubmitError {
    Full(AutosaveCommand),
    Closed(AutosaveCommand),
    Finishing(AutosaveCommand),
}

/// Nonblocking application facade. Keep polling events through Finished and thread
/// completion. Dropping this facade early is cancellation, not durable shutdown.
pub(super) struct AutosaveWorker {
    commands: SyncSender<AutosaveCommand>,
    events: Receiver<AutosaveEvent>,
    thread: JoinHandle<()>,
    finishing: bool,
}

impl AutosaveWorker {
    #[cfg(test)]
    pub(super) fn spawn_gated(
        command_capacity: usize,
        event_capacity: usize,
        entered: mpsc::Sender<()>,
        release: Receiver<()>,
    ) -> io::Result<Self> {
        let mut first = true;
        Self::spawn_with_factory(
            command_capacity,
            event_capacity,
            Box::new(move |base, sequence| {
                if first {
                    first = false;
                    entered
                        .send(())
                        .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
                    release
                        .recv_timeout(std::time::Duration::from_secs(3))
                        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?;
                }
                let (file, path, next_sequence) =
                    create_timestamped_log_file(base, Local::now(), sequence)?;
                Ok(OpenedFile {
                    sink: Box::new(file),
                    path,
                    next_sequence,
                })
            }),
        )
    }

    pub fn spawn(command_capacity: usize, event_capacity: usize) -> io::Result<Self> {
        Self::spawn_with_factory(
            command_capacity,
            event_capacity,
            Box::new(|base, sequence| {
                let (file, path, next_sequence) =
                    create_timestamped_log_file(base, Local::now(), sequence)?;
                Ok(OpenedFile {
                    sink: Box::new(file),
                    path,
                    next_sequence,
                })
            }),
        )
    }

    fn spawn_with_factory(
        command_capacity: usize,
        event_capacity: usize,
        factory: FileFactory,
    ) -> io::Result<Self> {
        if command_capacity == 0 || event_capacity == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Autosave queue capacities must be positive",
            ));
        }
        let (commands, command_rx) = mpsc::sync_channel(command_capacity);
        let (event_tx, events) = mpsc::sync_channel(event_capacity);
        let thread = thread::Builder::new()
            .name("tracex-autosave".to_owned())
            .spawn(move || {
                run_worker(command_rx, event_tx, factory);
            })?;
        Ok(Self {
            commands,
            events,
            thread,
            finishing: false,
        })
    }

    /// Never waits for storage or queue space. Retry a returned command in order.
    pub fn try_submit(&mut self, command: AutosaveCommand) -> Result<(), SubmitError> {
        if self.finishing {
            return Err(SubmitError::Finishing(command));
        }
        let is_finish = matches!(command, AutosaveCommand::Finish);
        match self.commands.try_send(command) {
            Ok(()) => {
                self.finishing = is_finish;
                Ok(())
            }
            Err(TrySendError::Full(command)) => Err(SubmitError::Full(command)),
            Err(TrySendError::Disconnected(command)) => Err(SubmitError::Closed(command)),
        }
    }

    /// Drain events before submitting additional work to release backpressure.
    pub fn try_event(&self) -> Result<AutosaveEvent, TryRecvError> {
        self.events.try_recv()
    }

    /// Poll after Finished; this also detects unexpected worker termination.
    pub fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }
}

trait SaveSink: Write + Send {
    fn sync_data(&mut self) -> io::Result<()>;
}
impl SaveSink for File {
    fn sync_data(&mut self) -> io::Result<()> {
        File::sync_data(self)
    }
}
struct OpenedFile {
    sink: Box<dyn SaveSink>,
    path: PathBuf,
    next_sequence: u32,
}
type FileFactory = Box<dyn FnMut(&Path, u32) -> io::Result<OpenedFile> + Send>;

struct Session {
    config: AutosaveConfig,
    output: OpenedFile,
    bytes: u64,
    faulted: bool,
}
struct WriterState {
    session: Option<Session>,
    factory: FileFactory,
}

impl WriterState {
    fn sync(&mut self) -> Result<(), AutosaveFailure> {
        if let Some(session) = &mut self.session {
            session.output.sink.sync_data().map_err(|error| {
                session.faulted = true;
                failure(AutosaveOperation::Sync, error, 0)
            })?;
        }
        Ok(())
    }

    fn start(&mut self, config: &AutosaveConfig) -> Result<AutosaveEvent, AutosaveFailure> {
        self.sync()?;
        // Keep the previous (synced) session on create failure; writes still fault.
        if let Some(session) = &mut self.session {
            session.faulted = true;
        }
        let output = (self.factory)(&config.base_path, config.sequence)
            .map_err(|error| failure(AutosaveOperation::Create, error, 0))?;
        let event = AutosaveEvent::Started {
            path: output.path.clone(),
            next_sequence: output.next_sequence,
        };
        self.session = Some(Session {
            config: config.clone(),
            output,
            bytes: 0,
            faulted: false,
        });
        Ok(event)
    }

    fn write(
        &mut self,
        request: &WriteRequest,
        notifications: &SyncSender<AutosaveEvent>,
    ) -> Result<Vec<AutosaveEvent>, AutosaveFailure> {
        let Some(session) = &mut self.session else {
            return Err(failure(
                AutosaveOperation::Write,
                io::Error::new(io::ErrorKind::NotConnected, "Start autosave before writing"),
                0,
            ));
        };
        if session.faulted {
            return Err(failure(
                AutosaveOperation::Write,
                io::Error::other("Autosave requires an explicit restart after failure"),
                0,
            ));
        }
        let mut events = Vec::with_capacity(2);
        if should_rotate_log(
            session.bytes,
            request.payload.len(),
            session.config.limit_mb,
        ) {
            session.output.sink.sync_data().map_err(|error| {
                session.faulted = true;
                failure(AutosaveOperation::Sync, error, 0)
            })?;
            let output = (self.factory)(&session.config.base_path, session.output.next_sequence)
                .map_err(|error| {
                    session.faulted = true;
                    failure(AutosaveOperation::Create, error, 0)
                })?;
            notifications
                .send(AutosaveEvent::Started {
                    path: output.path.clone(),
                    next_sequence: output.next_sequence,
                })
                .map_err(|_| {
                    failure(
                        AutosaveOperation::Create,
                        io::Error::from(io::ErrorKind::BrokenPipe),
                        0,
                    )
                })?;
            session.output = output;
            session.bytes = 0;
        }
        let bytes = request.payload.as_bytes();
        let mut written = 0;
        while written < bytes.len() {
            match session.output.sink.write(&bytes[written..]) {
                Ok(0) => {
                    session.faulted = true;
                    return Err(failure(
                        AutosaveOperation::Write,
                        io::Error::from(io::ErrorKind::WriteZero),
                        written,
                    ));
                }
                Ok(count) => written += count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    session.faulted = true;
                    return Err(failure(AutosaveOperation::Write, error, written));
                }
            }
        }
        session.bytes = session.bytes.saturating_add(written as u64);
        events.push(AutosaveEvent::Written {
            id: request.id,
            bytes: written,
            current_bytes: session.bytes,
        });
        Ok(events)
    }
}

fn failure(
    operation: AutosaveOperation,
    error: io::Error,
    written_prefix_bytes: usize,
) -> AutosaveFailure {
    AutosaveFailure {
        operation,
        error,
        written_prefix_bytes,
    }
}

fn export_recovery(path: &Path, records: &[RecoveryRecord]) -> Result<(), AutosaveFailure> {
    #[derive(serde::Serialize)]
    struct RecoveryDocument<'a> {
        notice: &'static str,
        records: &'a [RecoveryRecord],
    }
    let write = || -> io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        serde_json::to_writer_pretty(&mut file, &RecoveryDocument {
            notice: "Complete unsaved payloads. The original autosave file may already contain each record's confirmed written prefix; replaying whole records can duplicate those bytes.",
            records,
        }).map_err(io::Error::other)?;
        file.sync_data()
    };
    write().map_err(|error| failure(AutosaveOperation::RecoveryExport, error, 0))
}

fn run_worker(
    commands: Receiver<AutosaveCommand>,
    events: SyncSender<AutosaveEvent>,
    factory: FileFactory,
) {
    let mut state = WriterState {
        session: None,
        factory,
    };
    while let Ok(command) = commands.recv() {
        let outcome = match &command {
            AutosaveCommand::Start(config) => state.start(config).map(|event| vec![event]),
            AutosaveCommand::Write(request) => state.write(request, &events),
            AutosaveCommand::SetLimit(limit_mb) => {
                if let Some(session) = &mut state.session {
                    session.config.limit_mb = *limit_mb;
                }
                Ok(vec![AutosaveEvent::LimitUpdated])
            }
            AutosaveCommand::ExportRecovery { path, records } => export_recovery(path, records)
                .map(|()| {
                    vec![AutosaveEvent::RecoveryExported {
                        path: path.clone(),
                        count: records.len(),
                    }]
                }),
            AutosaveCommand::Stop => state.sync().map(|()| {
                state.session = None;
                vec![AutosaveEvent::Stopped]
            }),
            AutosaveCommand::Finish => {
                let failure = state.sync().err();
                // A terminal event means the file has been closed, even if sync failed.
                state.session = None;
                let _ = events.send(AutosaveEvent::Finished { failure });
                return;
            }
        };
        let notifications = match outcome {
            Ok(notifications) => notifications,
            Err(failure) => vec![AutosaveEvent::Failed { command, failure }],
        };
        for event in notifications {
            if events.send(event).is_err() {
                return;
            }
        }
    }
    // Sender loss is best-effort finalization, not a substitute for Finish.
    let failure = state.sync().err();
    state.session = None;
    let _ = events.send(AutosaveEvent::Finished { failure });
}

#[cfg(test)]
#[path = "autosave_worker_tests.rs"]
mod tests;
