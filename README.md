# TraceX

TraceX is a native, performance-first debugging workspace for firmware and
connected systems. The current release begins with UART logging and establishes
the architecture for additional transports, large-session search, and reliable
workspace recovery.

## Current Features

- Real-time serial logging
- Automatic reconnect after device removal and return
- Timestamped auto-save with configurable rotation
- Text and severity filtering
- Multi-row selection and copy
- Command sending and recent command history
- Virtualized two-axis log scrolling
- Filtered log export
- Native dark desktop interface

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

## Current Platform Status

- Windows 10 or later is the currently validated desktop target.
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

AI contributors must begin with [AGENTS.md](AGENTS.md).

## Current Source Layout

- `src/main.rs` - native executable entry point
- `src/domain/log.rs` - framework-independent log types and parsing
- `src/app/mod.rs` - current application state and orchestration
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

A project license will be selected before the first public production release.

---

**Developed by [Hitesh Honmane](https://www.linkedin.com/in/hitesh-honmane)**

[Buy me a coffee](https://buymeacoffee.com/hitesh_honmane)
