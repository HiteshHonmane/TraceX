use super::super::*;
use super::actions::{FileAction, SessionUi, UiIntent};

pub(super) fn render_session_actions(
    _session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ui: &mut egui::Ui,
) {
    if ui.add_enabled(meta.can_open, egui::Button::new("Open Session…"))
        .on_disabled_hover_text("Disconnect and finish capture, then save and clear retained history before opening another session.")
        .on_hover_text(format!("Validate and reopen a saved TraceX session into {}", meta.title)).clicked() {
        ui.close_menu();
        intents.push(UiIntent::File(meta.id, FileAction::Open));
    }
    if ui
        .add_enabled(meta.can_save, egui::Button::new("Save Session…"))
        .on_disabled_hover_text(
            "Stop capture and wait for other session file operations to finish.",
        )
        .on_hover_text(
            "Save all currently retained records; queued records are outside this snapshot.",
        )
        .clicked()
    {
        ui.close_menu();
        intents.push(UiIntent::File(meta.id, FileAction::Save));
    }
}

pub(super) fn render_app_menus(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
) {
    ui.menu_button("File", |ui| {
        ui.add_enabled_ui(meta.can_edit && !session.session_archive_busy(), |ui| {
            render_session_actions(session, meta, intents, ui);
            if ui
                .add_enabled(meta.can_export, egui::Button::new("Export filtered text…"))
                .clicked()
            {
                ui.close_menu();
                intents.push(UiIntent::File(meta.id, FileAction::Export));
            }
            ui.separator();
            if ui
                .add_enabled(meta.can_clear, egui::Button::new("Clear retained logs"))
                .clicked()
            {
                intents.push(UiIntent::Clear(meta.id));
                ui.close_menu();
            }
        });
        ui.separator();
        if ui.button("Exit").clicked() {
            intents.push(UiIntent::CloseWindow);
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
        ui.label("Monitor tabs own independent captures. Commands name their session and connection.");
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
