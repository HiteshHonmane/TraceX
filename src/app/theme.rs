use crate::domain::log::LogLevel;
use eframe::egui::{self, Color32, FontId, Stroke, TextStyle, Vec2};
use std::sync::Arc;

pub const BG_DARK: Color32 = Color32::from_gray(18);
pub const BG_PANEL: Color32 = Color32::from_gray(20);
pub const BG_INPUT: Color32 = Color32::from_gray(34);
pub const BG_COMMAND: Color32 = Color32::from_gray(26);
pub const BG_SIDEBAR: Color32 = Color32::from_gray(26);
pub const BG_HOVER: Color32 = Color32::from_gray(46);

pub const TEXT: Color32 = Color32::from_gray(231);
pub const TEXT_DIM: Color32 = Color32::from_gray(183);
pub const TEXT_MUTED: Color32 = Color32::from_gray(125);

pub const ACCENT: Color32 = Color32::from_gray(192);
pub const SUCCESS: Color32 = Color32::from_rgb(83, 180, 126);
pub const WARNING: Color32 = Color32::from_rgb(218, 174, 72);
pub const ERROR: Color32 = Color32::from_rgb(218, 85, 99);
pub const INFO: Color32 = Color32::from_rgb(56, 161, 105);
pub const DEBUG: Color32 = Color32::from_rgb(129, 161, 183);
pub const VERBOSE: Color32 = Color32::from_rgb(127, 140, 141);

pub const BORDER: Color32 = Color32::from_gray(48);
pub const INFO_PILL: Color32 = Color32::from_rgb(25, 49, 36);
pub const WARN_PILL: Color32 = Color32::from_gray(35);
pub const ERROR_PILL: Color32 = Color32::from_rgb(48, 26, 29);

pub fn medium_font(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("Inter Medium".into()))
}

pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("Inter Bold".into()))
}

pub fn command_font(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("Command Mono".into()))
}

fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        (
            "Inter Regular",
            include_bytes!("../../assets/fonts/inter/Inter-Regular.ttf").as_slice(),
        ),
        (
            "Inter Medium",
            include_bytes!("../../assets/fonts/inter/Inter-Medium.ttf").as_slice(),
        ),
        (
            "Inter Bold",
            include_bytes!("../../assets/fonts/inter/Inter-Bold.ttf").as_slice(),
        ),
        (
            "Liberation Mono",
            include_bytes!("../../assets/fonts/liberation/LiberationMono-Regular.ttf").as_slice(),
        ),
    ] {
        fonts.font_data.insert(
            name.to_owned(),
            Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    let proportional = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    proportional.insert(0, "Inter Regular".to_owned());
    let proportional = proportional.clone();
    for name in ["Inter Medium", "Inter Bold"] {
        let mut family = vec![name.to_owned()];
        family.extend(proportional.clone());
        fonts
            .families
            .insert(egui::FontFamily::Name(name.into()), family);
    }
    let mut command = vec!["Liberation Mono".to_owned()];
    command.extend(
        fonts
            .families
            .get(&egui::FontFamily::Monospace)
            .cloned()
            .unwrap_or_default(),
    );
    fonts
        .families
        .insert(egui::FontFamily::Name("Command Mono".into()), command);
    fonts
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
    ctx.set_fonts(font_definitions());
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG_DARK;
    v.window_fill = BG_PANEL;
    v.extreme_bg_color = BG_DARK;
    v.widgets.noninteractive.bg_fill = BG_PANEL;
    v.widgets.inactive.bg_fill = BG_INPUT;
    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.active.bg_fill = BG_HOVER;
    // Buttons and combo boxes use weak_bg_fill, independently of bg_fill.
    v.widgets.noninteractive.weak_bg_fill = BG_PANEL;
    v.widgets.inactive.weak_bg_fill = BG_INPUT;
    v.widgets.hovered.weak_bg_fill = BG_HOVER;
    v.widgets.active.weak_bg_fill = BG_HOVER;
    v.widgets.open.bg_fill = BG_HOVER;
    v.widgets.open.weak_bg_fill = BG_HOVER;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_DIM);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_DIM);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.selection.bg_fill = ACCENT.linear_multiply(0.2);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    v.window_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, TEXT_MUTED);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, TEXT_DIM);
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_fonts_register_real_weights_and_keep_default_fallbacks() {
        let definitions = font_definitions();
        assert_eq!(
            definitions.families[&egui::FontFamily::Proportional][0],
            "Inter Regular"
        );
        for (family, primary) in [
            (medium_font(12.0).family, "Inter Medium"),
            (bold_font(14.0).family, "Inter Bold"),
            (command_font(14.0).family, "Liberation Mono"),
        ] {
            assert_eq!(definitions.families[&family][0], primary);
            assert!(definitions.font_data.contains_key(primary));
            assert!(
                definitions.families[&family].len() > 1,
                "bundled fallback chain must remain"
            );
        }
        assert_eq!(
            definitions.families[&egui::FontFamily::Monospace][0],
            "Hack",
            "log font is outside this command/UI typography change"
        );
        let ctx = egui::Context::default();
        setup(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                for font in [
                    FontId::proportional(14.0),
                    medium_font(12.0),
                    bold_font(14.0),
                    command_font(14.0),
                ] {
                    assert!(fonts.has_glyphs(&font, "TraceX AT+STATUS café 23°C μs"));
                }
                assert!(
                    (fonts.glyph_width(&command_font(14.0), 'W')
                        - fonts.glyph_width(&command_font(14.0), 'i'))
                    .abs()
                        < 0.01
                );
            });
        });
    }

    #[test]
    fn ordinary_controls_use_theme_in_all_interaction_states() {
        let ctx = egui::Context::default();
        setup(&ctx);
        let style = ctx.style();
        let widgets = &style.visuals.widgets;
        assert_eq!(widgets.inactive.weak_bg_fill, BG_INPUT);
        assert_eq!(widgets.hovered.weak_bg_fill, BG_HOVER);
        assert_eq!(widgets.active.weak_bg_fill, BG_HOVER);
        assert_eq!(widgets.open.weak_bg_fill, BG_HOVER);
        assert_eq!(style.visuals.panel_fill, BG_DARK);
    }
}
