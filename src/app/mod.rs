use chrono::Local;
use directories::UserDirs;
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use serialport::{SerialPort, available_ports};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write as IoWrite};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const TOP_HEADER_HEIGHT: f32 = 64.0;
const APP_ROUNDING: f32 = 10.0;
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

impl App {
    fn level_visible(&self, level: LogLevel) -> bool {
        match level {
            LogLevel::Info => self.show_info,
            LogLevel::Warn => self.show_warn,
            LogLevel::Error => self.show_error,
            _ => true,
        }
    }
}

mod log;
mod storage;

use log::*;
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
            Stroke::new(1.0, theme::ACCENT.linear_multiply(0.85))
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
                    .fill(level.color().linear_multiply(0.1))
                    .stroke(Stroke::new(1.0, level.color().linear_multiply(0.35)))
                    .rounding(3.0)
                    .inner_margin(egui::Margin::symmetric(5.0, 1.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(level.label())
                                .monospace()
                                .size(9.0)
                                .color(level.color())
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
            Stroke::new(3.0, theme::ACCENT),
        );
    }
    response
}

const BAUD_RATES: [u32; 8] = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];

// ═══════════════════════════════════════════════════════════════════════════════
// APP
// ═══════════════════════════════════════════════════════════════════════════════

pub fn run() -> eframe::Result<()> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!(
        "../../assets/hi-logger-logo-transparent.png"
    ))
    .expect("bundled HI-Logger logo must be a valid PNG");
    let logo_image = egui::ColorImage::from(&icon);

    eframe::run_native(
        "HI-Logger",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 800.0])
                .with_min_inner_size([800.0, 600.0])
                .with_title("HI-Logger")
                .with_icon(icon),
            ..Default::default()
        },
        Box::new(move |cc| {
            theme::setup(&cc.egui_ctx);
            Ok(Box::new(App::new(&cc.egui_ctx, logo_image)))
        }),
    )
}

struct UiTab {
    title: String,
    setup: bool,
}

struct App {
    // Ports (updated in background) - (display_name, port_name)
    ports: Arc<Mutex<Vec<(String, String)>>>,
    selected_port: String,
    baud_rate: u32,

    // Connection
    connected: bool,
    paused: bool,
    serial_tx: Option<Sender<Vec<u8>>>,
    log_rx: Option<Receiver<String>>,
    disconnect_rx: Option<Receiver<()>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    last_connected_port: Option<String>,

    // Logs
    logs: Vec<LogEntry>,
    next_id: usize,

    // Command
    command_input: String,
    command_history: Vec<String>,
    send_newline: bool,

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
    auto_save_file: Option<std::fs::File>,
    auto_save_limit_mb: u64,
    auto_save_bytes_written: u64,
    auto_save_sequence: u32,
    auto_save_error: Option<String>,

    // UI
    logo_texture: egui::TextureHandle,
    baud_scroll_to_selected: bool,
    auto_scroll: bool,
    live_scroll_frames: u8,
    ui_tabs: Vec<UiTab>,
    active_tab: usize,
    show_info: bool,
    show_warn: bool,
    show_error: bool,

    // Stats
    rate: f32,
    rate_count: usize,
    rate_timer: Instant,

    // Auto-reconnect
    reconnect_cooldown: Instant,
}

impl App {
    fn default_log_path() -> PathBuf {
        if let Some(user_dirs) = UserDirs::new() {
            if let Some(doc_dir) = user_dirs.document_dir() {
                return doc_dir.join("HI-Logger.log");
            }
        }
        // Fallback to current directory if Documents folder not found
        PathBuf::from("HI-Logger.log")
    }

