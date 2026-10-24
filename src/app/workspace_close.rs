//! Per-session close ownership and native-window aggregation. No UI or joins.
use super::*;

impl Workspace {
    pub(in crate::app) fn window_closing(&self) -> bool {
        self.window_closing
    }
    pub(in crate::app) fn window_allows_close(&self) -> bool {
        self.window_closing
            && (self.force_window_close || self.sessions.iter().all(|s| s.shutdown.allows_close()))
    }
    pub(in crate::app) fn close_target(&self, id: SessionId) -> Option<CloseTarget> {
        self.close_requests
            .iter()
            .copied()
            .find(|target| target.session == id)
    }
    fn next_close_target(&mut self, id: SessionId) -> Result<CloseTarget, WorkspaceError> {
        let Some(request) = self.next_close_request else {
            return self.reject(Some(id), WorkspaceError::CloseRequestExhausted);
        };
        self.next_close_request = request.checked_add(1);
        Ok(CloseTarget {
            session: id,
            request: CloseRequestId(request),
        })
    }
    pub(in crate::app) fn request_close_session(
        &mut self,
        id: SessionId,
        now: Instant,
    ) -> Result<CloseTarget, WorkspaceError> {
        if self.window_closing {
            return self.reject(Some(id), WorkspaceError::WindowClosing);
        }
        if self.session(id).is_none() {
            return self.reject(None, WorkspaceError::UnknownSession);
        }
        if let Some(target) = self.close_target(id) {
            return Ok(target);
        }
        if self.next_close_request.is_none() {
            return self.reject(Some(id), WorkspaceError::CloseRequestExhausted);
        }
        let target = self.next_close_target(id)?;
        self.close_requests.push(target);
        if let Some(session) = self.session_mut(id) {
            session.request_shutdown(now);
        }
        Ok(target)
    }
    pub(in crate::app) fn request_window_close(
        &mut self,
        now: Instant,
    ) -> Result<(), WorkspaceError> {
        if self.window_closing {
            return Ok(());
        }
        // Reserve every decision identity before changing any capture/lifecycle.
        let mut next = self.next_close_request;
        let mut targets = Vec::new();
        for session in &self.sessions {
            let Some(request) = next else {
                return self.reject(None, WorkspaceError::CloseRequestExhausted);
            };
            targets.push(CloseTarget {
                session: session.id,
                request: CloseRequestId(request),
            });
            next = request.checked_add(1);
        }
        self.next_close_request = next;
        self.close_requests = targets;
        self.window_closing = true;
        self.force_window_close = false;
        for session in &mut self.sessions {
            session.request_shutdown(now);
        }
        Ok(())
    }
    fn validate_close_target(&mut self, target: CloseTarget) -> Result<(), WorkspaceError> {
        if self.close_target(target.session) != Some(target) {
            return self.reject(Some(target.session), WorkspaceError::StaleCloseTarget);
        }
        Ok(())
    }
    pub(in crate::app) fn keep_session_open(
        &mut self,
        target: CloseTarget,
    ) -> Result<(), WorkspaceError> {
        self.validate_close_target(target)?;
        if self.window_closing {
            self.keep_window_open();
            return Ok(());
        }
        if let Some(session) = self.session_mut(target.session) {
            session.shutdown.keep_open();
        }
        self.close_requests.retain(|request| *request != target);
        Ok(())
    }
    pub(in crate::app) fn close_without_saving_session(
        &mut self,
        target: CloseTarget,
    ) -> Result<(), WorkspaceError> {
        self.validate_close_target(target)?;
        let Some(session) = self.session_mut(target.session) else {
            return self.reject(None, WorkspaceError::UnknownSession);
        };
        if !session.shutdown.unsaved_session_confirmation() {
            return Err(WorkspaceError::OperationRejected);
        }
        session.shutdown.close_without_saving_session();
        Ok(())
    }
    pub(in crate::app) fn keep_window_open(&mut self) {
        if !self.window_closing {
            return;
        }
        for session in &mut self.sessions {
            session.shutdown.keep_open();
        }
        self.close_requests.clear();
        self.window_closing = false;
        self.force_window_close = false;
    }
    pub(in crate::app) fn force_close_window(
        &mut self,
        target: CloseTarget,
    ) -> Result<(), WorkspaceError> {
        self.validate_close_target(target)?;
        if !self.window_closing
            || !self
                .session(target.session)
                .is_some_and(|session| session.shutdown.decision().is_some())
        {
            return self.reject(Some(target.session), WorkspaceError::StaleCloseTarget);
        }
        self.force_window_close = true;
        Ok(())
    }
    pub(in crate::app) fn close_prompt(&self) -> Option<ClosePrompt> {
        let mut progress = None;
        for session in &self.sessions {
            let Some(target) = self.close_target(session.id) else {
                continue;
            };
            if session.shutdown.allows_close() {
                continue;
            }
            let kind = if let Some(reason) = session.shutdown.decision() {
                ClosePromptKind::Incomplete(reason.to_owned())
            } else if session.shutdown.unsaved_session_confirmation() {
                ClosePromptKind::UnsavedSession
            } else {
                ClosePromptKind::Progress
            };
            let prompt = ClosePrompt {
                target,
                title: session.title.clone(),
                kind,
                window: self.window_closing,
            };
            if prompt.kind == ClosePromptKind::Progress {
                if progress.is_none() {
                    progress = Some(prompt);
                }
            } else {
                return Some(prompt);
            }
        }
        progress
    }
    pub(super) fn reap_closed_sessions(&mut self) {
        let ready: Vec<_> = self
            .sessions
            .iter()
            .filter(|s| s.shutdown.allows_close() && self.close_target(s.id).is_some())
            .map(|s| s.id)
            .collect();
        for id in ready {
            let Some(index) = self.sessions.iter().position(|s| s.id == id) else {
                continue;
            };
            if self.sessions.len() == 1 {
                // Failed reservation preserves the old owner, including evidence.
                let replacement = match self.reserve_session_id() {
                    Ok(id) => id,
                    Err(_) => {
                        self.sessions[index].shutdown.keep_open();
                        self.close_requests.retain(|target| target.session != id);
                        continue;
                    }
                };
                self.sessions[index] = Self::fresh_session(replacement);
                self.active = replacement;
            } else {
                self.sessions.remove(index);
                if self.active == id {
                    self.active = self.sessions[index.min(self.sessions.len() - 1)].id;
                }
            }
            self.close_requests.retain(|target| target.session != id);
        }
    }
}
