use super::super::*;

impl App {
    pub(super) fn render_tabs(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("tabs")
            .exact_height(TOP_HEADER_HEIGHT)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_PANEL)
                    .rounding(egui::Rounding {
                        nw: 0.0,
                        ne: APP_ROUNDING,
                        sw: 0.0,
                        se: 0.0,
                    })
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(ctx, |ui| {
                const TAB_HEIGHT: f32 = 34.0;
                let top_padding = ((ui.available_height() - TAB_HEIGHT) * 0.5).max(0.0);
                ui.add_space(top_padding);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    let tabs: Vec<_> = self
                        .ui_tabs
                        .iter()
                        .enumerate()
                        .map(|(index, tab)| (index, tab.title.clone()))
                        .collect();
                    let mut close_tab = None;
                    for (index, title) in tabs {
                        let active = self.active_tab == index;
                        ui.push_id(("monitor_tab", index), |ui| {
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(132.0, TAB_HEIGHT),
                                egui::Sense::click(),
                            );
                            let fill = if active {
                                theme::BG_PANEL
                            } else if response.hovered() {
                                theme::BG_HOVER
                            } else {
                                Color32::TRANSPARENT
                            };
                            let stroke = if active {
                                Stroke::new(1.0, theme::BORDER)
                            } else {
                                Stroke::NONE
                            };
                            ui.painter().rect(rect, 5.0, fill, stroke);
                            ui.painter().text(
                                egui::pos2(rect.center().x - 5.0, rect.center().y),
                                egui::Align2::CENTER_CENTER,
                                title,
                                FontId::monospace(10.0),
                                if active { theme::TEXT } else { theme::TEXT_DIM },
                            );
                            ui.painter().text(
                                egui::pos2(rect.right() - 15.0, rect.center().y),
                                egui::Align2::CENTER_CENTER,
                                "×",
                                FontId::proportional(14.0),
                                theme::TEXT_MUTED,
                            );

                            let over_close = response
                                .hover_pos()
                                .map(|position| position.x >= rect.right() - 30.0)
                                .unwrap_or(false);
                            if over_close {
                                response.clone().on_hover_text("Close tab");
                            }
                            if response.clicked() {
                                if response
                                    .interact_pointer_pos()
                                    .map(|position| position.x >= rect.right() - 30.0)
                                    .unwrap_or(false)
                                {
                                    close_tab = Some(index);
                                } else {
                                    self.active_tab = index;
                                }
                            }
                        });
                    }
                    if let Some(index) = close_tab {
                        self.ui_tabs.remove(index);
                        if self.ui_tabs.is_empty() {
                            self.ui_tabs.push(UiTab {
                                title: "SERIAL / MONITOR".to_string(),
                                setup: false,
                            });
                            self.active_tab = 0;
                        } else if index < self.active_tab {
                            self.active_tab -= 1;
                        } else if index == self.active_tab {
                            self.active_tab = index.min(self.ui_tabs.len() - 1);
                        }
                    }
                    let new_window = ui
                        .add_sized(
                            [32.0, 32.0],
                            egui::Button::new(RichText::new("+").size(18.0).color(theme::TEXT_DIM))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .rounding(5.0),
                        )
                        .on_hover_text("Open a new monitor tab");
                    if new_window.clicked() {
                        let number = self.ui_tabs.len() + 1;
                        self.ui_tabs.push(UiTab {
                            title: format!("SERIAL {number} / SETUP"),
                            setup: true,
                        });
                        self.active_tab = self.ui_tabs.len() - 1;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("SERIAL WORKSPACE")
                                .monospace()
                                .size(9.0)
                                .color(theme::TEXT_MUTED),
                        );
                    });
                });
            });
    }
}
