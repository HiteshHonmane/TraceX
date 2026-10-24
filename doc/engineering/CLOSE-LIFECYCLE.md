# Orderly window close

Current behavior, 2026-10-23. Implemented by [the close coordinator](../../src/app/shutdown.rs), [autosave lifecycle API](../../src/app/autosave.rs), and [native UI wiring](../../src/app/ui.rs).

The coordinator now belongs to each independent session. Tab close retains that
owner through capture/archive/autosave and query-worker thread completion plus
its dirty decision; it has no individual force-drop action. Workspace window
close coordinates every owner, including inactive ones, with generation-scoped
decisions and an explicit global force-exit exception. Cancelling window close
resets all owners (including Ready and superseded tab requests), leaving captures
stopped. See [the full workspace lifecycle](WORKSPACE.md) for final-owner identity
reservation, cap accounting, query cancellation and stale-response rules.

Both the custom window X and operating-system close event start the same application-owned shutdown sequence. Operating-system close is cancelled while the sequence runs. The coordinator requests capture stop once, disables automatic reconnect, and lets normal nonblocking polling drain already received records into retained history and autosave. It does not discard the capture inbox to make closing appear complete.

After the capture inbox is fully drained, the controller queues the autosave `Finish` command behind pending payloads. Shutdown requires both the terminal `Finished` event and observed worker-thread completion; the UI never joins a worker. Accepted autosave writes and stop commands retain their FIFO order. In-flight session archive save/open operations must also complete before orderly close is allowed. A worker's successful finalization is not a guarantee against later filesystem or power failure.

A modal reports progress without changing the log workspace layout. A capture error blocks close while the capture inbox/lifecycle is still owned; retained-history capacity blocking that drain also produces an explicit incomplete-close decision. Historical continuity warnings remain visible after capture has fully drained, but do not alone imply current pending work or force an incomplete-close decision. Unresolved autosave and archive failures still block close independently. A ten-second decision deadline also reports incomplete shutdown if capture, search, filesystem or storage work has not finished. This deadline bounds waiting for a user decision; it does not forcibly interrupt system calls or assert pending data was saved.

**Keep open** cancels the close request and leaves capture stopped. The application continues polling outstanding work, preserving pending records and recovery payloads for review/export. If history capacity blocked draining, Save Session preserves all retained fields while pending inbox records remain owned; deliberately clear the saved history to make room before retrying close. Pending records are outside that snapshot and become dirty history when admitted. Filtered text export may omit evidence and must not be represented as a complete session save. If storage failed, use the recovery export controls and resolve the failure. A finalizing autosave worker must finish before a new capture is permitted; after it ends, autosave can start a fresh worker.

**Force close — pending logs may be lost** is a deliberate exit decision. It permits closing despite incomplete work and may lose pending capture, recovery or storage data. No success or saved status is inferred from force close. An operating-system process kill, machine shutdown or power loss can bypass this coordinator entirely; crash recovery and durable session indexing remain separate requirements.

Regression coverage includes injected-time capture/Finish/archive sequencing, deadline and failure decisions, Keep-open reset, an actual queued autosave payload followed by close, saturated autosave pending-write ordering, failed-create completion, and a headless egui OS-close cancellation test. Autosave worker tests separately inject final sync failure. Hardware unplug/close, clean Windows launch and very slow/full/network filesystem tests remain manual integration gates.

Retained session history also has an independent dirty state. Capturing an entry marks history dirty. After all capture, autosave and archive work has finished, nonempty dirty history requires a separate confirmation: **Keep open** or **Close without saving session**. The dialog explains that text autosave/export may exist but a reopenable TraceX session has not been saved. A successful Save Session or Open Session marks history clean; failed save/open preserve its previous state. Filtered/formatted text export and text auto-save do not make history clean. More capture after saving makes it dirty again. Deliberate Clear resets the state with the cleared history. This normal unsaved-session confirmation is distinct from the incomplete-work **Force close** decision.
