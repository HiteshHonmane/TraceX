# TraceX

TraceX is a native, performance-first debugging workspace for firmware and
connected systems. The current release begins with UART logging and establishes
the architecture for additional transports, large-session search, and reliable
workspace recovery.

The [Windows development preview](https://tracex.abloom-stoat-4996.chatgpt.site)
is available with a download, checksums and known limitations. It is unsigned
and is not a production-qualified capture or recovery tool.

## Current Features

- Real-time serial logging
- Automatic reconnect after device removal and return
- Timestamped auto-save with configurable rotation
- Text and severity filtering
- Multi-row selection and copy
- Command sending and recent command history
- Virtualized two-axis log scrolling
- Filtered log export
- Save and reopen validated `.tracex` session files while disconnected
- Bounded capture and history with explicit overload stops
- Orderly close that drains capture and waits for storage
- Native dark desktop interface

These describe the current source tree. The published preview ZIP may precede
the newest source changes; use the build instructions below to run them locally.

New auto-save files use names such as
`TraceX_2026-07-16_14-05-09.log`. Existing HI-Logger files are not renamed or
deleted.

## Usage

1. Select a serial port.
2. Choose a baud rate; the default is 115200.
3. Select **Connect**.
4. View, search, filter, and select live logs.
5. Send commands from the command bar.
6. Enable auto-save when the session must be retained on disk.
7. Disconnect and use **File → Save Session** to preserve all retained fields in
   a reopenable `.tracex` file. Text autosave/export is a separate formatted output.
8. Save or export current logs before deliberately clearing them, then use
   **File → Open Session** to reopen a validated session.

File, Edit and View menus expose session actions, copying, search results and
follow-scroll controls. Capture stops visibly when processing falls behind or
history reaches its safety budget; pending received data is preserved. Export
retained history and deliberately clear it to make space for pending records.

The application is still a development preview. Monitor tabs share one device
capture; independent device sessions, indexed background search and automatic
crash recovery remain unfinished. Memory accounting limits are safety defaults,
not measured throughput or whole-process memory guarantees.

## Current Platform Status

- The development preview targets Windows x86_64. Builds and automated checks
  have been run on Windows 11; physical-device and broader OS qualification
  remain in progress.
- Linux and macOS remain first-class architectural targets and will be added to
  the verification matrix as packaging work matures.

## Building from Source

```bash
cargo build --release
```

The Windows executable will be in `target/release/tracex.exe`.

## Engineering Documentation

- [Product vision](doc/vision.md)
- [Product requirements](doc/01-Product-Requirements.md)
- [System architecture](doc/02-System-Architecture.md)
- [Engineering handbook](doc/engineering/README.md)
- [AI task prompts](doc/prompts/README.md)
- [Contributor guide](CONTRIBUTING.md)
- [Engineering decisions](DECISIONS.md)
- [UART framing and disconnect behavior](doc/UART-FRAMING.md)
- [Bounded capture and overload behavior](doc/CAPTURE-BEHAVIOR.md)
- [Query behavior](doc/QUERY-BEHAVIOR.md)
- [Development preview packaging](doc/DEVELOPMENT-PREVIEW.md)
- [Asynchronous autosave behavior](doc/engineering/AUTOSAVE-WORKER.md)
- [History retention limits](doc/engineering/HISTORY-RETENTION.md)
- [Session file format and save/reopen](doc/engineering/SESSION-FORMAT.md)
- [Close lifecycle](doc/engineering/CLOSE-LIFECYCLE.md)

AI contributors must begin with [AGENTS.md](AGENTS.md).

## Current Source Layout

- `src/main.rs` - native executable entry point
- `src/domain/log.rs` - framework-independent log types and parsing
- `src/domain/session_format.rs` - bounded, versioned session segment codec
- `src/domain/history_budget.rs` - retained-history admission accounting
- `src/app/mod.rs` - current application state and orchestration
- `src/app/capture.rs` - bounded transport batches and retained terminal payload
- `src/app/session_archive.rs` - worker-owned session save/open and text export
- `src/app/shutdown.rs` - orderly close and explicit incomplete-close decisions
- `src/app/log_format.rs` - copy and export text formatting
- `src/app/ui.rs` - central log workspace and frame updates
- `src/app/ui/sidebar.rs` - connection, port, logging, and settings controls
- `src/app/ui/tabs.rs` - monitor-session tabs
- `src/app/ui/command_bar.rs` - command input and resend history
- `src/app/storage.rs` - timestamped file creation and rotation
- `src/app/theme.rs` - visual tokens and egui styling
- `src/app/tests.rs` - current application regression tests

This is a migration baseline. The target boundaries are defined in
`doc/02-System-Architecture.md` and are extracted incrementally according to
`DECISIONS.md`.

## Verification

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## License

Distributed development-preview binaries use the
[TraceX binary license](distribution/BINARY-LICENSE.txt), which permits free
personal and internal business use. Third-party components retain their own
licenses; notices and relevant source archives accompany public preview packages.
This binary grant does not license the TraceX source code. A source-code license
and commercial production terms have not been selected.

---

**Developed by [Hitesh Honmane](https://www.linkedin.com/in/hitesh-honmane)**

[Buy me a coffee](https://buymeacoffee.com/hitesh_honmane)
