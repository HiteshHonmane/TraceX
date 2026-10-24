use super::*;
use eframe::egui::text::{CCursor, CCursorRange};

fn raw() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(1200.0, 800.0),
        )),
        ..Default::default()
    }
}
fn draw(app: &mut TraceXApp, ctx: &egui::Context) {
    let _ = ctx.run(raw(), |ctx| app.update_native_ui(ctx));
}
fn cursor(ctx: &egui::Context, id: SessionId, start: usize, end: usize) {
    let key = widget_id(id, "command_editor");
    let mut state = egui::text_edit::TextEditState::load(ctx, key).unwrap();
    state.cursor.set_char_range(Some(CCursorRange::two(
        CCursor::new(start),
        CCursor::new(end),
    )));
    state.store(ctx, key);
}
fn assert_cursor(ctx: &egui::Context, id: SessionId, start: usize, end: usize) {
    let range = egui::text_edit::TextEditState::load(ctx, widget_id(id, "command_editor"))
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(
        (
            range.primary.index.min(range.secondary.index),
            range.primary.index.max(range.secondary.index)
        ),
        (start, end)
    );
}

#[test]
fn actual_editors_and_popups_follow_stable_ids_across_switch_and_lower_tab_close() {
    let ctx = egui::Context::default();
    let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new()))).prepare_native_ui(&ctx);
    let a = app.workspace.active_session_id();
    let b = app.workspace.create_session().unwrap();
    app.workspace.session_mut(a).unwrap().command_input = "AAAAAA".to_owned();
    app.workspace.session_mut(b).unwrap().command_input = "BBBBBBBB".to_owned();
    app.workspace.select_session(a).unwrap();
    draw(&mut app, &ctx);
    cursor(&ctx, a, 1, 3);
    app.workspace.select_session(b).unwrap();
    draw(&mut app, &ctx);
    cursor(&ctx, b, 4, 7);
    let library_a = widget_id(a, "library_popup");
    let library_b = widget_id(b, "library_popup");
    let settings_a = widget_id(a, "settings_popup");
    let settings_b = widget_id(b, "settings_popup");
    assert_ne!(library_a, library_b);
    assert_ne!(settings_a, settings_b);
    ctx.memory_mut(|m| m.open_popup(library_a));
    draw(&mut app, &ctx);
    assert!(!ctx.memory(|m| m.is_popup_open(library_b)));
    ctx.memory_mut(|m| m.close_popup());
    app.workspace.select_session(a).unwrap();
    draw(&mut app, &ctx);
    assert_cursor(&ctx, a, 1, 3);
    ctx.memory_mut(|m| m.open_popup(settings_b));
    draw(&mut app, &ctx);
    assert!(!ctx.memory(|m| m.is_popup_open(settings_a)));
    ctx.memory_mut(|m| m.close_popup());
    app.workspace
        .request_close_session(a, Instant::now())
        .unwrap();
    app.workspace.poll(Instant::now(), &[]);
    assert!(app.workspace.session(a).is_none());
    let c = app.workspace.create_session().unwrap();
    assert_ne!(a, c);
    assert_ne!(b, c);
    app.workspace.select_session(b).unwrap();
    draw(&mut app, &ctx);
    assert_cursor(&ctx, b, 4, 7);
    assert_eq!(app.workspace.session(b).unwrap().command_input, "BBBBBBBB");
    assert_ne!(
        widget_id(b, "command_editor"),
        widget_id(c, "command_editor")
    );
}

