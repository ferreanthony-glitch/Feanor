//! Pelure en bibliothèque (10a) : tous les modules, l'application egui (`lancer`), et le noyau
//! (`noyau`) que d'autres interfaces peuvent utiliser — l'appli compagnon Tauri (`src-tauri/`).
//! L'exécutable `pelure` (`main.rs`) ne fait qu'appeler `lancer`.

mod app;
mod audio;
#[cfg(test)]
mod audio_tests;
mod blend;
#[cfg(test)]
mod blend_tests;
mod board;
#[cfg(test)]
mod board_tests;
mod boardui;
mod bottomtabs;
mod brush;
mod brushtex;
#[cfg(test)]
mod brushtex_tests;
mod brushui;
mod dynui;
pub mod banc;
#[cfg(test)]
mod banc_tests;
pub mod dip;
pub mod tvpbrush;
pub mod kritabrush;
pub mod zipread;
pub mod abr;
pub mod procreate;
#[cfg(test)]
mod procreate_tests;
#[cfg(test)]
mod abr_tests;
#[cfg(test)]
mod kritabrush_tests;
#[cfg(test)]
mod tvpbrush_tests;
#[cfg(test)]
mod dip_tests;
#[cfg(test)]
mod dynamics_tests;
mod bulles;
#[cfg(test)]
mod bulles_tests;
mod camera;
#[cfg(test)]
mod camera_tests;
mod camtools;
mod canvas;
#[cfg(test)]
mod color_tests;
mod colorui;
mod couleur;
#[cfg(test)]
mod couleur_tests;
mod cues;
#[cfg(test)]
mod cues_tests;
#[cfg(test)]
mod depth_tests;
mod dock;
#[cfg(test)]
mod dock_tests;
mod dockui;
pub mod ctg;
#[cfg(test)]
mod ctg_tests;
mod ctgui;
mod document;
mod export;
#[cfg(test)]
mod export_tests;
mod exportui;
mod fill;
mod folders;
pub mod fx;
#[cfg(test)]
mod fx_tests;
mod fxui;
#[cfg(test)]
mod folders_tests;
mod frames;
#[cfg(test)]
mod frames_tests;
mod gpu;
mod groups;
mod groupsui;
mod handles;
#[cfg(test)]
mod handles_tests;
mod guides;
#[cfg(test)]
mod guides_tests;
mod history;
mod images;
#[cfg(test)]
mod images_tests;
mod io;
mod keymap;
#[cfg(test)]
mod latency_tests;
#[cfg(test)]
mod layerflags_tests;
mod layerheads;
mod layerops;
#[cfg(test)]
mod layerops_tests;
mod layersui;
mod layertools;
mod marks;
#[cfg(test)]
mod keymap_tests;
#[cfg(test)]
mod ops_tests;
mod model3d;
mod montagekeys;
mod springtools;
#[cfg(test)]
mod springtools_tests;
mod montagerows;
#[cfg(test)]
mod montagerows_tests;
mod montagehead;
#[cfg(test)]
mod montagehead_tests;
mod nudge;
#[cfg(test)]
mod nudge_tests;
pub mod moteur;
mod newlayer;
mod notes;
mod nuancier;
#[cfg(test)]
mod nuancier_tests;
mod nuancierui;
#[cfg(test)]
mod moteur_tests;
pub mod noyau;
#[cfg(test)]
mod noyau_tests;
pub mod flips;
#[cfg(test)]
mod flips_tests;
mod flipsui;
mod onion;
#[cfg(test)]
mod onion_tests;
mod onionui;
pub mod projetui;
#[cfg(test)]
mod panelclip_tests;
mod panels;
mod pdf;
#[cfg(test)]
mod pdf_tests;
mod persp;
#[cfg(test)]
mod persp_tests;
mod perstools;
mod playback;
#[cfg(test)]
mod playback_tests;
mod pixsel;
#[cfg(test)]
mod pixsel_tests;
mod pixtools;
#[cfg(test)]
mod plans_tests;
mod project;
mod propager;
#[cfg(test)]
mod propager_tests;
mod propagerui;
mod raccourcisui;
mod raster;
mod recovery;
#[cfg(test)]
mod recovery_tests;
mod recoveryui;
mod ref3dtool;
mod render;
#[cfg(test)]
mod resize_tests;
pub mod sbbatch;
pub mod sbexport;
#[cfg(test)]
mod sbexport_tests;
mod sbexportui;
pub mod sboard;
#[cfg(test)]
mod sboard_tests;
mod sbimport;
pub mod sbscript;
mod sbui;
pub mod sbvector;
#[cfg(test)]
mod sbvector_tests;
pub mod stretch;
pub mod xml;
pub mod roles;
#[cfg(test)]
mod roles_tests;
mod rolesui;
mod roue;
#[cfg(test)]
mod roue_tests;
#[cfg(test)]
mod scenelayer_tests;
mod settings;
mod shapes;
#[cfg(test)]
mod shapes_tests;
mod shapetools;
mod soundtrack;
mod stamp;
#[cfg(test)]
mod stamp_tests;
mod stamptools;
mod text;
#[cfg(test)]
mod text_tests;
mod texttools;
mod stencil;
#[cfg(test)]
mod tabs_tests;
mod stage3d;
#[cfg(test)]
mod stage3d_tests;
mod stage3dui;
#[cfg(test)]
mod stage_tests;
mod storyboard;
mod tbbrush;
#[cfg(test)]
mod tests;
mod theme;
#[cfg(test)]
mod theme_tests;
mod thumbs;
mod timeline;
mod toolbars;
mod topview;
#[cfg(test)]
mod topview_tests;
pub mod tween;
#[cfg(test)]
mod tween_tests;
mod tweenui;
#[cfg(test)]
mod transform_tests;
mod vector;
pub mod video;
#[cfg(test)]
mod video_tests;
mod videoui;
mod view;
mod view3d;
#[cfg(test)]
mod view3d_tests;
mod viewbar;
#[cfg(test)]
mod vtexture_tests;
mod vtools;
mod workspace;
#[cfg(test)]
mod workspace_tests;
mod workspaceui;
mod xsheet;
#[cfg(test)]
mod xsheet_tests;
mod xsheetui;

/// Lance Pelure (l'application egui). Un projet passé en argument (ou par double-clic sur un
/// .pelure) est ouvert au démarrage.
pub fn lancer() -> eframe::Result<()> {
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
        // Affichage sans attente de la synchro de l'écran, une seule image d'avance : chaque
        // mouvement du stylet est affiché aussitôt, pour que le cercle et le trait restent collés
        // à la croix. En fenêtre, Windows compose l'image à la synchro (pas de déchirement) ; le
        // rythme est limité dans `App::update` pour ne pas faire tourner la carte pour rien.
        // « Auto » : si la carte ne sait pas le faire, on revient à l'affichage synchronisé.
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            present_mode: eframe::egui_wgpu::wgpu::PresentMode::AutoNoVsync,
            desired_maximum_frame_latency: Some(1),
            ..Default::default()
        },
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
