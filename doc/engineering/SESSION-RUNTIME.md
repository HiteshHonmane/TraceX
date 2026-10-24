# Session runtime extraction

Decision 007 first extracted [`SessionRuntime`](../../src/app/session.rs) without
changing the shared-view behavior. The subsequent slice replaces that intermediate
shell with [`Workspace`](../../src/app/workspace.rs), retaining up to two real
independent runtimes including closing owners. The shell owns the port catalog,
startup/font preparation, asynchronous UI picker controller and workspace. See
the [workspace contract](WORKSPACE.md) for identity, polling and close rules.

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
Workspace reconnect admission; the runtime no longer owns the shared discovery
catalog or independently reopens a port.
Serial opening remains synchronous with the same timeout and capture startup.
Workspace assigns a frame-wide budget of at most 256 records, shared fairly
with one bounded redistribution pass. Runtime polling accepts an explicit share;
the zero-argument 256-record helper is test-only.
No additional transport, worker queue, durable-storage guarantee or general
message bus was introduced.

The [retention](HISTORY-RETENTION.md), [archive](SESSION-FORMAT.md),
[query](BACKGROUND-QUERY.md), [autosave](AUTOSAVE-WORKER.md) and
[close](CLOSE-LIFECYCLE.md) contracts retain their behavior. Save still snapshots
only retained records while preserving a stopped pending inbox. Normal close
still drains capture, waits for archive/autosave completion and obtains the
dirty-history decision. Tab close now retains its runtime through capture, archive,
autosave and query-worker completion plus its own dirty decision. It cannot
force-drop an owner. Window close coordinates every runtime; explicit global
force exit remains the exception. See [Decision 007](../../DECISIONS.md#decision-007-explicit-session-ownership-and-bounded-workspace).

Existing core tests construct the real runtime directly. UI startup tests keep
the actual shell/CreationContext font preparation path. QA's controlled two-owner
characterizations demonstrate runtime ownership isolation. Workspace regressions
now exercise the real collection/routing/lifecycle. Historical stage-1 evidence:
the integrated extraction passed formatting, warnings-denied
all-target Clippy, all 130 tests and a release build. A separate native instance
launched successfully, reopened the complete 18-record synthetic archive and
returned the expected three `timeout` matches. Its accepted UI geometry and
typography remained visible. This smoke check does not qualify hardware capture,
display scaling, independent sessions or sustained-load performance.
