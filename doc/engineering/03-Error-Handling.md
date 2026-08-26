# Error Handling

Errors are part of TraceX's product behavior. They must preserve context, protect
user data, and give the UI enough information to guide recovery.

## Error Categories

Distinguish at least these categories where behavior differs:

- invalid user configuration;
- transport unavailable or disconnected;
- malformed or unsupported device data;
- storage or export failure;
- workspace corruption or incompatible version;
- internal invariant violation.

Do not reduce every failure to a string too early.

## Typed Errors

- Libraries and domain modules return small typed error enums.
- Error variants describe actionable causes, not the function that failed.
- Preserve underlying sources for diagnostics.
- Add contextual fields such as operation, safe path, transport kind, or session
  identifier when they help investigation.
- Avoid exposing dependency-specific error types across architectural boundaries.

`thiserror` is the preferred approach for library error types if the dependency is
approved. `anyhow` may be used at a binary or task boundary for contextual
aggregation, but never as a domain API or as a substitute for error design.

## Propagation and Recovery

- Use `?` for propagation when the caller can add no useful recovery.
- Retry only transient operations and only with bounded backoff and cancellation.
- Never retry malformed input, permission errors, or deterministic configuration
  failures automatically.
- Treat channel closure and worker termination as explicit lifecycle events.
- When an operation partially succeeds, report what completed and what remains.

## User-Facing Errors

Application code maps technical errors to a stable user-facing model containing:

- a short summary;
- a recovery action when one exists;
- safe diagnostic detail;
- a correlation or session identifier when useful.

The UI renders this model. It must not parse error strings or decide retry policy.

## Data Integrity

- Flush or finalize durable state before reporting success.
- Use atomic replace patterns for small configuration and metadata files.
- Never overwrite the only valid copy after a failed parse or migration.
- Preserve recoverable logs when workspace metadata fails.
- Surface dropped data, truncated input, and failed writes immediately.

## Panics

Panics represent programmer defects, not operational errors. Production code must
not panic because a device disconnected, input was malformed, a file was missing,
or a channel closed. Tests may use `unwrap` and `expect` when the assertion intent
is obvious.

## Review Checklist

- Is the failure classified at the right layer?
- Does it retain useful context without leaking sensitive data?
- Can the caller recover, retry, or present it correctly?
- Could any error path lose logs or leave a session in a false state?
- Is the failure covered by a focused test?
