use super::super::*;
use super::icons::{self, Icon};

impl TraceXApp {
    pub(super) fn render_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
    ) -> [egui::Rect; 8] {
        let mut controls = [egui::Rect::NOTHING; 8];
        let previous_spacing = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::Frame::none()
            .fill(theme::BG_PANEL)
            .inner_margin(egui::Margin::symmetric(16.0, 0.0))
            .show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 52.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let search_width = (ui.available_width() - 370.0).clamp(140.0, 256.0);
                        let (search_slot, _) = ui.allocate_exact_size(
                            Vec2::new(search_width, 34.0),
                            egui::Sense::hover(),
                        );
                        controls[0] = render_search(ui, &mut self.search, search_slot);
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);
                        if ui.available_width() < 410.0 {
                            controls[1] = ui
                                .menu_button("Levels", |ui| {
                                    ui.checkbox(&mut self.show_info, "INFO");
                                    ui.checkbox(&mut self.show_warn, "WARN");
                                    ui.checkbox(&mut self.show_error, "ERROR");
                                })
                                .response
                                .rect;
                        } else {
                            controls[1] =
                                severity_chip(ui, "INFO", &mut self.show_info, theme::INFO, false);
                            controls[2] = severity_chip(
                                ui,
                                "WARN",
                                &mut self.show_warn,
                                theme::WARNING,
                                true,
                            );
                            controls[3] = severity_chip(
                                ui,
                                "ERROR",
                                &mut self.show_error,
                                theme::ERROR,
                                false,
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let action = icons::button(
                                ui,
                                Icon::Copy,
                                "Copy all retained logs",
                                Vec2::splat(28.0),
                                false,
                            );
                            controls[4] = action.rect;
                            if action.clicked() {
                                self.copy_all(ctx);
                            }
                            let action = icons::button(
                                ui,
                                Icon::Trash,
                                "Clear retained logs",
                                Vec2::splat(28.0),
                                false,
                            );
                            controls[5] = action.rect;
                            if action.clicked() {
                                self.clear();
                            }
                            let action = icons::button(
                                ui,
                                Icon::Follow,
                                "Follow incoming logs",
                                Vec2::splat(28.0),
                                self.auto_scroll,
                            );
                            controls[6] = action.rect;
                            if action.clicked() {
                                self.auto_scroll = !self.auto_scroll;
                                self.live_scroll_frames =
                                    live_scroll_frames_after_toggle(true, self.auto_scroll);
                            }
                            let action = icons::button(
                                ui,
                                Icon::Export,
                                "Export filtered text",
                                Vec2::splat(28.0),
                                false,
                            );
                            controls[7] = action.rect;
                            if action.clicked() {
                                self.export_logs();
                            }
                            if ui.available_width() > 150.0 && !self.logs.is_empty() {
                                ui.label(
                                    RichText::new(format!(
                                        "{:.0}/s · {} lines",
                                        self.rate,
                                        self.logs.len(),
                                    ))
                                    .size(10.0)
                                    .color(theme::TEXT_MUTED),
                                )
                                .on_hover_text(
                                    "Retained record count; search updates in the background",
                                );
                            }
                        });
                    },
                );
                render_query_status(ui, self.query_pending(), self.query_failure());
            });
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0_f32, theme::BORDER),
        );
        ui.spacing_mut().item_spacing.y = previous_spacing;
        controls
    }
}

fn render_search(ui: &mut egui::Ui, search: &mut String, slot: egui::Rect) -> egui::Rect {
    ui.painter().rect(
        slot,
        4.0,
        theme::BG_INPUT,
        Stroke::new(1.0_f32, theme::BORDER),
    );
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(slot.left() + 16.0, slot.center().y),
        Vec2::splat(14.0),
    );
    icons::paint(ui, icon_rect, Icon::Search, theme::TEXT_MUTED);
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(slot.left() + 32.0, slot.top() + 7.0),
        egui::pos2(slot.right() - 8.0, slot.bottom() - 7.0),
    );
    let response = ui.put(
        text_rect,
        egui::TextEdit::singleline(search)
            .font(FontId::proportional(14.0))
            .margin(egui::Margin::ZERO)
            .hint_text("Search")
            .frame(false),
    );
    if response.has_focus() {
        ui.painter()
            .rect_stroke(slot, 4.0, Stroke::new(1.0_f32, theme::TEXT_DIM));
    }
    slot
}

