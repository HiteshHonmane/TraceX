//! One nonblocking native picker, retaining the identity that requested it.
use crate::app::workspace::SessionId;
use eframe::egui;
use std::{
    fmt,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum FileDialogKind {
    OpenSession,
    SaveSession,
    ExportText,
    AutosavePath,
    RecoveryExport,
}

pub(in crate::app) struct FileDialogOutcome {
    pub session: SessionId,
    pub kind: FileDialogKind,
    // rfd returns None for cancellation or unavailable native selection. Neither
    // outcome authorizes an operation or implies a file was read or written.
    pub path: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::app) struct FileDialogBusy;

impl fmt::Display for FileDialogBusy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Finish or cancel the current file picker before opening another.")
    }
}

type PathFuture = Pin<Box<dyn Future<Output = Option<PathBuf>>>>;

struct PendingDialog {
    session: SessionId,
    kind: FileDialogKind,
    future: PathFuture,
}

#[derive(Default)]
pub(in crate::app) struct FileDialogs {
    pending: Option<PendingDialog>,
}

struct RepaintOnWake(egui::Context);

impl Wake for RepaintOnWake {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint();
    }
}

impl FileDialogs {
    #[cfg(test)]
    pub(in crate::app) fn start_test_future(
        &mut self,
        session: SessionId,
        kind: FileDialogKind,
        future: impl Future<Output = Option<PathBuf>> + 'static,
    ) -> Result<(), FileDialogBusy> {
        self.begin(session, kind, || Box::pin(future))
    }

    pub(in crate::app) fn pending_session(&self) -> Option<SessionId> {
        self.pending.as_ref().map(|pending| pending.session)
    }

    pub(in crate::app) fn start(
        &mut self,
        session: SessionId,
        kind: FileDialogKind,
        default_name: Option<String>,
    ) -> Result<(), FileDialogBusy> {
        self.begin(session, kind, move || {
            Box::pin(async move {
                let picker = rfd::AsyncFileDialog::new();
                let selected = match kind {
                    FileDialogKind::OpenSession => {
                        picker
                            .add_filter("TraceX session", &["tracex"])
                            .pick_file()
                            .await
                    }
                    FileDialogKind::SaveSession => {
                        picker
                            .add_filter("TraceX session", &["tracex"])
                            .set_file_name("TraceX-session.tracex")
                            .save_file()
                            .await
                    }
                    FileDialogKind::ExportText => {
                        picker
                            .add_filter("Log", &["log", "txt"])
                            .set_file_name("export.log")
                            .save_file()
                            .await
                    }
                    FileDialogKind::AutosavePath => {
                        picker
                            .add_filter("Log", &["log", "txt"])
                            .set_file_name(default_name.unwrap_or_else(|| "TraceX.log".into()))
                            .save_file()
                            .await
                    }
                    FileDialogKind::RecoveryExport => {
                        picker
                            .add_filter("Recovery JSON", &["json"])
                            .set_file_name("TraceX-recovery.json")
                            .save_file()
                            .await
                    }
                };
                selected.map(|file| file.path().to_path_buf())
            })
        })
    }

    fn begin(
        &mut self,
        session: SessionId,
        kind: FileDialogKind,
        make_future: impl FnOnce() -> PathFuture,
    ) -> Result<(), FileDialogBusy> {
        if self.pending.is_some() {
            return Err(FileDialogBusy);
        }
        self.pending = Some(PendingDialog {
            session,
            kind,
            future: make_future(),
        });
        Ok(())
    }

    /// Poll once, never block the native update or join a platform dialog thread.
    /// The UI must revalidate this explicit owner through Workspace on completion.
    pub(in crate::app) fn poll(&mut self, ctx: &egui::Context) -> Option<FileDialogOutcome> {
        let pending = self.pending.as_mut()?;
        let waker = Waker::from(Arc::new(RepaintOnWake(ctx.clone())));
        let mut context = Context::from_waker(&waker);
        let Poll::Ready(path) = pending.future.as_mut().poll(&mut context) else {
            return None;
        };
        let outcome = FileDialogOutcome {
            session: pending.session,
            kind: pending.kind,
            path,
        };
        self.pending = None;
        Some(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, future, sync::mpsc};

    struct ControlledPath(mpsc::Receiver<Option<PathBuf>>);

    impl Future for ControlledPath {
        type Output = Option<PathBuf>;

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            match self.0.try_recv() {
                Ok(path) => Poll::Ready(path),
                Err(mpsc::TryRecvError::Empty) => Poll::Pending,
                Err(mpsc::TryRecvError::Disconnected) => Poll::Ready(None),
            }
        }
    }

    #[test]
    fn pending_picker_is_bounded_and_returns_the_original_owner_once() {
        let mut dialogs = FileDialogs::default();
        let ctx = egui::Context::default();
        let (complete, receive) = mpsc::channel();
        dialogs
            .begin(SessionId(7), FileDialogKind::OpenSession, || {
                Box::pin(ControlledPath(receive))
            })
            .unwrap();
        assert!(dialogs.poll(&ctx).is_none());
        let second_started = Cell::new(false);
        assert_eq!(
            dialogs.begin(SessionId(8), FileDialogKind::SaveSession, || {
                second_started.set(true);
                Box::pin(future::ready(None))
            }),
            Err(FileDialogBusy)
        );
        assert!(
            !second_started.get(),
            "a refused picker must not start platform work"
        );
        assert_eq!(dialogs.pending_session(), Some(SessionId(7)));
        complete
            .send(Some(PathBuf::from("original-owner.tracex")))
            .unwrap();
        let result = dialogs.poll(&ctx).unwrap();
        assert_eq!(result.session, SessionId(7));
        assert_eq!(result.kind, FileDialogKind::OpenSession);
        assert_eq!(result.path, Some(PathBuf::from("original-owner.tracex")));
        assert!(dialogs.pending_session().is_none());
        assert!(dialogs.poll(&ctx).is_none(), "completion dispatches once");
    }

    #[test]
    fn absent_selection_releases_picker_without_authorizing_a_file_operation() {
        let mut dialogs = FileDialogs::default();
        let ctx = egui::Context::default();
        dialogs
            .begin(SessionId(9), FileDialogKind::RecoveryExport, || {
                Box::pin(future::ready(None))
            })
            .unwrap();
        let result = dialogs.poll(&ctx).unwrap();
        assert_eq!(result.session, SessionId(9));
        assert!(result.path.is_none());
        assert!(dialogs.pending_session().is_none());
        assert!(
            dialogs
                .start(SessionId(10), FileDialogKind::OpenSession, None)
                .is_ok()
        );
        // Dropping this unpolled async block starts no native dialog.
    }
}