    fn new(ctx: &egui::Context, logo_image: egui::ColorImage) -> Self {
        let ports = Arc::new(Mutex::new(Self::scan_ports()));
        let logo_texture =
            ctx.load_texture("hi_logger_logo", logo_image, egui::TextureOptions::LINEAR);

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

        Self {
            ports,
            selected_port: String::new(),
            baud_rate: 115200,

            connected: false,
            paused: false,
            serial_tx: None,
            log_rx: None,
            disconnect_rx: None,
            stop_flag: None,
            last_connected_port: None,

            logs: Vec::new(),
            next_id: 0,

            command_input: String::new(),
            command_history: Vec::new(),
            send_newline: true,

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
            auto_save_file: None,
            auto_save_limit_mb: 25,
            auto_save_bytes_written: 0,
            auto_save_sequence: 0,
            auto_save_error: None,

            logo_texture,
            baud_scroll_to_selected: true,
            auto_scroll: true,
            live_scroll_frames: 0,
            ui_tabs: vec![UiTab {
                title: "SERIAL / MONITOR".to_string(),
                setup: false,
            }],
            active_tab: 0,
            show_info: true,
            show_warn: true,
            show_error: true,

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

    fn open_new_auto_save_file(&mut self) {
        let base = self
            .auto_save_path
            .clone()
            .unwrap_or_else(Self::default_log_path);
        match create_timestamped_log_file(&base, Local::now(), self.auto_save_sequence) {
            Ok((file, path, next_sequence)) => {
                self.auto_save_file = Some(file);
                self.auto_save_current_path = Some(path);
                self.auto_save_bytes_written = 0;
                self.auto_save_sequence = next_sequence;
                self.auto_save_error = None;
            }
            Err(error) => {
                self.auto_save_file = None;
                self.auto_save_current_path = None;
                self.auto_save_bytes_written = 0;
                self.auto_save_error = Some(format!("Could not create log file: {error}"));
            }
        }
    }

    fn apply_auto_save_enabled_change(&mut self) {
        if self.auto_save_enabled {
            if self.auto_save_file.is_none() {
                self.open_new_auto_save_file();
            }
        } else {
            self.auto_save_file = None;
            self.auto_save_bytes_written = 0;
            self.auto_save_error = None;
        }
    }

    fn render_auto_save_status(&self, ui: &mut egui::Ui) {
        if let Some(error) = self.auto_save_error.as_ref() {
            ui.label(
                RichText::new(error)
                    .monospace()
                    .size(8.5)
                    .color(theme::ERROR),
            );
        } else if let Some(path) = self.auto_save_current_path.as_ref() {
            let label = if self.auto_save_enabled {
                "Saving to"
            } else {
                "Last file"
            };
            ui.label(
                RichText::new(format!("{label}: {}", path.display()))
                    .monospace()
                    .size(8.5)
                    .color(theme::TEXT_MUTED),
            );
        }
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("LOG FILES")
                .size(10.0)
                .color(theme::ACCENT)
                .strong(),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("File limit");
            ui.add(
                egui::DragValue::new(&mut self.auto_save_limit_mb)
                    .range(1..=4096)
                    .suffix(" MB"),
            );
        });
        ui.add_space(4.0);
        if ui.button("Choose save location").clicked() {
            self.choose_save_path();
        }
    }

    fn append_auto_save_line(&mut self, line: &str) {
        if !self.auto_save_enabled {
            return;
        }

        let payload = format!("{line}\n");
        if self.auto_save_file.is_none() {
            self.open_new_auto_save_file();
        }
        if should_rotate_log(
            self.auto_save_bytes_written,
            payload.len(),
            self.auto_save_limit_mb,
        ) {
            self.auto_save_file = None;
            self.open_new_auto_save_file();
        }

        if let Some(file) = self.auto_save_file.as_mut() {
            if file.write_all(payload.as_bytes()).is_ok() {
                self.auto_save_bytes_written = self
                    .auto_save_bytes_written
                    .saturating_add(payload.len() as u64);
            }
        }
    }

    fn record_log_line(&mut self, line: String) {
        let entry = LogEntry::new(self.next_id, line);
        self.next_id += 1;
        self.rate_count += 1;
        self.append_auto_save_line(&entry.format_line());

        if !self.paused {
            self.logs.push(entry);
        }
    }

    fn connect(&mut self) {
        if self.selected_port.is_empty() {
            return;
        }

        let port = match serialport::new(&self.selected_port, self.baud_rate)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(p) => p,
            Err(_) => return,
        };

        let (log_tx, log_rx) = channel();
        let (cmd_tx, cmd_rx) = channel::<Vec<u8>>();
        let (disconnect_tx, disconnect_rx) = channel();
        let stop = Arc::new(Mutex::new(false));
        let stop_clone = Arc::clone(&stop);

        self.log_rx = Some(log_rx);
        self.disconnect_rx = Some(disconnect_rx);
        self.serial_tx = Some(cmd_tx);
        self.stop_flag = Some(stop);

        thread::spawn(move || {
            Self::serial_thread(port, log_tx, cmd_rx, stop_clone, disconnect_tx);
        });

        self.connected = true;
        self.paused = false;
        self.last_connected_port = Some(self.selected_port.clone());

