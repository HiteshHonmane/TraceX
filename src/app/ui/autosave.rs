use super::super::*;

pub(super) fn render_auto_save_status(session: &mut SessionRuntime, ui: &mut egui::Ui) {
    if let Some(error) = &session.auto_save_error {
        ui.label(
            RichText::new(error)
                .monospace()
                .size(8.5)
                .color(theme::ERROR),
        );
    } else if session.auto_save_pending() {
        ui.label(
            RichText::new("Autosave queue full; capture waits for storage.")
                .monospace()
                .size(8.5)
                .color(theme::WARNING),
        );
    } else if session.auto_save.stopping {
        ui.label(
            RichText::new("Finalizing saved logs...")
                .monospace()
                .size(8.5)
                .color(theme::TEXT_MUTED),
        );
    } else if let Some(path) = &session.auto_save_current_path {
        let label = if session.auto_save_enabled && session.auto_save.active {
            "Saving to"
        } else {
            "Last file"
        };
        ui.label(
            RichText::new(format!("{label}: {}", path.display()))
                .monospace()
                .size(8.5)
                .color(theme::TEXT_MUTED),
        );
    } else if session.auto_save.active {
        ui.label(
            RichText::new("Opening log file...")
                .monospace()
                .size(8.5)
                .color(theme::TEXT_MUTED),
        );
    }
    render_recovery_export(session, ui);
}

fn render_recovery_export(session: &mut SessionRuntime, ui: &mut egui::Ui) {
    let count = session.unsaved_payload_count();
    if count == 0 {
        return;
    }
    ui.label(format!("{count} unsaved payloads retained"));
    if ui
        .add_enabled(
            !session.recovery_exporting(),
            egui::Button::new("Export unsaved payloads"),
        )
        .clicked()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("Recovery JSON", &["json"])
            .set_file_name("TraceX-recovery.json")
            .save_file()
    {
        session.export_recovery_to(path);
    }
}

pub(super) fn render_settings(session: &mut SessionRuntime, ui: &mut egui::Ui) {
    ui.label(
        RichText::new("LOG FILES")
            .size(10.0)
            .color(theme::ACCENT)
            .strong(),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("File limit");
        let mut limit = session.auto_save_limit_mb;
        if ui
            .add(
                egui::DragValue::new(&mut limit)
                    .range(1..=4096)
                    .suffix(" MB"),
            )
            .changed()
        {
            session.set_auto_save_limit_mb(limit);
        }
    });
    ui.add_space(4.0);
    if ui.button("Choose save location").clicked() {
        choose_save_path(session);
    }
}

fn choose_save_path(session: &mut SessionRuntime) {
    if !session.auto_save_path_ready() {
        return;
    }
    let default_name = session
        .auto_save_path
        .as_ref()
        .and_then(|path| path.file_name().and_then(|name| name.to_str()))
        .unwrap_or("TraceX.log");
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Log", &["log", "txt"])
        .set_file_name(default_name)
        .save_file()
    {
        session.set_auto_save_path(path);
    }
}
