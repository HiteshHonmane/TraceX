# Native workspace UI

The visual authority is the owner's [Figma frame 9:2](https://www.figma.com/design/QbApGHd6YRTcyoyJQxQYqF/Untitled?node-id=9-2).
The native implementation uses neutral charcoal surfaces, compact controls and
painted icons. It remains an egui/eframe application; no browser UI is embedded.
Source changes and headless layout tests are not proof of visual fidelity.
Rendered comparison, exact typography and multiple display scales remain pending.

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

`cargo run --example ui_review_fixture -- NEW_OUTPUT.tracex` creates a clearly
synthetic 18-record archive for native review using the production codec. It
checks complete field roundtrip and known query results and refuses overwrite.
This fixture is not hardware capture or evidence of completed desktop testing.

Actual window rendering, keyboard routing, file dialogs, display scaling,
physical UART and sustained-load qualification must be recorded separately.
