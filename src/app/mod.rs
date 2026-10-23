use crate::domain::log::*;
use crate::domain::query::{LogQuery, QueryScope, SeveritySelection};
use chrono::Local;
use directories::UserDirs;
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use serialport::available_ports;
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write as IoWrite};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const WINDOW_BAR_HEIGHT: f32 = 32.0;
const TOP_HEADER_HEIGHT: f32 = 48.0;
const LIVE_SCROLL_SETTLE_FRAMES: u8 = 2;
const COMMAND_HISTORY_LIMIT: usize = 20;

fn live_scroll_frames_after_toggle(changed: bool, enabled: bool) -> u8 {
    if changed && enabled {
        LIVE_SCROLL_SETTLE_FRAMES
    } else {
        0
    }
}

fn bottom_scroll_offset(row_count: usize, row_height: f32, viewport_height: f32) -> f32 {
    (row_count as f32 * row_height - viewport_height).max(0.0)
}

fn record_command_history(history: &mut Vec<String>, command: &str, limit: usize) {
    let command = command.trim();
    if command.is_empty() || limit == 0 {
        return;
    }

    history.retain(|existing| existing != command);
    history.push(command.to_string());
    if history.len() > limit {
        history.drain(..history.len() - limit);
    }
}

fn register_submitted_command(history: &mut Vec<String>, command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    record_command_history(history, command, COMMAND_HISTORY_LIMIT);
    Some(command.to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// ANSI CODE STRIPPER
// ═══════════════════════════════════════════════════════════════════════════════

impl TraceXApp {
    fn log_query(&self) -> LogQuery {
        LogQuery::new(
            &self.search,
            SeveritySelection {
                info: self.show_info,
                warn: self.show_warn,
                error: self.show_error,
            },
        )
    }

    fn query_indices(&self, scope: QueryScope) -> Vec<usize> {
        self.log_query().indices(&self.logs, scope)
    }
}

mod autosave;
mod autosave_worker;
mod log_format;
mod storage;
use autosave::AutosaveController;

use log_format::*;
#[cfg(test)]
use storage::*;
mod ui;

mod theme;

fn centered_row_scroll_offset(
    row_index: usize,
    row_count: usize,
    row_height: f32,
    viewport_height: f32,
) -> f32 {
    let content_height = row_count as f32 * row_height;
    let max_offset = (content_height - viewport_height).max(0.0);
    let row_center = (row_index as f32 + 0.5) * row_height;
    (row_center - viewport_height * 0.5).clamp(0.0, max_offset)
}

fn log_content_width<'a>(messages: impl IntoIterator<Item = &'a str>, minimum: f32) -> f32 {
    messages
        .into_iter()
        .map(|message| 205.0 + message.chars().count() as f32 * 7.2)
        .fold(minimum, f32::max)
}

fn add_copy_row_menu(
    response: &egui::Response,
    ctx: &egui::Context,
    line: &str,
    selected_text: Option<&str>,
    selected_count: usize,
) {
    response.context_menu(|ui| {
        if ui.button("Copy row").clicked() {
            ctx.copy_text(line.to_owned());
            ui.close_menu();
        }
        if let Some(selected_text) = selected_text {
            ui.separator();
            if ui
                .button(format!("Copy all selected rows ({selected_count})"))
                .clicked()
            {
                ctx.copy_text(selected_text.to_owned());
                ui.close_menu();
            }
        }
    });
}

fn render_primary_log_row(
    ui: &mut egui::Ui,
    line_number: usize,
    timestamp: &str,
    level: LogLevel,
    message: &str,
    selected: bool,
) -> egui::Response {
    let tint = if selected {
        theme::ACCENT.linear_multiply(0.20)
    } else if level == LogLevel::Error {
        theme::ERROR.linear_multiply(0.07)
    } else {
        Color32::TRANSPARENT
    };
    let response = egui::Frame::none()
        .fill(tint)
        .stroke(if selected {
            Stroke::new(1.0_f32, theme::ACCENT.linear_multiply(0.85))
        } else {
            Stroke::NONE
        })
        .inner_margin(egui::Margin::symmetric(0.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [43.0, 20.0],
                    egui::Label::new(
                        RichText::new(line_number.to_string())
                            .monospace()
                            .size(10.0)
                            .color(theme::TEXT_MUTED),
                    ),
                );
                ui.add_sized(
                    [92.0, 20.0],
                    egui::Label::new(
                        RichText::new(timestamp)
                            .monospace()
                            .size(10.0)
                            .color(theme::TEXT_MUTED),
                    ),
                );
                egui::Frame::none()
                    .fill(theme::log_level_color(level).linear_multiply(0.1))
                    .stroke(Stroke::new(
                        1.0_f32,
                        theme::log_level_color(level).linear_multiply(0.35),
                    ))
                    .rounding(3.0)
                    .inner_margin(egui::Margin::symmetric(5.0, 1.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(level.label())
                                .monospace()
                                .size(9.0)
                                .color(theme::log_level_color(level))
                                .strong(),
                        );
                    });
                ui.add_space(7.0);
                let color = match level {
                    LogLevel::Error => theme::ERROR,
                    LogLevel::Warn => theme::WARNING,
                    LogLevel::Debug => theme::SUCCESS,
                    _ => theme::TEXT,
                };
                ui.add(
                    egui::Label::new(RichText::new(message).monospace().size(11.5).color(color))
                        .wrap_mode(egui::TextWrapMode::Truncate),
                );
            });
        })
        .response
        .interact(egui::Sense::click());
    if selected {
        ui.painter().vline(
            response.rect.left() + 2.0,
            response.rect.y_range().shrink(3.0),
            Stroke::new(3.0_f32, theme::ACCENT),
        );
    }
    response
}