        if self.auto_save_enabled && self.auto_save_file.is_none() {
            self.open_new_auto_save_file();
        }
    }

    fn disconnect(&mut self) {
        if let Some(flag) = &self.stop_flag {
            if let Ok(mut f) = flag.lock() {
                *f = true;
            }
        }
        self.connected = false;
        self.log_rx = None;
        self.disconnect_rx = None;
        self.serial_tx = None;
        self.stop_flag = None;
        self.auto_save_file = None;
        // Clear last_connected_port on manual disconnect to prevent auto-reconnect
        self.last_connected_port = None;
    }

    fn serial_thread(
        mut port: Box<dyn SerialPort>,
        log_tx: Sender<String>,
        cmd_rx: Receiver<Vec<u8>>,
        stop: Arc<Mutex<bool>>,
        disconnect_tx: Sender<()>,
    ) {
        let mut buf = [0u8; 4096];
        let mut line_buf = String::new();

        loop {
            if stop.lock().map(|f| *f).unwrap_or(true) {
                break;
            }

            // Check for commands to send
            match cmd_rx.try_recv() {
                Ok(data) => {
                    if port.write_all(&data).is_err() {
                        // Port disconnected
                        let _ = disconnect_tx.send(());
                        break;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }

            // Read incoming data
            match port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    if let Ok(text) = std::str::from_utf8(&buf[..n]) {
                        line_buf.push_str(text);
                        while let Some(pos) = line_buf.find('\n') {
                            let line = line_buf[..pos].trim_end_matches('\r').to_string();
                            if !line.is_empty() {
                                let _ = log_tx.send(line);
                            }
                            line_buf = line_buf[pos + 1..].to_string();
                        }
                    }
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => {
                    // Port disconnected unexpectedly
                    let _ = disconnect_tx.send(());
                    break;
                }
            }

            thread::sleep(Duration::from_millis(1));
        }
    }

    fn poll(&mut self) {
        if let Some(rx) = self.disconnect_rx.as_ref() {
            if rx.try_recv().is_ok() {
                self.handle_disconnection();
            }
        }

        let mut received = Vec::new();
        let mut channel_disconnected = false;
        if let Some(rx) = self.log_rx.as_ref() {
            match rx.try_recv() {
                Ok(line) => {
                    received.push(line);
                    received.extend(rx.try_iter());
                }
                Err(TryRecvError::Disconnected) => channel_disconnected = true,
                Err(TryRecvError::Empty) => {}
            }
        }

        if channel_disconnected && self.connected {
            self.handle_disconnection();
        } else {
            for line in received {
                self.record_log_line(line);
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
        self.disconnect_rx = None;
        self.serial_tx = None;
        self.stop_flag = None;
        self.auto_save_file = None;
        // Reset reconnect cooldown
        self.reconnect_cooldown = Instant::now();
    }

    fn check_auto_reconnect(&mut self) {
        // Auto-reconnect if we were connected and the same port is available again
        // Add a small cooldown to avoid rapid reconnection attempts
        if !self.connected && self.reconnect_cooldown.elapsed() >= Duration::from_millis(500) {
            if let Some(ref last_port) = self.last_connected_port {
                // Check if the port is available
                let ports = self.ports.lock().map(|p| p.clone()).unwrap_or_default();
                let port_available = ports.iter().any(|(_, port_name)| port_name == last_port);

                if port_available && self.selected_port == *last_port {
                    // Same port is available again, auto-reconnect
                    self.connect();
                }
            }
        }
    }

    fn send_command(&mut self, cmd: &str) {
        let Some(cmd) = register_submitted_command(&mut self.command_history, cmd) else {
            return;
        };

        if let Some(ref tx) = self.serial_tx {
            let mut data = cmd.as_bytes().to_vec();
            if self.send_newline {
                data.push(b'\r');
                data.push(b'\n');
            }
            let _ = tx.send(data);
        }
    }

    fn filtered(&self) -> Vec<&LogEntry> {
        if self.search.is_empty() {
            self.logs.iter().collect()
        } else {
            let s = self.search.to_lowercase();
            self.logs
                .iter()
                .filter(|e| e.message.to_lowercase().contains(&s))
                .collect()
        }
    }

    fn copy_selected(&self, ctx: &egui::Context) {
        let text = if self.selected.is_empty() {
            format_log_entries(self.filtered())
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

    fn choose_save_path(&mut self) {
        let default_name = self
            .auto_save_path
            .as_ref()
            .and_then(|p| p.file_name().and_then(|n| n.to_str()))
            .unwrap_or("HI-Logger.log")
            .to_string();

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log", &["log", "txt"])
            .set_file_name(&default_name)
            .save_file()
        {
            self.auto_save_path = Some(path);
            self.auto_save_current_path = None;
            self.auto_save_file = None;
            self.auto_save_bytes_written = 0;
            self.auto_save_sequence = 0;
            self.auto_save_error = None;

            if self.auto_save_enabled {
                self.open_new_auto_save_file();
            }
        }
    }

    fn export_logs(&self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log", &["log", "txt"])
            .set_file_name("export.log")
            .save_file()
        {
            let content = self
                .filtered()
                .iter()
                .map(|e| e.format_line())
                .collect::<Vec<_>>()
                .join("\n");
            let _ = fs::write(path, content);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UI
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests;
