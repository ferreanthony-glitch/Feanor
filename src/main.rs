#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod brush;
mod canvas;
mod document;
mod fill;
mod history;
mod io;
mod keymap;
#[cfg(test)]
mod keymap_tests;
mod panels;
mod project;
mod raster;
mod render;
mod storyboard;
#[cfg(test)]
mod tests;
mod thumbs;
mod timeline;
mod vector;
mod view;
mod vtools;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Pelure")
            .with_inner_size([1500.0, 920.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };
    // Un projet passé en argument (ou par double-clic sur un .pelure) est ouvert au démarrage.
    let initial = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    eframe::run_native(
        "Pelure",
        options,
        Box::new(|cc| {
            let mut app = app::App::new(cc);
            if let Some(path) = initial {
                app.open_path(path);
            }
            Ok(Box::new(app))
        }),
    )
}
