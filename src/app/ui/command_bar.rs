use super::super::*;

impl TraceXApp {
    pub(super) fn render_command_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("command_rail")
            .exact_height(126.0)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_COMMAND)
                    .stroke(Stroke::new(1.0_f32, theme::BORDER))
                    .inner_margin(egui::Margin::symmetric(16.0, 10.0)),
            )
            .show(ctx, |ui| {
                let recent_commands: Vec<String> =
                    self.command_history.iter().rev().take(8).cloned().collect();
                let mut resend_command = None;
                let mut quick_command = None;

                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 32.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.label(
                            RichText::new("HISTORY:")
                                .monospace()
                                .size(9.0)
                                .color(theme::TEXT_DIM),
                        );
                        let history_width = (ui.available_width() - 220.0).max(80.0);
                        ui.allocate_ui(Vec2::new(history_width, 28.0), |ui| {
                            egui::ScrollArea::horizontal()
                                .id_salt("command_history_scroll")
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if recent_commands.is_empty() {
                                            for command in
                                                ["AT+CONFIG?", "AT+STATUS", "RESET_SYSTEM"]
                                            {
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            RichText::new(command)
                                                                .monospace()
                                                                .size(10.0)
                                                                .color(theme::TEXT_DIM),
                                                        )
                                                        .small()
                                                        .rounding(10.0),
                                                    )
                                                    .clicked()
                                                {
                                                    quick_command = Some(command.to_string());
                                                }
                                            }
                                        } else {
                                            for command in &recent_commands {
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            RichText::new(command)
                                                                .monospace()
                                                                .size(10.0)
                                                                .color(theme::TEXT_DIM),
                                                        )
                                                        .small()
                                                        .rounding(10.0),
                                                    )
                                                    .on_hover_text("Send this command again")
                                                    .clicked()
                                                {
                                                    resend_command = Some(command.clone());
                                                }
                                            }
                                        }
                                    });
                                });
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_sized(
                                    [48.0, 24.0],
                                    egui::Button::new(
                                        RichText::new("HELP")
                                            .monospace()
                                            .size(9.0)
                                            .color(theme::TEXT_DIM),
                                    ),
                                )
                                .clicked()
                            {
                                quick_command = Some("HELP".to_string());
                            }
                            if ui
                                .add_sized(
                                    [48.0, 24.0],
                                    egui::Button::new(
                                        RichText::new("PING")
                                            .monospace()
                                            .size(9.0)
                                            .color(theme::SUCCESS),
                                    ),
                                )
                                .clicked()
                            {
                                quick_command = Some("PING".to_string());
                            }
                            if ui
                                .add_sized(
                                    [62.0, 24.0],
                                    egui::Button::new(
                                        RichText::new("RESTART")
                                            .monospace()
                                            .size(9.0)
                                            .color(theme::ERROR),
                                    ),
                                )
                                .clicked()
                            {
                                quick_command = Some("RESET_SYSTEM".to_string());
                            }
                        });
                    },
                );

                if let Some(command) = resend_command.or(quick_command) {
                    self.send_command(&command);
                }

                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let input_width = (ui.available_width() - 310.0).max(180.0);
                    egui::Frame::none()
                        .fill(theme::BG_DARK)
                        .stroke(Stroke::new(1.0_f32, theme::BORDER))
                        .rounding(4.0)
                        .inner_margin(egui::Margin::symmetric(10.0, 3.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(">")
                                        .monospace()
                                        .size(15.0)
                                        .color(theme::TEXT_DIM),
                                );
                                let response = ui.add_sized(
                                    [input_width - 28.0, 30.0],
                                    egui::TextEdit::singleline(&mut self.command_input)
                                        .font(FontId::monospace(12.0))
                                        .hint_text("Enter command...")
                                        .frame(false),
                                );
                                let enter = response.lost_focus()
                                    && ui.input(|input| input.key_pressed(egui::Key::Enter));
                                if enter {
                                    let command = self.command_input.clone();
                                    if self.send_command(&command) && self.auto_clear_command {
                                        self.command_input.clear();
                                    }
                                }
                            });
                        });

                    ui.add_space(8.0);
                    ui.checkbox(&mut self.auto_clear_command, "Auto-clear");
                    if ui
                        .add_sized(
                            [74.0, 38.0],
                            egui::Button::new(
                                RichText::new(if self.send_newline { "NL & CR" } else { "NONE" })
                                    .monospace()
                                    .size(10.0)
                                    .color(theme::TEXT_DIM),
                            ),
                        )
                        .on_hover_text("Toggle command line ending")
                        .clicked()
                    {
                        self.send_newline = !self.send_newline;
                    }
                    if ui
                        .add_sized(
                            [96.0, 38.0],
                            egui::Button::new(
                                RichText::new("SEND  >")
                                    .size(12.0)
                                    .color(Color32::from_rgb(35, 49, 67))
                                    .strong(),
                            )
                            .fill(theme::ACCENT)
                            .stroke(Stroke::new(1.0_f32, theme::BORDER)),
                        )
                        .clicked()
                    {
                        let command = self.command_input.clone();
                        if self.send_command(&command) && self.auto_clear_command {
                            self.command_input.clear();
                        }
                    }
                });
            });
    }
}
