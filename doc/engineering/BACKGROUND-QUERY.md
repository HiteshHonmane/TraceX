# Background query and view snapshots

The native log panes previously scanned the complete retained history and measured
message widths every frame. The secondary pane also constructed every matching row,
and selected-row clipboard text was formatted even when no copy was requested.
These costs grew with the session while the user was merely scrolling or typing.

## Ownership and execution

Each independent session owns its own query controller and worker. Workspace
polls inactive completions but prepares fresh snapshots only for the active
session. Completion rejects an invalidated key immediately, even before a later
refresh increments generation. Closing owners cancel their worker and retain
their cap slot until actual thread completion; Keep open permits fresh querying
after cancellation without changing search text. See [workspace](WORKSPACE.md).

History now contains immutable `Arc<LogEntry>` records (`SharedLog`). Capture parses
one record and wraps it once. Session save/export still moves and returns the same
history vector; reopen wraps each decoded record once. No query snapshot clones
timestamp, raw, tag or message strings. The session file format is unchanged.

[The worker](../../src/app/query_worker.rs) owns query matching, include/exclude
indices and message-width calculation. It has one executing request, one latest
pending request and one latest result slot. Replacing a request cancels obsolete
work between records. Mutexes protect only state handoff, never scanning. There
is no UI-thread join; stop, unexpected termination and poisoned state are explicit.

[The application controller](../../src/app/query_view.rs) assigns generations and
accepts results only for the current query/history identity. Clear, history
replacement and archive handoff invalidate prior views, including equal-length
history replacement. Search text/severity changes immediately hide old results.
For appends, a valid earlier prefix stays visible with a searching indicator while
the next snapshot is computed. Appends do not repeatedly cancel an in-flight scan,
so continuous capture cannot starve it through repeated resubmission. An unchanged
history/query reuses the same result without rescanning.

Both panes render only visible fixed-height rows. Width comes from the worker
snapshot. Copy formatting happens on explicit copy intent; row menus no longer
preformat aggregate selections on every frame. A stopped worker is shown as a
search failure while captured records remain available for save/export.

## Bounds and limits

Snapshots copy O(n) shared handles when requesting new work. Results contain at
most one main index and one excluded index per retained record, with disjoint
sets. Source history still obeys its record/byte admission limit. Clear/reopen can
temporarily retain older records through executing/pending query handles; the
current history budget is not a total process memory cap. Superseded requests
release ownership, and cancellation bounds the number of retained generations.

This is background linear scanning, not an indexed or disk-paged search engine.
It retains the existing trimmed Unicode-lowercase substring/severity semantics.
Clipboard formatting, capture parsing and serial connection opening still have
UI-thread work requiring separate improvements. Query preparation and freeing
large collections also need real frame-time/soak qualification. No million-row,
device-throughput or end-to-end UI latency claim follows from this milestone.

## Reproducible evidence

Run `cargo run --release --example query_frame_baseline -- 100000` on Windows.
The fixed 100,000-record fixture has 4,850,000 raw bytes and passes the current
history budget. Three runs on 2026-10-23, no explicit warmup:

| Work | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| Previous main/exclude scans plus widths, ms | 27.128 | 25.103 | 26.149 |
| Shared-handle snapshot preparation, ms | 0.365 | 0.366 | 0.363 |
| Request handoff, ms | 0.009 | 0.004 | 0.004 |
| Preparation + worker + result polling, ms | 16.550 | 15.881 | 17.214 |
| Cached result metadata access, ns | 7.664 | 7.641 | 7.693 |

The old and new paths produce exactly 25,000 matching and 75,000 excluded indices
and identical widths. Cached access averages 100,000 iterations. Worker creation
and actual GUI rendering are excluded; these are synthetic CPU timings, not frame
latency or UART measurements. The previous path measures both panes and widths,
so it must not be compared directly with the older single-query benchmark.

Tests cover latest-request replacement, cancellation, shared ownership release,
worker failure, stale-result rejection, append visibility and unchanged-frame
reuse. A headless egui regression presents 10,000 secondary matches in a small
viewport and verifies only a bounded visible subset is rendered while preserving
selection. Archive tests verify the same vector, Arc and raw-string allocation
survive save success and failure.
