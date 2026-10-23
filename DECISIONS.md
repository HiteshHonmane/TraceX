# TraceX Engineering Decisions

This is the repository's lightweight architecture decision record (ADR) log.
It captures decisions that would otherwise be repeatedly debated or silently
reversed.

## How to Use This Log

- Add a sequential entry for a material, hard-to-reverse decision.
- Record context and alternatives, not just the chosen technology.
- Never rewrite an accepted decision to hide history. Supersede it with a new
  entry and link both records.
- Small implementation details do not require an ADR.

## Decision 001: Native Rust Desktop Application

- **Date:** 2026-07-20
- **Status:** Accepted
- **Owners:** TraceX product and engineering

### Context

TraceX must handle continuous device traffic, very large log sessions, fast
search and filtering, low startup time, offline operation, and long-running
stability. The UI must remain responsive while transports, parsing, indexing,
and storage run independently.

### Decision

Build TraceX as a native Rust desktop application. Use `egui` through `eframe`
for the current UI and keep business logic behind application and domain
boundaries so the UI technology can evolve without rewriting the engines.

### Alternatives Considered

- **Electron:** broad web ecosystem, but unacceptable baseline memory cost and a
  browser runtime that conflicts with the native-first product principle.
- **Tauri with a web UI:** smaller shell than Electron, but retains a web UI and
  two-language boundary that does not improve the core product.
- **Qt:** mature native toolkit, but adds C++ integration and distribution
  complexity to a Rust-first codebase.

### Consequences

- The team accepts the current limitations and evolution pace of `egui`.
- UI code must remain presentation-only to prevent framework lock-in.
- Native packaging and accessibility require deliberate cross-platform testing.
- Rust ownership and concurrency design become core engineering competencies.

## Decision 002: Separate Growing Log Data from Workspace Metadata

- **Date:** 2026-07-20
- **Status:** Accepted
- **Owners:** TraceX product and engineering

### Context

TraceX sessions may run for hours or days and produce data far larger than their
workspace metadata. The product needs crash tolerance, predictable sequential
writes, rotation, recovery, and future indexing without turning one database into
a write bottleneck or a single corruption domain.

### Decision

Store continuously growing log payloads in an append-only, versioned log format.
Use SQLite for bounded workspace metadata and indexes, and use TOML or another
reviewed human-readable format for user settings. Export formats such as text,
CSV, and JSON are interchange formats, not primary storage.

The exact binary format and recovery protocol require a separate accepted design
before implementation.

### Alternatives Considered

- **Store all logs in SQLite:** simplifies querying initially, but couples the hot
  ingestion path, metadata, compaction, and corruption recovery.
- **Store only text files:** simple and portable, but insufficient for efficient
  structured access, versioning, recovery metadata, and large-session indexing.
- **Keep all session data in memory:** fast for small sessions, but unbounded and
  unable to survive crashes or restarts.

### Consequences

- Storage must coordinate log segments, metadata, and indexes without reporting
  success before durable state is consistent.
- The log format needs versioning, checksums or equivalent corruption detection,
  bounded recovery, and migration tests.
- Search indexes must be rebuildable from the append-only source of truth.
- SQLite must never become the sink for the raw continuous log stream.

## Decision 003: Evolve to a Cargo Workspace Incrementally

- **Date:** 2026-07-20
- **Status:** Accepted
- **Owners:** TraceX engineering

### Context

The current application is a functioning single Rust package, while the target
architecture defines independently testable UI, application, domain, transport,
log, and storage boundaries. Creating every target crate immediately would add
APIs and coordination before those boundaries are proven. Keeping a monolith
indefinitely would leave architecture unenforced.

### Decision

Move toward the target Cargo workspace one proven boundary at a time. Extract a
crate only when it owns cohesive behavior, has tests, benefits from independent
dependencies or consumers, and enforces a useful dependency boundary. Each step
must preserve working behavior and keep the repository buildable.

