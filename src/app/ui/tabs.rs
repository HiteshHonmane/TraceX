use super::super::*;
use super::actions::{UiIntent, widget_id};

pub(super) fn render_tabs(workspace: &Workspace, ctx: &egui::Context, intents: &mut Vec<UiIntent>) {
    egui::TopBottomPanel::top("tabs")
        .exact_height(TOP_HEADER_HEIGHT)
        .frame(
            egui::Frame::none()
                .fill(Color32::from_gray(31))
                .stroke(Stroke::new(1.0_f32, theme::BORDER)),
        )
        .show(ctx, |ui| {
            ui.add_space(TOP_HEADER_HEIGHT - 36.0);
            ui.horizontal(|ui| {
                for session in workspace.sessions() {
                    let active = workspace.active_session_id() == session.id;
                    let closing = workspace.close_target(session.id).is_some();
                    let state = if closing {
                        "Closing"
                    } else if session.connected {
                        "Live"
                    } else if session.capture.is_some() {
                        "Draining"
                    } else if session.source_path.is_some() {
                        "Saved session · disconnected"
                    } else {
                        "Disconnected"
                    };
                    let details = format!(
                        "{} · {}{}{}",
                        session.title,
                        state,
                        if session.history_dirty {
                            " · Unsaved retained history"
                        } else {
                            ""
                        },
                        session
                            .occupied_endpoint
                            .as_ref()
                            .or_else(|| (!session.selected_port.is_empty())
                                .then_some(&session.selected_port))
                            .map_or(String::new(), |port| format!(" · {port}"))
                    );
                    let details = format!(
                        "{details}{}{}",
                        session
                            .source_path
                            .as_ref()
                            .map_or(String::new(), |p| format!(" · {}", p.display())),
                        session
                            .operation_error
                            .as_ref()
                            .or(session.capture_error.as_ref())
                            .map_or(String::new(), |error| format!(" · {error}"))
                    );
                    ui.push_id(widget_id(session.id, "tab"), |ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(132.0, 36.0), egui::Sense::click());
                        let fill = if active {
                            theme::BG_DARK
                        } else if response.hovered() {
                            theme::BG_HOVER
                        } else {
                            Color32::TRANSPARENT
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
                            if active || response.has_focus() {
                                Stroke::new(1.0_f32, theme::BORDER)
                            } else {
                                Stroke::NONE
                            },
                        );
                        let color = if closing {
                            theme::WARNING
                        } else if session.capture_error.is_some()
                            || session.operation_error.is_some()
                        {
                            theme::ERROR
                        } else if session.connected {
                            theme::SUCCESS
                        } else {
                            theme::TEXT_MUTED
                        };
                        let cue = egui::pos2(rect.left() + 8.0, rect.center().y);
                        if closing {
                            super::icons::paint(
                                ui,
                                egui::Rect::from_center_size(cue, Vec2::splat(10.0)),
                                super::icons::Icon::Follow,
                                color,
                            );
                        } else if session.connected {
                            ui.painter().circle_filled(cue, 2.5, color);
                        } else {
                            ui.painter()
                                .circle_stroke(cue, 2.5, Stroke::new(1.0_f32, color));
                        }
                        let title = format!(
                            "{}{}",
                            session.title,
                            if session.history_dirty { " *" } else { "" }
                        );
                        let clip = egui::Rect::from_min_max(
                            rect.left_top(),
                            egui::pos2(rect.right() - 30.0, rect.bottom()),
                        );
                        ui.painter().with_clip_rect(clip).text(
                            egui::pos2(rect.left() + 24.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            title,
                            if active {
                                theme::bold_font(14.0)
                            } else {
                                FontId::proportional(14.0)
                            },
                            if active {
                                Color32::WHITE
                            } else {
                                theme::TEXT_DIM
                            },
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::SelectableLabel,
                                ui.is_enabled(),
                                active,
                                &details,
                            )
                        });
                        let close_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.right() - 28.0, rect.top()),
                            Vec2::new(28.0, rect.height()),
                        );
                        let close = ui.interact(
                            close_rect,
                            widget_id(session.id, "tab_close"),
                            egui::Sense::click(),
                        );
                        close.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                !closing,
                                format!("Close {}", session.title),
                            )
                        });
                        if active || response.hovered() || closing {
                            super::icons::paint(
                                ui,
                                egui::Rect::from_center_size(
                                    close_rect.center(),
                                    Vec2::splat(12.0),
                                ),
                                super::icons::Icon::Close,
                                theme::TEXT_MUTED,
                            );
                        }
                        if close
                            .on_hover_text(format!("Request safe close of {}", session.title))
                            .clicked()
                            && !closing
                        {
                            intents.push(UiIntent::CloseSession(session.id));
                        } else if response.on_hover_text(&details).clicked() {
                            intents.push(UiIntent::Select(session.id));
                        }
                    });
                }
                let plus = ui.add_enabled_ui(workspace.can_create_session(), |ui| {
                    super::icons::button(
                        ui,
                        super::icons::Icon::Plus,
                        "Create independent monitor session",
                        Vec2::new(36.0, 36.0),
                        false,
                    )
                });
                if plus
                    .inner
                    .on_disabled_hover_text(
                        "Two-session preview limit reached; close a session first",
                    )
                    .clicked()
                {
                    intents.push(UiIntent::Create);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(14.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::hover());
                    super::icons::paint(
                        ui,
                        rect.shrink(2.0),
                        super::icons::Icon::Profile,
                        theme::TEXT_DIM,
                    );
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dirty_tab_titles_fit_between_inset_and_close_target() {
        let mut workspace = Workspace::new();
        let a = workspace.active_session_id();
        let b = workspace.create_session().unwrap();
        workspace.session_mut(a).unwrap().history_dirty = true;
        workspace.session_mut(b).unwrap().history_dirty = true;
        for width in [800.0, 1200.0, 1280.0] {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 800.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::SidePanel::left("sidebar")
                        .exact_width(256.0)
                        .show(ctx, |_| {});
                    render_tabs(&workspace, ctx, &mut Vec::new());
                },
            );
            for expected in ["Monitor 1 *", "Monitor 2 *"] {
                let title = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == expected => {
                            Some((shape.clip_rect, text))
                        }
                        _ => None,
                    })
                    .expect("dirty title must be painted");
                assert!((title.1.pos.x - title.0.left() - 24.0).abs() < 0.1);
                assert!(
                    title.1.pos.x + title.1.galley.size().x <= title.0.right(),
                    "{expected} exceeds close clip: {title:?}"
                );
            }
        }
    }
}
