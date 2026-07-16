# HI-Logger

A modern, feature-rich serial port logger application for firmware enthusiasts.

## Features

- **Real-time Serial Logging**: Connect to serial devices and view logs in real-time
- **Auto-Reconnect**: Automatically reconnects when device is unplugged and reconnected
- **Auto-Save Rotation**: Creates precise timestamped files such as `HI-Logger_2026-07-16_14-05-09.log` and rotates at a configurable size limit
- **Log Filtering**: Search and filter logs by content
- **Log Selection**: Select individual or multiple logs for copying
- **Command Interface**: Send commands to connected devices
- **Command History**: Re-send recent commands with one click
- **Two-Axis Log Scrolling**: Automatically follows new logs while preserving horizontal access to long lines
- **Export Logs**: Export filtered logs to file
- **Modern UI**: Dark theme with color-coded log levels

## Usage

1. Select a serial port from the list
2. Choose baud rate (default: 115200)
3. Click "Connect"
4. View logs in real-time
5. Use search to filter logs
6. Select logs and copy with Ctrl+C
7. Send commands via the command input at the bottom

## Auto-Save

When auto-save is enabled, logs are created in Documents by default with date and time down to seconds. You can:
- Enable or disable auto-save
- Choose a custom location and base filename
- Configure the maximum file size from Settings
- Automatically rotate to a new uniquely numbered timestamped file when the limit is reached

## Requirements

- Windows 10 or later
- No installation required - just run the executable

## Building from Source

```bash
cargo build --release
```

The executable will be in `target/release/hi_logger.exe`

## Source Layout

- `src/main.rs` — native executable entry point
- `src/app/mod.rs` — application state and orchestration
- `src/app/ui.rs` — central log workspace and top-level frame updates
- `src/app/ui/sidebar.rs` — connection, port, logging, and settings controls
- `src/app/ui/tabs.rs` — monitor session tabs
- `src/app/ui/command_bar.rs` — command input and resend history
- `src/app/log.rs` — log parsing, formatting, and selection rules
- `src/app/storage.rs` — timestamped file creation and rotation
- `src/app/theme.rs` — visual tokens and egui styling
- `src/app/tests.rs` — focused regression tests

Before submitting changes, run:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## License

This project is open source and available for firmware enthusiasts.

---

**Developed by [Hitesh Honmane](https://www.linkedin.com/in/hitesh-honmane) for firmware enthusiasts**

[☕ Buy me a coffee](https://buymeacoffee.com/hitesh_honmane)
