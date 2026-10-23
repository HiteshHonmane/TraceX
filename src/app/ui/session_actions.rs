use super::super::SessionRuntime;

pub(super) fn choose_save_session(session: &mut SessionRuntime) {
    if !session.save_session_ready() {
        return;
    }
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("TraceX session", &["tracex"])
        .set_file_name("TraceX-session.tracex")
        .save_file()
    {
        session.save_session_to(path);
    }
}

pub(super) fn choose_open_session(session: &mut SessionRuntime) {
    if !session.open_session_ready() {
        return;
    }
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("TraceX session", &["tracex"])
        .pick_file()
    {
        session.open_session_from(path);
    }
}

pub(super) fn choose_export_logs(session: &mut SessionRuntime) {
    if !session.text_export_ready() {
        return;
    }
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Log", &["log", "txt"])
        .set_file_name("export.log")
        .save_file()
    {
        session.export_logs_to(path);
    }
}
