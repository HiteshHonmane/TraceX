#![windows_subsystem = "windows"]

use chrono::Local;
use directories::UserDirs;
use eframe::egui::{self, Color32, FontId, RichText, Rounding, Stroke, Vec2};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serialport::{available_ports, SerialPort};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write as IoWrite};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════════════════
// ANSI CODE STRIPPER
// ═══════════════════════════════════════════════════════════════════════════════

fn strip_ansi(input: &str) -> String {
    let re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(input, "").to_string()
}

// ═══════════════════════════════════════════════════════════════════════════════
// ESP-IDF LOG PARSER
// ═══════════════════════════════════════════════════════════════════════════════

fn parse_log_level(text: &str) -> LogLevel {
    let upper = text.to_uppercase();
    if upper.contains("ERROR") { LogLevel::Error }
    else if upper.contains("WARN") { LogLevel::Warn }
    else if upper.contains("INFO") { LogLevel::Info }
    else if upper.contains("DEBUG") { LogLevel::Debug }
    else if upper.contains("VERBOSE") || upper.contains("TRACE") { LogLevel::Verbose }
    else { LogLevel::Log }
}

fn extract_tag(text: &str) -> Option<String> {
    let re = Regex::new(r"\]\[([A-Za-z][A-Za-z0-9_]*)\]\s").ok()?;
    let mut last = None;
    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let t = m.as_str();
            if !t.contains('.') && t.parse::<u32>().is_err() {
                last = Some(t.to_string());
            }
        }
    }
    last
}

fn extract_message(text: &str) -> String {
    if let Some(pos) = text.rfind(']') {
        let msg = text[pos + 1..].trim();
        if !msg.is_empty() { return msg.to_string(); }
    }
    text.to_string()
}

// ═══════════════════════════════════════════════════════════════════════════════
// THEME
// ═══════════════════════════════════════════════════════════════════════════════

mod theme {
    use super::*;

    pub const BG_DARK: Color32 = Color32::from_rgb(18, 18, 22);
    pub const BG_PANEL: Color32 = Color32::from_rgb(24, 24, 30);
    pub const BG_INPUT: Color32 = Color32::from_rgb(32, 32, 40);
    pub const BG_HOVER: Color32 = Color32::from_rgb(42, 42, 52);

    pub const TEXT: Color32 = Color32::from_rgb(230, 230, 235);
    pub const TEXT_DIM: Color32 = Color32::from_rgb(130, 130, 145);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(80, 80, 95);

    pub const ACCENT: Color32 = Color32::from_rgb(0, 200, 220);
    pub const SUCCESS: Color32 = Color32::from_rgb(46, 204, 113);
    pub const WARNING: Color32 = Color32::from_rgb(255, 193, 7);
    pub const ERROR: Color32 = Color32::from_rgb(255, 82, 82);
    pub const INFO: Color32 = Color32::from_rgb(52, 152, 219);
    pub const DEBUG: Color32 = Color32::from_rgb(155, 89, 182);
    pub const VERBOSE: Color32 = Color32::from_rgb(127, 140, 141);

    pub const BORDER: Color32 = Color32::from_rgb(50, 50, 60);

