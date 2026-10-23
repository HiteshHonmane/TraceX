use super::*;
use chrono::Local;
use std::fs;
use std::path::PathBuf;

fn test_entry(id: usize) -> LogEntry {
    LogEntry {
        id,
        timestamp: format!("12:00:{:02}.000", id % 60),
        level: LogLevel::Info,
        tag: None,
        message: format!("log message {id}"),
        raw: format!("log message {id}"),
    }
}

#[test]
fn formats_every_log_not_just_a_viewport_sized_subset() {
    let logs: Vec<_> = (0..64).map(test_entry).collect();

    let copied = format_log_entries(&logs);
    let lines: Vec<_> = copied.lines().collect();

    assert_eq!(lines.len(), 64);
    assert_eq!(lines.first(), Some(&"[12:00:00.000] [INF] log message 0"));
    assert_eq!(lines.last(), Some(&"[12:00:03.000] [INF] log message 63"));
}

#[test]
fn single_row_copy_uses_the_complete_formatted_line() {
    let entry = test_entry(7);
    let expected = "[12:00:07.000] [INF] log message 7";

    assert_eq!(format_log_entries([&entry]), expected);

    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput::default());
    ctx.copy_text(format_log_line(
        &entry.timestamp,
        entry.level,
        &entry.message,
    ));
    let output = ctx.end_pass();
    assert_eq!(output.platform_output.copied_text, expected);
}

#[test]
fn jump_offset_centers_middle_rows_and_clamps_edges() {
    let row_height = 30.0;
    let viewport_height = 300.0;

    let middle = centered_row_scroll_offset(50, 100, row_height, viewport_height);
    assert_eq!(middle, 1365.0);
    assert_eq!(middle + viewport_height / 2.0, 50.5 * row_height);

    assert_eq!(
        centered_row_scroll_offset(0, 100, row_height, viewport_height),
        0.0
    );
    assert_eq!(
        centered_row_scroll_offset(99, 100, row_height, viewport_height),
        2700.0
    );
}

#[test]
fn jump_offset_is_zero_when_all_rows_fit() {
    assert_eq!(centered_row_scroll_offset(2, 5, 30.0, 300.0), 0.0);
}

#[test]
fn timestamped_autosave_name_includes_seconds_and_rotation_sequence() {
    use chrono::TimeZone;

    let timestamp = Local
        .with_ymd_and_hms(2026, 7, 16, 14, 5, 9)
        .single()
        .unwrap();
    let base = PathBuf::from("logs/TraceX.log");

    assert_eq!(
        timestamped_log_path(&base, timestamp, 0)
            .file_name()
            .and_then(|name| name.to_str()),
        Some("TraceX_2026-07-16_14-05-09.log")
    );
    assert_eq!(
        timestamped_log_path(&base, timestamp, 3)
            .file_name()
            .and_then(|name| name.to_str()),
        Some("TraceX_2026-07-16_14-05-09_003.log")
    );
}

#[test]
fn autosave_rotation_only_happens_after_the_configured_limit() {
    let one_mb = 1024 * 1024;

    assert!(!should_rotate_log(0, one_mb + 1, 1));
    assert!(!should_rotate_log((one_mb - 10) as u64, 10, 1));
    assert!(should_rotate_log((one_mb - 10) as u64, 11, 1));
}

#[test]
fn filtered_copy_formats_only_the_supplied_rows() {
    let copied = format_log_rows([
        ("10:00:01.001", LogLevel::Warn, "first match"),
        ("10:00:02.002", LogLevel::Error, "second match"),
    ]);

    assert_eq!(
        copied,
        "[10:00:01.001] [WRN] first match\n[10:00:02.002] [ERR] second match"
    );
}

#[test]
fn runtime_clipboard_text_adapters_preserve_filtered_and_explicit_selection_scopes() {
    let mut session = SessionRuntime::new();
    session.record_log_line("INFO needle first".into());
    session.record_log_line("WARN hidden second".into());
    session.record_log_line("ERROR needle third".into());
    session.search = "needle".into();
    session.show_warn = false;
    let filtered = session.copy_selected_text();
    assert!(filtered.contains("needle first"));
    assert!(filtered.contains("needle third"));
    assert!(!filtered.contains("hidden second"));
    session.selected.insert(1);
    session.selected.insert(0);
    let selected = session.copy_selected_text();
    assert!(selected.contains("hidden second"));
    assert!(selected.find("first").unwrap() < selected.find("second").unwrap());
    assert!(!selected.contains("third"));
    let all = session.copy_all_text();
    assert!(all.contains("hidden second"));
    assert!(all.contains("needle third"));
    let query_rows = session.query_rows_text(&[2, usize::MAX]);
    assert!(query_rows.contains("needle third"));
    assert!(!query_rows.contains("first"));
}

