# TraceX Agent Guide

This file is the entry point for every AI agent working in this repository.
TraceX is both the product and current Rust package name. Historical HI-Logger
names may appear only in migration notes or legacy Git history.

## Required Reading

Before changing production code, read these documents in order:

1. `doc/vision.md`
2. `doc/01-Product-Requirements.md`
3. `doc/02-System-Architecture.md`
4. `doc/AI_ENGINEERING_RULES.md`
5. The relevant chapters in `doc/engineering/`
6. `doc/AI_REVIEW_CHECKLIST.md`
7. `DECISIONS.md` when the task involves architecture or dependencies

Use the matching task prompt in `doc/prompts/` when planning feature, bug-fix,
refactoring, review, performance, or architecture work.

## Product Priorities

In descending order:

1. Preserve user logs and session integrity.
2. Keep the UI responsive during long-running sessions.
3. Maintain clean dependency direction.
4. Make behavior testable and understandable.
5. Extend the product without special-casing UART.

## Non-Negotiable Rules

- Do not put parsing, storage, transport, search, or session logic in UI code.
- Do not block the UI thread with I/O or heavy computation.
- Do not add dependencies without explaining the gap they fill.
- Do not use `unsafe` without a written safety argument and explicit review.
- Do not use `unwrap`, `expect`, or `panic!` in production paths.
- Do not silently discard malformed input or operational errors.
- Do not introduce global mutable state.
- Do not rewrite unrelated code while completing a focused task.
- Add regression tests for every bug fix.
- Update documentation when behavior, architecture, or public APIs change.

## Current and Target Structure

The repository is currently a single Rust package. The target architecture is a
multi-crate workspace described in `doc/02-System-Architecture.md`. Do not create
empty crates or perform a broad migration incidentally. Extract a crate only
when a feature boundary requires it and record a material decision in
`DECISIONS.md`.

## Working Method

1. Inspect the relevant code and tests before editing.
2. State assumptions and identify affected architectural boundaries.
3. Make the smallest coherent change that solves the requirement.
4. Test normal behavior, edge cases, and failure paths.
5. Review the result against `doc/AI_REVIEW_CHECKLIST.md`.

## Verification

Run the checks appropriate to the change. Before declaring a code task complete,
the default full gate is:

```text
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Documentation-only changes must at least be checked for valid local links,
consistent terminology, and stale paths.

## Scope

This guide applies to the entire repository. A more specific `AGENTS.md` may add
rules for its subtree but must not weaken these requirements.
