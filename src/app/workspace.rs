//! Stable routing and bounded ownership for independent native sessions.
use super::session::SessionRuntime;
use std::{fmt, path::PathBuf, time::Instant};

const MAX_RETAINED_SESSIONS: usize = 2;
pub(super) const FRAME_RECORD_BUDGET: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct SessionId(pub(super) u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct ConnectionId(pub(super) u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct CloseRequestId(pub(super) u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CommandTarget {
    pub session: SessionId,
    pub connection: ConnectionId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CloseTarget {
    pub session: SessionId,
    pub request: CloseRequestId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ClosePromptKind {
    Progress,
    UnsavedSession,
    Incomplete(String),
}
pub(super) struct ClosePrompt {
    pub target: CloseTarget,
    pub title: String,
    pub kind: ClosePromptKind,
    pub window: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum WorkspaceError {
    UnknownSession,
    SessionLimit,
    SessionIdExhausted,
    CloseRequestExhausted,
    WindowClosing,
    SessionClosing,
    EndpointInUse(SessionId),
    StaleCommandTarget,
    StaleCloseTarget,
    OperationRejected,
}
impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSession => f.write_str("The addressed session no longer exists."),
            Self::SessionLimit => f.write_str("Session limit reached. Close a finished session before adding another."),
            Self::SessionIdExhausted => f.write_str("Session identifiers exhausted. Existing evidence remains retained; save and restart TraceX."),
            Self::CloseRequestExhausted => f.write_str("Close request identifiers exhausted. Existing sessions remain open."),
            Self::WindowClosing => f.write_str("Keep the application open before starting another operation."),
            Self::SessionClosing => f.write_str("Keep this session open before starting another operation."),
            Self::EndpointInUse(owner) => write!(f, "This endpoint is still owned by session {}. Disconnect and finish its capture first.", owner.0),
            Self::StaleCommandTarget => f.write_str("The command destination changed. Review the current connection and send again."),
            Self::StaleCloseTarget => f.write_str("This close decision is no longer current."),
            Self::OperationRejected => f.write_str("The addressed session could not accept the operation; review its status."),
        }
    }
}

/// The provisional retained-owner cap also bounds independent worker counts.
pub(super) struct Workspace {
    sessions: Vec<SessionRuntime>,
    active: SessionId,
    next_session_id: Option<u64>,
    max_sessions: usize,
    poll_cursor: usize,
    close_requests: Vec<CloseTarget>,
    next_close_request: Option<u64>,
    window_closing: bool,
    force_window_close: bool,
    pub(super) operation_error: Option<String>,
}

impl Workspace {
    pub(super) fn new() -> Self {
        Self {
            sessions: vec![SessionRuntime::new()],
            active: SessionId(1),
            next_session_id: Some(2),
            max_sessions: MAX_RETAINED_SESSIONS,
            poll_cursor: 0,
            close_requests: Vec::new(),
            next_close_request: Some(1),
            window_closing: false,
            force_window_close: false,
            operation_error: None,
        }
    }
    pub(super) fn sessions(&self) -> &[SessionRuntime] {
        &self.sessions
    }
    pub(super) fn active_session_id(&self) -> SessionId {
        self.active
    }
    pub(super) fn session(&self, id: SessionId) -> Option<&SessionRuntime> {
        self.sessions.iter().find(|session| session.id == id)
    }
    pub(super) fn session_mut(&mut self, id: SessionId) -> Option<&mut SessionRuntime> {
        self.sessions.iter_mut().find(|session| session.id == id)
    }
    pub(super) fn active_session(&self) -> Option<&SessionRuntime> {
        self.session(self.active)
    }
    pub(super) fn active_session_mut(&mut self) -> Option<&mut SessionRuntime> {
        self.session_mut(self.active)
    }
    fn reject<T>(
        &mut self,
        id: Option<SessionId>,
        error: WorkspaceError,
    ) -> Result<T, WorkspaceError> {
        let message = error.to_string();
        self.operation_error = Some(message.clone());
        if let Some(session) = id.and_then(|id| self.session_mut(id)) {
            session.operation_error = Some(message);
        }
        Err(error)
    }
    fn admitted(&mut self, id: SessionId) -> Result<&mut SessionRuntime, WorkspaceError> {
        let error = if self.window_closing {
            Some(WorkspaceError::WindowClosing)
        } else {
            match self.session(id) {
                None => Some(WorkspaceError::UnknownSession),
                Some(session) if session.shutdown.active() => Some(WorkspaceError::SessionClosing),
                _ => None,
            }
        };
        if let Some(error) = error {
            return self.reject(Some(id), error);
        }
        // Resolve again without an unchecked index or an active-session fallback.
        self.session_mut(id).ok_or(WorkspaceError::UnknownSession)
    }
    fn reserve_session_id(&mut self) -> Result<SessionId, WorkspaceError> {
        let Some(id) = self.next_session_id else {
            return self.reject(None, WorkspaceError::SessionIdExhausted);
        };
        self.next_session_id = id.checked_add(1);
        Ok(SessionId(id))
    }
    fn fresh_session(id: SessionId) -> SessionRuntime {
        let mut session = SessionRuntime::new();
        session.id = id;
        session.title = format!("Monitor {}", id.0);
        session
    }
    pub(super) fn can_create_session(&self) -> bool {
        !self.window_closing
            && self.sessions.len() < self.max_sessions
            && self.next_session_id.is_some()
    }
    pub(super) fn create_session(&mut self) -> Result<SessionId, WorkspaceError> {
        if self.window_closing {
            return self.reject(None, WorkspaceError::WindowClosing);
        }
        if self.sessions.len() >= self.max_sessions {
            return self.reject(None, WorkspaceError::SessionLimit);
        }
        let id = self.reserve_session_id()?;
        self.sessions.push(Self::fresh_session(id));
        self.active = id;
        self.operation_error = None;
        Ok(id)
    }
    pub(super) fn select_session(&mut self, id: SessionId) -> Result<(), WorkspaceError> {
        if self.session(id).is_none() {
            return self.reject(None, WorkspaceError::UnknownSession);
        }
        self.active = id;
        Ok(())
    }
    fn endpoint_owner(&self, id: SessionId) -> Option<SessionId> {
        let endpoint = &self.session(id)?.selected_port;
        self.sessions
            .iter()
            .find(|other| {
                other.id != id
                    && other
                        .occupied_endpoint
                        .as_ref()
                        .is_some_and(|occupied| occupied.eq_ignore_ascii_case(endpoint))
            })
            .map(|owner| owner.id)
    }
    pub(super) fn can_connect(&self, id: SessionId) -> bool {
        !self.window_closing
            && self.endpoint_owner(id).is_none()
            && self.session(id).is_some_and(|s| {
                !s.shutdown.active()
                    && !s.selected_port.is_empty()
                    && s.capture.is_none()
                    && !s.auto_save.stopping
                    && !s.session_archive_busy()
                    && !s.history_limit_reached
                    && s.next_connection_id.is_some()
            })
    }
    fn connect_using(
        &mut self,
        id: SessionId,
        open: impl FnOnce(&mut SessionRuntime) -> bool,
    ) -> Result<(), WorkspaceError> {
        self.admitted(id)?;
        if let Some(owner) = self.endpoint_owner(id) {
            return self.reject(Some(id), WorkspaceError::EndpointInUse(owner));
        }
        let session = self.admitted(id)?;
        if !session.connection_ready() {
            return Err(WorkspaceError::OperationRejected);
        }
        if !open(session) {
            return Err(WorkspaceError::OperationRejected);
        }
        Ok(())
    }
    pub(super) fn connect(&mut self, id: SessionId) -> Result<(), WorkspaceError> {
        self.connect_using(id, SessionRuntime::connect)
    }
    pub(super) fn disconnect(&mut self, id: SessionId) -> Result<(), WorkspaceError> {
        self.admitted(id)?.disconnect();
        Ok(())
    }
    pub(super) fn command_target(&self, id: SessionId) -> Option<CommandTarget> {
        if self.window_closing {
            return None;
        }
        let session = self.session(id)?;
        if !session.connected || session.serial_tx.is_none() || session.shutdown.active() {
            return None;
        }
        Some(CommandTarget {
            session: id,
            connection: session.connection_id?,
        })
    }
    pub(super) fn submit_command(
        &mut self,
        target: CommandTarget,
        text: &str,
    ) -> Result<(), WorkspaceError> {
        if self.command_target(target.session) != Some(target) {
            if let Some(session) = self.session_mut(target.session) {
                session.command_input = text.to_owned();
            }
            return self.reject(Some(target.session), WorkspaceError::StaleCommandTarget);
        }
        let session = self.admitted(target.session)?;
        if !session.send_command(text) {
            return Err(WorkspaceError::OperationRejected);
        }
        if session.auto_clear_command {
            session.command_input.clear();
        }
        Ok(())
    }
    pub(super) fn clear_session(&mut self, id: SessionId) -> Result<(), WorkspaceError> {
        let session = self.admitted(id)?;
        let busy = session.session_archive_busy();
        session.clear();
        if busy {
            return Err(WorkspaceError::OperationRejected);
        }
        Ok(())
    }
    pub(super) fn save_session_to(
        &mut self,
        id: SessionId,
        path: PathBuf,
    ) -> Result<(), WorkspaceError> {
        let session = self.admitted(id)?;
        if !session.save_session_ready() {
            return Err(WorkspaceError::OperationRejected);
        }
        session.save_session_to(path);
        if session.session_archive_busy() {
            Ok(())
        } else {
            Err(WorkspaceError::OperationRejected)
        }
    }
    pub(super) fn open_session_from(
        &mut self,
        id: SessionId,
        path: PathBuf,
    ) -> Result<(), WorkspaceError> {
        let session = self.admitted(id)?;
        if !session.open_session_ready() {
            return Err(WorkspaceError::OperationRejected);
        }
        session.open_session_from(path);
        if session.session_archive_busy() {
            Ok(())
        } else {
            Err(WorkspaceError::OperationRejected)
        }
    }
    pub(super) fn export_logs_to(
        &mut self,
        id: SessionId,
        path: PathBuf,
    ) -> Result<(), WorkspaceError> {
        let session = self.admitted(id)?;
        if !session.text_export_ready() {
            return Err(WorkspaceError::OperationRejected);
        }
        session.export_logs_to(path);
        if session.session_archive_busy() {
            Ok(())
        } else {
            Err(WorkspaceError::OperationRejected)
        }
    }
    pub(super) fn export_recovery_to(
        &mut self,
        id: SessionId,
        path: PathBuf,
    ) -> Result<(), WorkspaceError> {
        let session = self.admitted(id)?;
        if session.recovery_exporting() || session.unsaved_payload_count() == 0 {
            session.operation_error = Some("Wait for recovery export to finish, or capture unsaved payloads before exporting recovery.".into());
            return Err(WorkspaceError::OperationRejected);
        }
        session.export_recovery_to(path);
        if session.recovery_exporting() {
            Ok(())
        } else {
            Err(WorkspaceError::OperationRejected)
        }
    }
    pub(super) fn set_auto_save_path(
        &mut self,
        id: SessionId,
        path: PathBuf,
    ) -> Result<(), WorkspaceError> {
        if self.admitted(id)?.set_auto_save_path(path) {
            Ok(())
        } else {
            Err(WorkspaceError::OperationRejected)
        }
    }
    pub(super) fn set_auto_save_limit_mb(
        &mut self,
        id: SessionId,
        value: u64,
    ) -> Result<(), WorkspaceError> {
        if self.admitted(id)?.set_auto_save_limit_mb(value) {
            Ok(())
        } else {
            Err(WorkspaceError::OperationRejected)
        }
    }
    pub(super) fn set_auto_save_enabled(
        &mut self,
        id: SessionId,
        enabled: bool,
    ) -> Result<(), WorkspaceError> {
        let session = self.admitted(id)?;
        session.auto_save_enabled = enabled;
        session.apply_auto_save_enabled_change();
        if session.auto_save_enabled == enabled {
            Ok(())
        } else {
            Err(WorkspaceError::OperationRejected)
        }
    }
    /// Poll every retained owner within one frame-wide record budget.
    pub(super) fn poll(&mut self, now: Instant, ports: &[(String, String)]) -> usize {
        let count = self.sessions.len();
        let mut admitted = 0;
        if count == 0 {
            return 0;
        }
        let start = self.poll_cursor % count;
        let share = FRAME_RECORD_BUDGET / count;
        for offset in 0..count {
            let index = (start + offset) % count;
            admitted += self.sessions[index].poll_with_budget(share);
            self.sessions[index].poll_query_completion();
            if self.sessions[index].shutdown.active() {
                self.sessions[index].advance_shutdown(now);
            }
        }
        // One bounded second pass redistributes unused shares. An idle archive
        // must not halve the other session's existing ingestion allowance.
        for offset in 0..count {
            if admitted == FRAME_RECORD_BUDGET {
                break;
            }
            let index = (start + offset) % count;
            admitted += self.sessions[index].poll_with_budget(FRAME_RECORD_BUDGET - admitted);
            self.sessions[index].poll_query_completion();
            if self.sessions[index].shutdown.active() {
                self.sessions[index].advance_shutdown(now);
            }
        }
        self.poll_cursor = (start + 1) % count;
        if !self.window_closing {
            let reconnect: Vec<_> = self
                .sessions
                .iter()
                .filter(|s| s.reconnect_eligible(now, ports))
                .map(|s| s.id)
                .collect();
            for id in reconnect {
                let _ = self.connect(id);
            }
            self.reap_closed_sessions();
        }
        if let Some(session) = self.active_session_mut() {
            session.refresh_query_view();
        }
        admitted
    }
}

#[path = "workspace_close.rs"]
mod close;
#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