    pub fn setup(ctx: &egui::Context) {
        let mut v = egui::Visuals::dark();
        v.panel_fill = BG_DARK;
        v.window_fill = BG_PANEL;
        v.extreme_bg_color = BG_DARK;
        v.widgets.noninteractive.bg_fill = BG_PANEL;
        v.widgets.inactive.bg_fill = BG_INPUT;
        v.widgets.hovered.bg_fill = BG_HOVER;
        v.widgets.active.bg_fill = BG_HOVER;
        v.selection.bg_fill = ACCENT.linear_multiply(0.2);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        ctx.set_visuals(v);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(12.0, 6.0);
        ctx.set_style(style);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DATA
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
enum LogLevel {
    Error, Warn, Info, Debug, Verbose, Log,
}

impl LogLevel {
    fn color(&self) -> Color32 {
        match self {
            Self::Error => theme::ERROR,
            Self::Warn => theme::WARNING,
            Self::Info => theme::INFO,
            Self::Debug => theme::DEBUG,
            Self::Verbose => theme::VERBOSE,
            Self::Log => theme::TEXT_DIM,
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Error => "ERR",
            Self::Warn => "WRN",
            Self::Info => "INF",
            Self::Debug => "DBG",
            Self::Verbose => "VRB",
            Self::Log => "LOG",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct LogEntry {
    id: usize,
    timestamp: String,
    level: LogLevel,
    tag: Option<String>,
    message: String,
    raw: String,
}

impl LogEntry {
    fn new(id: usize, raw: String) -> Self {
        let clean = strip_ansi(&raw);
        Self {
            id,
            timestamp: Local::now().format("%H:%M:%S.%3f").to_string(),
            level: parse_log_level(&clean),
            tag: extract_tag(&clean),
            message: extract_message(&clean),
            raw,
        }
    }

    fn format_line(&self) -> String {
        format!("[{}] [{}] {}", self.timestamp, self.level.label(), self.message)
    }
}

const BAUD_RATES: [u32; 8] = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];

// ═══════════════════════════════════════════════════════════════════════════════
// APP
// ═══════════════════════════════════════════════════════════════════════════════

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "HI-Logger",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 800.0])
                .with_min_inner_size([800.0, 600.0])
                .with_title("HI-Logger"),
            ..Default::default()
        },
        Box::new(|cc| {
            theme::setup(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
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
    batch_text: String,
    send_newline: bool,

    // Filter
    search: String,
    selected: HashSet<usize>,

    // Auto-save
    auto_save_enabled: bool,
    auto_save_path: Option<PathBuf>,
    auto_save_file: Option<std::fs::File>,

    // UI
    auto_scroll: bool,
    prev_auto_scroll: bool,
    show_settings: bool,

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
                return doc_dir.join("hi_logger.log");
            }
        }
        // Fallback to current directory if Documents folder not found
        PathBuf::from("hi_logger.log")
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
            batch_text: String::new(),
            send_newline: true,

            search: String::new(),
            selected: HashSet::new(),

            auto_save_enabled: false,
            auto_save_path: Some(Self::default_log_path()),
            auto_save_file: None,

            auto_scroll: true,
            prev_auto_scroll: true,
            show_settings: false,

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
                            cleaned = cleaned.trim_end_matches(" ()").trim_end_matches("()").to_string();
                            // Remove generic terms that don't add value
                            let generic_terms = ["USB Serial Port", "USB SERIAL", "USB"];
                            for term in &generic_terms {
                                if cleaned == *term {
                                    cleaned.clear();
                                    break;
                                }
                            }
                            if cleaned.is_empty() { None } else { Some(cleaned) }
                        } else {
                            None
                        };
                        
                        // Clean manufacturer name by removing port name and generic terms
                        let clean_manufacturer = if !manufacturer.is_empty() {
                            let mut cleaned = manufacturer.replace(&port_name, "").trim().to_string();
                            // Remove trailing empty parentheses
                            cleaned = cleaned.trim_end_matches(" ()").trim_end_matches("()").to_string();
                            // Remove generic terms that don't add value
                            let generic_terms = ["USB Serial Port", "USB SERIAL", "USB"];
                            for term in &generic_terms {
                                if cleaned == *term {
                                    cleaned.clear();
                                    break;
                                }
                            }
                            if cleaned.is_empty() { None } else { Some(cleaned) }
                        } else {
                            None
                        };
                        
