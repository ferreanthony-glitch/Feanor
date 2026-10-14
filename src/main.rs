#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
#[cfg(test)]
mod audio_tests;
mod brush;
mod canvas;
mod document;
mod fill;
mod guides;
#[cfg(test)]
mod guides_tests;
mod history;
mod io;
mod keymap;
#[cfg(test)]
mod keymap_tests;
#[cfg(test)]
mod ops_tests;
mod panels;
mod pixsel;
#[cfg(test)]
mod pixsel_tests;
mod pixtools;
mod project;
mod raster;
mod render;
mod soundtrack;
mod storyboard;
#[cfg(test)]
mod tests;
mod thumbs;
mod timeline;
mod vector;
mod view;
mod vtools;

fn main() -> eframe::Result<()> {
    // Un projet passé en argument (ou par double-clic sur un .pelure) est ouvert au démarrage.
    let initial = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    // Affichage par la carte graphique (wgpu). PELURE_AFFICHAGE=opengl force l'ancien moteur, et si
    // wgpu ne démarre pas sur cette machine, on repart avec OpenGL.
    let opengl = std::env::var("PELURE_AFFICHAGE").is_ok_and(|v| v.eq_ignore_ascii_case("opengl"));
    if !opengl {
        match run(eframe::Renderer::Wgpu, initial.clone()) {
            Err(e) => eprintln!("affichage wgpu impossible ({e}) : on passe à OpenGL"),
            ok => return ok,
        }
    }
    run(eframe::Renderer::Glow, initial)
}

fn run(renderer: eframe::Renderer, initial: Option<std::path::PathBuf>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Pelure")
            .with_inner_size([1500.0, 920.0])
            .with_min_inner_size([960.0, 600.0]),
        renderer,
        ..Default::default()
    };
    eframe::run_native(
        "Pelure",
        options,
        Box::new(move |cc| {
            let mut app = app::App::new(cc);
            if let Some(path) = initial {
                app.open_path(path);
            }
            Ok(Box::new(app))
        }),
    )
}
