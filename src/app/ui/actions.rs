use super::super::*;
use super::file_dialog::{FileDialogKind, FileDialogs};
use std::path::PathBuf;

#[derive(Clone)]
pub(super) struct SessionUi {
    pub id: SessionId,
    pub title: String,
    pub identity: String,
    pub target: Option<CommandTarget>,
    pub can_connect: bool,
    pub can_edit: bool,
    pub can_save: bool,
    pub can_open: bool,
    pub can_export: bool,
    pub can_clear: bool,
}

impl SessionUi {
    #[cfg(test)]
    pub fn test(session: &SessionRuntime) -> Self {
        Self {
            id: session.id,
            title: session.title.clone(),
            identity: format!("{} · disconnected", session.title),
            target: None,
            can_connect: false,
            can_edit: true,
            can_save: true,
            can_open: true,
            can_export: true,
            can_clear: true,
        }
    }

    pub fn snapshot(workspace: &Workspace, id: SessionId) -> Option<Self> {
        let session = workspace.session(id)?;
        let can_edit = !workspace.window_closing() && !session.shutdown.active();
        let identity = if session.connected {
            format!(
                "Send to {} · {}",
                session.title,
                session
                    .occupied_endpoint
                    .as_deref()
                    .unwrap_or("connected device")
            )
        } else if session.capture.is_some() {
            format!("{} · capture draining", session.title)
        } else if session.source_path.is_some() {
            format!("{} · saved session · disconnected", session.title)
        } else {
            format!("{} · disconnected", session.title)
        };
        Some(Self {
            id,
            title: session.title.clone(),
            identity,
            target: workspace.command_target(id),
            can_connect: workspace.can_connect(id),
            can_edit,
            can_save: can_edit && session.can_save_session(),
            can_open: can_edit && session.can_open_session(),
            can_export: can_edit && !session.connected && !session.session_archive_busy(),
            can_clear: can_edit && !session.session_archive_busy(),
        })
    }
}

#[derive(Clone, Copy)]
pub(super) enum FileAction {
    Save,
    Open,
    Export,
    Recovery,
    AutosavePath,
}

pub(super) enum UiIntent {
    DismissWorkspaceError,
    Create,
    Select(SessionId),
    Connect(SessionId),
    Disconnect(SessionId),
    Submit(CommandTarget, String),
    Clear(SessionId),
    CloseSession(SessionId),
    CloseWindow,
    KeepSession(CloseTarget),
    DiscardSession(CloseTarget),
    KeepWindow,
    ForceWindow(CloseTarget),
    File(SessionId, FileAction),
    FilePath(SessionId, FileAction, PathBuf),
    AutosaveEnabled(SessionId, bool),
    AutosaveLimit(SessionId, u64),
}

pub(super) fn widget_id(id: SessionId, role: &str) -> egui::Id {
    egui::Id::new(("session", id, role))
}

pub(super) fn dispatch(
    workspace: &mut Workspace,
    dialogs: &mut FileDialogs,
    intents: Vec<UiIntent>,
) {
    for intent in intents {
        let origin = match &intent {
            UiIntent::Select(id)
            | UiIntent::Connect(id)
            | UiIntent::Disconnect(id)
            | UiIntent::Clear(id)
            | UiIntent::CloseSession(id)
            | UiIntent::File(id, _)
            | UiIntent::FilePath(id, _, _)
            | UiIntent::AutosaveEnabled(id, _)
            | UiIntent::AutosaveLimit(id, _) => Some(*id),
            UiIntent::Submit(target, _) => Some(target.session),
            UiIntent::KeepSession(target)
            | UiIntent::DiscardSession(target)
            | UiIntent::ForceWindow(target) => Some(target.session),
            _ => None,
        }
        .and_then(|id| workspace.session(id).map(|session| session.title.clone()));
        let result = match intent {
            UiIntent::DismissWorkspaceError => {
                workspace.operation_error = None;
                Ok(())
            }
            UiIntent::Create => workspace.create_session().map(|_| ()),
            UiIntent::Select(id) => workspace.select_session(id),
            UiIntent::Connect(id) => workspace.connect(id),
            UiIntent::Disconnect(id) => workspace.disconnect(id),
            UiIntent::Submit(target, text) => workspace.submit_command(target, &text),
            UiIntent::Clear(id) => workspace.clear_session(id),
            UiIntent::CloseSession(id) => workspace
                .request_close_session(id, Instant::now())
                .map(|_| ()),
            UiIntent::CloseWindow => workspace.request_window_close(Instant::now()),
            UiIntent::KeepSession(target) => workspace.keep_session_open(target),
            UiIntent::DiscardSession(target) => workspace.close_without_saving_session(target),
            UiIntent::KeepWindow => {
                workspace.keep_window_open();
                Ok(())
            }
            UiIntent::ForceWindow(target) => workspace.force_close_window(target),
            UiIntent::File(id, kind) => {
                start_file(workspace, dialogs, id, kind);
                Ok(())
            }
            UiIntent::FilePath(id, kind, path) => dispatch_path(workspace, id, kind, path),
            UiIntent::AutosaveEnabled(id, value) => workspace.set_auto_save_enabled(id, value),
            UiIntent::AutosaveLimit(id, value) => workspace.set_auto_save_limit_mb(id, value),
        };
        if let Err(error) = result {
            workspace.operation_error = Some(origin.map_or_else(
                || error.to_string(),
                |owner| format!("Action for {owner}: {error}"),
            ));
        }
    }
}