                        if let Some(product) = clean_product {
                            format!("{} ({})", port_name, product)
                        } else if let Some(manufacturer) = clean_manufacturer {
                            format!("{} ({})", port_name, manufacturer)
                        } else {
                            port_name.clone()
                        }
                    }
                    serialport::SerialPortType::PciPort => format!("{} (PCI)", port_name),
                    serialport::SerialPortType::BluetoothPort => format!("{} (Bluetooth)", port_name),
                    serialport::SerialPortType::Unknown => port_name.clone(),
                };
                (display_name, port_name)
            })
            .collect()
    }

    fn connect(&mut self) {
        if self.selected_port.is_empty() { return; }

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

        // Open auto-save file if enabled
        if self.auto_save_enabled {
            // Use default path if not set (shouldn't happen, but safety check)
            if self.auto_save_path.is_none() {
                self.auto_save_path = Some(Self::default_log_path());
            }
            if let Some(ref path) = self.auto_save_path {
                self.auto_save_file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .ok();
            }
        }
    }

    fn disconnect(&mut self) {
        if let Some(flag) = &self.stop_flag {
            if let Ok(mut f) = flag.lock() { *f = true; }
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
            if stop.lock().map(|f| *f).unwrap_or(true) { break; }

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
        // Check for unexpected disconnection
        if let Some(ref rx) = self.disconnect_rx {
            if rx.try_recv().is_ok() {
                // Unexpected disconnection detected
                self.handle_disconnection();
            }
        }

        if let Some(ref rx) = self.log_rx {
            // Check if channel is disconnected (backup detection method)
            match rx.try_recv() {
                Ok(line) => {
                    let entry = LogEntry::new(self.next_id, line);
                    self.next_id += 1;
                    self.rate_count += 1;

                    // Instant append to file
                    if let Some(ref mut file) = self.auto_save_file {
                        let _ = writeln!(file, "{}", entry.format_line());
                    }

                    if !self.paused {
                        self.logs.push(entry);
                    }
                    
                    // Process remaining messages
                    while let Ok(line) = rx.try_recv() {
                        let entry = LogEntry::new(self.next_id, line);
                        self.next_id += 1;
                        self.rate_count += 1;

                        // Instant append to file
                        if let Some(ref mut file) = self.auto_save_file {
                            let _ = writeln!(file, "{}", entry.format_line());
                        }

                        if !self.paused {
                            self.logs.push(entry);
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    // Channel disconnected - unexpected disconnection
                    if self.connected {
                        self.handle_disconnection();
                    }
                }
                Err(TryRecvError::Empty) => {}
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
        if let Some(ref tx) = self.serial_tx {
            let mut data = cmd.as_bytes().to_vec();
            if self.send_newline {
                data.push(b'\r');
                data.push(b'\n');
            }
            let _ = tx.send(data);
            if !cmd.is_empty() && !self.command_history.contains(&cmd.to_string()) {
                self.command_history.push(cmd.to_string());
            }
        }
    }

    fn send_batch(&mut self, batch: &str) {
        for line in batch.lines() {
            let cmd = line.trim();
            if !cmd.is_empty() && !cmd.starts_with('#') {
                self.send_command(cmd);
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    fn filtered(&self) -> Vec<&LogEntry> {
        if self.search.is_empty() {
            self.logs.iter().collect()
        } else {
            let s = self.search.to_lowercase();
            self.logs.iter().filter(|e| e.message.to_lowercase().contains(&s)).collect()
        }
    }

    fn copy_selected(&self, ctx: &egui::Context) {
        let text: String = if self.selected.is_empty() {
            self.filtered().iter().map(|e| e.format_line()).collect::<Vec<_>>().join("\n")
        } else {
            self.logs.iter()
                .filter(|e| self.selected.contains(&e.id))
                .map(|e| e.format_line())
                .collect::<Vec<_>>()
                .join("\n")
        };
        ctx.copy_text(text);
    }

    fn copy_all(&self, ctx: &egui::Context) {
        let text: String = self.filtered().iter().map(|e| e.format_line()).collect::<Vec<_>>().join("\n");
        ctx.copy_text(text);
    }

    fn clear(&mut self) {
        self.logs.clear();
        self.selected.clear();
        self.next_id = 0;
    }

    fn choose_save_path(&mut self) {
        let default_name = self.auto_save_path
            .as_ref()
            .and_then(|p| p.file_name().and_then(|n| n.to_str()))
            .unwrap_or("hi_logger.log")
            .to_string();
        
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log", &["log", "txt"])
            .set_file_name(&default_name)
            .save_file()
        {
            self.auto_save_path = Some(path.clone());
            
            // If connected, open the file immediately
            if self.connected && self.auto_save_enabled {
                self.auto_save_file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .ok();
            }
        }
    }

    fn export_logs(&self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Log", &["log", "txt"])
            .set_file_name("export.log")
            .save_file()
        {
            let content = self.filtered().iter().map(|e| e.format_line()).collect::<Vec<_>>().join("\n");
            let _ = fs::write(path, content);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UI
// ═══════════════════════════════════════════════════════════════════════════════

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.check_auto_reconnect();
        ctx.request_repaint_after(Duration::from_millis(16));

        // Keyboard shortcuts
        ctx.input(|i| {
            if (i.modifiers.ctrl || i.modifiers.mac_cmd) && i.key_pressed(egui::Key::C) {
                self.copy_selected(ctx);
            }
        });

        // Left panel - Connection
        egui::SidePanel::left("left_panel")
            .default_width(280.0)
            .min_width(250.0)
            .frame(egui::Frame::none().fill(theme::BG_PANEL).inner_margin(16.0))
            .show(ctx, |ui| {
                ui.label(RichText::new("HI-Logger").size(20.0).color(theme::ACCENT).strong());
                ui.add_space(16.0);

                // Port selection - Radio buttons
                ui.label(RichText::new("SERIAL PORT").size(11.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);

                let ports = self.ports.lock().map(|p| p.clone()).unwrap_or_default();
                
                if ports.is_empty() {
                    ui.label(RichText::new("No ports found").color(theme::TEXT_DIM).italics());
                } else {
                    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                        for (display_name, port_name) in &ports {
                            let selected = self.selected_port == *port_name;
                            let is_current = self.connected && selected;
                            
                            ui.horizontal(|ui| {
                                let resp = ui.radio(selected, "");
                                
                                let color = if is_current {
                                    theme::SUCCESS
                                } else if selected {
                                    theme::ACCENT
                                } else {
                                    theme::TEXT
                                };
                                
                                ui.label(RichText::new(display_name).monospace().size(12.0).color(color));
                                
                                if resp.clicked() && !self.connected {
                                    self.selected_port = port_name.clone();
                                }
                            });
                        }
                    });
                }

                ui.add_space(16.0);

                // Baud rate - Radio buttons
                ui.label(RichText::new("BAUD RATE").size(11.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);

                ui.horizontal_wrapped(|ui| {
                    for rate in BAUD_RATES {
                        let selected = self.baud_rate == rate;
                        if ui.radio(selected, RichText::new(format!("{}", rate)).size(12.0)).clicked() && !self.connected {
                            self.baud_rate = rate;
                        }
                    }
                });

                ui.add_space(16.0);

                // Connect/Disconnect
                let (btn_text, btn_color) = if self.connected {
                    ("Disconnect", theme::ERROR)
                } else {
                    ("Connect", theme::SUCCESS)
                };

                if ui.add_sized(
                    [ui.available_width(), 32.0],
                    egui::Button::new(RichText::new(btn_text).color(btn_color).strong())
                ).clicked() {
                    if self.connected { self.disconnect(); } else { self.connect(); }
                }

                if self.connected {
                    ui.add_space(8.0);
                    let (p_text, p_color) = if self.paused {
                        ("Resume", theme::SUCCESS)
                    } else {
                        ("Pause", theme::WARNING)
                    };
                    if ui.add_sized(
                        [ui.available_width(), 28.0],
                        egui::Button::new(RichText::new(p_text).color(p_color))
                    ).clicked() {
                        self.paused = !self.paused;
                    }
                }

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(12.0);

                // Auto-save settings
                ui.label(RichText::new("AUTO-SAVE").size(11.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);

                ui.checkbox(&mut self.auto_save_enabled, "Enable auto-save");

                if let Some(ref path) = self.auto_save_path {
                    ui.label(RichText::new(path.display().to_string()).size(10.0).color(theme::TEXT_DIM));
                }

                if ui.button("Choose file...").clicked() {
                    self.choose_save_path();
                }

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(12.0);

                // Actions
                ui.label(RichText::new("ACTIONS").size(11.0).color(theme::TEXT_MUTED));
                ui.add_space(4.0);

                if ui.button("Export logs...").clicked() {
                    self.export_logs();
                }

                if ui.button("Clear logs").clicked() {
                    self.clear();
                }

                let checkbox_response = ui.checkbox(&mut self.auto_scroll, "Auto-scroll");
                // Update prev_auto_scroll when checkbox changes (but don't update if we're about to force scroll)
                if checkbox_response.changed() && (self.prev_auto_scroll || !self.auto_scroll) {
                    self.prev_auto_scroll = self.auto_scroll;
                }

                // Stats at bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    // Branding at the very bottom
                    ui.add_space(4.0);
                    
                    // Buy me a coffee - more prominent
                    ui.hyperlink_to(
                        RichText::new("☕ Buy me a coffee").size(12.0).color(theme::WARNING).strong(),
                        "https://buymeacoffee.com/hitesh_honmane"
                    );
                    
                    ui.add_space(6.0);
                    
                    // Developer info - more visible
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Developed by").size(11.0).color(theme::TEXT_DIM));
                        ui.hyperlink_to(
                            RichText::new("Hitesh Honmane").size(11.0).color(theme::ACCENT).strong(),
                            "https://www.linkedin.com/in/hitesh-honmane"
                        );
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("For").size(11.0).color(theme::TEXT_DIM));
                        ui.label(RichText::new("Firmware Enthusiasts").size(11.0).color(theme::INFO).strong());
                    });
                    
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(4.0);
                    
                    // Stats above branding
                    ui.label(RichText::new(format!(
                        "{:.0}/s | {} logs | {} selected",
                        self.rate, self.logs.len(), self.selected.len()
                    )).size(11.0).color(theme::TEXT_DIM));

                    let status = if self.connected {
                        if self.paused { ("Paused", theme::WARNING) }
                        else { ("Connected", theme::SUCCESS) }
                    } else {
                        ("Disconnected", theme::TEXT_MUTED)
                    };
                    ui.label(RichText::new(format!("● {}", status.0)).size(12.0).color(status.1));
                });
            });

        // Bottom panel - Command input
        egui::TopBottomPanel::bottom("cmd_panel")
            .frame(egui::Frame::none().fill(theme::BG_PANEL).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("CMD").size(11.0).color(theme::TEXT_MUTED));
                    ui.add_space(8.0);

                    let resp = ui.add_sized(
                        [ui.available_width() - 180.0, 24.0],
                        egui::TextEdit::singleline(&mut self.command_input)
                            .font(FontId::monospace(13.0))
                            .hint_text("Enter command (e.g., AT+GMR)")
                    );

                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let cmd = self.command_input.clone();
                        self.send_command(&cmd);
                        self.command_input.clear();
                        resp.request_focus();
                    }

                    ui.checkbox(&mut self.send_newline, "\\r\\n");

                    if ui.button("Send").clicked() {
                        let cmd = self.command_input.clone();
                        self.send_command(&cmd);
                        self.command_input.clear();
                    }

                    let mut send_batch_now = false;
                    ui.menu_button("Batch", |ui| {
                        ui.label("Send multiple commands (one per line):");
                        ui.add_space(4.0);
                        
                        ui.add_sized(
                            [300.0, 120.0],
                            egui::TextEdit::multiline(&mut self.batch_text)
                                .font(FontId::monospace(12.0))
                                .hint_text("AT+GMR\nAT+RST\n...")
                        );

                        if ui.button("Send All").clicked() {
                            send_batch_now = true;
                            ui.close_menu();
                        }
                    });

                    if send_batch_now {
                        let batch = self.batch_text.clone();
                        self.send_batch(&batch);
                    }
                });

                // Command history
                if !self.command_history.is_empty() {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("History:").size(10.0).color(theme::TEXT_MUTED));
                        for cmd in self.command_history.iter().rev().take(5) {
                            if ui.small_button(cmd).clicked() {
                                self.command_input = cmd.clone();
                            }
                        }
                    });
                }
            });

        // Central panel - Log view
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::BG_DARK).inner_margin(0.0))
            .show(ctx, |ui| {
                // Search bar - VS Code style compact bar
                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(theme::BG_PANEL)
                    .rounding(Rounding::same(4.0))
                    .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Find:").size(11.0).color(theme::TEXT_MUTED));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search)
                                    .desired_width(180.0)
                                    .font(FontId::monospace(11.0))
                                    .hint_text("Search...")
                            );

                            if ui.small_button("Copy All").clicked() {
                                self.copy_all(ctx);
                            }

                            if !self.selected.is_empty() {
                                ui.separator();
                                ui.label(RichText::new(format!("{} selected", self.selected.len())).size(11.0).color(theme::ACCENT));
                                if ui.small_button("Copy").on_hover_text("Ctrl+C").clicked() {
                                    self.copy_selected(ctx);
                                }
                                if ui.small_button("Clear").clicked() {
                                    self.selected.clear();
                                }
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new("Click to select | Ctrl+Click multi | Shift+Click range").size(10.0).color(theme::TEXT_MUTED));
                            });
                        });
                    });

                ui.add_space(4.0);

                // Log list
                let filtered: Vec<(usize, String, LogLevel, String, bool)> = self.filtered()
                    .iter()
                    .map(|e| (e.id, e.timestamp.clone(), e.level, e.message.clone(), self.selected.contains(&e.id)))
                    .collect();

                let row_h = 24.0;

                // Check if we need to force scroll to bottom (auto_scroll just enabled)
                let should_force_scroll = !self.prev_auto_scroll && self.auto_scroll;

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(self.auto_scroll)
                    .show_rows(ui, row_h, filtered.len(), |ui, range| {
                        for i in range {
                            if let Some((id, ts, level, msg, is_sel)) = filtered.get(i) {
                                let bg = if *is_sel {
                                    theme::ACCENT.linear_multiply(0.15)
                                } else if i % 2 == 0 {
                                    theme::BG_DARK
                                } else {
                                    theme::BG_PANEL.linear_multiply(0.5)
                                };

                                let eid = *id;
                                let ets = ts.clone();
                                let elevel = *level;
                                let emsg = msg.clone();

                                let resp = ui.push_id(eid, |ui| {
                                    egui::Frame::none()
                                        .fill(bg)
                                        .inner_margin(egui::Margin::symmetric(12.0, 3.0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                // Timestamp
                                                ui.label(RichText::new(&ets).monospace().size(11.0).color(theme::TEXT_MUTED));

                                                ui.add_space(8.0);

                                                // Level badge
                                                egui::Frame::none()
                                                    .fill(elevel.color().linear_multiply(0.2))
                                                    .rounding(Rounding::same(3.0))
                                                    .inner_margin(egui::Margin::symmetric(6.0, 1.0))
                                                    .show(ui, |ui| {
                                                        ui.label(RichText::new(elevel.label()).monospace().size(10.0).color(elevel.color()));
                                                    });

                                                ui.add_space(8.0);

                                                // Message
                                                ui.add(egui::Label::new(
                                                    RichText::new(&emsg).monospace().size(12.0).color(theme::TEXT)
                                                ).wrap_mode(egui::TextWrapMode::Truncate));
                                            });
                                        })
                                }).response;

                                // Click to select/deselect
                                if resp.interact(egui::Sense::click()).clicked() {
                                    let mods = ui.input(|i| i.modifiers);
                                    if mods.shift {
                                        // Range select
                                        if let Some(&last) = self.selected.iter().max() {
                                            let (start, end) = if eid > last { (last, eid) } else { (eid, last) };
                                            for e in &self.logs {
                                                if e.id >= start && e.id <= end {
                                                    self.selected.insert(e.id);
                                                }
                                            }
                                        }
                                    } else if mods.ctrl || mods.mac_cmd {
                                        if self.selected.contains(&eid) {
                                            self.selected.remove(&eid);
                                        } else {
                                            self.selected.insert(eid);
                                        }
                                    } else {
                                        self.selected.clear();
                                        self.selected.insert(eid);
                                    }
                                }
                            }
                        }
                        
                        // Add spacer at bottom and scroll to it when auto_scroll is re-enabled
                        if should_force_scroll {
                            ui.add_space(1.0);
                            ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                        }
                    });
                
                // Update prev_auto_scroll after checking
                if should_force_scroll {
                    self.prev_auto_scroll = self.auto_scroll;
                }
            });
    }
}
