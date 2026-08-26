# Documentation Standards

Documentation is part of the product and implementation. It must describe the
current system clearly enough for a new engineer or agent to act safely.

## Sources of Truth

- `doc/vision.md` defines product direction.
- `doc/01-Product-Requirements.md` defines the problem and product needs.
- `doc/02-System-Architecture.md` defines target boundaries and constraints.
- `DECISIONS.md` records material choices and their history.
- `doc/engineering/` defines implementation standards.
- Rust documentation describes public APIs and local invariants.
- `README.md` explains how users and contributors start.

When sources conflict, correct them in the same change instead of choosing the
most convenient statement.

## Writing Rules

- Write for a reader who was not present for the decision.
- Lead with purpose, then constraints, behavior, and examples.
- Use precise terms consistently: TraceX is the product and `tracex` is the
  current Rust package name. HI-Logger is a historical name only.
- Prefer durable concepts over screenshots of rapidly changing implementation.
- State whether content describes current behavior, a target, or a proposal.
- Use relative repository paths and verify that links resolve.
- Use UTF-8 and avoid decorative characters that render inconsistently.

## Code Documentation

Public types and functions document purpose, parameters, return behavior, errors,
side effects, invariants, and examples when they clarify correct use. Comments
explain why a non-obvious choice exists, not what straightforward code does.

Safety-sensitive and performance-sensitive code must document assumptions close
to the implementation.

## Change Requirements

Update documentation when a change affects:

- user-visible behavior or configuration;
- architecture or dependency direction;
- persistent formats or migrations;
- public APIs or extension contracts;
- build, test, release, or support workflows;
- approved engineering policy.

Do not document speculative features as shipped behavior.

## Decision Records

Use `DECISIONS.md` for choices that are expensive to reverse, constrain multiple
subsystems, introduce a foundational dependency, or settle a recurring debate.
Supersede decisions; do not erase history.

## Review

Documentation review checks technical truth, navigability, local links,
terminology, examples, and whether required action is unambiguous. A document is
not complete merely because it is grammatically polished.
