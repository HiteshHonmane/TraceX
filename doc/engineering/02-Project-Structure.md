# Project Structure

This chapter defines how TraceX moves from its current single package toward the
target workspace without creating speculative crates or a distributed monolith.

## Current State

The current package is `tracex`. `src/domain/` contains framework-independent
domain behavior while `src/app/` temporarily contains UI, storage, theme, and
application orchestration modules. Treat this as a working migration baseline,
not as the final architecture described in `doc/02-System-Architecture.md`.

Do not perform a full workspace migration as incidental cleanup. Each extraction
must preserve behavior, have tests, and establish a real dependency boundary.

## Dependency Direction

Code dependencies flow inward:

```text
UI -> Application -> Domain contracts <- Infrastructure implementations
```

- **UI** renders view state and dispatches intent.
- **Application** coordinates use cases, sessions, and events.
- **Domain contracts** define stable types, invariants, and capabilities.
- **Infrastructure** implements transport, storage, indexing, and OS integration.

Infrastructure may depend on domain contracts. Domain code must not depend on
`egui`, serial-port libraries, file dialogs, or platform APIs.

## Target Workspace Boundaries

The target crates in the architecture document are a direction, not a quota.
Create one when it has independent behavior, tests, and a stable API. Likely
early boundaries are:

| Boundary | Owns | Must not own |
| --- | --- | --- |
| UI | rendering, input mapping, view state | parsing, storage, transport I/O |
| Core | identifiers, timestamps, log events, domain invariants | GUI or OS code |
| Transport | connection lifecycle and byte streams | parsing or presentation |
| Log engine | bounded ingestion and query-facing log data | serial configuration |
| Storage | workspace metadata and durable log writes | UI notifications |
| Application | use-case orchestration and event routing | device-specific details |

## Module Rules

- A module owns one concept and exposes the smallest useful surface.
- Parent modules define the public facade; implementation details remain private.
- Sibling modules communicate through narrow types or an application coordinator,
  not by reaching into each other's state.
- Events are immutable facts. Commands are explicit requests.
- Keep domain events independent of UI widget types and transport-library types.
- Never use a global event bus as a substitute for clear ownership.

## Extraction Criteria

Extract a crate when most of these are true:

- the boundary has a stable responsibility;
- it can be tested without the desktop UI;
- it needs different dependencies or concurrency behavior;
- at least two consumers may use it;
- compile-time enforcement prevents an actual architecture violation.

Do not extract solely to reduce file length or mirror an aspirational diagram.

## File Growth

Prefer files below 400 lines. At 600 lines, review responsibilities and split if a
cohesive boundary exists. No production file should exceed 1,000 lines without a
documented, temporary exception.

## Cross-Platform Placement

Keep platform-specific implementations behind a common trait in a clearly named
module such as `platform/windows.rs`. Do not scatter conditional compilation
through domain or UI logic.