### Alternatives Considered

- **Create the full workspace immediately:** visually matches the target diagram,
  but encourages empty crates, premature APIs, and broad migration risk.
- **Keep one package permanently:** minimizes short-term movement, but cannot
  enforce the long-term dependency direction or reuse engines independently.
- **Rewrite from scratch:** discards validated behavior and creates avoidable
  product and regression risk.

### Consequences

- The current package may temporarily contain modules that correspond to future
  crates.
- Feature work should improve boundaries without opportunistic large migrations.
- Every extraction needs characterization tests and a focused review.
- Temporary structure must be described honestly as current state, not presented
  as completed target architecture.

## Decision 004: Adopt TraceX as the Product, Package, and Repository Identity

- **Date:** 2026-07-24
- **Status:** Accepted
- **Owners:** TraceX product and engineering

### Context

HI-Logger described the first UART logger prototype but no longer represents the
planned multi-transport debugging platform. Keeping separate product, package,
file, and repository names would create permanent ambiguity in builds,
documentation, support, and future extension APIs.

### Decision

Use TraceX as the product name, GitHub repository identity, desktop title, Rust
package name (`tracex`), executable name, and default generated-file prefix.
Historical Git commits and existing user log files retain their original names.
Migration must not rename or delete files already created by users.

### Alternatives Considered

- **Keep `hi_logger` internally:** avoids immediate edits but preserves confusing
  split identity in binaries, diagnostics, and future crates.
- **Rename only the UI:** creates a cosmetic rebrand while technical and support
  surfaces continue exposing HI-Logger.
- **Start a clean repository without history:** produces a visually clean start
  but discards working behavior and engineering history.

### Consequences

- New builds produce `tracex` binaries and new auto-save files use `TraceX`.
- Existing HI-Logger log files remain valid user artifacts.
- Legacy visual assets are removed from product surfaces until approved TraceX
  brand assets replace them.
- The new TraceX repository becomes the publishing destination while the
  HI-Logger repository remains historical.

## Decision 005: Versioned append-only session segments

- **Date:** 2026-10-23
- **Status:** Accepted
- **Owners:** TraceX engineering; approved by the coordinating lead under delegated architecture authority

### Context

Decision 002 requires an accepted format before persistent session implementation.
Text exports cannot preserve all record fields or identify a corrupt/truncated tail.

### Decision

Use the v1 binary framing specified in [SESSION-FORMAT](doc/engineering/SESSION-FORMAT.md):
versioned header, bounded JSON record payloads, length and CRC32-IEEE checksums.
Preserve raw/derived fields, IDs and timestamps. Read sequentially, stop at the
first invalid frame and expose the valid-prefix offset without modifying originals.
Use existing serde/serde_json and standard-library I/O; no new dependency is needed.
Fresh-file creation and asynchronous application ownership are required. Append
acceptance is not a durability guarantee; a file-owning adapter must explicitly sync.

### Alternatives Considered

- Plain text/JSON export: lacks bounded checksummed framing and complete recovery semantics.
- SQLite payload storage: conflicts with Decision 002.
- New binary serialization/checksum dependencies: unnecessary for the bounded initial codec.

### Consequences

The first implementation is a segment codec and disconnected save/open workflow,
not SQLite metadata, crash-proof live capture or a complete persistent workspace.
Unknown versions and corruption are rejected; recovery never silently rewrites files.
CRC detects accidental corruption and is not an authentication mechanism.

## Decision Template

Copy this section for future decisions.

```markdown
## Decision NNN: Title

- **Date:** YYYY-MM-DD
- **Status:** Proposed | Accepted | Superseded by NNN | Rejected
- **Owners:** Names or roles

### Context

What problem or constraint requires a decision?

### Decision

What are we choosing?

### Alternatives Considered

What credible alternatives were evaluated and why were they not chosen?

### Consequences

What becomes easier, harder, required, or prohibited?
```
