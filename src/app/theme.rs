use eframe::egui::{self, Color32, Stroke, Vec2};

pub const BG_DARK: Color32 = Color32::from_rgb(13, 16, 20);
pub const BG_PANEL: Color32 = Color32::from_rgb(18, 22, 27);
pub const BG_INPUT: Color32 = Color32::from_rgb(22, 27, 33);
pub const BG_HOVER: Color32 = Color32::from_rgb(31, 38, 46);

pub const TEXT: Color32 = Color32::from_rgb(235, 240, 246);
pub const TEXT_DIM: Color32 = Color32::from_rgb(151, 162, 175);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(91, 103, 116);

pub const ACCENT: Color32 = Color32::from_rgb(80, 154, 255);
pub const SUCCESS: Color32 = Color32::from_rgb(70, 210, 142);
pub const WARNING: Color32 = Color32::from_rgb(245, 181, 74);
pub const ERROR: Color32 = Color32::from_rgb(255, 99, 113);
pub const INFO: Color32 = Color32::from_rgb(92, 183, 255);
pub const DEBUG: Color32 = Color32::from_rgb(138, 219, 158);
pub const VERBOSE: Color32 = Color32::from_rgb(127, 140, 141);

pub const BORDER: Color32 = Color32::from_rgb(39, 46, 55);

pub fn setup(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG_DARK;
    v.window_fill = BG_PANEL;
    v.extreme_bg_color = BG_DARK;
    v.widgets.noninteractive.bg_fill = BG_PANEL;
    v.widgets.inactive.bg_fill = BG_INPUT;
    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.active.bg_fill = BG_HOVER;
    v.selection.bg_fill = ACCENT.linear_multiply(0.2);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(62, 63, 57));
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.visuals.widgets.inactive.rounding = 5.0.into();
    style.visuals.widgets.hovered.rounding = 5.0.into();
    style.visuals.widgets.active.rounding = 5.0.into();
    ctx.set_style(style);
}
