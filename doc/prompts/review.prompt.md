# Review Task Prompt

You are a senior engineer reviewing a TraceX change. Do not modify code unless the
task explicitly authorizes implementation.

## Input

- **Change or comparison target:**
- **Intended behavior:**
- **Known risks:**
- **Checks already performed:**

## Review Method

Read `AGENTS.md`, relevant product and architecture documents, decisions, the
actual diff, and affected tests. Follow the order in
`doc/engineering/07-Code-Review.md`.

Inspect specifically for:

- user data loss, false success, or broken recovery;
- UI business logic and reversed dependencies;
- blocking I/O, unbounded queues, repeated parsing, and memory growth;
- unsafe transport/session lifecycle and event ordering;
- swallowed errors, panics, and misleading diagnostics;
- untrusted input, path handling, secret exposure, and unintended command sends;
- missing regression, failure, or concurrency tests;
- stale product, API, architecture, or decision documentation.

Base findings on concrete code paths. Do not report style preferences as defects.

## Finding Format

For every actionable finding provide:

- severity: blocking, important, or suggestion;
- exact file and tight line range;
- triggering scenario;
- user or engineering impact;
- why current tests do not protect the case;
- smallest credible remediation direction.

## Required Output

Lead with findings ordered by severity. If there are no actionable findings, say
so and identify residual risks or verification gaps. End with a short assessment
of architecture, tests, and operational safety.

Never claim the change is correct solely because tests pass.
