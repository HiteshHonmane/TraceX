use super::super::*;
use super::actions::{FileAction, SessionUi, UiIntent, widget_id};
use super::icons::{self, Icon};

pub(super) fn render_sidebar(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ports: &[(String, String)],
    ctx: &egui::Context,
) {
    egui::SidePanel::left(widget_id(meta.id, "connection_sidebar"))
        .exact_width(256.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(theme::BG_SIDEBAR)
                .inner_margin(egui::Margin::symmetric(24.0, 0.0)),
        )
        .show(ctx, |ui| {
            let content_height = (ui.available_height() - 170.0).max(200.0);
            egui::ScrollArea::vertical()
                .id_salt("connection_configuration")
                .max_height(content_height)
                .show(ui, |ui| {
                    render_connection_configuration(session, meta, intents, ports, ui)
                });
            render_sidebar_footer(session, meta, intents, ui);
        });
}

fn render_connection_configuration(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ports: &[(String, String)],
    ui: &mut egui::Ui,
) {
    ui.add_space(18.0);
    ui.label(
        RichText::new("TraceX")
            .font(theme::medium_font(31.0))
            .line_height(Some(28.0))
            .extra_letter_spacing(0.45)
            .color(Color32::WHITE),
    );
    ui.add_space(10.0);
    ui.add(
        egui::Label::new(
            RichText::new(&meta.identity)
                .size(10.0)
                .color(theme::TEXT_MUTED),
        )
        .truncate(),
    )
    .on_hover_text(session.source_path.as_ref().map_or_else(
        || meta.identity.clone(),
        |path| format!("{} · {}", meta.identity, path.display()),
    ));
    ui.add_space(10.0);
    let ready = meta.can_edit && (session.connected || meta.can_connect);
    let label = if session.connected {
        "Disconnect Device"
    } else {
        "+  Connect Device"
    };
    let color = if session.connected {
        theme::ERROR
    } else {
        theme::TEXT
    };
    let connection = ui.add_enabled(
        ready,
        egui::Button::new(RichText::new(label).size(13.0).color(color))
            .fill(Color32::from_gray(42))
            .stroke(Stroke::new(1.0_f32, theme::BORDER))
            .min_size(Vec2::new(ui.available_width(), 38.0)),
    );
    if connection
        .on_disabled_hover_text("Select a detected port first. Ports refresh automatically.")
        .clicked()
    {
        if session.connected {
            intents.push(UiIntent::Disconnect(meta.id));
        } else {
            intents.push(UiIntent::Connect(meta.id));
        }
    }
    ui.add_space(28.0);
    ui.label(
        RichText::new("CONFIGURATION")
            .size(10.0)
            .color(theme::TEXT_MUTED),
    );
    ui.add_space(12.0);
    render_port_configuration(session, meta, intents, ports, ui);
    render_sidebar_status(session, meta, intents, ui);
}

fn render_port_configuration(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    _intents: &mut Vec<UiIntent>,
    ports: &[(String, String)],
    ui: &mut egui::Ui,
) {
    ui.label(
        RichText::new("Port")
            .font(theme::medium_font(12.0))
            .line_height(Some(16.0))
            .color(theme::TEXT),
    );
    let selected_display = ports
        .iter()
        .find(|(_, port)| port == &session.selected_port)
        .map(|(display, _)| display.clone())
        .unwrap_or_else(|| {
            if session.selected_port.is_empty() {
                "Select a port".to_owned()
            } else {
                session.selected_port.clone()
            }
        });
    ui.add_enabled_ui(!session.connected && meta.can_edit, |ui| {
        ui.spacing_mut().interact_size.y = 34.0;
        egui::ComboBox::from_id_salt("port_combo")
            .width(ui.available_width())
            .selected_text(
                RichText::new(&selected_display)
                    .size(14.0)
                    .line_height(Some(20.0))
                    .color(theme::TEXT),
            )
            .show_ui(ui, |ui| {
                if ports.is_empty() {
                    ui.label("No serial ports detected");
                }
                for (display, port) in ports {
                    if ui
                        .selectable_label(session.selected_port == *port, display)
                        .clicked()
                    {
                        session.selected_port = port.clone();
                    }
                }
            })
            .response
            .on_hover_text(&selected_display);
    });
    ui.add_space(14.0);
    ui.label(
        RichText::new("Baud Rate")
            .font(theme::medium_font(12.0))
            .line_height(Some(16.0))
            .color(theme::TEXT),
    );
    ui.add_enabled_ui(!session.connected && meta.can_edit, |ui| {
        ui.spacing_mut().interact_size.y = 34.0;
        egui::ComboBox::from_id_salt("baud_combo")
            .width(ui.available_width())
            .selected_text(
                RichText::new(session.baud_rate.to_string())
                    .size(14.0)
                    .line_height(Some(20.0))
                    .color(theme::TEXT),
            )
            .show_ui(ui, |ui| {
                for rate in BAUD_RATES {
                    if ui
                        .selectable_label(session.baud_rate == rate, rate.to_string())
                        .clicked()
                    {
                        session.baud_rate = rate;
                    }
                }
            });
    });
}

