# Logging and Diagnostics

Operational diagnostics must help maintainers explain what happened without
capturing the user's log payloads or flooding hot paths.

## Structured Logging

`tracing` is the target logging facade once adopted through the dependency review
process. Emit structured fields instead of building prose-only strings. Do not
use `println!`, `eprintln!`, or ad hoc debug files in production code.

Useful stable fields include:

- `session_id`;
- `workspace_id`;
- `transport_kind`;
- operation name;
- byte or event counts;
- duration;
- retry attempt;
- error category.

Do not log raw device payloads by default.

## Levels

- **error:** an operation failed and user impact or data risk exists.
- **warn:** degraded behavior, recovery, dropping, or an unexpected condition.
- **info:** low-frequency lifecycle events such as session start and rotation.
- **debug:** diagnostic state useful during development or support.
- **trace:** high-volume details, disabled in normal builds and support bundles.

An expected disconnect is not automatically an error. Choose the level based on
product impact and recovery behavior.

## Spans and Correlation

Use spans for operations with a lifecycle: connect, import, export, index build,
workspace open, and log rotation. Attach identifiers once at the span boundary
instead of repeating them manually.

## Performance and Volume

- Never emit one normal-level diagnostic event per received log line.
- Aggregate throughput, drop, and queue-depth metrics over intervals.
- Rate-limit repeated failures.
- Avoid formatting or allocating disabled log fields on hot paths.
- Make diagnostic retention bounded and configurable.

## Sensitive Data

Never log credentials, tokens, complete user paths, command payloads, raw device
traffic, customer data, or unique hardware identifiers without explicit opt-in.
Redact at the source rather than relying on a later sink.

## Support Diagnostics

Future support bundles should be explicit, previewable, and user-authorized. They
should include product version, platform, sanitized configuration, relevant
diagnostic events, and failure summaries. Device logs remain excluded unless the
user deliberately selects them.

## Testing

Test important diagnostic events when they are part of recovery or observability
contracts. Do not couple tests to incidental prose; assert levels, event names,
and structured fields.
