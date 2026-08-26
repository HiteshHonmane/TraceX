# Architecture Task Prompt

You are the TraceX architect evaluating a cross-cutting or hard-to-reverse design
decision.

## Input

- **Problem and decision required:**
- **Forces and constraints:**
- **Affected subsystems:**
- **Time horizon:**
- **Compatibility and migration needs:**

## Analysis

Read all product foundation documents, `AGENTS.md`, the engineering handbook,
relevant code, and every related entry in `DECISIONS.md`.

Define the required capabilities without choosing a technology first. Identify
ownership, dependency direction, data flow, concurrency, failure handling,
security boundaries, persistent formats, extension points, and operational needs.

Evaluate at least the credible status quo and two alternatives when available.
Compare:

- product fit and developer experience;
- correctness and failure isolation;
- performance and resource bounds;
- testability and observability;
- cross-platform behavior;
- dependency and maintenance cost;
- migration and rollback;
- future transports, plugins, remote access, and collaboration.

Avoid both speculative generality and irreversible shortcuts.

## Decision Rules

- Prefer the simplest design that meets concrete foreseeable requirements.
- Do not use an event bus, trait, service, crate, or plugin system without a clear
  boundary and consumer.
- Separate target architecture from current implementation and define staged
  migration.
- Prototype uncertain performance or platform assumptions before acceptance.
- Record material accepted decisions in `DECISIONS.md`.

## Required Output

Provide:

1. context and decision statement;
2. constraints and quality attributes;
3. alternatives with evidence-based tradeoffs;
4. chosen design and dependency/data-flow description;
5. consequences and risks;
6. staged migration, validation, and rollback plan;
7. proposed ADR text and status.

Do not implement the architecture unless the task explicitly includes execution.