fn render_sidebar_status(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ui: &mut egui::Ui,
) {
    ui.add_space(18.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Auto-Save Logs")
                .size(14.0)
                .color(theme::TEXT_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut enabled = session.auto_save_enabled;
            if ui
                .add_enabled_ui(meta.can_edit, |ui| autosave_toggle(ui, &mut enabled))
                .inner
                .changed()
            {
                intents.push(UiIntent::AutosaveEnabled(meta.id, enabled));
            }
        });
    });
    super::autosave::render_auto_save_status(session, meta, intents, ui);
    for error in [&session.operation_error, &session.capture_error]
        .into_iter()
        .flatten()
    {
        ui.add(egui::Label::new(RichText::new(error).size(13.0).color(theme::ERROR)).wrap());
    }
    if session.session_archive_busy() {
        ui.label(
            RichText::new("Session file operation in progress…")
                .size(13.0)
                .color(theme::TEXT_DIM),
        );
    }
}

fn render_sidebar_footer(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ui: &mut egui::Ui,
) {
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.add_space(12.0);
        render_support_popup(ui);
        render_settings_popup(session, meta, intents, ui);
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        let library = icons::navigation(ui, Icon::Folder, "Library");
        let library_id = widget_id(meta.id, "library_popup");
        if library.clicked() {
            ui.memory_mut(|m| m.toggle_popup(library_id));
        }
        egui::popup::popup_above_or_below_widget(
            ui,
            library_id,
            &library,
            egui::AboveOrBelow::Above,
            egui::popup::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(208.0);
                super::menus::render_session_actions(session, meta, intents, ui);
                if ui
                    .add_enabled(meta.can_export, egui::Button::new("Export filtered text…"))
                    .clicked()
                {
                    ui.close_menu();
                    intents.push(UiIntent::File(meta.id, FileAction::Export));
                }
            },
        );
    });
}

fn render_support_popup(ui: &mut egui::Ui) {
    let support = icons::navigation(ui, Icon::Support, "Support");
    let popup_id = ui.make_persistent_id("support_help");
    if support.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    egui::popup::popup_above_or_below_widget(
        ui,
        popup_id,
        &support,
        egui::AboveOrBelow::Above,
        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_max_width(300.0);
            ui.label(format!(
                "TraceX {} · development preview",
                env!("CARGO_PKG_VERSION")
            ));
            ui.label("Select a port and baud rate, then connect. Each monitor owns its capture and command destination.");
            ui.label("Disconnect before saving or opening a session. Save preserves all retained records; text export follows current filters.");
        },
    );
}

fn render_settings_popup(
    session: &mut SessionRuntime,
    meta: &SessionUi,
    intents: &mut Vec<UiIntent>,
    ui: &mut egui::Ui,
) {
    let settings = icons::navigation(ui, Icon::Settings, "Settings");
    let popup_id = widget_id(meta.id, "settings_popup");
    if settings.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    egui::popup::popup_above_or_below_widget(
        ui,
        popup_id,
        &settings,
        egui::AboveOrBelow::Above,
        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(230.0);
            ui.set_max_width(270.0);
            ui.add_enabled_ui(meta.can_edit, |ui| {
                ui.checkbox(
                    &mut session.auto_clear_command,
                    "Clear command after accepted send",
                );
                ui.separator();
                super::autosave::render_settings(session, meta, intents, ui);
            });
        },
    );
}

fn autosave_toggle(ui: &mut egui::Ui, enabled: &mut bool) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(32.0, 20.0), egui::Sense::click());
    if response.clicked() {
        *enabled = !*enabled;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *enabled,
            "Auto-Save Logs",
        )
    });
    let fill = if *enabled {
        Color32::from_rgb(91, 105, 119)
    } else {
        theme::BG_HOVER
    };
    let border = if response.has_focus() {
        theme::TEXT
    } else {
        theme::BORDER
    };
    ui.painter()
        .rect(rect, 10.0, fill, Stroke::new(1.0_f32, border));
    let knob_x = if *enabled {
        rect.right() - 10.0
    } else {
        rect.left() + 10.0
    };
    ui.painter()
        .circle_filled(egui::pos2(knob_x, rect.center().y), 6.0, theme::TEXT);
    response.on_hover_text("Enable text autosave; actual write status appears below")
}
