use super::super::*;

impl TraceXApp {
    pub(super) fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("connection_sidebar")
            .exact_width(256.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_INPUT)
                    .stroke(Stroke::new(1.0_f32, theme::BORDER)),
            )
            .show(ctx, |ui| {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0))
                    .show(ui, |ui| {
                        ui.add_space(22.0);
                        ui.label(
                            RichText::new("TraceX")
                                .size(24.0)
                                .color(theme::TEXT)
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format!("V{}-FOUNDATION", env!("CARGO_PKG_VERSION")))
                                .monospace()
                                .size(10.0)
                                .color(theme::TEXT_DIM)
                                .strong(),
                        );
                        ui.add_space(26.0);

                        let (connection_label, connection_color, connection_text) =
                            if self.connected {
                                ("−  Disconnect Device", theme::ERROR, theme::TEXT)
                            } else {
                                (
                                    "+  Connect Device",
                                    theme::ACCENT,
                                    Color32::from_rgb(35, 49, 67),
                                )
                            };
                        let connection_ready = self.connected || !self.selected_port.is_empty();
                        if ui
                            .add_enabled(
                                connection_ready,
                                egui::Button::new(
                                    RichText::new(connection_label)
                                        .size(14.0)
                                        .color(connection_text),
                                )
                                .fill(connection_color)
                                .stroke(Stroke::new(1.0_f32, theme::BORDER))
                                .rounding(4.0)
                                .min_size(Vec2::new(ui.available_width(), 48.0)),
                            )
                            .clicked()
                        {
                            if self.connected {
                                self.disconnect();
                            } else {
                                self.connect();
                            }
                        }

                        ui.add_space(26.0);
                        ui.label(
                            RichText::new("CONFIGURATION")
                                .monospace()
                                .size(10.0)
                                .color(theme::TEXT_DIM)
                                .strong(),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            RichText::new("Port")
                                .monospace()
                                .size(11.0)
                                .color(theme::TEXT_DIM)
                                .strong(),
                        );
                        ui.add_space(4.0);

                        let ports = self.ports.lock().map(|p| p.clone()).unwrap_or_default();
                        let selected_display = ports
                            .iter()
                            .find(|(_, port)| port == &self.selected_port)
                            .map(|(display, _)| display.clone())
                            .unwrap_or_else(|| {
                                if self.selected_port.is_empty() {
                                    "Select a port".to_string()
                                } else {
                                    self.selected_port.clone()
                                }
                            });
                        ui.add_enabled_ui(!self.connected, |ui| {
                            egui::ComboBox::from_id_salt("port_combo")
                                .width(ui.available_width())
                                .selected_text(
                                    RichText::new(selected_display)
                                        .monospace()
                                        .size(12.0)
                                        .color(theme::TEXT),
                                )
                                .show_ui(ui, |ui| {
                                    if ports.is_empty() {
                                        ui.label(
                                            RichText::new("No serial ports detected")
                                                .size(11.0)
                                                .color(theme::TEXT_MUTED),
                                        );
                                    }
                                    for (display, port) in &ports {
                                        if ui
                                            .selectable_label(
                                                self.selected_port == *port,
                                                RichText::new(display).monospace().size(11.0),
                                            )
                                            .clicked()
                                        {
                                            self.selected_port = port.clone();
                                        }
                                    }
                                });
                        });

                        ui.add_space(14.0);
                        ui.label(
                            RichText::new("Baud Rate")
                                .monospace()
                                .size(11.0)
                                .color(theme::TEXT_DIM)
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.add_enabled_ui(!self.connected, |ui| {
                            egui::ComboBox::from_id_salt("baud_combo")
                                .width(ui.available_width())
                                .selected_text(
                                    RichText::new(self.baud_rate.to_string())
                                        .monospace()
                                        .size(12.0)
                                        .color(theme::TEXT),
                                )
                                .show_ui(ui, |ui| {
                                    for rate in BAUD_RATES {
                                        if ui
                                            .selectable_label(
                                                self.baud_rate == rate,
                                                RichText::new(rate.to_string())
                                                    .monospace()
                                                    .size(11.0),
                                            )
                                            .clicked()
                                        {
                                            self.baud_rate = rate;
                                        }
                                    }
                                });
                        });

                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            if ui
                                .checkbox(&mut self.auto_save_enabled, "Auto-Save Logs")
                                .changed()
                            {
                                self.apply_auto_save_enabled_change();
                            }
                        });
                        self.render_auto_save_status(ui);
                    });

                ui.add_space(24.0);
                ui.separator();
                ui.add_space(16.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 40.0],
                        egui::Button::new(
                            RichText::new("Library").size(13.0).color(theme::TEXT_DIM),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                    )
                    .on_hover_text("Export the visible log output")
                    .clicked()
                {
                    self.export_logs();
                }
                if let Some(error) = self.operation_error.as_ref() {
                    ui.label(
                        RichText::new(error)
                            .monospace()
                            .size(9.0)
                            .color(theme::ERROR),
                    );
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.add_sized(
                        [ui.available_width(), 38.0],
                        egui::Button::new(
                            RichText::new("?   Support")
                                .size(13.0)
                                .color(theme::TEXT_DIM),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                    );

                    let settings_response = ui.add_sized(
                        [ui.available_width(), 38.0],
                        egui::Button::new(
                            RichText::new("⚙   Settings")
                                .size(13.0)
                                .color(theme::TEXT_DIM),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                    );
                    let popup_id = ui.make_persistent_id("tracex_settings_popup");
                    if settings_response.clicked() {
                        ui.memory_mut(|memory| memory.toggle_popup(popup_id));
                    }
                    egui::popup::popup_above_or_below_widget(
                        ui,
                        popup_id,
                        &settings_response,
                        egui::AboveOrBelow::Above,
                        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                        |ui| {
                            ui.set_min_width(230.0);
                            ui.set_max_width(250.0);
                            self.render_settings(ui);
                        },
                    );
                });
            });
    }
}