fn start_file(
    workspace: &mut Workspace,
    dialogs: &mut FileDialogs,
    id: SessionId,
    kind: FileAction,
) {
    let Some(meta) = SessionUi::snapshot(workspace, id) else {
        workspace.operation_error = Some("The file picker's session no longer exists.".to_owned());
        return;
    };
    let allowed = match kind {
        FileAction::Save => meta.can_save,
        FileAction::Open => meta.can_open,
        FileAction::Export => meta.can_export,
        FileAction::Recovery => {
            meta.can_edit
                && workspace
                    .session(id)
                    .is_some_and(|s| s.unsaved_payload_count() > 0 && !s.recovery_exporting())
        }
        FileAction::AutosavePath => {
            meta.can_edit
                && workspace
                    .session(id)
                    .is_some_and(|s| !s.auto_save_pending() && !s.auto_save.stopping)
        }
    };
    if !allowed {
        workspace.operation_error = Some(format!(
            "{} cannot start this file operation; review its capture and close state.",
            meta.title
        ));
        return;
    }
    let picker = match kind {
        FileAction::Save => FileDialogKind::SaveSession,
        FileAction::Open => FileDialogKind::OpenSession,
        FileAction::Export => FileDialogKind::ExportText,
        FileAction::Recovery => FileDialogKind::RecoveryExport,
        FileAction::AutosavePath => FileDialogKind::AutosavePath,
    };
    let name = workspace
        .session(id)
        .and_then(|s| s.auto_save_path.as_ref())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::to_owned);
    if let Err(error) = dialogs.start(id, picker, name) {
        workspace.operation_error = Some(format!("File picker for {}: {error}", meta.title));
    }
}

pub(super) fn poll_dialogs(
    workspace: &mut Workspace,
    dialogs: &mut FileDialogs,
    ctx: &egui::Context,
) {
    let Some(outcome) = dialogs.poll(ctx) else {
        return;
    };
    let Some(path) = outcome.path else {
        return;
    };
    let kind = match outcome.kind {
        FileDialogKind::SaveSession => FileAction::Save,
        FileDialogKind::OpenSession => FileAction::Open,
        FileDialogKind::ExportText => FileAction::Export,
        FileDialogKind::RecoveryExport => FileAction::Recovery,
        FileDialogKind::AutosavePath => FileAction::AutosavePath,
    };
    dispatch(
        workspace,
        dialogs,
        vec![UiIntent::FilePath(outcome.session, kind, path)],
    );
}

fn dispatch_path(
    workspace: &mut Workspace,
    id: SessionId,
    kind: FileAction,
    path: PathBuf,
) -> Result<(), WorkspaceError> {
    match kind {
        FileAction::Save => workspace.save_session_to(id, path),
        FileAction::Open => workspace.open_session_from(id, path),
        FileAction::Export => workspace.export_logs_to(id, path),
        FileAction::Recovery => workspace.export_recovery_to(id, path),
        FileAction::AutosavePath => workspace.set_auto_save_path(id, path),
    }
}
