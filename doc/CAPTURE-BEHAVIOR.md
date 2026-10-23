# Bounded capture ingress

The native capture worker in [capture.rs](../src/app/capture.rs) reads and frames
transport data off the UI thread. The application owns a bounded inbox of complete
read batches and an independently published terminal result. This replaces the
legacy unbounded incoming string channel.

## Initial safety budgets

These defaults are conservative interim engineering limits, not hardware or
throughput qualification:

| Resource | Default |
| --- | --- |
| Transport read | 4,096 bytes |
| Accepted inbox batches, including a partially consumed batch | 32 |
| Accounted inbox allocation | 4 MiB |
| Inbox records, including blank lines | 32,768 |
| Outgoing command queue | 16 commands |
| Complete outgoing command, including appended CRLF | 4,096 bytes |
| UI ingestion per poll | 256 records |

`CaptureBudget::validate` rejects zero/unsupported read sizes, zero/oversized
queue capacities, command limits above 4,096 bytes, and inbox budgets too small
for the maximum expanded read batch. Read sizes up to 4,096 and tiny queue limits
are injectable for tests; batch/command queue capacities above 1,024 are rejected.

Byte charges include each batch struct, allocated record-container capacity, and
owned String capacities. A batch stays charged until its final record is removed,
so partially consumed batches cannot evade limits. The conservative single-read
expansion accounts for a previous 65,537-byte framing tail, threefold hex
expansion, invalid-record markers, up to `read_bytes + 1` emitted records, and
container capacity growth. See [UART framing](UART-FRAMING.md) for encoding and
line normalization details.

The terminal reserve holds at most one rejected read batch and one final tail
record, separately from the accepted inbox. The reader also owns its fixed read
buffer and bounded framer. This accounting is not an exact process resident-memory
limit: channel synchronization/allocator overhead and OS driver/device buffers
are separate. History and storage queues have their own limits. No end-to-end
lossless or flow-control guarantee is made.

## Saturation and terminal ordering

If any inbox limit would be exceeded, the worker retains the entire rejected
batch, finishes its pending frame once, stops issuing reads/commands, releases
the port, and publishes the terminal result in an independent owned slot. It never
waits for space in the full data inbox. All records emitted by the already
completed read, including blank lines and malformed input representations, remain
owned. Queued batches drain first, followed by the rejected batch and final tail,
exactly once and in order. Status can be observed before that drain completes.

The UI shows that processing could not keep up, that received data remains
pending, and that additional device output may be missing. Automatic reconnect is
disabled after saturation, consumer loss, identifier exhaustion, history limits,
or a storage error. No loss count is invented for device/driver buffers. Captured
received-byte totals, processed-record counts, retained inbox allocation, and
known host batch rejections are distinct. A rejected batch is retained terminal
data, not silently discarded data.

The stop request remains actionable during saturation. Manual disconnect retains
the inbox, terminal payload, and autosave ordering until processing completes.
Reconnect waits for full drain and storage finalization. Clearing explicitly after
export releases history space so retained pending records can continue processing;
record IDs and batch IDs do not restart. A record that fails history preflight or
would exhaust record identifiers remains in the inbox.

Normal transport errors and command failures retain the final incoming tail and
publish an explanatory terminal reason. `Read::read` returning zero is treated as
end of stream and stops the worker, rather than spinning. Interrupted operations
retry according to the standard I/O contract; timed-out reads retain the frame.
If the data channel closes without a terminal result, the inbox reports an
unexpected worker exit, drains accepted batches, disables automatic reconnect,
and explicitly marks unfinished-frame/continuity and received-byte totals unknown.
There is no promise to recover a worker-local frame after panic or process failure.

Outgoing command enqueue is nonblocking. Full, oversized, disconnected, and
stopped-capture submissions produce visible errors and preserve editor text for
retry. Auto-clear applies only after successful enqueue. Command history is bounded
and oversized submissions never enter it. Enqueue acknowledgement is not a device
execution acknowledgement: queued commands may remain unexecuted at termination.
Partial command-write failures report the confirmed written prefix and never
blindly replay the command.

## Application boundaries and verification

The application checks history allocation and row limits before popping a record,
then respects autosave backpressure. It does not consume inbox records while an
archive/export worker temporarily owns history. A full history stops capture and
retains the triggering record; it does not evict older rows.

[Capture tests](../src/app/capture_tests.rs) cover a capacity-one stalled consumer,
whole-read blank-line saturation, near-maximum invalid frames plus final tail,
split Unicode, EOF, independent terminal ownership after consumer loss, unexpected
sender loss, partial command failure, and invalid budgets. Application tests cover
manual drain, history full/clear with stable IDs, record-ID exhaustion, command
rejection/retry, and autosave ordering. These tests use fake transports and bounded
channel/deadline synchronization without serial hardware or sleeps.

Actual UART adapters, hardware flow control, sustained throughput, supported OS
combinations, and process-memory soak behavior still require qualification.
