use crate::domain::log::LogLevel;
use eframe::egui::{self, Color32, FontId, Stroke, TextStyle, Vec2};

pub const BG_DARK: Color32 = Color32::from_rgb(6, 14, 32);
pub const BG_PANEL: Color32 = Color32::from_rgb(11, 19, 38);
pub const BG_INPUT: Color32 = Color32::from_rgb(19, 27, 46);
pub const BG_COMMAND: Color32 = Color32::from_rgb(34, 42, 62);
pub const BG_HOVER: Color32 = Color32::from_rgb(45, 52, 73);

pub const TEXT: Color32 = Color32::from_rgb(219, 226, 253);
pub const TEXT_DIM: Color32 = Color32::from_rgb(196, 198, 205);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(142, 145, 151);

pub const ACCENT: Color32 = Color32::from_rgb(185, 200, 222);
pub const SUCCESS: Color32 = Color32::from_rgb(163, 184, 148);
pub const WARNING: Color32 = Color32::from_rgb(184, 169, 148);
pub const ERROR: Color32 = Color32::from_rgb(184, 148, 159);
pub const INFO: Color32 = Color32::from_rgb(163, 184, 148);
pub const DEBUG: Color32 = Color32::from_rgb(163, 184, 148);
pub const VERBOSE: Color32 = Color32::from_rgb(127, 140, 141);

pub const BORDER: Color32 = Color32::from_rgb(42, 48, 65);

pub fn tint(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn log_level_color(level: LogLevel) -> Color32 {
    match level {
        LogLevel::Error => ERROR,
        LogLevel::Warn => WARNING,
        LogLevel::Info => INFO,
        LogLevel::Debug => DEBUG,
        LogLevel::Verbose => VERBOSE,
        LogLevel::Log => TEXT_DIM,
    }
}

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
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    v.window_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(22.0)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Small, FontId::proportional(11.0)),
    ]
    .into();
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.visuals.widgets.inactive.rounding = 4.0.into();
    style.visuals.widgets.hovered.rounding = 4.0.into();
    style.visuals.widgets.active.rounding = 4.0.into();
    style.visuals.widgets.open.rounding = 4.0.into();
    style.visuals.window_rounding = 8.0.into();
    ctx.set_style(style);
}
