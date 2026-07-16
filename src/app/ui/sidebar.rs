use super::super::*;

impl App {
    pub(super) fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("connection_sidebar")
            .exact_width(216.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_INPUT)
                    .rounding(egui::Rounding {
                        nw: APP_ROUNDING,
                        ne: 0.0,
                        sw: APP_ROUNDING,
                        se: 0.0,
                    })
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(ctx, |ui| {
                let header = ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), TOP_HEADER_HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add_space(12.0);
                        ui.add(
                            egui::Image::new(&self.logo_texture)
                                .fit_to_exact_size(Vec2::splat(42.0)),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("HI-LOGGER")
                                .size(15.0)
                                .color(theme::TEXT)
                                .strong(),
                        );
                    },
                );
                ui.painter().hline(
                    header.response.rect.x_range(),
                    header.response.rect.bottom(),
                    Stroke::new(1.0, theme::BORDER),
                );

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(14.0);

                        egui::Frame::none()
                            .inner_margin(egui::Margin::symmetric(12.0, 0.0))
                            .show(ui, |ui| {
                                let (connection_label, connection_color) = if self.connected {
                                    ("DISCONNECT", theme::ERROR)
                                } else {
                                    ("CONNECT", theme::SUCCESS)
                                };
                                let connection_ready =
                                    self.connected || !self.selected_port.is_empty();
                                if ui
                                    .add_enabled(
                                        connection_ready,
                                        egui::Button::new(
                                            RichText::new(connection_label)
                                                .size(12.0)
                                                .color(connection_color)
                                                .strong(),
                                        )
                                        .min_size(Vec2::new(ui.available_width(), 34.0)),
                                    )
                                    .clicked()
                                {
                                    if self.connected {
                                        self.disconnect();
                                    } else {
                                        self.connect();
                                    }
                                }
                                ui.add_space(18.0);
                                ui.label(
                                    RichText::new("PORT")
                                        .size(9.0)
                                        .color(theme::TEXT_MUTED)
                                        .strong(),
                                );
                                ui.add_space(6.0);
                                let ports =
                                    self.ports.lock().map(|p| p.clone()).unwrap_or_default();
                                let port_list_height = if ports.is_empty() {
                                    24.0
                                } else {
                                    (ports.len().min(4) as f32 * 24.0).max(28.0)
                                };
                                egui::Frame::none()
                                    .fill(theme::BG_INPUT)
                                    .stroke(Stroke::new(1.0, theme::BORDER))
                                    .rounding(5.0)
                                    .inner_margin(4.0)
                                    .show(ui, |ui| {
                                        egui::ScrollArea::vertical()
                                            .id_salt("port_scroll")
                                            .min_scrolled_height(port_list_height)
                                            .max_height(port_list_height)
                                            .auto_shrink([false, false])
                                            .show(ui, |ui| {
                                                if ports.is_empty() {
                                                    ui.label(
                                                        RichText::new("No serial ports detected")
                                                            .size(10.0)
                                                            .color(theme::TEXT_MUTED),
                                                    );
                                                }
                                                for (display, port) in ports {
                                                    let selected = self.selected_port == port;
                                                    if ui
                                                        .selectable_label(
                                                            selected,
                                                            RichText::new(display)
                                                                .monospace()
                                                                .size(10.0),
                                                        )
                                                        .clicked()
                                                        && !self.connected
                                                    {
                                                        self.selected_port = port;
                                                    }
                                                }
                                            });
                                    });

                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new("BAUD RATE")
                                        .size(9.0)
                                        .color(theme::TEXT_MUTED)
                                        .strong(),
                                );
                                ui.add_space(6.0);
                                egui::Frame::none()
                                    .fill(theme::BG_INPUT)
                                    .stroke(Stroke::new(1.0, theme::BORDER))
                                    .rounding(5.0)
                                    .inner_margin(4.0)
                                    .show(ui, |ui| {
                                        const BAUD_ROW_HEIGHT: f32 = 24.0;
                                        const BAUD_VIEWPORT_HEIGHT: f32 = 74.0;
                                        let center_selected = self.baud_scroll_to_selected;
                                        self.baud_scroll_to_selected = false;
                                        let selected_index = BAUD_RATES
                                            .iter()
                                            .position(|rate| *rate == self.baud_rate)
                                            .unwrap_or(0);
                                        let mut baud_scroll = egui::ScrollArea::vertical()
                                            .id_salt("baud_scroll")
                                            .max_height(BAUD_VIEWPORT_HEIGHT)
                                            .auto_shrink([false, false]);
                                        if center_selected {
                                            baud_scroll = baud_scroll.vertical_scroll_offset(
                                                centered_row_scroll_offset(
                                                    selected_index,
                                                    BAUD_RATES.len(),
                                                    BAUD_ROW_HEIGHT,
                                                    BAUD_VIEWPORT_HEIGHT,
                                                ),
                                            );
                                        }
                                        baud_scroll.show(ui, |ui| {
                                            for rate in BAUD_RATES {
                                                if ui
                                                    .add_sized(
                                                        [ui.available_width(), BAUD_ROW_HEIGHT],
                                                        egui::Button::new(
                                                            RichText::new(rate.to_string())
                                                                .monospace()
                                                                .size(10.0),
                                                        )
                                                        .selected(self.baud_rate == rate),
                                                    )
                                                    .clicked()
                                                    && !self.connected
                                                {
                                                    self.baud_rate = rate;
                                                    self.baud_scroll_to_selected = true;
                                                }
                                            }
                                        });
                                    });

                                ui.add_space(20.0);
                                ui.label(
                                    RichText::new("LOG SETTINGS")
                                        .size(9.0)
                                        .color(theme::TEXT_MUTED)
                                        .strong(),
                                );
                                ui.add_space(10.0);
                                if ui
                                    .checkbox(&mut self.auto_save_enabled, "Auto-save logs")
                                    .changed()
                                {
                                    self.apply_auto_save_enabled_change();
                                }
                                self.render_auto_save_status(ui);
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 30.0],
                                        egui::Button::new(
                                            RichText::new("CHOOSE AUTO-SAVE FILE")
                                                .size(9.0)
                                                .color(theme::TEXT_DIM),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.choose_save_path();
                                }
                                let auto_scroll_changed = ui
                                    .checkbox(&mut self.auto_scroll, "Auto-scroll output")
                                    .changed();
                                let live_scroll_frames = live_scroll_frames_after_toggle(
                                    auto_scroll_changed,
                                    self.auto_scroll,
                                );
                                if auto_scroll_changed {
                                    self.live_scroll_frames = live_scroll_frames;
                                }
                                ui.add_space(12.0);
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 30.0],
                                        egui::Button::new(
                                            RichText::new("EXPORT LOGS")
                                                .size(9.0)
                                                .color(theme::TEXT_DIM),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.export_logs();
                                }
                            });
                    });

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    let settings_response = ui.add_sized(
                        [192.0, 34.0],
                        egui::Button::new(
                            RichText::new("⚙  SETTINGS")
                                .size(10.0)
                                .color(theme::TEXT_DIM),
                        ),
                    );
                    let popup_id = ui.make_persistent_id("hi_logger_settings_popup");
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
                    ui.add_space(10.0);
                    ui.separator();
                });
            });
    }
}
