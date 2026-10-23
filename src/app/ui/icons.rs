use eframe::egui::{self, Color32, Pos2, Rect, Response, Stroke, Ui, Vec2};

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Panel,
    Back,
    Forward,
    Close,
    Maximize,
    Minimize,
    Copy,
    Trash,
    Follow,
    Export,
    Search,
    Settings,
    Support,
    Folder,
    Profile,
    Plus,
}

// All coordinates are normalized to a 16-point square. No icon depends on a font.
pub(super) fn paint(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.3_f32, color);
    let point = |x: f32, y: f32| -> Pos2 {
        rect.center() + egui::vec2(x - 8.0, y - 8.0) * (rect.width().min(rect.height()) / 16.0)
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
    };
    let outline = |a: (f32, f32), b: (f32, f32), radius| {
        painter.rect_stroke(
            Rect::from_two_pos(point(a.0, a.1), point(b.0, b.1)),
            radius,
            stroke,
        );
    };
    match icon {
        Icon::Panel => {
            outline((2.0, 2.0), (14.0, 14.0), 2.0);
            line((6.0, 2.0), (6.0, 14.0));
        }
        Icon::Back | Icon::Forward => {
            let (start, end) = if matches!(icon, Icon::Back) {
                (12.0, 4.0)
            } else {
                (4.0, 12.0)
            };
            line((start, 8.0), (end, 8.0));
            line((8.0, 4.0), (end, 8.0));
            line((8.0, 12.0), (end, 8.0));
        }
        Icon::Close => {
            line((4.0, 4.0), (12.0, 12.0));
            line((12.0, 4.0), (4.0, 12.0));
        }
        Icon::Maximize => outline((3.0, 3.0), (13.0, 13.0), 0.5),
        Icon::Minimize => line((3.0, 8.0), (13.0, 8.0)),
        Icon::Plus => {
            line((3.0, 8.0), (13.0, 8.0));
            line((8.0, 3.0), (8.0, 13.0));
        }
        Icon::Copy => {
            outline((6.0, 6.0), (14.0, 14.0), 1.5);
            line((3.0, 11.0), (2.0, 11.0));
            line((2.0, 11.0), (2.0, 2.0));
            line((2.0, 2.0), (11.0, 2.0));
            line((11.0, 2.0), (11.0, 3.0));
        }
        Icon::Trash => {
            outline((4.0, 5.0), (12.0, 14.0), 1.0);
            line((2.0, 4.0), (14.0, 4.0));
            line((6.0, 2.0), (10.0, 2.0));
            line((7.0, 7.0), (7.0, 11.0));
            line((9.0, 7.0), (9.0, 11.0));
        }
        Icon::Follow => {
            let points = (0..=16)
                .map(|i| {
                    let angle = -0.6 + i as f32 / 16.0 * 5.3;
                    rect.center() + Vec2::angled(angle) * rect.width() * 0.35
                })
                .collect();
            painter.add(egui::Shape::line(points, stroke));
            line((13.0, 2.0), (13.0, 6.0));
            line((13.0, 6.0), (9.0, 6.0));
            line((8.0, 5.0), (8.0, 8.0));
            line((8.0, 8.0), (10.0, 9.0));
        }
        Icon::Export => {
            line((8.0, 2.0), (8.0, 10.0));
            line((4.0, 7.0), (8.0, 11.0));
            line((12.0, 7.0), (8.0, 11.0));
            line((3.0, 12.0), (3.0, 14.0));
            line((3.0, 14.0), (13.0, 14.0));
            line((13.0, 14.0), (13.0, 12.0));
        }
        Icon::Search => {
            painter.circle_stroke(point(7.0, 7.0), rect.width() * 0.26, stroke);
            line((10.0, 10.0), (14.0, 14.0));
        }
        Icon::Folder => {
            painter.add(egui::Shape::closed_line(
                vec![
                    point(2.0, 4.0),
                    point(6.0, 4.0),
                    point(8.0, 6.0),
                    point(14.0, 6.0),
                    point(14.0, 13.0),
                    point(2.0, 13.0),
                ],
                stroke,
            ));
        }
        Icon::Settings => {
            painter.circle_stroke(point(8.0, 8.0), rect.width() * 0.3, stroke);
            painter.circle_stroke(point(8.0, 8.0), rect.width() * 0.12, stroke);
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::TAU / 8.0;
                let direction = Vec2::angled(angle);
                painter.line_segment(
                    [
                        rect.center() + direction * rect.width() * 0.31,
                        rect.center() + direction * rect.width() * 0.44,
                    ],
                    stroke,
                );
            }
        }
        Icon::Profile => {
            painter.circle_stroke(point(8.0, 8.0), rect.width() * 0.43, stroke);
            painter.circle_stroke(point(8.0, 6.0), rect.width() * 0.13, stroke);
            painter.add(egui::Shape::line(
                vec![
                    point(4.0, 12.0),
                    point(5.0, 10.0),
                    point(11.0, 10.0),
                    point(12.0, 12.0),
                ],
                stroke,
            ));
        }
        Icon::Support => {
            painter.circle_stroke(point(8.0, 8.0), rect.width() * 0.4, stroke);
            painter.add(egui::Shape::line(
                vec![
                    point(5.0, 5.0),
                    point(8.0, 4.0),
                    point(11.0, 6.0),
                    point(8.0, 9.0),
                ],
                stroke,
            ));
            painter.circle_filled(point(8.0, 12.0), 0.8, color);
        }
    }
}

pub(super) fn button(ui: &mut Ui, icon: Icon, label: &str, size: Vec2, selected: bool) -> Response {
    let response = ui.add_sized(
        size,
        egui::Button::new("").frame(selected).selected(selected),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let color = if !ui.is_enabled() {
        super::theme::TEXT_MUTED.gamma_multiply(0.5)
    } else if response.hovered() || selected {
        super::theme::TEXT
    } else {
        super::theme::TEXT_DIM
    };
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            3.0,
            Stroke::new(1.0_f32, super::theme::TEXT),
        );
    }
    paint(
        ui,
        Rect::from_center_size(response.rect.center(), Vec2::splat(14.0)),
        icon,
        color,
    );
    response.on_hover_text(label)
}

pub(super) fn navigation(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let response = ui.add_sized(
        [ui.available_width(), 38.0],
        egui::Button::new("").frame(false),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let color = if response.hovered() {
        super::theme::TEXT
    } else {
        super::theme::TEXT_DIM
    };
    if response.has_focus() {
        ui.painter()
            .rect_stroke(response.rect, 3.0, Stroke::new(1.0_f32, super::theme::TEXT));
    }
    let center = egui::pos2(response.rect.left() + 12.0, response.rect.center().y);
    paint(
        ui,
        Rect::from_center_size(center, Vec2::splat(16.0)),
        icon,
        color,
    );
    ui.painter().text(
        center + egui::vec2(22.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        color,
    );
    response
}
