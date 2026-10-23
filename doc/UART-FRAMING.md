# UART text framing behavior

TraceX frames incoming bytes on LF before interpreting UTF-8. Serial read
boundaries do not affect text decoding, so multibyte Unicode characters may span
reads. Empty LF-delimited records are retained. Exactly one CR immediately before
LF is removed as the CRLF delimiter; other CR bytes remain payload. A trailing
LF does not create an additional unterminated empty record.

On worker shutdown (including transport failure, command write failure, and
manual disconnect), a nonempty unterminated payload is emitted once. Timeouts do
not finish a record. Queued batches drain before the final payload held in a
separate terminal slot. The worker releases its transport before publishing the
terminal payload and closing its batch channel. The application drains all
received records before completing disconnect and closing autosave.
Manual disconnect requests cancellation without joining the worker on the UI
thread; it retains the log receiver and autosave until that drain completes.
Reconnect is rejected while the previous log receiver is still active, preventing
old records from being attached to a new connection. This guarantees orderly
disconnect delivery while the application continues polling. The
[close coordinator](engineering/CLOSE-LIFECYCLE.md) handles normal exit; crashes
and force close can still lose pending records.

A frame containing malformed or incomplete UTF-8 is rendered in full as
`[TraceX invalid UTF-8 bytes: 61 FF 00]`, with two uppercase hexadecimal digits
per original payload byte. The example represents bytes `61 FF 00`. This makes
invalid input visible and preserves all payload bytes in text exports without
adding replacement characters or discarding neighboring valid bytes. This text
representation is not a binary capture format: a device can also emit identical
literal text, and LF/CRLF delimiters are normalized rather than archived.

To prevent a device with no newline from growing the framing buffer indefinitely,
a logical line longer than 64 KiB is emitted as continuation records. No payload
bytes are discarded. An incomplete UTF-8 scalar is retained for the next record;
one extra CR byte may be held to recognize CRLF at the size boundary. Pending
payload is therefore bounded by 65,537 bytes, independent of session length.
Continuation records use the existing log record format and do not include a
continuation marker; exports cannot recover the original logical line boundaries.
This bound applies to the framer. The capture inbox separately limits queued
batches, accounted bytes and record count. Saturation stops further reads and
preserves the rejected read batch plus final fragment outside the full queue.
The application reports possible missing subsequent device output and disables
automatic reconnect. [Retained history](engineering/HISTORY-RETENTION.md) also
has a safety budget and never silently evicts records.

The pure framing implementation and regression tests live in
[`src/domain/stream_framing.rs`](../src/domain/stream_framing.rs).

Parsed ASCII tags stop at the first invalid character after a `][` marker.
This preserves the last valid tag while preventing repeated malformed markers
from causing quadratic scans of a long record. A Windows optimized standalone
comparison on 2026-10-23 used a 48,016-byte input (`"][A"` repeated 16,000 times,
then `"][FINAL] payload`), 20 parses per run, and three runs. The prior tag
scanner took 91.3–93.1 ms per record; the revised scanner took 0.069–0.070 ms.
Both returned the same tag. This isolates tag scanning; it is not a UI, full
parser, capture-throughput or physical-device benchmark.