const BAUD_RATES: [u32; 8] = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];

// ═══════════════════════════════════════════════════════════════════════════════
// APP
// ═══════════════════════════════════════════════════════════════════════════════

pub fn run() -> eframe::Result<()> {
    eframe::run_native(
        "TraceX",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 800.0])
                .with_min_inner_size([800.0, 600.0])
                .with_decorations(false)
                .with_title("TraceX"),
            ..Default::default()
        },
        Box::new(move |_cc| Ok(Box::new(TraceXApp::new()))),
    )
}

struct UiTab {
    title: String,
    setup: bool,
}

struct TraceXApp {
    // Ports (updated in background) - (display_name, port_name)
    ports: Arc<Mutex<Vec<(String, String)>>>,
    selected_port: String,
    baud_rate: u32,

    // Connection
    connected: bool,
    paused: bool,
    serial_tx: Option<Sender<Vec<u8>>>,
    log_rx: Option<Receiver<String>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    last_connected_port: Option<String>,

    // Logs
    logs: Vec<LogEntry>,
    next_id: usize,

    // Command
    command_input: String,
    command_history: Vec<String>,
    send_newline: bool,
    auto_clear_command: bool,

    // Filter
    search: String,
    selected: HashSet<usize>,
    filter_panel_open: bool,
    filter_exclude: bool,
    filter_results_height: f32,
    scroll_to_log: Option<usize>,
    scroll_jump_settle_frames: u8,

    // Auto-save
    auto_save_enabled: bool,
    auto_save_path: Option<PathBuf>,
    auto_save_current_path: Option<PathBuf>,
    auto_save: AutosaveController,
    auto_save_limit_mb: u64,
    auto_save_bytes_written: u64,
    auto_save_sequence: u32,
    auto_save_error: Option<String>,
    operation_error: Option<String>,

    // UI
    auto_scroll: bool,
    live_scroll_frames: u8,
    ui_tabs: Vec<UiTab>,
    active_tab: usize,
    show_info: bool,
    show_warn: bool,
    show_error: bool,
    theme_ready: bool,

