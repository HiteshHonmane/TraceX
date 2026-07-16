use super::super::*;

impl App {
    pub(super) fn render_command_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("command_rail")
            .exact_height(70.0)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_PANEL)
                    .rounding(egui::Rounding {
                        nw: 0.0,
                        ne: 0.0,
                        sw: 0.0,
                        se: APP_ROUNDING,
                    })
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .inner_margin(9.0),
            )
            .show(ctx, |ui| {
                let recent_commands: Vec<String> =
                    self.command_history.iter().rev().cloned().collect();
                let mut resend_command = None;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("HISTORY").size(8.0).color(theme::TEXT_MUTED));
                    if recent_commands.is_empty() {
                        ui.label(
                            RichText::new("Submitted commands will appear here")
                                .size(9.0)
                                .color(theme::TEXT_MUTED),
                        );
                    } else {
                        egui::ScrollArea::horizontal()
                            .id_salt("command_history_scroll")
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    for command in &recent_commands {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new(command)
                                                        .monospace()
                                                        .size(9.0)
                                                        .color(theme::TEXT_DIM),
                                                )
                                                .small(),
                                            )
                                            .on_hover_text("Send this command again")
                                            .clicked()
                                        {
                                            resend_command = Some(command.clone());
                                        }
                                    }
                                });
                            });
                    }
                });
                if let Some(command) = resend_command {
                    self.send_command(&command);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(">")
                            .monospace()
                            .size(18.0)
                            .color(theme::SUCCESS)
                            .strong(),
                    );
                    let response = ui.add_sized(
                        [ui.available_width() - 94.0, 28.0],
                        egui::TextEdit::singleline(&mut self.command_input)
                            .font(FontId::monospace(12.0))
                            .hint_text("Enter ASCII command (HEX supported, e.g. \\xAA)…")
                            .frame(false),
                    );
                    let enter =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui
                        .add_sized(
                            [72.0, 30.0],
                            egui::Button::new(
                                RichText::new("SEND")
                                    .size(10.0)
                                    .color(theme::ACCENT)
                                    .strong(),
                            ),
                        )
                        .clicked()
                        || enter
                    {
                        let cmd = self.command_input.clone();
                        self.send_command(&cmd);
                        self.command_input.clear();
                    }
                });
            });
    }
}
