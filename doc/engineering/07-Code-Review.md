# Code Review Standards

Review protects users and architecture. It is not a formatting exercise or a
search for personal style preferences.

## Review Order

Review in this order so foundational problems are found early:

1. requirement and acceptance criteria;
2. user data and reliability risks;
3. architecture and dependency direction;
4. correctness and failure behavior;
5. concurrency and performance;
6. security and privacy;
7. tests and documentation;
8. readability and maintainability.

Do not spend time polishing code that solves the wrong problem.

## Author Responsibilities

Before requesting review, the author must:

- keep the change focused and explain the problem;
- describe the chosen design and significant alternatives;
- call out risky code, migrations, compatibility effects, and known limits;
- provide verification results and performance evidence when applicable;
- complete `doc/AI_REVIEW_CHECKLIST.md` honestly;
- remove debugging artifacts and unrelated edits.

## Reviewer Responsibilities

- Read the relevant product, architecture, and decision documents.
- Trace critical success and failure paths rather than reviewing only the diff's
  happy path.
- Verify that tests would fail without the intended behavior.
- Distinguish blocking correctness issues from optional suggestions.
- Explain impact and reasoning; do not prescribe a large rewrite without showing
  why the current design fails.
- Never approve unresolved data-loss, panic, security, or UI-blocking risks.

## Severity

- **Blocking:** correctness, data loss, security, architecture violation,
  unbounded resource use, missing critical test, or broken compatibility.
- **Important:** maintainability or observability weakness likely to cause defects.
- **Suggestion:** a nonessential improvement that does not block merging.
- **Question:** clarification needed before the reviewer can assess impact.

## AI Self-Review

AI-generated work must be self-reviewed before presentation. The agent should
inspect the actual diff, run applicable checks, identify remaining uncertainty,
and avoid claiming tests passed without command evidence.

## Completion

A review is complete when blocking discussions are resolved, required checks
pass, documentation and ADRs match behavior, and the change is safe to operate.
Approval is not permission to defer known critical problems.
