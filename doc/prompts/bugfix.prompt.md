# Bug-Fix Task Prompt

You are an engineer fixing a verified TraceX defect.

## Input

- **Observed behavior:**
- **Expected behavior:**
- **Reproduction steps or evidence:**
- **Affected versions/platforms:**
- **Data-loss or security impact:**

## Diagnose First

Read `AGENTS.md`, relevant documentation, code, tests, and recent decisions.
Reproduce the defect when feasible. Trace the failure to its first incorrect state
or violated invariant; do not stop at the visible symptom.

Distinguish among:

- product misunderstanding;
- UI state or event ordering;
- transport lifecycle;
- parsing or filtering;
- concurrency, backpressure, or shutdown;
- storage, rotation, or recovery;
- platform-specific behavior.

## Fix Rules

- Add a regression test that fails for the original defect.
- Correct the owning layer, not a downstream symptom.
- Keep the patch focused; do not mix broad refactoring into an urgent fix.
- Preserve valid stored data and compatibility.
- Do not silence errors, increase unbounded buffering, add sleeps, or add retries
  without a defined transient-failure policy.
- If the written requirement caused the defect, update it with the fix.

## Verification

Run the regression test, relevant subsystem tests, and the full gate when
practical. For races or intermittent defects, run a bounded repeated or stress
test and explain its limitations.

## Required Output

Provide:

1. root cause;
2. user impact and risk assessment;
3. fix and why it addresses the root cause;
4. regression coverage;
5. verification results;
6. remaining uncertainty or release considerations.

Complete `doc/AI_REVIEW_CHECKLIST.md` before marking the task done.
