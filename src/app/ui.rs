use super::*;

mod autosave;
mod command_bar;
mod icons;
mod menus;
mod session_actions;
mod sidebar;
mod tabs;
mod toolbar;

fn render_filter_rows(
    session: &mut SessionRuntime,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    results: &[usize],
    content_width: f32,
) -> usize {
    let mut rendered = 0;
    // The allocated row and spacing together remain exactly 25 points.
    let row_height = (25.0 - ui.spacing().item_spacing.y).max(1.0);
    egui::ScrollArea::both()
        .id_salt("filter_results_scroll")
        .auto_shrink([false; 2])
        .show_rows(ui, row_height, results.len(), |ui, range| {
            ui.set_min_width(content_width);
            for row in range {
                let index = results[row];
                rendered += 1;
                let entry = &session.logs[index];
                let (id, timestamp, level, message) =
                    (&entry.id, &entry.timestamp, &entry.level, &entry.message);
                let selected = session.selected.contains(id);
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), row_height),
                    egui::Sense::click(),
                );
                let fill = if selected {
                    theme::ACCENT.linear_multiply(0.20)
                } else if response.hovered() {
                    theme::BG_HOVER
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter().rect(
                    rect,
                    0.0,
                    fill,
                    if selected {
                        Stroke::new(1.0_f32, theme::ACCENT.linear_multiply(0.85))
                    } else {
                        Stroke::NONE
                    },
                );
                if selected {
                    ui.painter().vline(
                        rect.left() + 2.0,
                        rect.y_range().shrink(3.0),
                        Stroke::new(3.0_f32, theme::ACCENT),
                    );
                }
                let center_y = rect.center().y;
                ui.painter().text(
                    egui::pos2(rect.left() + 14.0, center_y),
                    egui::Align2::LEFT_CENTER,
                    timestamp,
                    FontId::monospace(9.5),
                    theme::TEXT_MUTED,
                );
                ui.painter().text(
                    egui::pos2(rect.left() + 116.0, center_y),
                    egui::Align2::LEFT_CENTER,
                    level.label(),
                    FontId::monospace(9.5),
                    theme::log_level_color(*level),
                );
                let message_clip = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + 158.0, rect.top()),
                    rect.right_bottom(),
                );
                ui.painter().with_clip_rect(message_clip).text(
                    egui::pos2(rect.left() + 158.0, center_y),
                    egui::Align2::LEFT_CENTER,
                    message,
                    FontId::monospace(10.5),
                    theme::TEXT,
                );
                let response = response
                    .on_hover_text("Jump to this log in the upper pane • Right-click to copy");
                add_copy_row_menu(&response, ctx, entry, &session.logs, &session.selected);
                if response.clicked() {
                    session.selected.clear();
                    session.selected.insert(*id);
                    session.scroll_to_log = Some(*id);
                    session.scroll_jump_settle_frames = 2;
                }
            }
        });
    rendered
}

fn intercept_window_close(session: &mut SessionRuntime, ctx: &egui::Context, now: Instant) {
    if ctx.input(|input| input.viewport().close_requested()) && !session.shutdown.allows_close() {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        session.request_shutdown(now);
    }
}

