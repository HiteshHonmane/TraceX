use super::super::*;

impl TraceXApp {
    pub(super) fn render_tabs(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("tabs")
            .exact_height(TOP_HEADER_HEIGHT)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_INPUT)
                    .stroke(Stroke::new(1.0_f32, theme::BORDER)),
            )
            .show(ctx, |ui| {
                const TAB_HEIGHT: f32 = 40.0;
                ui.add_space(TOP_HEADER_HEIGHT - TAB_HEIGHT);
                ui.horizontal(|ui| {
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
                                Vec2::new(150.0, TAB_HEIGHT),
                                egui::Sense::click(),
                            );
                            let fill = if active {
                                theme::BG_DARK
                            } else if response.hovered() {
                                theme::BG_HOVER
                            } else {
                                Color32::TRANSPARENT
                            };
                            let stroke = if active {
                                Stroke::new(1.0_f32, theme::BORDER)
                            } else {
                                Stroke::NONE
                            };
                            ui.painter().rect(
                                rect,
                                egui::Rounding {
                                    nw: 6.0,
                                    ne: 6.0,
                                    sw: 0.0,
                                    se: 0.0,
                                },
                                fill,
                                stroke,
                            );
                            ui.painter().text(
                                egui::pos2(rect.left() + 20.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                title,
                                FontId::proportional(13.0),
                                if active { theme::TEXT } else { theme::TEXT_DIM },
                            );
                            if active || response.hovered() {
                                ui.painter().text(
                                    egui::pos2(rect.right() - 18.0, rect.center().y),
                                    egui::Align2::CENTER_CENTER,
                                    "×",
                                    FontId::proportional(14.0),
                                    theme::TEXT_MUTED,
                                );
                            }

                            if response.clicked() {
                                let over_close = response
                                    .interact_pointer_pos()
                                    .map(|position| position.x >= rect.right() - 34.0)
                                    .unwrap_or(false);
                                if over_close {
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
                                title: "Monitor 1".to_string(),
                                setup: false,
                            });
                            self.active_tab = 0;
                        } else if index < self.active_tab {
                            self.active_tab -= 1;
                        } else if index == self.active_tab {
                            self.active_tab = index.min(self.ui_tabs.len() - 1);
                        }
                    }

                    if ui
                        .add_sized(
                            [38.0, 36.0],
                            egui::Button::new(RichText::new("+").size(18.0).color(theme::TEXT_DIM))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        )
                        .on_hover_text("Open a new monitor")
                        .clicked()
                    {
                        let number = self.ui_tabs.len() + 1;
                        self.ui_tabs.push(UiTab {
                            title: format!("Monitor {number}"),
                            setup: true,
                        });
                        self.active_tab = self.ui_tabs.len() - 1;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new("◎").size(22.0).color(theme::TEXT_DIM))
                            .on_hover_text("Profile");
                    });
                });
            });
    }
}
