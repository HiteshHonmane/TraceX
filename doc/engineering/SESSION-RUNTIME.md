# Session runtime extraction

Decision 007 is implemented in stages. This first increment moves the existing
single live owner into [`SessionRuntime`](../../src/app/session.rs). The native
shell owns the port catalog, startup/font preparation and shared-view tab labels;
it owns exactly one runtime. Independent sessions, workspace admission, stable
session/connection IDs and aggregate multi-owner closure are not enabled yet.

The runtime owns transport/inbox state, retained immutable records and admission
budget, query controller and investigation state, archive handoff, autosave
controller/settings/recovery, command draft/history/options, shutdown coordinator,
rate counters and reconnect eligibility. [`session_capture.rs`](../../src/app/session_capture.rs)
contains its existing connection, disconnect and ordered polling operations.
Construction starts no capture, query or storage worker and performs no port
enumeration or log-file access. Default autosave path discovery stays unchanged.

Native drawing code explicitly borrows the runtime and uses its plain fields and
application methods. The runtime stores no egui context/widget/window type and
does not perform clipboard operations, render settings/status, or show a file
dialog. Clipboard adapters consume `copy_selected_text`, `copy_all_text` and
`query_rows_text`. File dialogs submit paths to the existing archive operations;
those operations recheck application admission before starting any worker.

Autosave UI adapters use owner-derived backlog/recovery status and core mutations.
`set_auto_save_limit_mb` enforces 1..=4096 MB, matching the existing UI range;
invalid input or queue refusal preserves the previous value. This does not
change the storage helper's internal zero/unlimited meaning for other tested
callers. `set_auto_save_path` preserves the original path and rotation sequence
when pending/finalizing work or restart refusal prevents changing the destination.
Accepted settings remain ordered with the existing worker commands.

The shell supplies a read-only snapshot of discovered ports to
`check_auto_reconnect`; the runtime no longer owns the shared discovery catalog.
Serial opening remains synchronous with the same timeout and capture startup.
The polling loop still admits at most 256 records per call for the sole runtime.
No additional transport, worker queue, durable-storage guarantee or general
message bus was introduced.

The [retention](HISTORY-RETENTION.md), [archive](SESSION-FORMAT.md),
[query](BACKGROUND-QUERY.md), [autosave](AUTOSAVE-WORKER.md) and
[close](CLOSE-LIFECYCLE.md) contracts retain their behavior. Save still snapshots
only retained records while preserving a stopped pending inbox. Normal close
still drains capture, waits for archive/autosave completion and obtains the
dirty-history decision. Shared-view tab close still removes a view label rather
than dropping the runtime. Replacing that behavior requires the later complete
workspace lifecycle accepted in
[Decision 007](../../DECISIONS.md#decision-007-explicit-session-ownership-and-bounded-workspace).

Existing core tests construct the real runtime directly. UI startup tests keep
the actual shell/CreationContext font preparation path. QA's controlled two-owner
characterizations demonstrate runtime ownership isolation, not a delivered
multi-session UI. The integrated extraction passed formatting, warnings-denied
all-target Clippy, all 130 tests and a release build. A separate native instance
launched successfully, reopened the complete 18-record synthetic archive and
returned the expected three `timeout` matches. Its accepted UI geometry and
typography remained visible. This smoke check does not qualify hardware capture,
display scaling, independent sessions or sustained-load performance.
