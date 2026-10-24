use super::super::*;
use super::actions::{FileAction, SessionUi, UiIntent};

pub(super) fn render_auto_save_status(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ui: &mut egui::Ui,
) {
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
    render_recovery_export(session, meta, intents, ui);
}

fn render_recovery_export(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ui: &mut egui::Ui,
) {
    let count = session.unsaved_payload_count();
    if count == 0 {
        return;
    }
    ui.label(format!("{count} unsaved payloads retained"));
    if ui
        .add_enabled(
            meta.can_edit && !session.recovery_exporting(),
            egui::Button::new("Export unsaved payloads"),
        )
        .clicked()
    {
        intents.push(UiIntent::File(meta.id, FileAction::Recovery));
    }
}

pub(super) fn render_settings(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ui: &mut egui::Ui,
) {
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
            intents.push(UiIntent::AutosaveLimit(meta.id, limit));
        }
    });
    ui.add_space(4.0);
    if ui.button("Choose save location").clicked() {
        choose_save_path(session, meta, intents);
    }
}

fn choose_save_path(_session: &mut SessionRuntime, meta: &SessionUi, intents: &mut Vec<UiIntent>) {
    intents.push(UiIntent::File(meta.id, FileAction::AutosavePath));
}
