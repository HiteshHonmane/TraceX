# Rust Standards

These rules apply to all production and test Rust code in TraceX.

## Toolchain and Edition

- Use the Rust edition declared by each package; the current package uses 2024.
- Code must pass `rustfmt` and Clippy with warnings denied.
- Do not rely on nightly features in production code without an accepted ADR.
- When an MSRV is established, declare it in Cargo manifests and CI together.

## Design for Clarity

- Prefer small cohesive types over manager objects with unrelated duties.
- Model valid states with enums and newtypes instead of flags and sentinel values.
- Make illegal states difficult to construct.
- Keep functions focused. Extract policy from orchestration and I/O.
- Prefer explicit control flow over clever iterator chains when error or state
  behavior would become harder to read.
- Avoid generic abstractions until at least two real consumers need them.

## Ownership and Allocation

- Borrow data when the callee does not need ownership.
- Accept `&str` rather than `&String`, and slices rather than `&Vec<T>`.
- Clone only at an intentional ownership boundary; hot-path clones require review.
- Use `Arc` only for genuine shared ownership, not to avoid designing ownership.
- Keep locks short-lived and never hold a lock across I/O, callbacks, or UI work.

## APIs and Visibility

- Default to private. Use `pub(crate)` before `pub` when external stability is not
  required.
- Public APIs must document purpose, errors, side effects, and important
  invariants.
- Constructors should validate input and return a valid object or a typed error.
- Traits represent stable capabilities or replaceable boundaries, not every type.
- Avoid boolean parameters when an enum makes intent clearer.

## Naming and Layout

- Follow Rust conventions: `snake_case` modules/functions, `CamelCase` types and
  traits, and `SCREAMING_SNAKE_CASE` constants.
- Name types for their responsibility: `SessionStore`, not `DataManager`.
- Do not create catch-all `utils`, `helpers`, or `common` modules.
- Keep imports explicit. Avoid glob imports outside tightly scoped test modules.
- Place tests close to private logic; use `tests/` for public integration behavior.

## Error and Control Flow

- Follow `03-Error-Handling.md`.
- `unwrap`, `expect`, `panic!`, `todo!`, and `unimplemented!` are prohibited in
  reachable production paths.
- Ignoring a `Result` must be explicit, justified, and observable when failure
  affects users or data.

## Unsafe Rust

TraceX is safe Rust by default. Any proposed `unsafe` block requires:

1. evidence that a safe design cannot meet the requirement;
2. a written safety invariant adjacent to the block;
3. focused tests, including boundary and failure cases;
4. explicit human review and an ADR for a systemic use.

Keep unsafe code in the smallest possible module behind a safe API.

## Dependencies

Before adding a crate, document the capability gap, maintenance status, license,
platform support, transitive cost, and credible alternatives. Prefer existing
dependencies and the standard library when they solve the problem clearly.

## Completion Gate

Rust changes are not complete until formatting, Clippy, tests, and the relevant
build targets pass, or a concrete environmental blocker is reported.
