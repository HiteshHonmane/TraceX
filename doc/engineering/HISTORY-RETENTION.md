# Bounded history retention

The current native workspace keeps records in memory. Until persistent paged
history is integrated, it must not grow indefinitely or silently evict evidence.

The application uses `HistoryBudget` with interim safety defaults of 100,000
records and 64 MiB of conservatively accounted retained allocations. These are
protective configuration defaults, not measured capacity or throughput promises.
Tests inject smaller limits; production defaults remain centralized in the type.

The limits apply independently to each retained session. Workspace admits at most
two owners including closing ones, bounding this history accounting to 128 MiB
across those owners, with separate query/queue/UI overhead. Capacity failure in
one owner does not block the other's ingestion. See [workspace](WORKSPACE.md).

Admission checks happen before a record is removed from the capture inbox. The
preflight reserves raw string capacity, up to the raw length for each parsed
message and tag, 64 bytes for the current timestamp, and four times the size of
`LogEntry` for record/shared-ownership and vector allocation. Committed accounting
uses actual owned string capacities and the same conservative allowance. History
stores immutable shared records; the allowance covers the record object, Arc
metadata and shared-handle vector growth. Arithmetic cannot wrap.

On refusal, capture stops, automatic reconnect is disabled, and the triggering
record remains owned by the bounded inbox. Existing history is unchanged. The
application reports the capacity stop. Save Session can preserve all retained
history fields while the stopped inbox remains pending; filtered text export
preserves only its matching formatted subset. After a successful retained-session
snapshot, clear history deliberately to make space for remaining received records.
The snapshot does not include that pending data; admitting it marks history dirty
again. A failed save retains the original records and unread inbox. Clearing does not
automatically reconnect and does not reuse record IDs. Additional device output
after the stop may be missing; no claim of UART flow-control losslessness is made.

This is an accounting bound for retained history, not a total process RSS bound.
The capture worker, autosave queues, parsing temporaries, query/selection vectors,
renderer and allocator have separate overheads. Background queries can temporarily
retain older immutable records after Clear while their bounded executing/latest
pending requests are cancelled; see [query ownership](BACKGROUND-QUERY.md).
Opening a saved session requires
a fully drained capture and an empty workspace after deliberately saving/exporting and clearing existing
history. The candidate session is subject to the same history budget and is
installed only after the entire file validates.

Captured records are always retained after admission. The dormant `paused` flag
had no UI setter and previously allowed internal callers to discard visible
records; it has been removed. Pausing scrolling remains a presentation concern.

Safe shutdown must surface a history limit that prevents complete draining and
offer explicit user choices. Session save/reopen and eventual paged durable
storage must preserve this admission contract rather than silently dropping data.
