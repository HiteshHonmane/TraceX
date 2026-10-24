# Asynchronous autosave worker

## Current implementation

Each independent session owns its controller, private worker channel, destination
settings, backlog and recovery payloads. Workspace polls inactive and closing
owners; one owner's storage failure does not stop another capture. Deferred
picker/settings/recovery intents carry the originating SessionId and pass workspace
close admission. See [workspace](WORKSPACE.md) for the retained-owner resource cap.

TraceX text autosave runs through
[`AutosaveWorker`](../../src/app/autosave_worker.rs), coordinated by
[`autosave.rs`](../../src/app/autosave.rs). File creation, directory creation,
log writes, rotation, data synchronization, recovery export, and file closure run
on the storage worker. The application only formats a line, submits commands with
`try_send`, consumes typed events, and renders state.

This is an incremental boundary inside the existing package. It does not change
the target append-only binary storage decision in [DECISIONS.md](../../DECISIONS.md).
TXT autosave remains a compatibility output, not a versioned raw-byte archive.

## Ordering and lifecycle

Commands use a bounded `sync_channel`: `Start(config)`, `Write(request)`,
`SetLimit(megabytes)`, `Stop`, `ExportRecovery`, and terminal `Finish`. All commands
execute in accepted order. Start creates a fresh file without requiring a first
log record. Reconfiguration synchronizes the preceding file before opening the
new destination. Failed creation or synchronization faults the old session so
later queued writes cannot accidentally reach the previous destination.

Existing timestamped naming, exclusive `create_new` behavior, collision handling,
and rotation thresholds are reused from [storage.rs](../../src/app/storage.rs).
A nonempty file rotates before a new payload would exceed the configured MiB
limit; an oversized first payload remains whole. Zero retains the helper's
existing unlimited-size meaning. Changing the limit queues an ordered update.
Normal text payloads retain the existing formatted line and trailing LF.

`Written` acknowledges bytes accepted by the OS write API, not per-line durable
storage. Stop synchronizes file data and closes the file before its `Stopped`
event. A failed stop is visible and faults further writes until explicit Start.
Transport disconnect first drains complete records and the final fragment, then
queues Stop behind their writes. Automatic reconnect waits for that stop result.

`Finish` rejects later submissions, processes earlier accepted commands,
synchronizes and closes the file, emits a terminal event (including any sync
failure), and exits. The API never joins or waits on the UI thread: callers poll
`try_event` and `is_finished`. A shutdown coordinator must use a finite deadline
and visibly report an incomplete finalization rather than wait indefinitely.
Blocking OS filesystem calls cannot be safely cancelled by this API.

The [close coordinator](CLOSE-LIFECYCLE.md) intercepts normal window closure,
stops capture, drains received records, then waits for Finish acknowledgement
and worker completion. Failures or a ten-second deadline require an explicit
keep-open or force-close decision. Drop remains best effort for abnormal teardown;
process termination, crashes and force close can still lose pending data.

## Backpressure and failures

The initial application budgets are 64 commands and 64 result events, plus one
caller-owned pending command. Queue refusal returns the original owned command.
When the command queue fills, the caller retains one pending write and stops
removing records from the transport receiver until that command is accepted.
The UI displays the backlog state. Configuration controls cannot overtake a
pending command; a busy setting change is rejected visibly and the setting is
restored. Each UI poll consumes at most 256 transport records.

Result events are also bounded. If the UI stops draining them, the worker waits
and the command queue fills, propagating backpressure without blocking the UI.
These are command-count budgets, not byte budgets. The framing limit bounds
current UART payload sizes; arbitrary future callers must enforce their own
payload-size contract. The transport now uses bounded batches and a separate
terminal payload slot; [retained history](HISTORY-RETENTION.md) has an admission
budget. Storage failure stops capture and freezes consumption while autosave is
enabled. Already received records remain pending. These accounting bounds are
not a total process RSS guarantee or device-side flow-control guarantee.

A write failure returns the complete original request, including text and record
identifier, plus the confirmed prefix byte count. The application retains failed
payloads and shows a failure. Accepted writes queued behind a faulted session
are returned as failures rather than written to the wrong file or discarded.
The original request for every successful write is not retained separately.
Unexpected worker/process termination can therefore still lose accepted data.

Normal capture stops at the first pending request or observed failure. Failed
payload retention is bounded by the in-flight worker queues and single pending
slot during this capture flow; calling the private record adapter repeatedly
while bypassing `can_ingest_log` is not a supported producer contract.

## Explicit recovery export

The status area provides **Export unsaved payloads** when failure payloads are
retained. The worker writes a new JSON file using `create_new`, synchronizes it,
and acknowledges success. Existing destinations are never overwritten. An export
failure leaves the original retained payloads available for another attempt.

Each JSON record includes the complete original text payload, its record ID,
and `written_prefix_bytes`. The file explains that replaying an entire failed
record can duplicate bytes already written to its original destination. This is
an explicit recovery artifact; TraceX does not automatically retry partial writes.
Only records covered by the successful export acknowledgement are released from
failure retention. Later failures remain available for another export.

Captured records remain in visible history after admission; the unused internal
paused-record discard path has been removed. Ordinary filtered text export is not a
substitute for recovery export. Recoveries remain in memory until exported and
are not crash safe. If the worker itself terminates unexpectedly, recovery export
through that worker may be unavailable; worker restart and exit recovery remain
follow-up work for unexpected worker termination. Normal exit coordination is
implemented separately.

## Verification

[Worker tests](../../src/app/autosave_worker_tests.rs) use controlled in-memory
sinks and channel handshakes for ordering, rotation, partial failures, saturation,
creation failure, terminal synchronization failure, and failed Start/Stop
followed by queued writes.
[Controller tests](../../src/app/autosave_tests.rs) cover asynchronous enable and
creation, ordered stop, queue saturation with one retained write, manual final
tail drain, failed creation, visible payload retention, and recovery export with
existing-file protection. Temporary filesystem fixtures use no serial hardware.
