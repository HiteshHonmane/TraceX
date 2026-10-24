use super::super::*;
use super::actions::UiIntent;

pub(super) fn intercept_window_close(workspace: &mut Workspace, ctx: &egui::Context, now: Instant) {
    if ctx.input(|i| i.viewport().close_requested()) && !workspace.window_allows_close() {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        if let Err(error) = workspace.request_window_close(now) {
            workspace.operation_error = Some(error.to_string());
        }
    }
}

pub(super) fn render_close_dialog(
    workspace: &Workspace,
    ctx: &egui::Context,
    intents: &mut Vec<UiIntent>,
) {
    let Some(prompt) = workspace.close_prompt() else {
        return;
    };
    egui::Modal::new(egui::Id::new(("session_close", prompt.target.session, prompt.target.request))).show(ctx, |ui| {
        ui.set_max_width(460.0);
        match &prompt.kind {
            ClosePromptKind::Progress => {
                ui.heading(format!("Closing {}…", prompt.title));
                ui.label("Waiting for received records, autosave, session files and search work to finish.");
                ui.spinner();
            }
            ClosePromptKind::UnsavedSession => {
                ui.heading(format!("{} has not been saved as a TraceX session", prompt.title));
                ui.label("Retained session history has not been saved. Text autosave/export may exist. Keep open to save, or close without saving this session.");
            }
            ClosePromptKind::Incomplete(reason) => {
                ui.heading(format!("{} could not finish closing", prompt.title));
                ui.label(reason);
                ui.label("Keep open to review retained logs and recover pending evidence. Capture remains stopped.");
            }
        }
        ui.horizontal(|ui| {
            if ui.button(if prompt.window { "Keep application open" } else { "Keep session open" }).clicked() {
                intents.push(if prompt.window { UiIntent::KeepWindow } else { UiIntent::KeepSession(prompt.target) });
            }
            if matches!(prompt.kind, ClosePromptKind::UnsavedSession) && ui.button("Close without saving session").clicked() { intents.push(UiIntent::DiscardSession(prompt.target)); }
            if prompt.window && matches!(prompt.kind, ClosePromptKind::Incomplete(_)) && ui.button("Force application exit — pending logs may be lost").clicked() { intents.push(UiIntent::ForceWindow(prompt.target)); }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_unsaved_dialog_and_reply_stay_on_origin_after_selection_changes() {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut workspace = Workspace::new();
        let a = workspace.active_session_id();
        let b = workspace.create_session().unwrap();
        for id in [a, b] {
            let session = workspace.session_mut(id).unwrap();
            session.logs.push(Arc::new(LogEntry::new(
                0,
                format!("evidence {}", id.0),
                "12:00:00",
            )));
            session.history_dirty = true;
        }
        let title_a = workspace.session(a).unwrap().title.clone();
        let title_b = workspace.session(b).unwrap().title.clone();
        let target = workspace.request_close_session(a, Instant::now()).unwrap();
        workspace.poll(Instant::now(), &[]);
        workspace.select_session(b).unwrap();
        assert_eq!(workspace.close_prompt().unwrap().target, target);
        let mut intents = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            render_close_dialog(&workspace, ctx, &mut intents)
        });
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            render_close_dialog(&workspace, ctx, &mut intents)
        });
        let text: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            text.iter()
                .any(|text| text.contains(&title_a) && text.contains("not been saved"))
        );
        assert!(!text.iter().any(|text| text.contains(&title_b)));
        assert!(
            !text.iter().any(|text| text.contains("Force")),
            "individual close has no force-drop action"
        );
        let mut dialogs = super::super::file_dialog::FileDialogs::default();
        super::super::actions::dispatch(
            &mut workspace,
            &mut dialogs,
            vec![UiIntent::DiscardSession(target)],
        );
        workspace.poll(Instant::now(), &[]);
        assert!(workspace.session(a).is_none());
        assert_eq!(
            workspace.session(b).unwrap().logs[0].message,
            format!("evidence {}", b.0)
        );
        assert!(workspace.session(b).unwrap().history_dirty);
    }
}
