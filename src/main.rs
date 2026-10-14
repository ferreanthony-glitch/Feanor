#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
#[cfg(test)]
mod audio_tests;
mod brush;
mod brushtex;
#[cfg(test)]
mod brushtex_tests;
mod brushui;
mod camera;
#[cfg(test)]
mod camera_tests;
mod camtools;
mod canvas;
#[cfg(test)]
mod depth_tests;
mod document;
mod export;
#[cfg(test)]
mod export_tests;
mod exportui;
mod fill;
mod gpu;
mod guides;
#[cfg(test)]
mod guides_tests;
mod history;
mod io;
mod keymap;
mod layertools;
#[cfg(test)]
mod keymap_tests;
#[cfg(test)]
mod ops_tests;
mod onion;
#[cfg(test)]
mod onion_tests;
mod panels;
mod pdf;
#[cfg(test)]
mod pdf_tests;
mod playback;
#[cfg(test)]
mod playback_tests;
mod pixsel;
#[cfg(test)]
mod pixsel_tests;
mod pixtools;
mod project;
mod raster;
mod recovery;
#[cfg(test)]
mod recovery_tests;
mod recoveryui;
mod render;
mod settings;
mod soundtrack;
#[cfg(test)]
mod stage_tests;
mod storyboard;
mod tbbrush;
#[cfg(test)]
mod tests;
mod thumbs;
mod timeline;
mod topview;
#[cfg(test)]
mod topview_tests;
#[cfg(test)]
mod transform_tests;
mod vector;
mod view;
mod vtools;
mod workspace;
#[cfg(test)]
mod workspace_tests;
mod workspaceui;

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
