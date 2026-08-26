# Contributing to TraceX

TraceX welcomes focused, well-tested contributions that improve the debugging
experience without weakening reliability, performance, or architecture.

## Before You Start

Read `AGENTS.md` and the documents it lists. For a substantial change, open a
discussion or issue describing:

- the developer problem;
- the proposed behavior;
- the affected subsystem;
- important alternatives and tradeoffs;
- compatibility or migration concerns.

Architecture is documented before it is implemented. Significant choices belong
in `DECISIONS.md`.

## Development Workflow

1. Create a focused branch.
2. Reproduce the problem or define acceptance criteria.
3. Add or update tests with the implementation.
4. Keep commits logically scoped and explain why the change is needed.
5. Run the verification gate from `AGENTS.md`.
6. Review the work against `doc/AI_REVIEW_CHECKLIST.md`.

## Pull Request Expectations

A pull request should include:

- a concise problem statement;
- the chosen solution and relevant tradeoffs;
- tests performed;
- screenshots for visible UI changes;
- performance evidence for hot-path changes;
- documentation and decision-record updates where applicable.

Avoid mixing formatting sweeps, dependency upgrades, refactors, and feature work
in one pull request unless they are inseparable.

## Code Standards

The engineering handbook under `doc/engineering/` is authoritative. In
particular:

- keep UI code declarative;
- propagate errors with context;
- use structured logging without sensitive data;
- design blocking work away from the UI thread;
- make ownership and concurrency boundaries explicit;
- preserve Windows, Linux, and macOS as first-class targets.

## Reporting Bugs

Include the TraceX version, operating system, transport settings, reproduction
steps, expected behavior, actual behavior, and relevant sanitized logs. Remove
credentials, device identifiers, customer data, and proprietary payloads.

## Security Issues

Do not publish exploitable security details in a public issue. Contact the
maintainers privately and provide the smallest safe reproduction.
