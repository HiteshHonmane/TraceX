# Performance Task Prompt

You are an engineer investigating or improving TraceX performance.

## Input

- **User-visible performance problem:**
- **Workload and data size:**
- **Platform and build profile:**
- **Baseline measurements:**
- **Target or budget:**

## Measure Before Changing

Read `AGENTS.md`, architecture, `06-Performance.md`, relevant code and decisions.
Create a reproducible workload. Record release-build throughput, latency
distribution, memory high-water mark, queue depth or drops, and UI responsiveness
as applicable.

Profile to locate the actual bottleneck. Do not assume that allocation, locking,
rendering, parsing, I/O, or a dependency is responsible without evidence.

## Optimization Rules

- Preserve correctness, ordering, recovery, and data integrity.
- Keep all blocking and heavy work off the UI thread.
- Prefer eliminating repeated work and copies before adding concurrency.
- Bound memory and queues; never improve a benchmark by hiding drops.
- Batch only within an explicit latency budget.
- Do not add unsafe code or a dependency without separate justification.
- Keep the change narrow enough to attribute measured effects.

## Verification

Run the identical workload before and after. Report raw measurements, environment,
variance, and any tradeoff. Run correctness and stress tests. Inspect long-running
memory behavior rather than relying only on a short throughput test.

## Required Output

Provide:

1. measured bottleneck;
2. baseline and target;
3. implementation and why it changes the bottleneck;
4. before/after results using the same workload;
5. correctness verification;
6. tradeoffs, limits, and next bottleneck.

If evidence does not justify a change, report that instead of optimizing blindly.
