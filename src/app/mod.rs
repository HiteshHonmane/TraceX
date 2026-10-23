#[cfg(test)]
use crate::domain::history_budget::HistoryBudget;
use crate::domain::log::*;
#[cfg(test)]
use crate::domain::query::QueryScope;
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use serialport::available_ports;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const WINDOW_BAR_HEIGHT: f32 = 24.0;
const TOP_HEADER_HEIGHT: f32 = 44.0;
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

mod autosave;
mod autosave_worker;
mod capture;
mod log_format;
mod query_view;
mod query_worker;
mod session;
mod session_archive;
mod session_capture;
use session::SessionRuntime;
mod shutdown;
mod storage;

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

#[cfg(test)]
fn log_content_width<'a>(messages: impl IntoIterator<Item = &'a str>, minimum: f32) -> f32 {
    messages
        .into_iter()
        .map(|message| 205.0 + message.chars().count() as f32 * 7.2)
        .fold(minimum, f32::max)
}

fn add_copy_row_menu(
    response: &egui::Response,
    ctx: &egui::Context,
    entry: &LogEntry,
    logs: &[SharedLog],
    selected: &HashSet<usize>,
) {
    response.context_menu(|ui| {
        if ui.button("Copy row").clicked() {
            ctx.copy_text(format_log_line(
                &entry.timestamp,
                entry.level,
                &entry.message,
            ));
            ui.close_menu();
        }
        if selected.len() > 1 {
            ui.separator();
            if ui
                .button(format!("Copy all selected rows ({})", selected.len()))
                .clicked()
            {
                ctx.copy_text(format_log_entries(
                    logs.iter()
                        .filter(|entry| selected.contains(&entry.id))
                        .map(AsRef::as_ref),
                ));
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
        Box::new(move |cc| Ok(Box::new(TraceXApp::new().prepare_native_ui(&cc.egui_ctx)))),
    )
}

struct UiTab {
    title: String,
    setup: bool,
}

struct TraceXApp {
    // A single shared port catalog belongs to the native application shell.
    ports: Arc<Mutex<Vec<(String, String)>>>,
    session: SessionRuntime,
    ui_tabs: Vec<UiTab>,
    active_tab: usize,
}

impl TraceXApp {
    fn prepare_native_ui(self, ctx: &egui::Context) -> Self {
        theme::setup(ctx);
        self
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
            session: SessionRuntime::new(),
            ui_tabs: vec![UiTab {
                title: "Monitor 1".to_owned(),
                setup: false,
            }],
            active_tab: 0,
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
}

#[cfg(test)]
mod tests;
