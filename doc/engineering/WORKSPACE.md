# Independent session workspace

Decision 007 is implemented by [workspace routing](../../src/app/workspace.rs),
[close coordination](../../src/app/workspace_close.rs) and independently owned
[session runtimes](../../src/app/session.rs). These are application modules in
the existing Rust package; no new crate, dependency or mutable global was added.

## Identity and admission

Workspace retains at most two owners, counting inactive and closing sessions.
Session IDs and close-request IDs are checked monotonic u64 identities; exhaustion
rejects explicitly without wrapping/reusing identity or evicting existing data.
Titles remain stable `Monitor N`, independent of vector order or archive filename.
Source path is metadata set only after a successful validated Open. Archive
connection/settings restoration and persistent workspace metadata are not provided.
Deliberate Clear removes that source metadata with the cleared history while
leaving the Monitor title stable. Rejected Clear and failed Open preserve it.

Each owner has separate transport/inbox, history/accounting, query/selection,
command draft/history/options, archive/autosave and shutdown state. The provisional
history budget remains 100,000 records / 64 MiB accounted allocations per owner;
two owners therefore allow 128 MiB of that accounting plus separately bounded
queues, query generations, UI and allocator overhead. This is not a process RSS
or throughput guarantee. Reaching a limit stops its owner without evicting logs.

Connection admission checks owner/window close, capture/storage/history state,
selected endpoint, endpoint occupancy and connection-ID availability before I/O.
The serial adapter still opens synchronously with its existing timeout. A
successful reader startup publishes its connection generation and occupied
endpoint; failed open/start does not consume a generation or claim an endpoint.
The endpoint stays occupied through stopped-inbox draining and is released only
when capture ownership is fully drained. Reconnect uses the same workspace
admission, and cannot take another session's live or draining endpoint.
Capture drain also requires observed reader-thread completion. An unexpectedly
unwinding reader can close its data sender before a slow transport destructor
returns; channel closure alone cannot release the endpoint or retained owner.
The inbox retains the worker handle and polls completion without joining it.

Send/Enter carries `CommandTarget { SessionId, ConnectionId }` captured from the
rendered owner/connection. Workspace validates it before accessing the sender.
Stale/unknown/closing targets reject without falling back to the active session.
Known-owner refusal preserves its draft; queue admission applies only its own
auto-clear preference. Admitted commands remain bounded and private to that
capture worker; no replay occurs on tab selection or reconnect. Queue acceptance
is not device acknowledgement, successful execution or a per-command durable log.

## Polling and asynchronous ownership

Each frame polls every retained owner's capture/archive/autosave and query
completion, including inactive and closing owners. One 256-record budget is
shared across the workspace. The first owner rotates; first shares are 128 each
with two owners. A single bounded second pass redistributes unused slots, so
one busy owner beside an idle archive may ingest 256. Two backlogged owners
receive 128 each. There is no unbounded drain or 256-times-owner-count allowance.

Only the active session prepares fresh O(n) query snapshots. Inactive completions
return to their private `QueryView`; worker results never resolve the active tab.
Clear, archive handoff and history replacement invalidate the query key and
snapshot immediately. Completion polling rejects an invalidated key even before
a later active refresh increments generation, including equal-length replacement.

Archive save/open/export and recovery settings are dispatched with the origin
SessionId. Receivers and moved record vectors remain with that runtime while
another is selected or capturing. Save preserves every retained field regardless
of the visible filter; stopped inbox records stay pending and outside that
snapshot. Their later admission marks only that owner dirty. Open requires its
addressed owner drained and empty; corrupt input preserves original state.

Native pickers belong to a bounded UI-shell controller. They must not block the
workspace frame poll while another owner captures. Completion carries the
captured origin ID and revalidates workspace admission before file work. A closed
origin cannot silently apply a result to a new/active owner. These UI pickers are
distinct from background archive workers that actually read/write records.

## Closing and cancellation

A tab X requests close by stable ID; it never erases the owner vector directly.
The session stops capture/reconnect and requests query cancellation. Its tab and
cap slot remain owned while received records, archive work and autosave FIFO
Finish acknowledgement/thread completion are pending. Query stop must also be
observed through thread completion before removal, releasing retained snapshots
without UI-thread joining. Keep open can finish cancellation and start a fresh
query worker for the unchanged query, without a permanent stopped-search error.

Dirty retained history receives a session-named unsaved-session decision only
after pending work is complete. `CloseWithoutSavingSession` accepts only that
current state and close-request generation. Incomplete work keeps the owner
available for Keep open/recovery; there is no per-tab force-drop path. A final
owner's fresh replacement identity is reserved before the old owner is removed.
If allocation is exhausted, that close is cancelled and the existing owner is
reset to Open with an explicit error, retaining logs and Save/query access.

Window close requests every retained owner, superseding tab requests with fresh
decision generations. Creation/reconnect/command/file mutations are blocked while
closing; navigation remains possible for inspection. Decisions identify the
affected owner in tab order; approving one dirty owner cannot approve another.
Native Close is permitted only when all owners are Ready, or through the existing
explicit global force-exit exception. Force exit validates the displayed
`CloseTarget` and current incomplete decision; stale replies cannot authorize a
later close. There is no claim that forced exit or process termination preserves
pending records or guarantees crash durability.

Keep application open clears the aggregate intent and every participating close
request/coordinator, including already-Ready inactive owners and superseded tab
requests. Captures remain stopped and their evidence remains owned; reconnect
does not restart implicitly. Keep session open cancels only its tab request,
or safely cancels the aggregate if invoked with a current window-close target.

## Verification boundary

Controlled tests use real Workspace/SessionRuntime values, private worker channels,
batched capture feeds and gated query/archive completions. They prove addressed
routing, owner isolation, budget/cap behavior, cancellation and lifecycle rules;
they are not native rendering, hardware unplug/reconnect or long-running memory
measurements. See the [workspace regressions](../../src/app/workspace_tests.rs),
[UI state tests](../../src/app/ui/state_tests.rs), and [native UI contract](../NATIVE-UI.md)
for executable coverage and desktop qualification scope. The lead owns the
integrated Rust gate and actual desktop checks.
