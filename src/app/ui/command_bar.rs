use super::super::*;
use super::actions::{SessionUi, UiIntent, widget_id};

pub(super) fn render_command_bar(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ctx: &egui::Context,
) {
    egui::TopBottomPanel::bottom(widget_id(meta.id, "command_rail")).exact_height(116.0)
        .frame(egui::Frame::none().fill(theme::BG_PANEL).inner_margin(egui::Margin::symmetric(16.0, 10.0)))
        .show(ctx, |ui| {
            let rail = ui.max_rect().expand2(Vec2::new(16.0, 10.0));
            ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(rail.left(), rail.top() + 44.0), rail.right_bottom()), 0.0, theme::BG_COMMAND);
            ui.painter().hline(rail.x_range(), rail.top(), Stroke::new(1.0_f32, theme::BORDER));
            ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 32.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.label(RichText::new("HISTORY:").size(10.0).color(theme::TEXT_MUTED));
                let target_width = (ui.available_width() * 0.48).clamp(160.0, 340.0);
                let history_width = (ui.available_width() - target_width - 12.0).max(40.0);
                ui.allocate_ui(Vec2::new(history_width, 28.0), |ui| {
                    egui::ScrollArea::horizontal().id_salt(widget_id(meta.id, "command_history_scroll")).auto_shrink([false, true]).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if session.command_history.is_empty() { ui.label(RichText::new("No command history").size(11.0).color(theme::TEXT_MUTED)); }
                            let mut recalled = None;
                            for command in session.command_history.iter().rev().take(8) {
                                if ui.add_enabled(meta.can_edit, egui::Button::new(RichText::new(command).monospace().size(10.0).color(theme::TEXT_DIM)).rounding(4.0)).on_hover_text("Recall into this session's editor; Send or Enter executes").clicked() { recalled = Some(command.clone()); }
                            }
                            if let Some(command) = recalled { recall_command(session, &command); }
                        });
                    });
                });
                ui.add_sized([target_width, 24.0], egui::Label::new(RichText::new(&meta.identity).size(11.0).color(theme::TEXT_DIM)).truncate()).on_hover_text(&meta.identity);
            });
            ui.add_space(8.0);
            render_command_controls(session, meta, intents, ui);
        });
}

fn recall_command(session: &mut SessionRuntime, command: &str) {
    session.command_input = command.to_owned();
}

// Return the actual widget rectangles so headless layout tests can guard
// containment at the supported minimum window size.
fn render_command_controls(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ui: &mut egui::Ui,
) -> [egui::Rect; 3] {
    let mut rects = [egui::Rect::NOTHING; 3];
    ui.horizontal(|ui| {
        let editor_width = (ui.available_width() - 190.0).max(80.0);
        let editor = ui
            .add_enabled_ui(meta.can_edit, |ui| {
                command_editor(
                    ui,
                    &mut session.command_input,
                    editor_width,
                    widget_id(meta.id, "command_editor"),
                )
            })
            .inner;
        rects[0] = editor.frame;
        if editor.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit_editor_command(session, meta, intents);
        }
        let ending = ui.add_sized(
            [74.0, 32.0],
            egui::Button::new(
                RichText::new(if session.send_newline {
                    "NL & CR"
                } else {
                    "NONE"
                })
                .size(11.0)
                .color(theme::TEXT_DIM),
            ),
        );
        rects[1] = ending.rect;
        if ending.on_hover_text("Toggle command line ending").clicked() {
            session.send_newline = !session.send_newline;
        }
        let send = ui
            .add_enabled_ui(meta.target.is_some(), |ui| {
                ui.add_sized(
                    [96.0, 32.0],
                    egui::Button::new(
                        RichText::new("SEND    ")
                            .size(12.0)
                            .color(Color32::from_gray(24))
                            .strong(),
                    )
                    .fill(Color32::from_gray(134)),
                )
            })
            .inner;
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
            .on_hover_text("Send to the displayed session and connection")
            .clicked()
        {
            submit_editor_command(session, meta, intents);
        }
    });
    rects
}

fn submit_editor_command(session: &SessionRuntime, meta: &SessionUi, intents: &mut Vec<UiIntent>) {
    if let Some(target) = meta.target {
        intents.push(UiIntent::Submit(target, session.command_input.clone()));
    }
}

struct CommandEditor {
    response: egui::Response,
    frame: egui::Rect,
    #[cfg(test)]
    prompt: egui::Rect,
}

fn command_editor(ui: &mut egui::Ui, text: &mut String, width: f32, id: egui::Id) -> CommandEditor {
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
                        .id(id)
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

    fn app() -> SessionRuntime {
        SessionRuntime::new()
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
        recall_command(&mut app, "AT+STATUS");
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
    fn captured_send_intent_keeps_origin_and_rejects_old_connection_after_switch() {
        use std::sync::mpsc;
        let mut workspace = Workspace::new();
        let a = workspace.active_session_id();
        let b = workspace.create_session().unwrap();
        let (tx_a, rx_a) = mpsc::sync_channel(4);
        let (tx_b, rx_b) = mpsc::sync_channel(4);
        for (id, tx, endpoint) in [(a, tx_a, "test-A"), (b, tx_b, "test-B")] {
            let session = workspace.session_mut(id).unwrap();
            session.connected = true;
            session.serial_tx = Some(tx);
            session.connection_id = Some(ConnectionId(1));
            session.occupied_endpoint = Some(endpoint.to_owned());
            session.command_input = if id == a { "PING A" } else { "draft B" }.to_owned();
        }
        let meta_a = SessionUi::snapshot(&workspace, a).unwrap();
        let mut intents = Vec::new();
        submit_editor_command(workspace.session(a).unwrap(), &meta_a, &mut intents);
        workspace.select_session(b).unwrap();
        let mut dialogs = super::super::file_dialog::FileDialogs::default();
        super::super::actions::dispatch(&mut workspace, &mut dialogs, intents);
        assert_eq!(rx_a.try_recv().unwrap(), b"PING A\r\n");
        assert!(rx_b.try_recv().is_err());
        assert!(workspace.session(a).unwrap().command_input.is_empty());
        assert_eq!(workspace.session(b).unwrap().command_input, "draft B");
        let (tx_new, rx_new) = mpsc::sync_channel(4);
        let session = workspace.session_mut(a).unwrap();
        session.serial_tx = Some(tx_new);
        session.connection_id = Some(ConnectionId(2));
        session.command_input = "keep A".to_owned();
        let mut intents = Vec::new();
        submit_editor_command(workspace.session(a).unwrap(), &meta_a, &mut intents);
        super::super::actions::dispatch(&mut workspace, &mut dialogs, intents);
        assert!(rx_new.try_recv().is_err());
        assert!(rx_b.try_recv().is_err());
        assert_eq!(workspace.session(a).unwrap().command_input, "keep A");
    }

    #[test]
    fn prompt_and_hint_have_separate_non_overlapping_layout_regions() {
        for text in ["", "AT+STATUS"] {
            let mut text = text.to_owned();
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let editor = command_editor(ui, &mut text, 300.0, egui::Id::new("test_editor"));
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
        submit_editor_command(&app, &SessionUi::test(&app), &mut Vec::new());
        assert_eq!(app.command_input, "AT+STATUS");
    }

    #[test]
    fn composer_controls_fit_minimum_and_regular_client_widths() {
        for width in [800.0, 1200.0] {
            let mut app = app();
            let meta = SessionUi::test(&app);
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
                        let rects = render_command_controls(&mut app, &meta, &mut Vec::new(), ui);
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