#[test]
fn selected_row_context_copy_preserves_log_order() {
    let logs: Vec<_> = (0..5).map(test_entry).collect();
    let selected = HashSet::from([3, 1, 4]);

    let copied = selected_log_text(&logs, &selected).unwrap();
    let lines: Vec<_> = copied.lines().collect();

    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("log message 1"));
    assert!(lines[1].ends_with("log message 3"));
    assert!(lines[2].ends_with("log message 4"));
}

#[test]
fn selected_row_context_copy_requires_multiple_rows() {
    let logs: Vec<_> = (0..2).map(test_entry).collect();

    assert!(selected_log_text(&logs, &HashSet::new()).is_none());
    assert!(selected_log_text(&logs, &HashSet::from([1])).is_none());
    assert!(selected_log_text(&logs, &HashSet::from([0, 1])).is_some());
}

#[test]
fn enabling_auto_scroll_requests_an_immediate_live_jump() {
    assert_eq!(
        live_scroll_frames_after_toggle(true, true),
        LIVE_SCROLL_SETTLE_FRAMES
    );
    assert_eq!(live_scroll_frames_after_toggle(true, false), 0);
    assert_eq!(live_scroll_frames_after_toggle(false, true), 0);
}

#[test]
fn live_jump_uses_a_finite_virtualized_scroll_offset() {
    let row_count = 50_000;
    let row_height = 30.0;
    let viewport_height = 300.0;
    let offset = bottom_scroll_offset(row_count, row_height, viewport_height);

    assert!(offset.is_finite());
    assert_eq!(offset, 1_499_700.0);

    egui::__run_test_ui(|ui| {
        ui.allocate_ui(Vec2::new(600.0, viewport_height), |ui| {
            let output = egui::ScrollArea::both()
                .id_salt("auto_scroll_regression")
                .vertical_scroll_offset(offset)
                .show_rows(ui, 24.0, row_count, |ui, range| {
                    for row in range {
                        ui.label(format!("row {row}"));
                    }
                });

            assert!(output.state.offset.y.is_finite());
            assert!(output.state.offset.y <= offset);
        });
    });
}

#[test]
fn live_jump_is_zero_when_logs_fit_in_the_viewport() {
    assert_eq!(bottom_scroll_offset(5, 30.0, 300.0), 0.0);
}

#[test]
fn command_history_keeps_recent_unique_commands_in_send_order() {
    let mut history = Vec::new();

    record_command_history(&mut history, "AT", 3);
    record_command_history(&mut history, "AT+GMR", 3);
    record_command_history(&mut history, "AT+RST", 3);
    record_command_history(&mut history, "AT+GMR", 3);

    assert_eq!(history, ["AT", "AT+RST", "AT+GMR"]);
}

#[test]
fn command_history_ignores_blank_commands_and_respects_limit() {
    let mut history = Vec::new();

    record_command_history(&mut history, "   ", 2);
    record_command_history(&mut history, " AT ", 2);
    record_command_history(&mut history, "AT+GMR", 2);
    record_command_history(&mut history, "AT+RST", 2);

    assert_eq!(history, ["AT+GMR", "AT+RST"]);
}

#[test]
fn submitting_a_command_records_it_without_requiring_a_serial_transport() {
    let mut history = Vec::new();

    let submitted = register_submitted_command(&mut history, "  AT+CSQ  ");

    assert_eq!(submitted.as_deref(), Some("AT+CSQ"));
    assert_eq!(history, ["AT+CSQ"]);
}

#[test]
fn submitting_a_blank_command_does_not_change_history() {
    let mut history = vec!["AT".to_string()];

    let submitted = register_submitted_command(&mut history, "   ");

    assert!(submitted.is_none());
    assert_eq!(history, ["AT"]);
}

#[test]
fn long_logs_expand_the_horizontal_scrollable_width() {
    let short = log_content_width(["short"], 500.0);
    let long = log_content_width(["x".repeat(200).as_str()], 500.0);

    assert_eq!(short, 500.0);
    assert!(long > short);
}

