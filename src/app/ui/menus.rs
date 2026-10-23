use super::super::*;

pub(super) fn render_session_actions(session: &mut SessionRuntime, ui: &mut egui::Ui) {
    if ui.add_enabled(session.can_open_session(), egui::Button::new("Open Session…"))
        .on_disabled_hover_text("Disconnect and finish capture, then save and clear retained history before opening another session.")
        .on_hover_text("Validate and reopen a saved TraceX session").clicked() {
        ui.close_menu();
        super::session_actions::choose_open_session(session);
    }
    if ui
        .add_enabled(
            session.can_save_session(),
            egui::Button::new("Save Session…"),
        )
        .on_disabled_hover_text(
            "Stop capture and wait for other session file operations to finish.",
        )
        .on_hover_text(
            "Save all currently retained records; queued records are outside this snapshot.",
        )
        .clicked()
    {
        ui.close_menu();
        super::session_actions::choose_save_session(session);
    }
}

pub(super) fn render_app_menus(
    session: &mut SessionRuntime,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
) {
    ui.menu_button("File", |ui| {
        ui.add_enabled_ui(
            !session.shutdown.active() && !session.session_archive_busy(),
            |ui| {
                render_session_actions(session, ui);
                if ui.button("Export filtered text…").clicked() {
                    ui.close_menu();
                    super::session_actions::choose_export_logs(session);
                }
                ui.separator();
                if ui.button("Clear retained logs").clicked() {
                    session.clear();
                    ui.close_menu();
                }
            },
        );
        ui.separator();
        if ui.button("Exit").clicked() {
            session.request_shutdown(Instant::now());
            ui.close_menu();
        }
    });
    ui.menu_button("Edit", |ui| {
        ui.add_enabled_ui(!session.session_archive_busy(), |ui| {
            if ui.button("Copy selected / filtered logs").clicked() {
                ctx.copy_text(session.copy_selected_text());
                ui.close_menu();
            }
            if ui.button("Copy all retained logs").clicked() {
                ctx.copy_text(session.copy_all_text());
                ui.close_menu();
            }
        });
    });
    ui.menu_button("View", |ui| {
        ui.checkbox(&mut session.filter_panel_open, "Search results panel");
        if ui
            .checkbox(&mut session.auto_scroll, "Follow incoming logs")
            .changed()
        {
            session.live_scroll_frames = live_scroll_frames_after_toggle(true, session.auto_scroll);
        }
        ui.separator();
        ui.checkbox(&mut session.show_info, "INFO records");
        ui.checkbox(&mut session.show_warn, "WARN records");
        ui.checkbox(&mut session.show_error, "ERROR records");
    });
    ui.menu_button("Help", |ui| {
        ui.set_max_width(300.0);
        ui.label(format!("TraceX {} · development preview", env!("CARGO_PKG_VERSION")));
        ui.label("Select a port and baud rate, then connect. Disconnect before saving or reopening a session.");
        ui.label("Save Session preserves all retained records. Text export follows the current search and severity filters.");
        ui.label("Monitor tabs currently share one device capture.");
        ui.separator();
        ui.menu_button("Third-party font licenses", |ui| {
            ui.set_width(480.0);
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for (name, license) in [
                    ("Inter 4.1", include_str!("../../../assets/fonts/inter/LICENSE.txt")),
                    ("Liberation Fonts 2.1.5", include_str!("../../../assets/fonts/liberation/LICENSE")),
                ] {
                    ui.heading(name);
                    ui.add(egui::Label::new(license).wrap());
                    ui.separator();
                }
            });
        });
    });
}
