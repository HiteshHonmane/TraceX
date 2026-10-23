# TraceX query behavior

Date: 2026-10-23.

The main log view, exported text and keyboard copy with no selection use the same domain query. Leading and trailing query whitespace is ignored. Matching is a substring search using Unicode lowercase conversion against either the raw device line or parsed message. A raw-only match is retained even when its matching text is absent from the displayed message. This is lowercase matching, not locale-specific collation or Unicode normalization.

INFO, WARN and ERROR toggles apply together with text matching. Other severity levels remain visible. An empty or whitespace-only query shows every entry permitted by severity. Export preserves capture order and uses the existing formatted text representation.

The secondary filter-results pane uses the same severity and text contract. INCLUDE selects matching entries; EXCLUDE selects entries that do not match the text. Both modes return no entries for an empty query. The secondary mode never changes the main view, export, or no-selection keyboard copy. COPY FILTERED copies the currently displayed secondary result snapshot.

Explicit selected-row copy continues to copy selected entries in capture order, including selected entries hidden by the current query. Copy all logs continues to mean all captured entries. These intentional operations are independent of the current query.

Query logic lives in [the domain query module](../src/domain/query.rs). A bounded background worker computes match indices and widths over immutable shared records; unchanged frames reuse results. Search/severity changes invalidate old results, while live appends retain a valid prefix until the worker catches up. Both panes virtualize visible rows. See [background query ownership and measurements](engineering/BACKGROUND-QUERY.md). Indexed and disk-paged search remain unfinished. [History admission](engineering/HISTORY-RETENTION.md) limits retained records without silently evicting them.

Text export runs on a worker while disconnected, streams the same main-view query to a new file and synchronizes it before reporting success. It retains and returns ownership of the original history on success or failure. Existing files are not overwritten. Pending capture consumption waits until the export finishes.

Regression coverage combines raw-only and parsed-message matches, case/whitespace handling, severity exclusion, empty secondary queries, secondary exclusion, and application export parity. No visual layout changes are required by this behavior contract.