#[test]
fn rendered_log_scroll_offsets_do_not_follow_vector_position() {
    let ctx = egui::Context::default();
    theme::setup(&ctx);
    let mut workspace = Workspace::new();
    let a = workspace.active_session_id();
    let b = workspace.create_session().unwrap();
    for id in [a, b] {
        let session = workspace.session_mut(id).unwrap();
        session.logs = (0..1000)
            .map(|n| LogEntry::new(n, format!("session {} row {n}", id.0), "12:00:00").into())
            .collect();
    }
    let results: Vec<_> = (0..1000).collect();
    let render = |workspace: &mut Workspace, id, ctx: &egui::Context| {
        let mut actual = egui::Id::NULL;
        let _ = ctx.run(raw(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                actual = render_filter_rows(
                    workspace.session_mut(id).unwrap(),
                    id,
                    ui,
                    ctx,
                    &results,
                    1100.0,
                )
                .1;
            });
        });
        actual
    };
    let scroll_a = render(&mut workspace, a, &ctx);
    let mut state = egui::scroll_area::State::load(&ctx, scroll_a).unwrap();
    state.offset.y = 400.0;
    state.store(&ctx, scroll_a);
    let scroll_b = render(&mut workspace, b, &ctx);
    assert_ne!(scroll_a, scroll_b);
    let mut state = egui::scroll_area::State::load(&ctx, scroll_b).unwrap();
    state.offset.y = 800.0;
    state.store(&ctx, scroll_b);
    render(&mut workspace, a, &ctx);
    render(&mut workspace, b, &ctx);
    assert!(
        (egui::scroll_area::State::load(&ctx, scroll_a)
            .unwrap()
            .offset
            .y
            - 400.0)
            .abs()
            < 1.0
    );
    assert!(
        (egui::scroll_area::State::load(&ctx, scroll_b)
            .unwrap()
            .offset
            .y
            - 800.0)
            .abs()
            < 1.0
    );
    workspace.clear_session(a).unwrap();
    workspace.request_close_session(a, Instant::now()).unwrap();
    workspace.poll(Instant::now(), &[]);
    assert!(workspace.session(a).is_none());
    workspace.create_session().unwrap();
    assert_eq!(render(&mut workspace, b, &ctx), scroll_b);
    assert!(
        (egui::scroll_area::State::load(&ctx, scroll_b)
            .unwrap()
            .offset
            .y
            - 800.0)
            .abs()
            < 1.0
    );
}

#[test]
fn pending_async_picker_does_not_stop_either_session_from_polling() {
    let ctx = egui::Context::default();
    let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new()))).prepare_native_ui(&ctx);
    let a = app.workspace.active_session_id();
    let b = app.workspace.create_session().unwrap();
    let (feed_a, inbox_a) = capture::CaptureInbox::test_channel();
    let (feed_b, inbox_b) = capture::CaptureInbox::test_channel();
    app.workspace.session_mut(a).unwrap().capture = Some(inbox_a);
    app.workspace.session_mut(b).unwrap().capture = Some(inbox_b);
    app.dialogs
        .start_test_future(
            b,
            super::file_dialog::FileDialogKind::SaveSession,
            std::future::pending(),
        )
        .unwrap();
    for n in 0..2 {
        feed_a.send(format!("A record {n}")).unwrap();
        feed_b.send(format!("B record {n}")).unwrap();
        draw(&mut app, &ctx);
        assert_eq!(app.workspace.session(a).unwrap().logs.len(), n + 1);
        assert_eq!(app.workspace.session(b).unwrap().logs.len(), n + 1);
        assert_eq!(app.dialogs.pending_session(), Some(b));
    }
}

#[test]
fn completed_picker_mutates_origin_even_after_switching_active_owner() {
    let ctx = egui::Context::default();
    let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new()))).prepare_native_ui(&ctx);
    let a = app.workspace.active_session_id();
    let b = app.workspace.create_session().unwrap();
    let original_b = app.workspace.session(b).unwrap().auto_save_path.clone();
    let chosen = std::env::temp_dir().join("tracex-ui-owner-path-not-written.log");
    app.dialogs
        .start_test_future(
            a,
            super::file_dialog::FileDialogKind::AutosavePath,
            std::future::ready(Some(chosen.clone())),
        )
        .unwrap();
    app.workspace.select_session(b).unwrap();
    draw(&mut app, &ctx);
    assert_eq!(
        app.workspace.session(a).unwrap().auto_save_path,
        Some(chosen)
    );
    assert_eq!(app.workspace.session(b).unwrap().auto_save_path, original_b);
    assert_eq!(app.dialogs.pending_session(), None);
}

#[test]
fn focused_search_copy_keeps_editor_selection_with_session_namespace() {
    let ctx = egui::Context::default();
    let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new()))).prepare_native_ui(&ctx);
    let a = app.workspace.active_session_id();
    app.workspace.session_mut(a).unwrap().search = "timeout".to_owned();
    app.workspace
        .session_mut(a)
        .unwrap()
        .logs
        .push(Arc::new(LogEntry::new(
            0,
            "timeout log record".to_owned(),
            "12:00:00",
        )));
    draw(&mut app, &ctx);
    let id = widget_id(a, "search_editor");
    let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(7))));
    state.store(&ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
    let mut copy = raw();
    copy.modifiers = egui::Modifiers::CTRL;
    copy.events = vec![
        egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        },
        egui::Event::Copy,
    ];
    let output = ctx.run(copy, |ctx| app.update_native_ui(ctx));
    assert_eq!(output.platform_output.copied_text, "timeout");
}