    // Stats
    rate: f32,
    rate_count: usize,
    rate_timer: Instant,

    // Auto-reconnect
    reconnect_cooldown: Instant,
}

impl TraceXApp {
    fn default_log_path() -> PathBuf {
        if let Some(user_dirs) = UserDirs::new()
            && let Some(doc_dir) = user_dirs.document_dir()
        {
            return doc_dir.join("TraceX.log");
        }
        // Fallback to current directory if Documents folder not found
        PathBuf::from("TraceX.log")
    }

    fn new() -> Self {
        let ports = Arc::new(Mutex::new(Self::scan_ports()));

        // Background port scanner
        let ports_clone = Arc::clone(&ports);
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(1));
                let new_ports = Self::scan_ports();
                if let Ok(mut p) = ports_clone.lock() {
                    *p = new_ports;
                }
            }
        });

        Self::with_ports(ports)
    }

    fn with_ports(ports: Arc<Mutex<Vec<(String, String)>>>) -> Self {
        Self {
            ports,
            selected_port: String::new(),
            baud_rate: 115200,

            connected: false,
            paused: false,
            serial_tx: None,
            log_rx: None,
            stop_flag: None,
            last_connected_port: None,

            logs: Vec::new(),
            next_id: 0,

            command_input: String::new(),
            command_history: Vec::new(),
            send_newline: true,
            auto_clear_command: true,

            search: String::new(),
            selected: HashSet::new(),
            filter_panel_open: false,
            filter_exclude: false,
            filter_results_height: 180.0,
            scroll_to_log: None,
            scroll_jump_settle_frames: 0,

            auto_save_enabled: false,
            auto_save_path: Some(Self::default_log_path()),
            auto_save_current_path: None,
            auto_save: AutosaveController::default(),
            auto_save_limit_mb: 25,
            auto_save_bytes_written: 0,
            auto_save_sequence: 0,
            auto_save_error: None,
            operation_error: None,

            auto_scroll: true,
            live_scroll_frames: 0,
            ui_tabs: vec![UiTab {
                title: "Monitor 1".to_string(),
                setup: false,
            }],
            active_tab: 0,
            show_info: true,
            show_warn: true,
            show_error: true,
            theme_ready: false,

            rate: 0.0,
            rate_count: 0,
            rate_timer: Instant::now(),

            reconnect_cooldown: Instant::now(),
        }
    }

    fn scan_ports() -> Vec<(String, String)> {
        available_ports()
            .unwrap_or_default()
            .into_iter()
            .map(|p| {
                let port_name = p.port_name.clone();
                let display_name = match p.port_type {
                    serialport::SerialPortType::UsbPort(info) => {
                        let product = info.product.unwrap_or_default();
                        let manufacturer = info.manufacturer.unwrap_or_default();

                        // Clean product name by removing port name and generic terms
                        let clean_product = if !product.is_empty() {
                            let mut cleaned = product.replace(&port_name, "").trim().to_string();
                            // Remove trailing empty parentheses
                            cleaned = cleaned
                                .trim_end_matches(" ()")
                                .trim_end_matches("()")
                                .to_string();
                            // Remove generic terms that don't add value
                            let generic_terms = ["USB Serial Port", "USB SERIAL", "USB"];
                            for term in &generic_terms {
                                if cleaned == *term {
                                    cleaned.clear();
                                    break;
                                }
                            }
                            if cleaned.is_empty() {
                                None
                            } else {
                                Some(cleaned)
                            }
                        } else {
                            None
                        };

                        // Clean manufacturer name by removing port name and generic terms
                        let clean_manufacturer = if !manufacturer.is_empty() {
                            let mut cleaned =
                                manufacturer.replace(&port_name, "").trim().to_string();
                            // Remove trailing empty parentheses
                            cleaned = cleaned
                                .trim_end_matches(" ()")
                                .trim_end_matches("()")
                                .to_string();
                            // Remove generic terms that don't add value
                            let generic_terms = ["USB Serial Port", "USB SERIAL", "USB"];
                            for term in &generic_terms {
                                if cleaned == *term {
                                    cleaned.clear();
                                    break;
                                }
                            }
                            if cleaned.is_empty() {
                                None
                            } else {
                                Some(cleaned)
                            }
                        } else {
                            None
                        };

                        if let Some(product) = clean_product {
                            format!("{port_name} ({product})")
                        } else if let Some(manufacturer) = clean_manufacturer {
                            format!("{port_name} ({manufacturer})")
                        } else {
                            port_name.clone()
                        }
                    }
                    serialport::SerialPortType::PciPort => format!("{port_name} (PCI)"),
                    serialport::SerialPortType::BluetoothPort => {
                        format!("{port_name} (Bluetooth)")
                    }
                    serialport::SerialPortType::Unknown => port_name.clone(),
                };
                (display_name, port_name)
            })
            .collect()
    }

    fn record_log_line(&mut self, line: String) {
        let timestamp = Local::now().format("%H:%M:%S.%3f").to_string();
        let entry = LogEntry::new(self.next_id, line, timestamp);
        self.next_id += 1;
        self.rate_count += 1;
        self.append_auto_save_line(&format_log_line(
            &entry.timestamp,
            entry.level,
            &entry.message,
        ));

        if !self.paused {
            self.logs.push(entry);
        }
    }

    fn connect(&mut self) {
        if self.log_rx.is_some() || self.auto_save.stopping {
            self.operation_error =
                Some("Wait for the previous connection to finish disconnecting.".to_owned());
            return;
        }
        if self.selected_port.is_empty() {
            self.operation_error = Some("Select a serial port before connecting.".to_owned());
            return;
        }

        let port = match serialport::new(&self.selected_port, self.baud_rate)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(p) => p,
            Err(error) => {
                self.operation_error = Some(format!("Could not connect to serial port: {error}"));
                return;
            }
        };

        let (log_tx, log_rx) = channel();
        let (cmd_tx, cmd_rx) = channel::<Vec<u8>>();
        let stop = Arc::new(Mutex::new(false));
        let stop_clone = Arc::clone(&stop);

        self.log_rx = Some(log_rx);
        self.serial_tx = Some(cmd_tx);
        self.stop_flag = Some(stop);

        thread::spawn(move || {
            Self::serial_thread(port, log_tx, cmd_rx, stop_clone);
        });

        self.connected = true;
        self.paused = false;
        self.operation_error = None;
        self.last_connected_port = Some(self.selected_port.clone());

        if self.auto_save_enabled && !self.auto_save.active {
            self.open_new_auto_save_file();
        }
    }

    fn disconnect(&mut self) {
        if let Some(flag) = &self.stop_flag
            && let Ok(mut f) = flag.lock()
        {
            *f = true;
        }
        self.connected = false;
        // Retain incoming logs and autosave until the worker closes its channel.
        self.serial_tx = None;
        // Clear last_connected_port on manual disconnect to prevent auto-reconnect
        self.last_connected_port = None;
    }

    fn serial_thread(
        mut port: impl Read + IoWrite,
        log_tx: Sender<String>,
        cmd_rx: Receiver<Vec<u8>>,
        stop: Arc<Mutex<bool>>,
    ) {
        let mut buf = [0u8; 4096];
        let mut framer = crate::domain::stream_framing::StreamFramer::default();

        loop {
            if stop.lock().map(|f| *f).unwrap_or(true) {
                break;
            }

            // Check for commands to send
            match cmd_rx.try_recv() {
                Ok(data) => {
                    if port.write_all(&data).is_err() {
                        // Port disconnected
                        break;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }

            // Read incoming data
            match port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    framer.push(&buf[..n], |line| {
                        let _ = log_tx.send(line);
                    });
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => {
                    // Port disconnected unexpectedly
                    break;
                }
            }

            thread::sleep(Duration::from_millis(1));
        }
        if let Some(line) = framer.finish() {
            let _ = log_tx.send(line);
        }
        // Release the transport before channel closure permits reconnect.
        drop(port);
        drop(log_tx);
    }

    fn poll(&mut self) {
        self.poll_auto_save();
        // Limit per-frame work and stop consuming when storage applies backpressure.
        for _ in 0..256 {
            if !self.can_ingest_log() {
                break;
            }
            let received = self.log_rx.as_ref().map(Receiver::try_recv);
            match received {
                Some(Ok(line)) => self.record_log_line(line),
                Some(Err(TryRecvError::Disconnected)) => {
                    self.handle_disconnection();
                    break;
                }
                Some(Err(TryRecvError::Empty)) | None => break,
            }
        }

        if self.rate_timer.elapsed() >= Duration::from_secs(1) {
            self.rate = self.rate_count as f32;
            self.rate_count = 0;
            self.rate_timer = Instant::now();
        }
    }

    fn handle_disconnection(&mut self) {
        // Clean up connection state but keep last_connected_port for auto-reconnect
        self.connected = false;
        self.log_rx = None;
        self.serial_tx = None;
        self.stop_flag = None;
        self.stop_auto_save();
        // Reset reconnect cooldown
        self.reconnect_cooldown = Instant::now();
    }

    fn check_auto_reconnect(&mut self) {
        // Auto-reconnect if we were connected and the same port is available again
        // Add a small cooldown to avoid rapid reconnection attempts
        if !self.connected
            && self.log_rx.is_none()
            && !self.auto_save.stopping
            && self.reconnect_cooldown.elapsed() >= Duration::from_millis(500)
            && let Some(ref last_port) = self.last_connected_port
        {
            // Check if the port is available
            let ports = self.ports.lock().map(|p| p.clone()).unwrap_or_default();
            let port_available = ports.iter().any(|(_, port_name)| port_name == last_port);

            if port_available && self.selected_port == *last_port {
                // Same port is available again, auto-reconnect
                self.connect();
            }
        }
    }

    fn send_command(&mut self, cmd: &str) {
        let Some(cmd) = register_submitted_command(&mut self.command_history, cmd) else {
            return;
        };

        let mut data = cmd.as_bytes().to_vec();
        if self.send_newline {
            data.push(b'\r');
            data.push(b'\n');
        }

        match self.serial_tx.as_ref() {
            Some(tx) if tx.send(data).is_ok() => self.operation_error = None,
            Some(_) => {
                self.operation_error =
                    Some("Could not send command because the connection ended.".to_owned());
            }
            None => {
                self.operation_error =
                    Some("Connect to a device before sending commands.".to_owned());
            }
        }
    }

    fn filtered(&self) -> Vec<&LogEntry> {
        self.query_indices(QueryScope::Main)
            .into_iter()
            .map(|index| &self.logs[index])
            .collect()
    }

    fn export_text(&self) -> String {
        format_log_entries(self.filtered())
    }

    fn copy_selected(&self, ctx: &egui::Context) {
        let text = if self.selected.is_empty() {
            self.export_text()
        } else {
            format_log_entries(self.logs.iter().filter(|e| self.selected.contains(&e.id)))
        };
        ctx.copy_text(text);
    }

    fn copy_all(&self, ctx: &egui::Context) {
        ctx.copy_text(format_log_entries(&self.logs));
    }

    fn clear(&mut self) {
        self.logs.clear();
        self.selected.clear();
        self.next_id = 0;
    }

    fn export_logs(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log", &["log", "txt"])
            .set_file_name("export.log")
            .save_file()
        {
            let content = self.export_text();
            self.operation_error = fs::write(path, content)
                .err()
                .map(|error| format!("Could not export logs: {error}"));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UI
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests;
