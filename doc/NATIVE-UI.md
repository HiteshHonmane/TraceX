# Native workspace UI

The visual authority is the owner's [Figma frame 9:2](https://www.figma.com/design/QbApGHd6YRTcyoyJQxQYqF/Untitled?node-id=9-2).
The native implementation uses neutral charcoal surfaces, compact controls and
painted icons. It remains an egui/eframe application; no browser UI is embedded.
Source changes and headless layout tests are not proof of visual fidelity.
Rendered comparison is performed against the approved frame, including the
actual Figma typography inspector. Multiple display scales remain pending.

The application embeds Inter Regular/Medium/Bold and Liberation Mono Regular;
startup does not read machine-specific font files. The wordmark, field labels,
selector values, navigation, active tabs and command editor use the roles
verified in Figma. Upstream asset versions, source URLs and SHA-256 hashes are
recorded in the [Inter manifest](../assets/fonts/inter/provenance.json) and
[Liberation manifest](../assets/fonts/liberation/provenance.json). Both complete
font licenses are embedded under Help → Third-party font licenses, so notices
remain available with a standalone executable. Existing fallback faces remain;
this change does not provide complete multilingual shaping.
Font registration occurs in eframe's creation callback before the first pass;
registering named families during the first update is too late. A regression
exercises the complete first and second updates through the production
preparation path and checks the first-frame named-font wordmark.

## Actions and states

- File and the bottom Library menu expose Open Session and Save Session through
  the same application capabilities. Save preserves all currently retained
  records; a stopped inbox can still contain records outside that snapshot.
  Open additionally requires drained capture and empty retained history. See
  [session format and workflow](engineering/SESSION-FORMAT.md).
- Export is labeled **Export filtered text**. It uses the current search and
  severity filters and must not be treated as complete session preservation.
- Command history contains real attempts. Selecting a history item recalls it
  into that session's editor; Send or Enter submits to the displayed session and
  captured connection generation through workspace admission.
  There are no invented executable command presets. Failed submission preserves
  editor text. The owner-scoped line-ending dropdown offers the existing
  **NL & CR** (append CRLF) and **NONE** (unchanged bytes) choices; it follows
  the owner's edit capability. Selection alone never sends a command. The
  auto-clear preference is available in Settings.
- Monitor tabs now represent distinct retained session owners, with a provisional
  cap of two including closing sessions. Titles remain stable; lifecycle cues and
  an independent `*` identify unsaved retained history. Sidebar configuration,
  records, errors, query/selection, editor and autosave settings follow the owner.
  The command history rail persistently names the session and actual connected
  endpoint, or the named disconnected/archive owner. Archive filename detail is
  available in scoped hover text; Open never reconnects or replaces another owner.
- UI rendering borrows only the active runtime. Every retained owner continues
  workspace-owned polling within the aggregate ingestion budget. Editor, scroll,
  Library/Settings popup, tab and focus IDs use stable session identity rather
  than title or vector position. Destructive/admission actions are collected with
  their origin ID and dispatched after rendering; no active-owner fallback is used.
- All five native picker kinds use one bounded asynchronous controller: Open,
  Save, filtered export, recovery export and autosave location. A pending picker
  does not block the frame or workspace polling. Concurrent picker requests are
  rejected visibly. Completion carries the original session ID and is admitted
  again by the workspace; cancelled/unavailable selection starts no operation.
  A late result for a removed or closing owner cannot mutate a replacement tab.
- Tab Close requests core drain/finalization and retains the owner until its own
  records, storage, search work and unsaved-history decision are complete. Named
  dialogs offer Keep session open or Close without saving session as appropriate.
  There is no force-tab-close action. Window Close addresses all owners; Keep
  application open cancels aggregate requests while leaving captures stopped.
  Only a generation-validated incomplete window-close decision offers explicit
  force application exit with its pending-data loss warning. Closing the last
  owner uses core's fresh-ID replacement policy; tabs never directly drop runtimes.
- At narrow window widths, severity selection moves into a Levels menu. Search
  progress and failures occupy a separate wrapping status line, independent of
  optional throughput/count labels. Existing results can be an older valid
  prefix while a refresh is pending or has failed; see the
  [background query contract](engineering/BACKGROUND-QUERY.md).
- Workspace notifications have an explicit Dismiss action. It clears only that
  shell notification; session archive/capture/recovery errors, drafts and close
  blockers remain intact. Successful unrelated actions do not silently erase it.
- Capture, storage and archive diagnostics remain available in the scrollable
  sidebar. Closing continues to use the [orderly-close lifecycle](engineering/CLOSE-LIFECYCLE.md).

## Verification scope

Headless egui regressions check toolbar containment and overlap at 800, 1200 and
1280 logical pixels, full command-rail containment at all three widths, and visible wrapped
query status at narrow widths. Other regressions cover command recall without
transmission, failed-send editor preservation, independent widget state and owner-bound action routing, and complete themed
widget states. Workspace regressions additionally cover lifecycle/resource/routing
invariants; see [workspace ownership](engineering/WORKSPACE.md).

