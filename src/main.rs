#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod domain;

fn main() -> eframe::Result<()> {
    app::run()
}