// Operational state must remain visible even when the compact toolbar hides counts.
// A failed refresh can retain a valid older snapshot, so the log pane is not a
// reliable place to infer that current search results are incomplete.
fn render_query_status(ui: &mut egui::Ui, pending: bool, failure: Option<&str>) {
    if let Some(failure) = failure {
        ui.add(
            egui::Label::new(
                RichText::new(format!("Search failed: {failure}"))
                    .size(12.0)
                    .color(theme::ERROR),
            )
            .wrap(),
        );
    } else if pending {
        ui.add(
            egui::Label::new(
                RichText::new("Updating search results…")
                    .size(12.0)
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
    }
}

fn severity_chip(
    ui: &mut egui::Ui,
    label: &str,
    selected: &mut bool,
    color: Color32,
    warning: bool,
) -> egui::Rect {
    let selected_fill = match label {
        "INFO" => theme::INFO_PILL,
        "WARN" => theme::WARN_PILL,
        _ => theme::ERROR_PILL,
    };
    let selected_border = if warning {
        theme::BORDER
    } else {
        selected_fill.gamma_multiply(1.4)
    };
    let response = ui.add_sized(
        [if label == "ERROR" { 78.0 } else { 70.0 }, 26.0],
        egui::Button::new("")
            .small()
            .fill(if *selected {
                selected_fill
            } else {
                theme::BG_INPUT
            })
            .stroke(Stroke::new(
                1.0_f32,
                if *selected {
                    selected_border
                } else {
                    theme::BORDER
                },
            ))
            .rounding(12.0),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *selected,
            label,
        )
    });
    if response.clicked() {
        *selected = !*selected;
    }
    if response.has_focus() {
        ui.painter()
            .rect_stroke(response.rect, 12.0, Stroke::new(1.0_f32, theme::TEXT));
    }
    let color = if *selected { color } else { theme::TEXT_MUTED };
    let center = egui::pos2(response.rect.left() + 16.0, response.rect.center().y);
    if warning {
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(0.0, -4.0),
                center + egui::vec2(4.0, 3.0),
                center + egui::vec2(-4.0, 3.0),
            ],
            color,
            Stroke::NONE,
        ));
    } else {
        ui.painter().circle_filled(center, 3.5, color);
    }
    ui.painter().text(
        center + egui::vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        theme::medium_font(12.0),
        if warning && *selected {
            theme::TEXT_DIM
        } else {
            color
        },
    );
    let rect = response.rect;
    response.on_hover_text(format!(
        "{} {} records",
        if *selected { "Hide" } else { "Show" },
        label
    ));
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_severity_pills_use_opaque_dark_surfaces() {
        for (label, color, warning, expected) in [
            ("INFO", theme::INFO, false, theme::INFO_PILL),
            ("WARN", theme::WARNING, true, theme::WARN_PILL),
            ("ERROR", theme::ERROR, false, theme::ERROR_PILL),
        ] {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let mut rect = egui::Rect::NOTHING;
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    rect = severity_chip(ui, label, &mut true, color, warning);
                });
            });
            let surface = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(shape) if shape.rect == rect => Some(shape.fill),
                    _ => None,
                })
                .expect("severity control background must render");
            assert_eq!(surface, expected);
            assert_eq!(
                surface.a(),
                255,
                "pill fill must not depend on alpha blending"
            );
        }
    }

    #[test]
    fn toolbar_stays_at_approved_52_point_height_without_status() {
        let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())));
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1024.0, 600.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let top = ui.cursor().top();
                    app.render_toolbar(ui, ctx);
                    assert!(
                        (ui.cursor().top() - top - 52.0).abs() < 0.1,
                        "toolbar includes unexpected vertical spacing"
                    );
                });
            },
        );
    }

    #[test]
    fn search_failure_and_pending_status_remain_visible_at_narrow_widths() {
        for width in [140.0, 480.0, 900.0] {
            for (pending, failure, expected) in [
                (true, None, "Updating search results…"),
                (
                    true,
                    Some("Worker stopped; captured logs are retained."),
                    "Search failed: Worker stopped; captured logs are retained.",
                ),
            ] {
                let ctx = egui::Context::default();
                theme::setup(&ctx);
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(width, 300.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .show(ctx, |ui| render_query_status(ui, pending, failure));
                    },
                );
                let status = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Text(text) if text.galley.text() == expected => {
                            Some(text)
                        }
                        _ => None,
                    })
                    .expect(
                        "operational search state must produce visible text regardless of width",
                    );
                assert!(
                    status.galley.size().x <= width,
                    "status must wrap inside the viewport"
                );
            }
        }
    }

    #[test]
    fn search_levels_and_actions_fit_minimum_and_regular_clients() {
        for width in [800.0, 1200.0, 1280.0] {
            let mut app = TraceXApp::with_ports(Arc::new(Mutex::new(Vec::new())));
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 600.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::SidePanel::left("sidebar")
                        .exact_width(256.0)
                        .show(ctx, |_| {});
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let available = ui.available_rect_before_wrap();
                        let controls = app.render_toolbar(ui, ctx);
                        if controls[2].is_positive() {
                            for chip in &controls[1..4] {
                                assert!(
                                    (chip.height() - 26.0).abs() < 0.1,
                                    "severity returned height must match requested 26 points"
                                );
                                assert!(
                                    (chip.center().y - controls[0].center().y).abs() < 0.1,
                                    "search outer frame and severity pills must share a centerline"
                                );
                            }
                        }

                        let active: Vec<_> =
                            controls.into_iter().filter(|r| r.is_positive()).collect();
                        for (i, rect) in active.iter().enumerate() {
                            assert!(
                                available.contains_rect(*rect),
                                "control {rect:?} exceeds {available:?} at {width}"
                            );
                            for other in active.iter().skip(i + 1) {
                                assert!(
                                    !rect.intersects(*other),
                                    "controls overlap at {width}: {rect:?}, {other:?}"
                                );
                            }
                        }
                    });
                },
            );
        }
    }
}
