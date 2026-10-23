use super::*;

mod command_bar;
mod sidebar;
mod tabs;

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
                    .fill(theme::BG_INPUT)
                    .inner_margin(egui::Margin::symmetric(8.0, 0.0)),
            )
            .show(ctx, |ui| {
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
                    for icon in ["▣", "←", "→"] {
                        ui.add_sized(
                            [32.0, WINDOW_BAR_HEIGHT],
                            egui::Button::new(
                                RichText::new(icon).size(16.0).color(theme::TEXT_DIM),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                        );
                    }
                    ui.add_space(12.0);
                    for item in ["File", "Edit", "View", "Help"] {
                        ui.add_sized(
                            [44.0, WINDOW_BAR_HEIGHT],
                            egui::Button::new(
                                RichText::new(item).size(12.0).color(theme::TEXT_DIM),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let close = ui.add_sized(
                            [44.0, WINDOW_BAR_HEIGHT],
                            egui::Button::new(RichText::new("×").size(17.0).color(theme::TEXT_DIM))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        );
                        if close.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let maximize = ui.add_sized(
                            [44.0, WINDOW_BAR_HEIGHT],
                            egui::Button::new(RichText::new("□").size(14.0).color(theme::TEXT_DIM))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        );
                        if maximize.clicked() {
                            let maximized =
                                ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }
                        let minimize = ui.add_sized(
                            [44.0, WINDOW_BAR_HEIGHT],
                            egui::Button::new(RichText::new("—").size(14.0).color(theme::TEXT_DIM))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
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
        if !self.theme_ready {
            theme::setup(ctx);
            self.theme_ready = true;
        }
        self.poll();
        self.check_auto_reconnect();
        ctx.request_repaint_after(Duration::from_millis(50));

        let copy_requested =
            ctx.input(|i| (i.modifiers.ctrl || i.modifiers.mac_cmd) && i.key_pressed(egui::Key::C));
        if copy_requested {
            self.copy_selected(ctx);
        }

        self.render_window_bar(ctx);

        self.render_sidebar(ctx);

        self.render_tabs(ctx);

        self.render_command_bar(ctx);

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
                        let ready = !self.selected_port.is_empty();
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
                let previous_item_spacing_y = ui.spacing().item_spacing.y;
                ui.spacing_mut().item_spacing.y = 0.0;
                egui::Frame::none()
                    .fill(theme::BG_PANEL)
                    .inner_margin(egui::Margin::symmetric(16.0, 0.0))
                    .show(ui, |ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), 52.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add_sized(
                                    [280.0, 34.0],
                                    egui::TextEdit::singleline(&mut self.search)
                                        .font(FontId::proportional(13.0))
                                        .vertical_align(egui::Align::Center)
                                        .margin(egui::Margin::symmetric(10.0, 6.0))
                                        .hint_text("Search logs"),
                                );
                                ui.add_space(12.0);
                                ui.separator();
                                ui.add_space(12.0);
                                if ui
                                    .add_sized(
                                        [66.0, 30.0],
                                        egui::Button::new(
                                            RichText::new("⊙  INFO")
                                                .monospace()
                                                .size(9.0)
                                                .color(theme::INFO)
                                                .strong(),
                                        )
                                        .fill(if self.show_info {
                                            theme::tint(theme::INFO, 24)
                                        } else {
                                            theme::BG_INPUT
                                        })
                                                .stroke(Stroke::new(1.0_f32, theme::tint(theme::INFO, 92)))
                                        .rounding(4.0),
                                    )
                                    .clicked()
                                {
                                    self.show_info = !self.show_info;
                                }
                                if ui
                                    .add_sized(
                                        [68.0, 30.0],
                                        egui::Button::new(
                                            RichText::new("⊙  WARN")
                                                .monospace()
                                                .size(9.0)
                                                .color(theme::WARNING)
                                                .strong(),
                                        )
                                        .fill(if self.show_warn {
                                            theme::tint(theme::WARNING, 24)
                                        } else {
                                            theme::BG_INPUT
                                        })
                                                .stroke(Stroke::new(1.0_f32, theme::tint(theme::WARNING, 92)))
                                        .rounding(4.0),
                                    )
                                    .clicked()
                                {
                                    self.show_warn = !self.show_warn;
                                }
                                if ui
                                    .add_sized(
                                        [72.0, 30.0],
                                        egui::Button::new(
                                            RichText::new("⊙  ERROR")
                                                .monospace()
                                                .size(9.0)
                                                .color(theme::ERROR)
                                                .strong(),
                                        )
                                        .fill(if self.show_error {
                                            theme::tint(theme::ERROR, 24)
                                        } else {
                                            theme::BG_INPUT
                                        })
                                                .stroke(Stroke::new(1.0_f32, theme::tint(theme::ERROR, 92)))
                                        .rounding(4.0),
                                    )
                                    .clicked()
                                {
                                    self.show_error = !self.show_error;
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add_sized(
                                                [36.0, 32.0],
                                                egui::Button::new(
                                                    RichText::new("▤")
                                                        .size(15.0)
                                                        .color(theme::ERROR),
                                                )
                                                .rounding(4.0),
                                            )
                                            .on_hover_text("Clear logs")
                                            .clicked()
                                        {
                                            self.clear();
                                        }
                                        if ui
                                            .add_sized(
                                                [36.0, 32.0],
                                                egui::Button::new(
                                                    RichText::new("▣")
                                                        .size(14.0)
                                                        .color(theme::TEXT_DIM),
                                                )
                                                .rounding(4.0),
                                            )
                                            .on_hover_text("Copy all logs")
                                            .clicked()
                                        {
                                            self.copy_all(ctx);
                                        }
                                        let follow_clicked = ui
                                            .add_sized(
                                                [36.0, 32.0],
                                                egui::Button::new(
                                                    RichText::new("↓")
                                                        .size(16.0)
                                                        .color(if self.auto_scroll {
                                                            theme::ACCENT
                                                        } else {
                                                            theme::TEXT_MUTED
                                                        }),
                                                )
                                                .fill(theme::BG_INPUT)
                                            .stroke(Stroke::new(1.0_f32, theme::BORDER)),
                                            )
                                            .on_hover_text("Follow incoming logs")
                                            .clicked();
                                        if follow_clicked {
                                            self.auto_scroll = !self.auto_scroll;
                                            self.live_scroll_frames =
                                                live_scroll_frames_after_toggle(true, self.auto_scroll);
                                        }
                                        ui.label(
                                            RichText::new(format!(
                                                "{:.0}/s  ·  {} lines",
                                                self.rate,
                                                self.logs.len()
                                            ))
                                            .monospace()
                                            .size(9.0)
                                            .color(theme::TEXT_MUTED),
                                        );
                                    },
                                );
                            },
                        );
                    });
                ui.separator();
                ui.spacing_mut().item_spacing.y = previous_item_spacing_y;

                let rows = self.query_indices(QueryScope::Main);
                let selected_copy = selected_log_text(&self.logs, &self.selected);
                const LOG_ROW_HEIGHT: f32 = 30.0;
                let jump_target = self.scroll_to_log;
                if jump_target.is_some() {
                    self.auto_scroll = false;
                }
                let available_log_height = ui.available_height();
                let results_height = if self.filter_panel_open {
                    self.filter_results_height
                        .clamp(120.0, (available_log_height - 100.0).max(120.0))
                } else {
                    0.0
                };
                let upper_height = (ui.available_height()
                    - results_height
                    - if self.filter_panel_open { 8.0 } else { 0.0 })
                .max(90.0);
                ui.allocate_ui(Vec2::new(ui.available_width(), upper_height), |ui| {
                    let row_height_sans_spacing =
                        (LOG_ROW_HEIGHT - ui.spacing().item_spacing.y).max(1.0);
                    let target_index = jump_target.and_then(|target_id| {
                        rows.iter().position(|&index| self.logs[index].id == target_id)
                    });
                    let content_width = log_content_width(
                        rows.iter().map(|&index| self.logs[index].message.as_str()),
                        ui.available_width(),
                    );
                    let mut log_scroll = egui::ScrollArea::both()
                        .id_salt("primary_log_scroll")
                        .auto_shrink([false; 2])
                        .stick_to_bottom(self.auto_scroll);
                    if let Some(index) = target_index {
                        log_scroll = log_scroll.vertical_scroll_offset(centered_row_scroll_offset(
                            index,
                            rows.len(),
                            LOG_ROW_HEIGHT,
                            upper_height,
                        ));
                    } else if self.live_scroll_frames > 0 {
                        log_scroll = log_scroll.vertical_scroll_offset(bottom_scroll_offset(
                            rows.len(),
                            LOG_ROW_HEIGHT,
                            upper_height,
                        ));
                    }
                    log_scroll.show_rows(
                        ui,
                        row_height_sans_spacing,
                        rows.len(),
                        |ui, range| {
                            ui.set_min_width(content_width);
                            for i in range {
                                let entry = &self.logs[rows[i]];
                                let (id, ts, level, msg) = (&entry.id, &entry.timestamp, &entry.level, &entry.message);
                                let selected = self.selected.contains(id);
                                let line = format_log_line(ts, *level, msg);
                                let response =
                                    render_primary_log_row(ui, i + 1, ts, *level, msg, selected)
                                        .on_hover_text(
                                            "Click to select • Right-click to copy this row",
                                        );
                                add_copy_row_menu(
                                    &response,
                                    ctx,
                                    &line,
                                    selected_copy.as_deref(),
                                    self.selected.len(),
                                );
                                if response.clicked() {
                                    if self.selected.contains(id) {
                                        self.selected.remove(id);
                                    } else {
                                        self.selected.insert(*id);
                                    }
                                }
                            }
                        },
                    );
                    if jump_target.is_some() {
                        if self.scroll_jump_settle_frames > 0 {
                            self.scroll_jump_settle_frames -= 1;
                        }
                        if self.scroll_jump_settle_frames == 0 {
                            self.scroll_to_log = None;
                        }
                    }
                    if self.live_scroll_frames > 0 {
                        self.live_scroll_frames -= 1;
                    }
                });
                if self.filter_panel_open {
                    let (divider_rect, divider_response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 8.0),
                        egui::Sense::drag(),
                    );
                    let divider_response =
                        divider_response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
                    let divider_color = if divider_response.hovered() || divider_response.dragged()
                    {
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
                        self.filter_results_height = (self.filter_results_height - pointer_delta)
                            .clamp(120.0, (available_log_height - 100.0).max(120.0));
                    }
                    let query_empty = self.log_query().is_empty();
                    let scope = if self.filter_exclude { QueryScope::Exclude } else { QueryScope::Include };
                    let results = self.query_indices(scope);
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
                                        .selected(!self.filter_exclude),
                                    )
                                    .clicked()
                                {
                                    self.filter_exclude = false;
                                }
                                if ui
                                    .add_sized(
                                        [94.0, 22.0],
                                        egui::Button::new(
                                            RichText::new("COPY FILTERED")
                                                .monospace()
                                                .size(9.0)
                                                .color(theme::ACCENT),
                                        ),
                                    )
                                    .clicked()
                                {
                                    ctx.copy_text(format_log_entries(results.iter().map(|&index| &self.logs[index])));
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
                                        .selected(self.filter_exclude),
                                    )
                                    .clicked()
                                {
                                    self.filter_exclude = true;
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.add_space(8.0);
                                        if ui.small_button("×").clicked() {
                                            self.filter_panel_open = false;
                                        }
                                    },
                                );
                            },
                        );
                        ui.separator();

                        if query_empty {
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
                                    RichText::new("No matching logs")
                                        .size(10.0)
                                        .color(theme::TEXT_MUTED),
                                );
                            });
                        } else {
                            let content_width = log_content_width(
                                results.iter().map(|&index| self.logs[index].message.as_str()),
                                ui.available_width(),
                            );
                            egui::ScrollArea::both()
                                .id_salt("filter_results_scroll")
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    ui.set_min_width(content_width);
                                    for &index in &results {
                                        let entry = &self.logs[index];
                                        let (id, timestamp, level, message) = (&entry.id, &entry.timestamp, &entry.level, &entry.message);
                                        let selected = self.selected.contains(id);
                                        let (rect, response) = ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), 25.0),
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
                                                Stroke::new(
                                                    1.0_f32,
                                                    theme::ACCENT.linear_multiply(0.85),
                                                )
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
                                        let response = response.on_hover_text(
                                            "Jump to this log in the upper pane • Right-click to copy",
                                        );
                                        let line = format_log_line(timestamp, *level, message);
                                        add_copy_row_menu(
                                            &response,
                                            ctx,
                                            &line,
                                            selected_copy.as_deref(),
                                            self.selected.len(),
                                        );
                                        if response.clicked() {
                                            self.selected.clear();
                                            self.selected.insert(*id);
                                            self.scroll_to_log = Some(*id);
                                            self.scroll_jump_settle_frames = 2;
                                        }
                                    }
                                });
                        }
                    });
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

    // No serial enumeration, worker thread, user directories, or disk access.
    fn app_without_io() -> TraceXApp {
        TraceXApp {
            ports: Arc::new(Mutex::new(Vec::new())),
            selected_port: String::new(),
            baud_rate: 115200,

            connected: false,
            paused: false,
            serial_tx: None,
            log_rx: None,
            stop_flag: None,
            last_connected_port: None,

            logs: Vec::new(),
            next_id: 0,

            command_input: String::new(),
            command_history: Vec::new(),
            send_newline: true,
            auto_clear_command: true,

            search: String::new(),
            selected: HashSet::new(),
            filter_panel_open: false,
            filter_exclude: false,
            filter_results_height: 180.0,
            scroll_to_log: None,
            scroll_jump_settle_frames: 0,

            auto_save_enabled: false,
            auto_save_path: None,
            auto_save_current_path: None,
            auto_save: AutosaveController::default(),
            auto_save_limit_mb: 25,
            auto_save_bytes_written: 0,
            auto_save_sequence: 0,
            auto_save_error: None,
            operation_error: None,

            auto_scroll: true,
            live_scroll_frames: 0,
            ui_tabs: vec![UiTab {
                title: "Monitor 1".to_string(),
                setup: false,
            }],
            active_tab: 0,
            show_info: true,
            show_warn: true,
            show_error: true,
            theme_ready: false,

            rate: 0.0,
            rate_count: 0,
            rate_timer: Instant::now(),

            reconnect_cooldown: Instant::now(),
        }
    }

    #[test]
    fn opening_monitor_preserves_shared_capture_and_selection() {
        let mut app = app_without_io();
        app.logs.push(LogEntry::new(
            7,
            "captured before opening".to_owned(),
            "12:00:00",
        ));
        app.next_id = 8;
        app.selected.insert(7);
        app.selected_port = "test-device".to_owned();
        app.ui_tabs.push(UiTab {
            title: "Monitor 2".to_owned(),
            setup: true,
        });
        app.active_tab = 1;

        app.open_monitor_view();
        app.open_monitor_view();

        assert!(!app.ui_tabs[1].setup);
        assert_eq!(app.logs.len(), 1);
        assert_eq!(app.logs[0].id, 7);
        assert_eq!(app.logs[0].raw, "captured before opening");
        assert_eq!(app.next_id, 8);
        assert!(app.selected.contains(&7));
        assert_eq!(app.selected_port, "test-device");
    }
}