#[test]
fn timestamped_autosave_file_is_created_immediately_and_never_overwritten() {
    use chrono::TimeZone;

    let timestamp = Local
        .with_ymd_and_hms(2026, 7, 16, 15, 4, 27)
        .single()
        .unwrap();
    let test_dir =
        std::env::temp_dir().join(format!("tracex-autosave-test-{}", std::process::id()));
    let base = test_dir.join("TraceX.log");

    let (first_file, first_path, next_sequence) =
        create_timestamped_log_file(&base, timestamp, 0).unwrap();
    drop(first_file);
    assert!(first_path.exists());
    assert_eq!(
        first_path.file_name().and_then(|name| name.to_str()),
        Some("TraceX_2026-07-16_15-04-27.log")
    );

    let (second_file, second_path, _) = create_timestamped_log_file(&base, timestamp, 0).unwrap();
    drop(second_file);
    assert!(second_path.exists());
    assert_ne!(first_path, second_path);
    assert_eq!(next_sequence, 1);
    assert_eq!(
        second_path.file_name().and_then(|name| name.to_str()),
        Some("TraceX_2026-07-16_15-04-27_001.log")
    );

    let _ = fs::remove_file(first_path);
    let _ = fs::remove_file(second_path);
    let _ = fs::remove_dir(test_dir);
}

fn app_without_devices() -> SessionRuntime {
    SessionRuntime::new()
}

#[test]
fn manual_disconnect_retains_capture_until_final_records_drain_and_blocks_connect() {
    let mut app = app_without_devices();
    let (sender, inbox) = capture::CaptureInbox::test_channel();
    let stop = Arc::new(Mutex::new(false));
    app.capture = Some(inbox);
    app.stop_flag = Some(Arc::clone(&stop));
    app.connected = true;
    app.last_connected_port = Some("test port".to_owned());
    sender.send("queued".to_owned()).unwrap();
    app.disconnect();
    assert!(*stop.lock().unwrap());
    assert!(app.last_connected_port.is_none());
    app.connect();
    assert!(
        app.operation_error
            .as_deref()
            .unwrap()
            .contains("finish disconnecting")
    );
    app.poll();
    assert_eq!(app.logs[0].raw, "queued");
    assert!(app.capture.is_some());
    sender.send("final tail".to_owned()).unwrap();
    drop(sender);
    app.poll();
    assert_eq!(app.logs[1].raw, "final tail");
    assert!(app.capture.is_none());
    assert!(app.stop_flag.is_none());
}

fn poll_until(app: &mut SessionRuntime, mut ready: impl FnMut(&SessionRuntime) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready(app) {
        assert!(
            Instant::now() < deadline,
            "application did not reach expected state"
        );
        app.poll();
        std::thread::yield_now();
    }
}

