use super::*;

mod command_bar;
mod sidebar;
mod tabs;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.check_auto_reconnect();
        ctx.request_repaint_after(Duration::from_millis(50));

        let copy_requested =
            ctx.input(|i| (i.modifiers.ctrl || i.modifiers.mac_cmd) && i.key_pressed(egui::Key::C));
        if copy_requested {
            self.copy_selected(ctx);
        }

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
                            RichText::new("Choose a port and baud rate from the scrollable lists.")
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
                            self.clear();
                            if let Some(tab) = self.ui_tabs.get_mut(self.active_tab) {
                                tab.title = format!("{} / MONITOR", self.selected_port);
                                tab.setup = false;
                            }
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
                    .fill(theme::BG_DARK)
                    .inner_margin(egui::Margin::symmetric(12.0, 0.0))
                    .show(ui, |ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(ui.available_width(), 40.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add_sized(
                                    [240.0, 28.0],
                                    egui::TextEdit::singleline(&mut self.search)
                                        .font(FontId::monospace(10.0))
                                        .vertical_align(egui::Align::Center)
                                        .margin(egui::Margin::symmetric(8.0, 4.0))
                                        .hint_text("Search log output…"),
                                );
                                ui.add_space(6.0);
                                if ui
                                    .add_sized(
                                        [62.0, 28.0],
                                        egui::Button::new(
                                            RichText::new("FILTER")
                                                .monospace()
                                                .size(10.0)
                                                .color(theme::ACCENT)
                                                .strong(),
                                        )
                                        .selected(self.filter_panel_open)
                                        .rounding(5.0),
                                    )
                                    .clicked()
                                {
                                    self.filter_panel_open = !self.filter_panel_open;
                                }
                                ui.add_space(4.0);
                                if ui
                                    .add_sized(
                                        [52.0, 28.0],
                                        egui::Button::new(
                                            RichText::new("INFO")
                                                .monospace()
                                                .size(10.0)
                                                .color(theme::INFO)
                                                .strong(),
                                        )
                                        .selected(self.show_info)
                                        .rounding(5.0),
                                    )
                                    .clicked()
                                {
                                    self.show_info = !self.show_info;
                                }
                                if ui
                                    .add_sized(
                                        [52.0, 28.0],
                                        egui::Button::new(
                                            RichText::new("WARN")
                                                .monospace()
                                                .size(10.0)
                                                .color(theme::WARNING)
                                                .strong(),
                                        )
                                        .selected(self.show_warn)
                                        .rounding(5.0),
                                    )
                                    .clicked()
                                {
                                    self.show_warn = !self.show_warn;
                                }
                                if ui
                                    .add_sized(
                                        [56.0, 28.0],
                                        egui::Button::new(
                                            RichText::new("ERROR")
                                                .monospace()
                                                .size(10.0)
                                                .color(theme::ERROR)
                                                .strong(),
                                        )
                                        .selected(self.show_error)
                                        .rounding(5.0),
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
                                                [52.0, 28.0],
                                                egui::Button::new(
                                                    RichText::new("CLEAR")
                                                        .monospace()
                                                        .size(10.0)
                                                        .color(theme::ERROR),
                                                )
                                                .rounding(5.0),
                                            )
                                            .clicked()
                                        {
                                            self.clear();
                                        }
                                        if ui
                                            .add_sized(
                                                [68.0, 28.0],
                                                egui::Button::new(
                                                    RichText::new("COPY ALL")
                                                        .monospace()
                                                        .size(10.0)
                                                        .color(theme::TEXT_DIM),
                                                )
                                                .rounding(5.0),
                                            )
                                            .clicked()
                                        {
                                            self.copy_all(ctx);
                                        }
                                        ui.label(
                                            RichText::new(format!(
                                                "{:.0}/s  •  {} lines",
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

                let rows: Vec<_> = self
                    .logs
                    .iter()
                    .filter(|e| self.level_visible(e.level))
                    .map(|e| {
                        (
                            e.id,
                            e.timestamp.clone(),
                            e.level,
                            e.message.clone(),
                            self.selected.contains(&e.id),
                        )
                    })
                    .collect();
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
                        rows.iter().position(|(id, _, _, _, _)| *id == target_id)
                    });
                    let content_width = log_content_width(
                        rows.iter().map(|(_, _, _, message, _)| message.as_str()),
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
                                let (id, ts, level, msg, selected) = &rows[i];
                                let line = format_log_line(ts, *level, msg);
                                let response =
                                    render_primary_log_row(ui, i + 1, ts, *level, msg, *selected)
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
                        Stroke::new(1.0, divider_color),
                    );
                    if divider_response.dragged() {
                        let pointer_delta = ctx.input(|input| input.pointer.delta().y);
                        self.filter_results_height = (self.filter_results_height - pointer_delta)
                            .clamp(120.0, (available_log_height - 100.0).max(120.0));
                    }
                    let query = self.search.trim().to_lowercase();
                    let results: Vec<_> = self
                        .logs
                        .iter()
                        .filter(|entry| {
                            if !self.level_visible(entry.level) || query.is_empty() {
                                return false;
                            }
                            let contains = entry.message.to_lowercase().contains(&query)
                                || entry.raw.to_lowercase().contains(&query);
                            if self.filter_exclude {
                                !contains
                            } else {
                                contains
                            }
                        })
                        .map(|entry| {
                            (
                                entry.id,
                                entry.timestamp.clone(),
                                entry.level,
                                entry.message.clone(),
                            )
                        })
                        .collect();

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
                                    ctx.copy_text(format_log_rows(results.iter().map(
                                        |(_, timestamp, level, message)| {
                                            (timestamp.as_str(), *level, message.as_str())
                                        },
                                    )));
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

                        if query.is_empty() {
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
                                results.iter().map(|(_, _, _, message)| message.as_str()),
                                ui.available_width(),
                            );
                            egui::ScrollArea::both()
                                .id_salt("filter_results_scroll")
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    ui.set_min_width(content_width);
                                    for (id, timestamp, level, message) in &results {
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
                                                    1.0,
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
                                                Stroke::new(3.0, theme::ACCENT),
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
                                            level.color(),
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
