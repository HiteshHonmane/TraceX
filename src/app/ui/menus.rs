use super::super::*;

impl TraceXApp {
    pub(super) fn render_app_menus(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.menu_button("File", |ui| {
            ui.add_enabled_ui(
                !self.shutdown.active() && !self.session_archive_busy(),
                |ui| {
                    if ui.button("Open Session…").clicked() {
                        ui.close_menu();
                        self.choose_open_session();
                    }
                    if ui.button("Save Session…").clicked() {
                        ui.close_menu();
                        self.choose_save_session();
                    }
                    if ui.button("Export filtered text…").clicked() {
                        ui.close_menu();
                        self.export_logs();
                    }
                    ui.separator();
                    if ui.button("Clear retained logs").clicked() {
                        self.clear();
                        ui.close_menu();
                    }
                },
            );
            ui.separator();
            if ui.button("Exit").clicked() {
                self.request_shutdown(Instant::now());
                ui.close_menu();
            }
        });
        ui.menu_button("Edit", |ui| {
            ui.add_enabled_ui(!self.session_archive_busy(), |ui| {
                if ui.button("Copy selected / filtered logs").clicked() {
                    self.copy_selected(ctx);
                    ui.close_menu();
                }
                if ui.button("Copy all retained logs").clicked() {
                    self.copy_all(ctx);
                    ui.close_menu();
                }
            });
        });
        ui.menu_button("View", |ui| {
            ui.checkbox(&mut self.filter_panel_open, "Search results panel");
            if ui
                .checkbox(&mut self.auto_scroll, "Follow incoming logs")
                .changed()
            {
                self.live_scroll_frames = live_scroll_frames_after_toggle(true, self.auto_scroll);
            }
            ui.separator();
            ui.checkbox(&mut self.show_info, "INFO records");
            ui.checkbox(&mut self.show_warn, "WARN records");
            ui.checkbox(&mut self.show_error, "ERROR records");
        });
        ui.menu_button("Help", |ui| {
            ui.set_max_width(300.0);
            ui.label(format!("TraceX {} · development preview", env!("CARGO_PKG_VERSION")));
            ui.label("Select a port and baud rate, then connect. Disconnect before saving or reopening a session.");
            ui.label("Save Session preserves all retained records. Text export follows the current search and severity filters.");
            ui.label("Monitor tabs currently share one device capture.");
        });
    }
}