#[test]
fn late_picker_result_for_removed_owner_cannot_mutate_surviving_or_replacement_session() {
    use std::{sync::mpsc, task::Poll};
    let ctx = egui::Context::default();
    let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new()))).prepare_native_ui(&ctx);
    let a = app.workspace.active_session_id();
    let b = app.workspace.create_session().unwrap();
    let original_b = app.workspace.session(b).unwrap().auto_save_path.clone();
    app.workspace.session_mut(b).unwrap().command_input = "keep B draft".to_owned();
    let (complete, result) = mpsc::channel();
    let future = std::future::poll_fn(move |_| match result.try_recv() {
        Ok(path) => Poll::Ready(Some(path)),
        Err(mpsc::TryRecvError::Empty) => Poll::Pending,
        Err(mpsc::TryRecvError::Disconnected) => Poll::Ready(None),
    });
    app.dialogs
        .start_test_future(a, super::file_dialog::FileDialogKind::AutosavePath, future)
        .unwrap();
    app.workspace
        .request_close_session(a, Instant::now())
        .unwrap();
    app.workspace.poll(Instant::now(), &[]);
    assert!(app.workspace.session(a).is_none());
    let c = app.workspace.create_session().unwrap();
    let original_c = app.workspace.session(c).unwrap().auto_save_path.clone();
    app.workspace.select_session(b).unwrap();
    complete
        .send(std::env::temp_dir().join("tracex-removed-owner-not-written.log"))
        .unwrap();
    draw(&mut app, &ctx);
    assert_eq!(app.dialogs.pending_session(), None);
    assert!(
        app.workspace
            .operation_error
            .as_deref()
            .is_some_and(|error| error.contains("no longer exists"))
    );
    assert_eq!(app.workspace.session(b).unwrap().auto_save_path, original_b);
    assert_eq!(app.workspace.session(c).unwrap().auto_save_path, original_c);
    assert_eq!(
        app.workspace.session(b).unwrap().command_input,
        "keep B draft"
    );
    assert!(!app.workspace.session(b).unwrap().session_archive_busy());
    assert!(!app.workspace.session(c).unwrap().session_archive_busy());
}

#[test]
fn dismiss_workspace_notification_preserves_owner_failures_draft_and_close_blocker() {
    let mut workspace = Workspace::new();
    let a = workspace.active_session_id();
    let (_feed, inbox) = capture::CaptureInbox::test_channel();
    let session = workspace.session_mut(a).unwrap();
    session.capture = Some(inbox);
    session.capture_error = Some("capture failure with retained inbox".to_owned());
    session.operation_error = Some("owner archive failure".to_owned());
    session.auto_save_error = Some("owner recovery warning".to_owned());
    session.command_input = "unsent owner draft".to_owned();
    workspace.request_window_close(Instant::now()).unwrap();
    workspace.poll(Instant::now(), &[]);
    let prompt_before = workspace.close_prompt().unwrap();
    assert!(matches!(prompt_before.kind, ClosePromptKind::Incomplete(_)));
    workspace.operation_error =
        Some("Finish or cancel the current file picker before opening another.".to_owned());
    let mut dialogs = super::file_dialog::FileDialogs::default();
    super::actions::dispatch(
        &mut workspace,
        &mut dialogs,
        vec![UiIntent::DismissWorkspaceError],
    );
    assert!(workspace.operation_error.is_none());
    let session = workspace.session(a).unwrap();
    assert_eq!(
        session.capture_error.as_deref(),
        Some("capture failure with retained inbox")
    );
    assert_eq!(
        session.operation_error.as_deref(),
        Some("owner archive failure")
    );
    assert_eq!(
        session.auto_save_error.as_deref(),
        Some("owner recovery warning")
    );
    assert_eq!(session.command_input, "unsent owner draft");
    assert!(session.capture.is_some());
    assert!(workspace.window_closing());
    assert!(!workspace.window_allows_close());
    let prompt_after = workspace.close_prompt().unwrap();
    assert_eq!(prompt_after.target, prompt_before.target);
    assert_eq!(prompt_after.kind, prompt_before.kind);
}
