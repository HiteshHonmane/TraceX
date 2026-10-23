# Session segment format v1

Date: 2026-10-23. Status: accepted as Decision 005 by the coordinating lead under delegated architecture authority. Scope: segment codec and disconnected save/open; not connected to continuous capture or workspace metadata. Complements Decision 002; does not replace existing text auto-save.

## Representation

All integers are unsigned little endian. A segment starts with 12 bytes: ASCII `TRACEXSG` (8 bytes), version `u16` equal to 1, reserved `u16` equal to zero. Other versions/reserved bits are rejected rather than guessed.

Each frame is a payload length `u32`, CRC32-IEEE `u32`, then exactly that many payload bytes. Maximum payload is 1 MiB; zero length is invalid. CRC covers the four encoded length bytes followed by payload. Polynomial is reflected `0xEDB88320`, initial value `0xFFFFFFFF`, final XOR `0xFFFFFFFF`. CRC is for accidental corruption, not authenticity against malicious edits.

Payload is UTF-8 JSON for a private wire schema: `id` as `u64`, `timestamp` string, `level` enum, nullable `tag`, `message` string and `raw` string. All fields are required; unknown fields are rejected. Raw means the supplied record's `raw` field, not an exact serial byte archive: upstream framing may normalize delimiters and represent invalid bytes as hex. Preserve every supplied field without reconstructing/reparsing it. Converting a wire ID beyond platform `usize` is an explicit error.

Binary framing, versioning and checksum make this an append-only segment format with JSON payloads, not a plain JSON export. No external compression/checksum dependency is needed. Standard-library CRC32 favors inspectability; throughput should be measured before optimization.

## Reader and valid prefix

Reader uses a caller-owned `Read`, not a path. It validates the header at construction, streams one bounded frame at a time, and yields owned records. Clean EOF at a frame boundary is a clean end. An incomplete frame header/payload is a truncated tail. Bad length, checksum or JSON/schema is corruption. Every terminal problem includes the last complete valid-prefix byte offset; construction errors imply no accepted header.

Truncation exactly at a complete frame boundary is indistinguishable from a shorter clean segment without an external checkpoint/manifest. This version does not contain a footer or expected-record count, so it cannot detect missing whole trailing records.

Stop at the first problem, including middle-of-file corruption. Do not scan for a later frame, skip malformed bytes, truncate or rewrite the original. After a terminal error, subsequent calls return end-of-stream. Recovery copies the validated prefix into a different file only under future explicit application orchestration; the reader itself is read-only.

One bounded payload is allocated per frame. There is no method collecting the entire file. The caller owns total record retention/cancellation policy. Length validation happens before allocating. The maximum payload constrains serialized JSON bytes, not decoded object accounting or whole-application resident memory.

## Writer and durability

Writer uses a caller-owned fresh `Write` stream, emits the header once, then appends frames. Caller must create a new file safely (for example `create_new`); the codec does not append to existing segments. Serialization/size errors happen before writing a frame. An I/O error poisons the writer; subsequent appends/flushes must fail, since a partial frame may exist. No silent retry or continuation over a damaged tail.

Successful append means the `Write` interface accepted all bytes. `flush` means only the sink's flush succeeded; neither establishes durable media persistence. File-owning infrastructure must expose `sync_data`/checkpoint semantics before any durable-session claim. Crash after an acknowledgement can still lose OS-cached bytes. Reader recovery detects a valid prefix but cannot guarantee acknowledged bytes survived.

## Required tests and integration gate

Round-trip fields including ANSI, Unicode, blank and unusual raw/message combinations; fixed CRC known vector; every byte truncation offset; complete header-only segment; unsupported version/reserved/magic; bad/zero/oversized length before payload read; checksum and valid-checksum malformed-schema payload; rejected oversized serialization leaves stream unchanged; partial header/frame write and flush failure poison state; explicit ID conversion boundaries.

Register the module only after architecture acceptance. No SQLite, UI, transport, file deletion/repair or broad crate extraction is part of this codec. Full repository formatting, warnings-denied Clippy, tests and release build are required before integration completion. Fault-injected codec tests establish codec behavior, not real-device capture or production crash durability.

## Disconnected archive workflow

The application archive worker creates fresh `.tracex` files, writes every retained record, flushes and calls file `sync_data` before reporting save success. It does not promise filesystem directory-entry durability or hardware guarantees. A failed save may leave a partial new file; it never overwrites an existing file. Original in-memory records return on success or failure and remain available for a new-path retry. The UI moves the original vector into the worker handoff instead of cloning it and shows operation-in-progress status.

Save/Open require capture disconnected/drained, autosave stop completed, no archive operation and no shutdown underway. Capture/clear must be guarded while archive work runs. Open requires empty current history, validates the complete candidate before restoring it, rejects every detected truncated/corrupt file, and applies the shared default provisional HistoryBudget (100,000 records / 64 MiB conservative allocation accounting) before retaining each decoded record. IDs must strictly increase and leave room for the next ID. Successful open resets selection/scroll target and derives next ID; failure leaves the application history empty and existing selection unchanged. The candidate's accounting limit is not a 64 MiB process-RSS guarantee. Recovery prefix inspection is a future workflow; no implicit file repair occurs.

Open additionally requires empty current history: save/export the current records, then deliberately Clear before opening another session. A request with nonempty history is rejected before spawning a worker or changing records/selection. This avoids replacing unsaved work through an Open action.

The native sidebar exposes Save Session/Open Session. These actions save retained records, not records no longer present in memory or the full device byte stream. Workers are polled without joining on the UI thread; archive-specific failures remain visible to shutdown coordination until a successful subsequent operation.

## Asynchronous text export

Text Export uses the same move-and-return worker ownership. It snapshots the Main query (trimmed case-insensitive raw/message match and severity) before dispatch, formats matching records on the worker and streams via a bounded `BufWriter` to a fresh `create_new` file. It separates lines with LF without a trailing LF, matching previous text export semantics; successful completion includes flush and file `sync_data`. Existing paths are never overwritten. Failure returns the original vector and an explicit error, even if a partial new export exists.

Export requires a disconnected transport, no active archive and no shutdown. Unlike Save/Open Session it allows stopped capture with pending unread batches: this lets a history-limit user export retained evidence, deliberately Clear, then drain the next received records. Application polling pauses ingestion while archive ownership is in flight. Export describes the retained records at invocation, not pending capture records. It does not change explicit selected-row clipboard semantics, and clipboard formatting is outside this worker change.

Successful Save Session/Open Session clears the application's unsaved-history flag. Text export leaves it unchanged because filters and formatted output may omit evidence. All failures preserve its prior value. Shutdown waits for archive completion before deciding whether unsaved session confirmation is needed.