#[test]
fn manual_disconnect_saves_final_queued_records_before_closing_autosave() {
    let mut app = app_without_devices();
    let directory =
        std::env::temp_dir().join(format!("tracex-disconnect-save-{}", std::process::id()));
    app.auto_save_path = Some(directory.join("TraceX.log"));
    app.auto_save_enabled = true;
    app.apply_auto_save_enabled_change();
    poll_until(&mut app, |app| app.auto_save_current_path.is_some());
    let path = app.auto_save_current_path.clone().unwrap();
    let (sender, inbox) = capture::CaptureInbox::test_channel();
    app.capture = Some(inbox);
    app.connected = true;

    app.disconnect();
    assert!(app.auto_save.active);
    sender.send("final saved tail".to_owned()).unwrap();
    drop(sender);
    poll_until(&mut app, |app| {
        app.capture.is_none() && !app.auto_save.active && !app.auto_save.stopping
    });

    let saved = fs::read_to_string(&path).unwrap();
    assert!(saved.contains("final saved tail"));
    fs::remove_file(path).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn export_and_main_view_share_trimmed_raw_matching_and_severity() {
    let mut app = app_without_devices();
    let mut raw_match = LogEntry::new(21, "hidden raw needle".into(), "12:00:00");
    raw_match.message = "parsed body".into();
    raw_match.level = LogLevel::Info;
    let mut severity_hidden = raw_match.clone();
    severity_hidden.id = 22;
    severity_hidden.level = LogLevel::Warn;
    app.logs = vec![Arc::new(raw_match), Arc::new(severity_hidden)];
    app.search = "  NeEdLe \t".into();
    app.show_warn = false;
    app.filter_exclude = true;

    assert_eq!(app.query_indices(QueryScope::Main), [0]);
    assert_eq!(app.export_text(), "[12:00:00] [INF] parsed body");
    app.filter_exclude = false;
    assert_eq!(app.export_text(), "[12:00:00] [INF] parsed body");
    app.search = " \t\n".into();
    assert_eq!(app.export_text(), "[12:00:00] [INF] parsed body");
    app.show_info = false;
    assert!(app.export_text().is_empty());
}

#[test]
fn history_limit_keeps_triggering_capture_record_and_clear_drains_with_stable_ids() {
    let mut app = app_without_devices();
    app.history = HistoryBudget::new(1, usize::MAX);
    let (sender, inbox) = capture::CaptureInbox::test_channel();
    sender.send("first".to_owned()).unwrap();
    sender.send("retained second".to_owned()).unwrap();
    drop(sender);
    app.capture = Some(inbox);
    app.stop_flag = Some(Arc::new(Mutex::new(false)));
    app.connected = true;
    app.last_connected_port = Some("fake".to_owned());
    app.poll();
    assert_eq!(app.logs.len(), 1);
    assert_eq!(app.logs[0].id, 0);
    assert!(app.history_limit_reached);
    assert!(!app.connected);
    assert!(app.last_connected_port.is_none());
    assert_eq!(
        app.capture.as_mut().unwrap().peek_record().unwrap(),
        "retained second"
    );
    app.clear();
    app.poll();
    assert_eq!(app.logs.len(), 1);
    assert_eq!(app.logs[0].raw, "retained second");
    assert_eq!(app.logs[0].id, 1);
    assert!(app.capture.is_none());
    assert_eq!(app.next_capture_batch, 2);
}

#[test]
fn exhausted_record_id_keeps_capture_payload_and_stops_without_overflow() {
    let mut app = app_without_devices();
    let (sender, inbox) = capture::CaptureInbox::test_channel();
    sender.send("must remain owned".to_owned()).unwrap();
    drop(sender);
    app.capture = Some(inbox);
    app.next_id = usize::MAX;
    app.connected = true;
    app.poll();
    assert!(app.logs.is_empty());
    assert_eq!(
        app.capture.as_mut().unwrap().peek_record().unwrap(),
        "must remain owned"
    );
    assert!(app.history_limit_reached);
    assert!(!app.connected);
    assert_eq!(app.next_id, usize::MAX);
    assert!(
        app.capture_error
            .as_deref()
            .unwrap()
            .contains("identifiers")
    );
}

#[test]
fn bounded_command_queue_and_oversize_rejections_preserve_input_for_retry() {
    let mut app = app_without_devices();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    app.serial_tx = Some(sender);
    app.connected = true;
    assert!(app.send_command("first"));
    assert!(!app.send_command("retry second"));
    assert_eq!(app.command_input, "retry second");
    assert!(app.operation_error.as_deref().unwrap().contains("full"));
    assert_eq!(receiver.try_recv().unwrap(), b"first\r\n");
    let oversize = "x".repeat(capture::MAX_COMMAND_BYTES);
    assert!(!app.send_command(&oversize));
    assert_eq!(app.command_input, oversize);
    assert!(app.operation_error.as_deref().unwrap().contains("limit"));
    assert!(receiver.try_recv().is_err());
    app.connected = false;
    assert!(!app.send_command("not routed after stop"));
    assert!(receiver.try_recv().is_err());
}

#[test]
fn empty_unexpected_capture_exit_is_observed_before_drained_inbox_is_dropped() {
    let mut app = app_without_devices();
    let (sender, inbox) = capture::CaptureInbox::test_channel();
    sender.close_unexpectedly();
    app.capture = Some(inbox);
    app.connected = true;
    app.last_connected_port = Some("must not reconnect".to_owned());
    app.poll();
    assert!(app.capture.is_none());
    assert!(!app.connected);
    assert!(app.last_connected_port.is_none());
    assert!(
        app.capture_error
            .as_deref()
            .unwrap()
            .contains("ended unexpectedly")
    );
}

#[test]
fn unexpected_exit_records_last_batch_id_before_drained_inbox_is_dropped() {
    let mut app = app_without_devices();
    let (sender, inbox) = capture::CaptureInbox::test_channel();
    sender.send("accepted before exit".to_owned()).unwrap();
    sender.close_unexpectedly();
    app.capture = Some(inbox);
    app.connected = true;
    app.last_connected_port = Some("must not reconnect".to_owned());
    app.poll();
    assert_eq!(app.logs[0].raw, "accepted before exit");
    assert!(app.capture.is_none());
    assert_eq!(app.next_capture_batch, 1);
    assert!(app.last_connected_port.is_none());
    assert!(
        app.capture_error
            .as_deref()
            .unwrap()
            .contains("ended unexpectedly")
    );
}
