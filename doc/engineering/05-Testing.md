# Testing Standards

Tests protect product behavior, data integrity, and architectural boundaries.
They are designed with the implementation, not added after it.

## Test Layers

- **Unit tests:** invariants, parsing, filtering, rotation policy, and state
  transitions without external resources.
- **Component tests:** one subsystem with controlled adapters such as an in-memory
  transport or temporary storage.
- **Integration tests:** public behavior across real module boundaries.
- **End-to-end smoke tests:** a small set of critical desktop workflows.
- **Benchmarks and soak tests:** throughput, memory stability, and long-running
  behavior for performance-sensitive engines.

Prefer the lowest layer that proves the requirement.

## Required Coverage

Every change covers applicable cases:

- expected behavior;
- boundary values and empty input;
- malformed or partial input;
- cancellation and disconnection;
- I/O and permission failure;
- repeated operation and idempotency;
- regression reproduction for a fixed bug.

Coverage percentage is a signal, not a completion target. Critical state and data
paths must be tested even when line coverage is already high.

## Determinism

- Inject clocks, randomness, transports, and storage boundaries where needed.
- Do not depend on local serial devices, global user directories, network access,
  test execution order, or wall-clock sleeps.
- Use temporary directories and clean ownership of test artifacts.
- Replace timing guesses with explicit synchronization or observable state.

## Test Design

- Name tests by behavior and condition, for example
  `rotates_before_writing_past_configured_limit`.
- Keep arrange, act, and assert phases obvious.
- Use representative fixtures that are small enough to understand.
- Avoid duplicating production algorithms inside assertions.
- Assert public outcomes and invariants rather than private implementation steps.

## Concurrency Tests

Test shutdown, cancellation, queue saturation, producer loss, worker failure, and
out-of-order events. A test that can hang must have a bounded harness timeout.
Do not make production timeouts shorter only to make tests faster.

## Platform Coverage

Portable logic should run identically on every platform. Put OS-specific behavior
behind adapters and test the shared contract. CI should eventually cover Windows,
Linux, and macOS; platform exclusions require a documented reason.

## Verification Gate

The local default is:

```text
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Performance-sensitive changes also require the relevant benchmark or a recorded
before/after measurement using the same workload.
