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
  into the editor; Send or Enter explicitly submits it to the shared device.
  There are no invented executable command presets. Failed submission preserves
  editor text. The auto-clear preference is available in Settings.
- Monitor tabs still share one capture and command destination. Their appearance
  does not establish independent sessions. Setup/help/tooltips disclose this limit.
- At narrow window widths, severity selection moves into a Levels menu. Search
  progress and failures occupy a separate wrapping status line, independent of
  optional throughput/count labels. Existing results can be an older valid
  prefix while a refresh is pending or has failed; see the
  [background query contract](engineering/BACKGROUND-QUERY.md).
- Capture, storage and archive diagnostics remain available in the scrollable
  sidebar. Closing continues to use the [orderly-close lifecycle](engineering/CLOSE-LIFECYCLE.md).

## Verification scope

Headless egui regressions check toolbar containment and overlap at 800, 1200 and
1280 logical pixels, command containment at 800 and 1200, and visible wrapped
query status at narrow widths. Other regressions cover command recall without
transmission, failed-send editor preservation, shared capture preservation and
complete themed widget states.

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
