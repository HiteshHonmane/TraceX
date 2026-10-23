use super::super::*;

impl TraceXApp {
    pub(super) fn render_command_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("command_rail").exact_height(116.0)
            .frame(egui::Frame::none().fill(theme::BG_PANEL).inner_margin(egui::Margin::symmetric(16.0, 10.0)))
            .show(ctx, |ui| {
                let rail = ui.max_rect().expand2(Vec2::new(16.0, 10.0));
                ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(rail.left(), rail.top() + 44.0), rail.right_bottom()), 0.0, theme::BG_COMMAND);
                ui.painter().hline(rail.x_range(), rail.top(), Stroke::new(1.0_f32, theme::BORDER));
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 32.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.label(RichText::new("HISTORY:").size(10.0).color(theme::TEXT_MUTED));
                    egui::ScrollArea::horizontal().id_salt("command_history_scroll").auto_shrink([false, true]).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if self.command_history.is_empty() {
                                ui.label(RichText::new("No command history").size(11.0).color(theme::TEXT_MUTED));
                            }
                            let mut recalled = None;
                            for command in self.command_history.iter().rev().take(8) {
                                if ui.add(egui::Button::new(RichText::new(command).monospace().size(10.0).color(theme::TEXT_DIM)).rounding(4.0)).on_hover_text("Recall command attempt into editor; Send or Enter executes").clicked() { recalled = Some(command.clone()); }
                            }
                            if let Some(command) = recalled { self.recall_command(&command); }
                        });
                    });
                });
                ui.add_space(8.0);
                self.render_command_controls(ui);
            });
    }

    fn recall_command(&mut self, command: &str) {
        self.command_input = command.to_owned();
    }

    // Return the actual widget rectangles so headless layout tests can guard
    // containment at the supported minimum window size.
    fn render_command_controls(&mut self, ui: &mut egui::Ui) -> [egui::Rect; 3] {
        let mut rects = [egui::Rect::NOTHING; 3];
        ui.horizontal(|ui| {
            let editor_width = (ui.available_width() - 190.0).max(80.0);
            let editor = command_editor(ui, &mut self.command_input, editor_width);
            rects[0] = editor.frame;
            if editor.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.submit_editor_command();
            }
            let ending = ui.add_sized(
                [74.0, 32.0],
                egui::Button::new(
                    RichText::new(if self.send_newline { "NL & CR" } else { "NONE" })
                        .size(11.0)
                        .color(theme::TEXT_DIM),
                ),
            );
            rects[1] = ending.rect;
            if ending.on_hover_text("Toggle command line ending").clicked() {
                self.send_newline = !self.send_newline;
            }
            let send = ui.add_sized(
                [96.0, 32.0],
                egui::Button::new(
                    RichText::new("SEND    ")
                        .size(12.0)
                        .color(Color32::from_gray(24))
                        .strong(),
                )
                .fill(Color32::from_gray(134)),
            );
            rects[2] = send.rect;
            super::icons::paint(
                ui,
                egui::Rect::from_center_size(
                    egui::pos2(send.rect.right() - 17.0, send.rect.center().y),
                    Vec2::splat(14.0),
                ),
                super::icons::Icon::Forward,
                Color32::from_gray(24),
            );
            if send
                .on_hover_text("Send to the current shared capture device")
                .clicked()
            {
                self.submit_editor_command();
            }
        });
        rects
    }

    fn submit_editor_command(&mut self) {
        let command = self.command_input.clone();
        if self.send_command(&command) && self.auto_clear_command {
            self.command_input.clear();
        }
    }
}

struct CommandEditor {
    response: egui::Response,
    frame: egui::Rect,
    #[cfg(test)]
    prompt: egui::Rect,
}

fn command_editor(ui: &mut egui::Ui, text: &mut String, width: f32) -> CommandEditor {
    let mut prompt = egui::Rect::NOTHING;
    let editor = egui::Frame::none()
        .fill(Color32::from_gray(10))
        .stroke(Stroke::new(1.0_f32, theme::BORDER))
        .rounding(4.0)
        .inner_margin(egui::Margin::symmetric(10.0, 9.0))
        .show(ui, |ui| {
            ui.set_width(width - 20.0);
            ui.horizontal(|ui| {
                (prompt, _) = ui.allocate_exact_size(Vec2::new(14.0, 20.0), egui::Sense::hover());
                ui.painter().text(
                    prompt.center(),
                    egui::Align2::CENTER_CENTER,
                    ">",
                    FontId::monospace(14.0),
                    theme::TEXT_MUTED,
                );
                ui.add_sized(
                    [width - 42.0, 20.0],
                    egui::TextEdit::singleline(text)
                        .font(theme::command_font(14.0))
                        .margin(egui::Margin::ZERO)
                        .hint_text(
                            RichText::new("Enter command...")
                                .font(theme::command_font(14.0))
                                .color(Color32::from_gray(102)),
                        )
                        .frame(false),
                )
            })
            .inner
        });
    if editor.inner.has_focus() {
        ui.painter().rect_stroke(
            editor.response.rect,
            4.0,
            Stroke::new(1.0_f32, theme::TEXT_DIM),
        );
    }
    CommandEditor {
        response: editor.inner,
        frame: editor.response.rect,
        #[cfg(test)]
        prompt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> TraceXApp {
        TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())))
    }

    #[test]
    fn recalling_history_only_populates_editor_preserving_capture() {
        let mut app = app();
        app.logs.push(Arc::new(LogEntry::new(
            7,
            "captured".to_owned(),
            "12:00:00",
        )));
        app.selected.insert(7);
        app.recall_command("AT+STATUS");
        assert_eq!(app.command_input, "AT+STATUS");
        assert!(app.command_history.is_empty());
        assert!(
            app.operation_error.is_none(),
            "recall must not attempt disconnected send"
        );
        assert_eq!(app.logs.len(), 1);
        assert!(app.selected.contains(&7));
    }

    #[test]
    fn prompt_and_hint_have_separate_non_overlapping_layout_regions() {
        for text in ["", "AT+STATUS"] {
            let mut text = text.to_owned();
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let editor = command_editor(ui, &mut text, 300.0);
                    assert!(editor.prompt.right() + 4.0 <= editor.response.rect.left());
                    assert!(editor.frame.contains_rect(editor.prompt));
                    assert!(editor.frame.contains_rect(editor.response.rect));
                });
            });
            assert!(!output.shapes.is_empty());
        }
    }

    #[test]
    fn failed_editor_send_retains_text() {
        let mut app = app();
        app.command_input = "AT+STATUS".to_owned();
        app.auto_clear_command = true;
        app.submit_editor_command();
        assert_eq!(app.command_input, "AT+STATUS");
    }

    #[test]
    fn composer_controls_fit_minimum_and_regular_client_widths() {
        for width in [800.0, 1200.0] {
            let mut app = app();
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 600.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::SidePanel::left("sidebar")
                        .exact_width(256.0)
                        .show(ctx, |_| {});
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let available = ui.available_rect_before_wrap();
                        let rects = app.render_command_controls(ui);
                        for rect in rects {
                            assert!(
                                available.contains_rect(rect),
                                "control {rect:?} exceeds {available:?} at width {width}"
                            );
                        }
                        assert!(rects[0].right() <= rects[1].left());
                        assert!(rects[1].right() <= rects[2].left());
                    });
                },
            );
        }
    }
}
