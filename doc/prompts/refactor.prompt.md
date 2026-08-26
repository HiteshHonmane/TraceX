# Refactoring Task Prompt

You are an engineer improving TraceX's internal structure without intentionally
changing user-visible behavior.

## Input

- **Structural problem:**
- **Evidence of cost or risk:**
- **Behavior that must remain unchanged:**
- **Desired boundary or simplification:**
- **Explicit non-goals:**

## Before Editing

Read `AGENTS.md`, architecture, relevant handbook chapters, current code and tests,
and related decisions. State the invariant behavior and how it will be verified.

Identify whether the problem is duplication, mixed responsibility, dependency
direction, unclear ownership, oversized state, poor testability, or premature
abstraction. Do not refactor solely for aesthetic preference.

## Refactoring Rules

- Preserve observable behavior and persistent formats.
- Separate mechanical movement from semantic changes when possible.
- Improve dependency direction; never add a facade that merely hides coupling.
- Extract a crate only when `02-Project-Structure.md` criteria are satisfied.
- Prefer deleting indirection and duplication over adding framework abstractions.
- Keep public API changes deliberate and documented.
- Do not combine a product feature with an unbounded cleanup.

## Verification

Run existing characterization tests before and after the change. Add tests where
behavior was previously implicit or unsafe to move. Compare performance when a
hot path or allocation pattern changes.

## Required Output

Provide:

1. structural problem and evidence;
2. preserved behavior and invariants;
3. resulting ownership and dependency boundaries;
4. verification results;
5. any intentional API or performance effect;
6. remaining cleanup, clearly separated from this task.

Complete `doc/AI_REVIEW_CHECKLIST.md` before marking the task done.
