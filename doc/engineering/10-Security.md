# Security and Privacy

TraceX processes untrusted device traffic, user commands, local files, and future
remote connections. Treat every external boundary as hostile or malformed.

## Threat Model

At minimum, consider:

- crafted serial or network input;
- oversized messages and resource exhaustion;
- malicious workspace, import, or plugin files;
- unsafe paths and symlink or traversal behavior;
- command disclosure and accidental credential logging;
- corrupted persistent state;
- compromised remote endpoints or update channels.

Security review expands as cloud, collaboration, plugins, and remote control are
introduced.

## Input Handling

- Validate lengths, encodings, versions, ranges, and state transitions.
- Bound buffers, queues, decompression, recursion, and collection growth.
- Parsing malformed input must return an error or a documented partial result,
  never panic.
- Treat filenames, paths, device names, and protocol metadata as untrusted.
- Avoid executing or interpreting device data as code, markup, or shell input.

## File Safety

- Resolve writes within an explicitly selected workspace or export destination.
- Prevent path traversal and unsafe archive extraction.
- Use atomic writes for small metadata and configuration files.
- Preserve the last valid data when migration or replacement fails.
- Do not follow unexpected links when an operation could escape its intended root.

## Secrets and Privacy

- Never hardcode or commit credentials.
- Store future secrets using platform credential facilities, not plain settings.
- Do not log raw device payloads, commands, tokens, unique identifiers, customer
  data, or complete private paths by default.
- Make telemetry and support uploads explicit, minimal, and user-controlled.
- Give users a clear preview before packaging diagnostic data.

## Commands and Remote Control

Sending commands can change or damage connected hardware. The application must
make the target session clear, prevent accidental cross-session sends, and avoid
automatic replay after reconnect unless the user explicitly configured a safe
policy. Future remote command features require authentication, authorization,
audit events, and replay protection.

## Dependencies and Supply Chain

Review maintenance, license, provenance, advisories, transitive footprint, and
platform behavior before adding a dependency. Pin and reproduce release inputs.
The release pipeline must eventually sign artifacts and protect update metadata.

## Vulnerability Handling

Security reports are handled privately until a fix and disclosure plan exist.
Fixes include a regression test when safe, an impact assessment, affected
versions, and release guidance. Never place live secrets or customer data in an
issue, fixture, or test log.

## Review Checklist

- What untrusted data reaches this code?
- Which resources can an attacker exhaust?
- Can paths or identifiers escape their intended scope?
- Could diagnostics expose user or device data?
- Does this action require explicit user intent or authorization?
- Does failure preserve data and leave the system in a safe state?
