# HI-Logger

A modern, feature-rich serial port logger application for firmware enthusiasts.

## Features

- **Real-time Serial Logging**: Connect to serial devices and view logs in real-time
- **Auto-Reconnect**: Automatically reconnects when device is unplugged and reconnected
- **Auto-Save**: Automatically saves logs to file (default: `hi_logger.log` in Documents)
- **Log Filtering**: Search and filter logs by content
- **Log Selection**: Select individual or multiple logs for copying
- **Command Interface**: Send commands to connected devices
- **Batch Commands**: Send multiple commands at once
- **Auto-Scroll**: Automatically scroll to latest logs
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

Logs are automatically saved to `Documents/hi_logger.log` by default. You can:
- Enable/disable auto-save
- Choose a custom save location
- Logs append to the same file each session

## Requirements

- Windows 10 or later
- No installation required - just run the executable

## Building from Source

```bash
cargo build --release
```

The executable will be in `target/release/hi_logger.exe`

## License

This project is open source and available for firmware enthusiasts.

---

**Developed by [Hitesh Honmane](https://www.linkedin.com/in/hitesh-honmane) for firmware enthusiasts**

[☕ Buy me a coffee](https://buymeacoffee.com/hitesh_honmane)
