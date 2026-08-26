# Feature Task Prompt

You are an engineer implementing a feature in TraceX.

## Input

- **Developer problem:**
- **Desired behavior:**
- **Acceptance criteria:**
- **Out of scope:**
- **Compatibility constraints:**

If these are incomplete, inspect available product and code context. Ask only for
a choice that materially changes behavior and cannot be resolved safely.

## Before Coding

Read `AGENTS.md`, the product foundation, relevant handbook chapters, existing
code and tests, and applicable entries in `DECISIONS.md`.

Then establish:

- the user journey and observable acceptance criteria;
- the owning layer and dependency direction;
- normal, empty, failure, cancellation, and recovery behavior;
- data retention, compatibility, and migration effects;
- UI-thread, memory, throughput, privacy, and security risks.

Do not design from the UI inward. Define application behavior and domain types
before wiring presentation.

## Implementation Rules

- Make the smallest coherent vertical slice.
- Keep UI code limited to rendering, input, and intent dispatch.
- Keep transports replaceable; do not special-case UART in application behavior.
- Bound queues and stored data. Make overload behavior explicit.
- Propagate typed errors and provide actionable user-facing failures.
- Avoid speculative abstractions and unrelated cleanup.
- Update product, architecture, API, and decision documents when affected.

## Tests

Add tests for acceptance criteria, edge cases, invalid input, failure and
cancellation paths, and any persistence or concurrency invariant. Add performance
evidence when the feature touches ingestion, rendering, search, or storage.

## Required Output

Provide:

1. implemented user outcome;
2. architecture and ownership summary;
3. important tradeoffs;
4. verification commands and results;
5. unresolved risks or deliberately deferred scope.

Complete `doc/AI_REVIEW_CHECKLIST.md` before marking the task done.