Native review on 2026-10-23 verified the second candidate at 1200 x 800:
separate prompt/editor text, dark severity pills, subtle Follow selection,
52-point toolbar, and distinct history/composer surfaces. Opening the synthetic
archive displayed all 18 records; searching `timeout` displayed three, and
disabling INFO left two while retaining all 18. Library correctly disabled Open
while history was present. Saving with those filters active preserved the entire
archive byte-for-byte. Enter with no connected device displayed an explicit
error and preserved the command text. The saved review instance closed normally.
These results precede the subsequent typography pass and do not establish
physical-device, minimum-window or display-scaling acceptance.

The final typography candidate passed formatting, warnings-denied all-target
Clippy, 122 tests and a release build. It launched successfully after the startup
ordering correction. Native review reopened the previously saved complete
18-record archive and repeated the three-match search with visible focus/caret.
Both embedded font license sections were opened and scrolled. Independent design
and QA reviews accepted the demonstrated layout, palette, prompt separation and
typography corrections; these scoped results are not full product acceptance.

The existing text renderer still lacks complete Unicode font coverage and
complex-script shaping. The fixture exposes replacement glyphs for Chinese,
Devanagari and the crab emoji; the archived Unicode data remains intact. Do not
claim complete multilingual rendering based on successful archive roundtrips.

`cargo run --example ui_review_fixture -- NEW_OUTPUT.tracex` creates a clearly
synthetic 18-record archive for native review using the production codec. It
checks complete field roundtrip and known query results and refuses overwrite.
This fixture is not hardware capture or evidence of completed desktop testing.

Actual window rendering, keyboard routing, file dialogs, display scaling,
physical UART and sustained-load qualification must be recorded separately.


The two-owner UI/workspace integration is a later milestone than the native
review results above. Focused headless UI tests now exercise actual first/second
application updates, inactive-owner capture polling, editor/popup state across
switching and lower-index closure, independent rendered log scroll offsets,
captured/stale command destinations, session-named close decisions, and selected
search-text clipboard behavior. Controlled pending picker futures prove both
owners continue ingesting across frames; completed and late removed-owner
results verify origin-only mutation and explicit rejection without native input.
These are deterministic regression results. Native evidence below is separate
from physical UART, sustained-load and display-scale qualification.

The historical integrated two-owner checkpoint passed formatting, all-target Clippy with
warnings denied, all 154 tests and a release build on 2026-10-23. The separate
executable is `target/session-ui/release/tracex.exe` (SHA-256
`42E5C9A1AF94E0AE9A25AB05B0B20EE0C16AAB9EE0E5CC4E9AD7BFD21E5D4055`).
Desktop verification initially stopped when the owner pressed Escape before
that build was launched. The owner resumed native review on 2026-10-24 against
the later `a1c96c2` capture-lifetime build, for which the lead reports formatting,
warnings-denied Clippy, 155 tests and a release build passed.

The lead's native review used synthetic session archives: A retained 18 records
with three `timeout` matches; B retained four with two `voltage` matches and its
own draft. B's native picker completed while A was active and still targeted B.
Saving B while filtered preserved all four records; the saved archive was byte
identical to its fixture (SHA-256
`B404DA29D9526C4E0FFF552A5BF83DFE602FF2E7F2051AE4EF623F3A21FAF284`).
Cleanly closing A preserved B's title, query and draft. A new Monitor 3 opened
the saved B archive with all four records and an empty query/draft. This supports
those native owner/picker/archive/close workflows; it does not establish device
sends, physical UART, multiple display scales or sustained-load acceptance.

The subsequent focused visual correction uses 28-point command-rail horizontal
insets, a 92-point ending dropdown, 24-point tab-title inset and a 12-point
configuration heading with 0.7-point letter spacing. Heading size/tracking are
estimates from the approved raster, not verified Figma inspector tokens. Four
new headless regressions pass, including actual dropdown interaction, owner
isolation and unchanged command byte behavior. The lead's integrated formatting,
warnings-denied all-target Clippy, 159 tests and release build passed. Because E:
was full, the separate executable is
`C:/Users/Admin/.codex/tmp/tracex-ui-fidelity-oct24/release/tracex.exe`, SHA-256
`4F56DD287DFC5383E4B176BB8D01E418453888532CC34200C34FD7F37DC72D69`.
Existing running builds were preserved.

The corrected build launched and was reviewed at 1200 x 800. The real ending
popup showed both supported choices above the footer. Selecting NONE in Monitor 1,
creating Monitor 2 with its default NL & CR, then returning to Monitor 1 preserved
the independent choices without sending a command. Reopening the synthetic
18-record archive and entering `timeout` displayed three matches while retaining
all 18 records. Design and QA independently reviewed the corrected empty/popup
screenshots and found no blocking defect in those views. Evidence is saved in
`C:/Users/Admin/.codex/tmp/tracex-ui-review-oct24/` (empty, ending-popup,
monitor2-ending-default, monitor1-ending-restored, reopened and search JPEGs).
The approximately 20-point leading File-menu spacing difference remains a minor
fidelity follow-up; full pixel parity is not claimed. A corner-drag attempt did
not resize the native window, so minimum-height and multi-DPI acceptance remain
pending alongside Unicode coverage and hardware/soak qualification.