fn render_shutdown_dialog(session: &mut SessionRuntime, ctx: &egui::Context) {
    if !session.shutdown.active() || session.shutdown.allows_close() {
        return;
    }
    let reason = session.shutdown.decision().map(str::to_owned);
    let unsaved_session = session.shutdown.unsaved_session_confirmation();
    egui::Modal::new(egui::Id::new("orderly_shutdown")).show(ctx, |ui| {
        ui.set_max_width(440.0);
        if unsaved_session {
            ui.heading("Session history has not been saved");
            ui.label("Retained session history has not been saved as a TraceX session. Text autosave/export may exist. Keep open to save, or close without saving the session.");
            ui.horizontal(|ui| {
                if ui.button("Keep open").clicked() {
                    session.shutdown.keep_open();
                }
                if ui.button("Close without saving session").clicked() {
                    session.shutdown.close_without_saving_session();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        } else if let Some(reason) = reason {
            ui.heading("Shutdown incomplete");
            ui.label(reason);
            ui.label("Keep open to review or export pending logs. Capture remains stopped.");
            ui.horizontal(|ui| {
                if ui.button("Keep open").clicked() {
                    session.shutdown.keep_open();
                }
                if ui
                    .button("Force close — pending logs may be lost")
                    .clicked()
                {
                    session.shutdown.force_close();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        } else {
            ui.heading("Finishing capture and saved logs");
            ui.label("Waiting for received logs, autosave and session files to finish.");
            ui.spinner();
            if ui.button("Keep open").clicked() {
                session.shutdown.keep_open();
            }
        }
    });
}

impl TraceXApp {
    // Monitor tabs currently present the same capture; opening a view must not
    // reset logs, selection, or the active transport.
    fn open_monitor_view(&mut self) {
        if let Some(tab) = self.ui_tabs.get_mut(self.active_tab) {
            tab.setup = false;
        }
    }

    fn render_window_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("window_bar")
            .exact_height(WINDOW_BAR_HEIGHT)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(8.0, 0.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().button_padding = Vec2::new(6.0, 2.0);
                ui.spacing_mut().interact_size.y = 20.0;
                ui.spacing_mut().item_spacing.x = 4.0;
                let bar_rect = ui.max_rect();
                let drag_response = ui.interact(
                    bar_rect,
                    ui.id().with("window_drag_region"),
                    egui::Sense::click_and_drag(),
                );
                if drag_response.double_clicked() {
                    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                } else if drag_response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.horizontal(|ui| {
                    for (icon, label) in [
                        (icons::Icon::Panel, "Workspace"),
                        (icons::Icon::Back, "Back"),
                        (icons::Icon::Forward, "Forward"),
                    ] {
                        ui.add_enabled_ui(false, |ui| {
                            icons::button(
                                ui,
                                icon,
                                label,
                                Vec2::new(24.0, WINDOW_BAR_HEIGHT),
                                false,
                            );
                        });
                    }
                    ui.add_space(12.0);
                    ui.scope(|ui| {
                        ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                        ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
                        menus::render_app_menus(&mut self.session, ui, ctx);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let close = icons::button(
                            ui,
                            icons::Icon::Close,
                            "Close",
                            Vec2::new(40.0, WINDOW_BAR_HEIGHT),
                            false,
                        );
                        if close.clicked() {
                            self.session.request_shutdown(Instant::now());
                        }
                        let maximize = icons::button(
                            ui,
                            icons::Icon::Maximize,
                            "Maximize or restore",
                            Vec2::new(40.0, WINDOW_BAR_HEIGHT),
                            false,
                        );
                        if maximize.clicked() {
                            let maximized =
                                ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }
                        let minimize = icons::button(
                            ui,
                            icons::Icon::Minimize,
                            "Minimize",
                            Vec2::new(40.0, WINDOW_BAR_HEIGHT),
                            false,
                        );
                        if minimize.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    });
                });
            });
    }
}

impl eframe::App for TraceXApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_native_ui(ctx);
    }
}

impl TraceXApp {
    fn update_native_ui(&mut self, ctx: &egui::Context) {
        let ports = self
            .ports
            .lock()
            .map(|ports| ports.clone())
            .unwrap_or_default();
        intercept_window_close(&mut self.session, ctx, Instant::now());
        self.session.poll();
        self.session.refresh_query_view();
        if self.session.shutdown.active() {
            if self.session.advance_shutdown(Instant::now()) == shutdown::ShutdownAction::Close {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else {
            self.session.check_auto_reconnect(&ports);
        }
        ctx.request_repaint_after(Duration::from_millis(50));

        let copy_requested =
            ctx.input(|i| (i.modifiers.ctrl || i.modifiers.mac_cmd) && i.key_pressed(egui::Key::C));
        if copy_requested {
            ctx.copy_text(self.session.copy_selected_text());
        }

        self.render_window_bar(ctx);

        sidebar::render_sidebar(&mut self.session, &ports, ctx);

        self.render_tabs(ctx);

        command_bar::render_command_bar(&mut self.session, ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::BG_DARK))
            .show(ctx, |ui| {
                let showing_setup = self
                    .ui_tabs
                    .get(self.active_tab)
                    .map(|tab| tab.setup)
                    .unwrap_or(false);
                if showing_setup {
                    ui.vertical_centered(|ui| {
                        ui.add_space(90.0);
                        ui.label(
                            RichText::new("NEW MONITOR SESSION")
                                .size(18.0)
                                .color(theme::TEXT)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Monitor tabs share the current capture and connection.")
                                .size(11.0)
                                .color(theme::TEXT_DIM),
                        );
                        ui.add_space(20.0);
                        let ready = !self.session.selected_port.is_empty();
                        if ui
                            .add_enabled(
                                ready,
                                egui::Button::new(
                                    RichText::new("OPEN MONITOR")
                                        .size(10.0)
                                        .color(theme::ACCENT)
                                        .strong(),
                                )
                                .min_size(Vec2::new(132.0, 34.0)),
                            )
                            .clicked()
                        {
                            self.open_monitor_view();
                        }
                        ui.add_space(6.0);
                        if ui.small_button("Back to current monitor").clicked() {
                            self.active_tab = 0;
                        }
                    });
                    return;
                }
                render_monitor(&mut self.session, ui, ctx);
            });
        render_shutdown_dialog(&mut self.session, ctx);
    }
}

fn render_monitor(session: &mut SessionRuntime, ui: &mut egui::Ui, ctx: &egui::Context) {
    toolbar::render_toolbar(session, ui, ctx);
    session.refresh_query_view();
    let snapshot = session
        .query_snapshot()
        .filter(|result| result.record_count <= session.logs.len());
    let rows = snapshot
        .as_ref()
        .map_or(&[][..], |result| result.rows.as_slice());
    const LOG_ROW_HEIGHT: f32 = 30.0;
    let jump_target = session.scroll_to_log;
    if jump_target.is_some() {
        session.auto_scroll = false;
    }
    let available_log_height = ui.available_height();
    let results_height = if session.filter_panel_open {
        session
            .filter_results_height
            .clamp(120.0, (available_log_height - 100.0).max(120.0))
    } else {
        0.0
    };
    let upper_height = (ui.available_height()
        - results_height
        - if session.filter_panel_open { 8.0 } else { 0.0 })
    .max(90.0);
    ui.allocate_ui(Vec2::new(ui.available_width(), upper_height), |ui| {
        let row_height_sans_spacing = (LOG_ROW_HEIGHT - ui.spacing().item_spacing.y).max(1.0);
        let target_index = jump_target.and_then(|target_id| {
            rows.iter()
                .position(|&index| session.logs[index].id == target_id)
        });
        if snapshot.is_none() {
            ui.label(session.query_failure().unwrap_or("Searching…"));
            return;
        }
        let content_width = snapshot
            .as_ref()
            .map_or(0.0, |result| result.main_width)
            .max(ui.available_width());
        let mut log_scroll = egui::ScrollArea::both()
            .id_salt("primary_log_scroll")
            .auto_shrink([false; 2])
            .stick_to_bottom(session.auto_scroll);
        if let Some(index) = target_index {
            log_scroll = log_scroll.vertical_scroll_offset(centered_row_scroll_offset(
                index,
                rows.len(),
                LOG_ROW_HEIGHT,
                upper_height,
            ));
        } else if session.live_scroll_frames > 0 {
            log_scroll = log_scroll.vertical_scroll_offset(bottom_scroll_offset(
                rows.len(),
                LOG_ROW_HEIGHT,
                upper_height,
            ));
        }
        log_scroll.show_rows(ui, row_height_sans_spacing, rows.len(), |ui, range| {
            ui.set_min_width(content_width);
            for i in range {
                let entry = &session.logs[rows[i]];
                let (id, ts, level, msg) =
                    (&entry.id, &entry.timestamp, &entry.level, &entry.message);
                let selected = session.selected.contains(id);
                let response = render_primary_log_row(ui, i + 1, ts, *level, msg, selected)
                    .on_hover_text("Click to select • Right-click to copy this row");
                add_copy_row_menu(&response, ctx, entry, &session.logs, &session.selected);
                if response.clicked() {
                    if session.selected.contains(id) {
                        session.selected.remove(id);
                    } else {
                        session.selected.insert(*id);
                    }
                }
            }
        });
        if jump_target.is_some() {
            if session.scroll_jump_settle_frames > 0 {
                session.scroll_jump_settle_frames -= 1;
            }
            if session.scroll_jump_settle_frames == 0 {
                session.scroll_to_log = None;
            }
        }
        if session.live_scroll_frames > 0 {
            session.live_scroll_frames -= 1;
        }
    });
    if session.filter_panel_open {
        let (divider_rect, divider_response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), egui::Sense::drag());
        let divider_response = divider_response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
        let divider_color = if divider_response.hovered() || divider_response.dragged() {
            theme::ACCENT
        } else {
            theme::BORDER
        };
        ui.painter().hline(
            divider_rect.x_range(),
            divider_rect.center().y,
            Stroke::new(1.0_f32, divider_color),
        );
        if divider_response.dragged() {
            let pointer_delta = ctx.input(|input| input.pointer.delta().y);
            session.filter_results_height = (session.filter_results_height - pointer_delta)
                .clamp(120.0, (available_log_height - 100.0).max(120.0));
        }
        let query_empty = snapshot.as_ref().is_some_and(|result| result.query_empty);
        let results = snapshot.as_ref().map_or(&[][..], |result| {
            if session.filter_exclude {
                result.excluded.as_slice()
            } else if result.query_empty {
                &[]
            } else {
                result.rows.as_slice()
            }
        });
        ui.allocate_ui(Vec2::new(ui.available_width(), results_height), |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 30.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("FILTER RESULTS")
                            .monospace()
                            .size(10.0)
                            .color(theme::ACCENT)
                            .strong(),
                    );
                    ui.label(
                        RichText::new(format!("{} matches", results.len()))
                            .monospace()
                            .size(9.0)
                            .color(theme::TEXT_MUTED),
                    );
                    if ui
                        .add_sized(
                            [62.0, 22.0],
                            egui::Button::new(
                                RichText::new("INCLUDE")
                                    .monospace()
                                    .size(9.0)
                                    .color(theme::SUCCESS),
                            )
                            .selected(!session.filter_exclude),
                        )
                        .clicked()
                    {
                        session.filter_exclude = false;
                    }
                    if ui
                        .add_enabled_ui(snapshot.is_some(), |ui| {
                            ui.add_sized(
                                [94.0, 22.0],
                                egui::Button::new(
                                    RichText::new("COPY FILTERED")
                                        .monospace()
                                        .size(9.0)
                                        .color(theme::ACCENT),
                                ),
                            )
                        })
                        .inner
                        .clicked()
                    {
                        ctx.copy_text(session.query_rows_text(results));
                    }
                    if ui
                        .add_sized(
                            [62.0, 22.0],
                            egui::Button::new(
                                RichText::new("EXCLUDE")
                                    .monospace()
                                    .size(9.0)
                                    .color(theme::ERROR),
                            )
                            .selected(session.filter_exclude),
                        )
                        .clicked()
                    {
                        session.filter_exclude = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);
                        if icons::button(
                            ui,
                            icons::Icon::Close,
                            "Close filter results",
                            Vec2::splat(22.0),
                            false,
                        )
                        .clicked()
                        {
                            session.filter_panel_open = false;
                        }
                    });
                },
            );
            ui.separator();

            if snapshot.is_none() {
                ui.label(session.query_failure().unwrap_or("Searching…"));
            } else if query_empty {
                ui.add_space(16.0);
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Enter a search term to find log entries")
                            .size(10.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
            } else if results.is_empty() {
                ui.add_space(16.0);
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(if session.query_pending() {
                            "Searching…"
                        } else {
                            "No matching logs"
                        })
                        .size(10.0)
                        .color(theme::TEXT_MUTED),
                    );
                });
            } else {
                let content_width = snapshot
                    .as_ref()
                    .map_or(0.0, |result| {
                        if session.filter_exclude {
                            result.excluded_width
                        } else {
                            result.main_width
                        }
                    })
                    .max(ui.available_width());
                render_filter_rows(session, ui, ctx, results, content_width);
            }
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ESP-IDF LOG PARSER
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod monitor_view_tests {
    use super::*;

    // No serial enumeration, capture worker, or disk writes. Full update tests
    // may start the existing bounded background query worker.
    fn app_without_io() -> TraceXApp {
        let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())));
        app.session.auto_save_path = None;
        app
    }

    #[test]
    fn shared_setup_view_still_polls_sole_session_before_rendering_setup() {
        let ctx = egui::Context::default();
        let mut app = app_without_io().prepare_native_ui(&ctx);
        let (feed, inbox) = capture::CaptureInbox::test_channel();
        app.session.capture = Some(inbox);
        app.session.command_input = "unsent while setup is open".to_owned();
        app.ui_tabs.push(UiTab {
            title: "Monitor 2".to_owned(),
            setup: true,
        });
        app.active_tab = 1;
        feed.send("queued before setup frame".to_owned()).unwrap();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1200.0, 800.0),
            )),
            ..Default::default()
        };
        let first = ctx.run(input.clone(), |ctx| app.update_native_ui(ctx));
        assert!(first.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Text(text) => text.galley.job.text == "NEW MONITOR SESSION",
            _ => false,
        }));
        assert_eq!(app.active_tab, 1);
        assert!(app.ui_tabs[1].setup);
        assert_eq!(app.session.logs.len(), 1);
        assert_eq!(app.session.logs[0].raw, "queued before setup frame");
        assert_eq!(app.session.next_id, 1);
        assert!(app.session.history_dirty);
        feed.send("queued for the next setup frame".to_owned())
            .unwrap();
        let _ = ctx.run(input, |ctx| app.update_native_ui(ctx));
        assert_eq!(app.session.logs.len(), 2);
        assert_eq!(app.session.logs[1].raw, "queued for the next setup frame");
        assert_eq!(app.session.command_input, "unsent while setup is open");
    }

    #[test]
    fn prepared_creation_path_renders_named_fonts_on_first_actual_update() {
        let ctx = egui::Context::default();
        // Use the same preparation method as eframe's CreationContext callback,
        // then execute the complete update path used by eframe::App::update.
        let mut app = app_without_io().prepare_native_ui(&ctx);
        app.session.command_input = "unsent command".to_owned();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1200.0, 800.0),
            )),
            ..Default::default()
        };
        let first = ctx.run(input.clone(), |ctx| app.update_native_ui(ctx));
        assert!(
            first.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Text(text) => {
                    text.galley.job.text == "TraceX"
                        && text
                            .galley
                            .job
                            .sections
                            .iter()
                            .any(|section| section.format.font_id == theme::medium_font(31.0))
                }
                _ => false,
            }),
            "the first update must render the named-font wordmark"
        );
        let _ = ctx.run(input, |ctx| app.update_native_ui(ctx));
        assert_eq!(app.session.command_input, "unsent command");
        assert!(app.session.logs.is_empty());
        assert!(!app.session.connected);
    }

    #[test]
    fn secondary_results_render_only_visible_rows_from_large_snapshot() {
        let mut app = app_without_io();
        app.session.logs = (0..10_000)
            .map(|id| LogEntry::new(id, format!("record {id}"), "12:00:00").into())
            .collect();
        let results: Vec<_> = (0..app.session.logs.len()).collect();
        app.session.selected.insert(9_999);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 300.0),
            )),
            ..Default::default()
        };
        let mut rendered = 0;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                rendered = render_filter_rows(&mut app.session, ui, ctx, &results, 800.0);
            });
        });
        assert!(rendered > 0);
        assert!(
            rendered < 100,
            "a 300-point viewport must not render all 10,000 results: {rendered}"
        );
        assert!(app.session.selected.contains(&9_999));
    }

    #[test]
    fn os_close_is_cancelled_until_shutdown_is_explicitly_ready() {
        let mut app = app_without_io();
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        let output = ctx.run(input.clone(), |ctx| {
            intercept_window_close(&mut app.session, ctx, Instant::now())
        });
        assert!(app.session.shutdown.active());
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
        );
        app.session.shutdown.force_close();
        let output = ctx.run(input, |ctx| {
            intercept_window_close(&mut app.session, ctx, Instant::now())
        });
        assert!(
            !output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
        );
    }
    #[test]
    fn opening_monitor_preserves_shared_capture_and_selection() {
        let mut app = app_without_io();
        app.session.logs.push(Arc::new(LogEntry::new(
            7,
            "captured before opening".to_owned(),
            "12:00:00",
        )));
        app.session.next_id = 8;
        app.session.selected.insert(7);
        app.session.selected_port = "test-device".to_owned();
        app.ui_tabs.push(UiTab {
            title: "Monitor 2".to_owned(),
            setup: true,
        });
        app.active_tab = 1;

        app.open_monitor_view();
        app.open_monitor_view();

        assert!(!app.ui_tabs[1].setup);
        assert_eq!(app.session.logs.len(), 1);
        assert_eq!(app.session.logs[0].id, 7);
        assert_eq!(app.session.logs[0].raw, "captured before opening");
        assert_eq!(app.session.next_id, 8);
        assert!(app.session.selected.contains(&7));
        assert_eq!(app.session.selected_port, "test-device");
    }
}
