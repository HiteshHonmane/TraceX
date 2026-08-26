# Performance Engineering

Performance is a product requirement for TraceX. Optimize from measurements and
clear budgets, while designing hot paths to avoid predictable waste.

## User-Centered Performance

The most important outcomes are:

- input remains responsive during continuous logging;
- scrolling and filtering remain smooth as sessions grow;
- memory usage is bounded by explicit retention policy;
- received logs are not silently dropped;
- connection, search, and workspace actions provide prompt feedback;
- long-running sessions remain stable.

## Establish Budgets

Every performance-sensitive subsystem must define a reproducible workload and
budgets before it is considered mature. Record:

- target event and byte throughput;
- maximum accepted queue depth and drop policy;
- retained log count or byte budget;
- search and filter latency percentiles;
- UI frame-time goal during ingestion;
- steady-state and growth memory limits;
- startup and workspace-open targets.

Do not invent numbers in feature code. Put approved budgets beside benchmarks and
update them deliberately as product requirements become concrete.

## Hot-Path Rules

- Parse each byte sequence once and share immutable structured results.
- Batch cross-thread messages and UI updates when latency permits.
- Bound every producer-consumer queue and define overload behavior.
- Avoid per-line regex compilation, formatting, allocation, and full-list scans.
- Keep disk writes buffered and off the UI thread.
- Use incremental indexes and filters instead of rebuilding on every frame.
- Virtualize rendering; do not construct widgets for millions of invisible rows.

## Concurrency

Concurrency boundaries must have explicit ownership, shutdown, cancellation, and
backpressure behavior. More threads are not automatically faster. Prefer a small
number of measurable pipelines over task-per-line designs.

Never hold a mutex while performing device I/O, disk I/O, parsing callbacks, or
UI rendering. Avoid unbounded channels.

## Measurement

- Measure release builds on a fixed fixture and documented machine profile.
- Capture throughput, latency distribution, memory high-water mark, and drops.
- Warm caches consistently and separate startup from steady-state results.
- Compare against a recorded baseline; one isolated number is not evidence.
- Profile before optimizing and attach evidence to performance pull requests.

## Regressions

A change that materially regresses an approved budget must be redesigned or
explicitly accepted with the tradeoff recorded. Functional correctness does not
override freezes, unbounded growth, or lost logs.

## Review Questions

- What grows with session duration?
- What happens when producers outrun consumers?
- Which work occurs on the UI thread?
- Which data is copied, formatted, parsed, or indexed more than once?
- How will this behavior be measured and prevented from regressing?
