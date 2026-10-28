//! SHOT (Frame the story ; anciennement Pelure) en bibliothèque (10a) : tous les modules,
//! l'application egui (`lancer`), et le noyau (`noyau`) que d'autres interfaces peuvent utiliser
//! — l'appli compagnon Tauri (`src-tauri/`). L'exécutable `shot` (`main.rs`) ne fait qu'appeler
//! `lancer`. La bibliothèque garde le nom `pelure` (les chemins `pelure::` ne changent pas).

mod accueil;
mod accueilimages;
#[cfg(test)]
mod accueilimages_tests;
#[cfg(test)]
mod accueil_tests;
// Accueil et confort (passe « simplicité » du 2026-10-21) : mode concentration, conseils d'un seul passage, taille de
// l'interface, aide-mémoire des raccourcis, réglage du stylet à la main.
mod aidetouches;
#[cfg(test)]
mod aidetouches_tests;
mod concentration;
#[cfg(test)]
mod concentration_tests;
mod confort;
mod conseils;
#[cfg(test)]
mod conseils_tests;
mod echelle;
#[cfg(test)]
mod echelle_tests;
mod reglagestylet;
#[cfg(test)]
mod reglagestylet_tests;
mod aproposui;
#[cfg(test)]
mod aproposui_tests;
mod diagbanc;
mod diagnostic;
#[cfg(test)]
mod diagnostic_tests;
mod diagnosticui;
mod donnees;
#[cfg(test)]
mod donnees_tests;
mod materiel;
// Le budget de mémoire vidéo du système : le cache des textures de dessin se fait plus petit quand il manque.
mod memoirevideo;
#[cfg(test)]
mod memoirevideo_tests;
#[cfg(test)]
mod demarrage_tests;
mod ouverture;
// Onglets de projets : l'état échangé et les opérations (`onglets`), la bande d'onglets (`ongletsui`).
mod onglets;
#[cfg(test)]
mod onglets_tests;
mod ongletsui;
// Coller depuis un projet d'une autre taille de feuille : la règle d'adaptation (`adaptation`, pure) et le parcours
// du collage, avec sa question et son calcul de fond (`collage`).
mod adaptation;
#[cfg(test)]
mod adaptation_tests;
mod collage;
#[cfg(test)]
mod collage_tests;
mod enregistrement;
#[cfg(test)]
mod enregistrement_tests;
#[cfg(test)]
mod enregistrement_cliche_tests;
// Le manifeste des projets passe de `.pelure` à `.shot` (2026-10-23) : nouveaux projets, conversion, plantage au milieu.
#[cfg(test)]
mod extension_tests;
#[cfg(test)]
mod enregistrement_identique_tests;
// Travailler à plusieurs sur un film (`docs/COOPERATION.md`) : l'équipier (`equipe`), les prises (`verrous`, pur), le
// stockage (`depot`), les commentaires de chacun (`partagecom`), la session (`cooperation`) et son interface
// (`cooperationui`). Le format est `projet_partage.rs`, sous-module de `project`.
mod equipe;
mod verrous;
mod depot;
mod partagecom;
mod cooperation;
#[cfg(test)]
mod cooperation_tests;
#[cfg(test)]
mod cooperation_ui_tests;
#[cfg(test)]
mod cooperation_modele_tests;
#[cfg(test)]
mod cooperation_mesures_tests;
mod cooperationui;
// Empreintes de contenu, envois (« v2 au réal ») et plans modifiés depuis le dernier : le modèle et le
// fichier (`envois`), l'interface (`envoisui`).
mod empreinte;
mod envois;
#[cfg(test)]
mod envois_tests;
mod envoisui;
// Versions d'un plan (V1, V2, V3…) : le modèle et le fichier (`planvers`), l'interface (`planversui`).
mod planvers;
#[cfg(test)]
mod planvers_tests;
mod planversui;
// Variantes de séquence (A · B · C) : le modèle et le fichier (`variantes`), l'interface (`variantesui`).
mod variantes;
#[cfg(test)]
mod variantes_tests;
mod variantesui;
// La pile des versions au survol de « V2 » sur le bloc d'un plan : la logique sans interface (`pilevers`) et son
// dessin (`pileversui`).
mod pilevers;
#[cfg(test)]
mod pilevers_tests;
mod pileversui;
// Comparer un plan avec lui-même (deux versions, ou aujourd'hui et un envoi) : le calcul (`comparer`),
// la fenêtre (`comparerui`).
mod comparer;
#[cfg(test)]
mod comparer_tests;
mod comparerui;
#[cfg(test)]
mod gpu_contenu_tests;
mod parallele;
#[cfg(test)]
mod parallele_tests;
mod perf;
#[cfg(test)]
mod perf_tests;
#[cfg(test)]
mod optim_rendu_tests;
// Projets lourds (300 plans et plus) : générateur et bancs, tous `#[ignore]` (`docs/PERFORMANCE.md`, section 25).
#[cfg(test)]
mod lourd_tests;
mod lacher;
#[cfg(test)]
mod lacher_tests;
pub mod logo;
#[cfg(test)]
mod logo_tests;
mod ajuste;
#[cfg(test)]
mod ajuste_tests;
mod app;
mod audio;
#[cfg(test)]
mod audio_tests;
// Effets sonores des extraits et des pistes : fondus, enchaîné, volume, égaliseur, écho, solo.
mod audiofx;
#[cfg(test)]
mod audiofx_tests;
mod audiofxui;
// Bibliothèque de bruitages (panneau Bruitages) : dossiers, recherche, écoute, pose dans le montage.
mod bruitages;
#[cfg(test)]
mod bruitages_tests;
mod bruitagesui;
// Sons proposés par la lecture du storyboard (panneau Suggestions sonores) : le modèle d'une
// proposition et son fichier, la lecture (par règles ou par un modèle), les fournisseurs locaux et en
// ligne, les tâches, l'interface et les Préférences.
mod base64;
#[cfg(test)]
mod base64_tests;
mod clessecretes;
mod fournisseurs;
mod fournisseursapi;
#[cfg(test)]
mod fournisseurs_tests;
mod generation;
mod genprefsui;
mod genreglages;
mod httplocal;
mod indications;
#[cfg(test)]
mod indications_tests;
mod lecturerules;
mod lecturesb;
mod serveurlocal;
#[cfg(test)]
mod serveurlocal_tests;
mod sidecar;
mod suggestionsui;
#[cfg(test)]
mod suggestions_tests;
mod barrepanneaux;
#[cfg(test)]
mod barrepanneaux_tests;
mod barres;
#[cfg(test)]
mod barres_tests;
mod legendecles;
#[cfg(test)]
mod legendecles_tests;
#[cfg(test)]
mod doublons_tests;
#[cfg(test)]
mod menus_tests;
pub mod fond;
#[cfg(test)]
mod fond_tests;
#[cfg(test)]
mod aplat_tests;
#[cfg(test)]
mod perte_tests;
#[cfg(test)]
mod rejeu_tests;
pub mod flottante;
pub mod warp;
#[cfg(test)]
mod warp_tests;
pub mod contours;
#[cfg(test)]
mod contours_tests;
#[cfg(test)]
mod selection_affichage_tests;
#[cfg(test)]
mod clesselection_tests;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod affichage_tests;
#[cfg(test)]
mod flottante_tests;
mod fondfil;
#[cfg(test)]
mod fondfil_tests;
mod fondui;
mod blend;
#[cfg(test)]
mod blend_tests;
mod board;
#[cfg(test)]
mod board_tests;
#[cfg(test)]
mod boardalignement_tests;
mod boardjapon;
#[cfg(test)]
mod boardjapon_tests;
mod boardmove;
mod boardsens;
#[cfg(test)]
mod boardtextes_tests;
mod boardui;
mod vignettespanneau;
mod debord;
#[cfg(test)]
mod debogage_tests;
// Le décor des Vignettes : un calque par plan, partagé par les cases d'une continuité, fait par trois brosses de la colonne des pastilles ;
// et les réglages des Vignettes (Préférences > Vignettes).
mod cases;
#[cfg(test)]
mod cases_tests;
#[cfg(test)]
mod retours_cases_tests;
#[cfg(test)]
mod retours_collage_tests;
#[cfg(test)]
mod retours_aplat_tests;
#[cfg(test)]
mod notes_cases_tests;
mod decor;
#[cfg(test)]
mod decor_tests;
mod decorui;
mod vignettesreglages;
#[cfg(test)]
mod vignettes_tests;
#[cfg(test)]
mod indices_tests;
#[cfg(test)]
mod vignettes_retouches_tests;
#[cfg(test)]
mod vignettes_retouches2_tests;
#[cfg(test)]
mod vignettes_maquette_tests;
// Annotations de la vue Vignettes (stylo et gomme, par-dessus la planche, sans effet sur le storyboard) : le modèle
// et le fichier (`annotations`), l'interface (`annotationsui`).
mod annotations;
mod annotationsui;
// Calques d'annotation : ceux de la planche (liste, lasso) et le calque d'annotation d'un plan (`calqueannot`).
mod calqueannot;
#[cfg(test)]
mod annotations_calques_tests;
#[cfg(test)]
mod annotations_retouches_tests;
#[cfg(test)]
mod calqueannot_tests;
mod deposer;
mod planche;
mod colorscript;
mod colorscriptui;
mod plancheui;
mod marie;
#[cfg(test)]
mod annotationsui_tests;
mod bottomtabs;
mod brush;
mod brushtex;
#[cfg(test)]
mod brushtex_tests;
mod brushui;
mod pinceaux;
#[cfg(test)]
mod pinceaux_tests;
#[cfg(test)]
mod pinceaux_reglages_tests;
mod pinceauxui;
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
#[cfg(test)]
mod toucher_tests;
#[cfg(test)]
mod lissage_tests;
mod bulles;
#[cfg(test)]
mod bulles_tests;
mod cadrage;
#[cfg(test)]
mod cadrage_tests;
mod cadrageui;
#[cfg(test)]
mod regle_cadrage_tests;
mod camera;
#[cfg(test)]
mod camera_tests;
mod camfx;
#[cfg(test)]
mod camfx_tests;
mod camfxui;
mod camkeys;
#[cfg(test)]
mod camkeys_tests;
mod clescommun;
#[cfg(test)]
mod clescommun_tests;
#[cfg(test)]
mod clesmulti_tests;
#[cfg(test)]
mod multiall_tests;
mod zones;
#[cfg(test)]
mod zones_tests;
mod zoombande;
#[cfg(test)]
mod zoombande_tests;
mod camtools;
mod canvas;
mod crayonnesheritage;
#[cfg(test)]
mod color_tests;
mod commentaires;
#[cfg(test)]
mod commentaires_tests;
mod commentairesui;
mod colorsel;
#[cfg(test)]
mod colorsel_tests;
mod recolorsel;
#[cfg(test)]
mod recolorsel_tests;
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
// Remplissage au griffonnage d'un calque de couleur (ce qu'était le calque CTG, fondu dans le calque de couleur).
pub mod griffonnage;
#[cfg(test)]
mod griffonnage_tests;
/// Ancien nom du remplissage au griffonnage : `propager.rs` (qu'on ne touche pas ici) écrit encore `crate::ctg::input`.
pub mod ctg {
    pub use crate::griffonnage::input;
}
// Calque de couleur : la couleur sous le trait d'un calque, qui suit ses images.
pub mod calquecouleur;
#[cfg(test)]
mod calquecouleur_tests;
mod calquecouleurui;
mod calquemenu;
mod repli;
pub mod monde;
// Le hors-feuille d'avant `optim-vram-monde`, pour les mesures avant / après dans le même processus.
#[cfg(test)]
mod monde_avant;
#[cfg(test)]
mod monde_tests;
// Toile infinie : tuiles hors de la feuille (essais).
#[cfg(test)]
mod toile_tests;
mod historyui;
#[cfg(test)]
mod historyui_tests;
mod messages;
#[cfg(test)]
mod messages_tests;
mod palette;
#[cfg(test)]
mod palette_tests;
mod prefsonglets;
mod prefsui;
#[cfg(test)]
mod prefsui_tests;
mod recents;
#[cfg(test)]
mod recents_tests;
mod nouveau;
#[cfg(test)]
mod nouveau_tests;
mod nouveauui;
mod aller;
#[cfg(test)]
mod aller_tests;
mod envelope;
#[cfg(test)]
mod envelope_tests;
mod envelopetools;
mod flip;
#[cfg(test)]
mod flip_tests;
mod pasteimage;
mod plantage;
#[cfg(test)]
mod plantage_tests;
#[cfg(test)]
mod pasteimage_tests;
mod cutter;
#[cfg(test)]
mod cutter_tests;
mod sculpt;
mod sculptapp;
#[cfg(test)]
mod sculpt_tests;
mod document;
mod export;
mod exportrender;
#[cfg(test)]
mod export_tests;
mod exportui;
mod fill;
mod folders;
pub mod demarrage;
// Filet de sécurité de la fiabilité (`docs/FIABILITE.md`) : un projet d'essai complet et les essais
// qui l'enregistrent, le rouvrent, le font tourner dans l'application et lui jettent des fichiers abîmés.
#[cfg(test)]
mod fiabilite_fixture;
// Essais au hasard des gestes dans l'application (passe de débogage du 2026-10-22).
#[cfg(test)]
mod hasard_tests;
// Postérisation temporelle de la caméra (« saccade »).
#[cfg(test)]
mod saccade_tests;
#[cfg(test)]
mod saccade_ecran_tests;
// L'aperçu du tremblement par défaut (Préférences > Dessin).
mod tremblementapercu;
#[cfg(test)]
mod tremblementapercu_tests;
// Fermer un projet : jamais de question quand rien n'a changé, « Enregistrer » enregistre puis ferme.
#[cfg(test)]
mod fermeture_tests;
// Transformer une sélection bitmap au zéro latence (feuille flottante bitmap).
#[cfg(test)]
mod selection_bitmap_tests;
// Fusionner deux plans en glissant l'un sur l'autre dans le montage.
mod fusionglisse;
#[cfg(test)]
mod fusionglisse_tests;
#[cfg(test)]
mod fiabilite_aller_retour_tests;
#[cfg(test)]
mod fiabilite_entrees_tests;
#[cfg(test)]
mod fiabilite_raster_tests;
#[cfg(test)]
mod fiabilite_annexes_tests;
#[cfg(test)]
mod fiabilite_manifestes_tests;
#[cfg(test)]
mod fiabilite_dessins_tests;
#[cfg(test)]
mod fiabilite_fumee_tests;
#[cfg(test)]
mod fiabilite_demarrage_tests;
pub mod fx;
mod fxcache;
mod fxproxy;
#[cfg(test)]
mod fx_tests;
#[cfg(test)]
mod fxexact_tests;
#[cfg(test)]
mod fxperf_tests;
mod fxfilters;
#[cfg(test)]
mod fxfilters_tests;
mod fxwarp;
#[cfg(test)]
mod fxwarp_tests;
// Calques d'effets et préréglages d'effets (impact frame, lueur, flash, secousse, timecode…).
mod fxcine;
#[cfg(test)]
mod fxcine_tests;
mod calqueffet;
#[cfg(test)]
mod calqueffet_tests;
mod incrustation;
#[cfg(test)]
mod incrustation_tests;
mod prereglages;
#[cfg(test)]
mod prereglages_tests;
mod prereglagesui;
#[cfg(test)]
mod prereglagesui_tests;
#[cfg(test)]
mod effetsperf_tests;
#[cfg(test)]
mod passe_partout_tests;
mod tremblement;
#[cfg(test)]
mod tremblement_tests;
#[cfg(test)]
mod tremblement_defaut_tests;
mod fxtrait;
#[cfg(test)]
mod fxtrait_tests;
mod fxmatiere;
#[cfg(test)]
mod fxmatiere_tests;
mod fxui;
#[cfg(test)]
mod notes_calques_tests;
mod fxkeys;
#[cfg(test)]
mod fxkeys_tests;
mod fxcopie;
#[cfg(test)]
mod fxclavier_tests;
#[cfg(test)]
mod fxcopie_tests;
#[cfg(test)]
mod folders_tests;
mod frames;
#[cfg(test)]
mod frames_tests;
mod gpu;
mod gpufx;
#[cfg(test)]
mod gpufx_tests;
#[cfg(test)]
mod gpufxperf_tests;
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
#[cfg(test)]
mod panneaux_tests;
mod layersui;
mod layertools;
mod marks;
#[cfg(test)]
mod matiere_tests;
#[cfg(test)]
mod vecto_tests;
#[cfg(test)]
mod revue_tests;
#[cfg(test)]
mod keymap_tests;
#[cfg(test)]
mod keymap_jeux_tests;
#[cfg(test)]
mod ops_tests;
mod montagekeys;
mod references;
#[cfg(test)]
mod references_tests;
mod referencesui;
// Relecture par le réalisateur : la page HTML hors ligne, les notes qui reviennent (`relecture`), la tâche
// d'export (`relecturejob`) et les fenêtres (`relectureui`).
mod relecture;
#[cfg(test)]
mod relecture_tests;
mod relecturejob;
mod relectureui;
mod rognage;
#[cfg(test)]
mod rognage_tests;
mod rognageui;
mod springtools;
#[cfg(test)]
mod springtools_tests;
// Notes d'Anthony, lot « dessin » (2026-10-20) : tracé droit, sélection d'un objet au clic, remplir la
// sélection, clés de position.
mod tracedroit;
mod remplirsel;
mod selobjet;
pub mod poskeys;
mod poskeysui;
#[cfg(test)]
mod poskeysmontage_tests;
#[cfg(test)]
mod dessin_notes_tests;
mod montagerows;
#[cfg(test)]
mod montagerows_tests;
mod montagehead;
#[cfg(test)]
mod montagehead_tests;
#[cfg(test)]
mod montageonglet_tests;
mod vignettecam;
mod vignettecamui;
#[cfg(test)]
mod vignettes_camera_tests;
mod pistes;
#[cfg(test)]
mod pistes_tests;
#[cfg(test)]
mod pastilles_storyboard_tests;
mod brushfill;
#[cfg(test)]
mod brushfill_tests;
mod nudge;
#[cfg(test)]
mod nudge_tests;
mod multisel;
#[cfg(test)]
mod multisel_tests;
mod multitools;
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
mod persofond;
mod personnages;
#[cfg(test)]
mod personnages_tests;
mod personnagesui;
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
mod coloriage;
#[cfg(test)]
mod coloriage_tests;
mod coloriageui;
mod propager;
#[cfg(test)]
mod propager_tests;
#[cfg(test)]
mod propager_cadence_tests;
mod propagerui;
mod raccourcisui;
mod raster;
mod rejeu;
mod recovery;
#[cfg(test)]
mod recovery_tests;
mod recoveryui;
mod render;
#[cfg(test)]
mod rendu_audit_tests;
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
mod scenario;
mod scenariofdx;
mod scenarioplans;
mod scenarioui;
#[cfg(test)]
mod scenario_tests;
#[cfg(test)]
mod scenelayer_tests;
mod selbox;
#[cfg(test)]
mod selbox_pointfixe_tests;
#[cfg(test)]
mod selbox_tests;
mod seltools;
mod settings;
mod shapes;
#[cfg(test)]
mod shapes_tests;
mod shapetools;
mod soundtrack;
// Numérotation des plans : suivie ou verrouillée (12, 12A, 13), renuméroter.
mod numerotation;
#[cfg(test)]
mod numerotation_tests;
mod numerotationui;
// Exports pour le montage (EDL, Final Cut XML, OpenTimelineIO) : un fichier vidéo par plan.
mod editorial;
#[cfg(test)]
mod editorial_tests;
mod editorialjob;
mod editorialui;
// Fiche technique (tableur CSV / XLSX, un plan par ligne) : le contenu (`fiche`), les formats
// (`ficheformats`, `zipwrite`), la tâche (`fichejob`) et la fenêtre (`ficheui`).
mod fiche;
#[cfg(test)]
mod fiche_tests;
mod fichejob;
mod ficheformats;
mod ficheui;
mod zipwrite;
// Transitions du montage (fondus, volet, noir) : modèle, rendu, interface.
mod transitions;
#[cfg(test)]
mod transitions_tests;
mod transitionsui;
mod text;
#[cfg(test)]
mod text_tests;
mod texttools;
mod stencil;
#[cfg(test)]
mod tabs_tests;
// Espace 3D (`docs/ESPACE3D.md`) : les calques (cartes) et la caméra du plan dans un seul espace, sans objets 3D.
mod espace3d;
mod espace3dgizmo;
mod espace3dmath;
#[cfg(test)]
mod espace3dmath_tests;
mod espace3dui;
mod espace3dview;
#[cfg(test)]
mod espace3d_tests;
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

/// Lance SHOT (l'application egui). Un projet passé en argument (`.shot`, ou l'ancien `.pelure` ; un double-clic
/// quand une installation aura associé l'extension) est ouvert au démarrage.
pub fn lancer() -> eframe::Result<()> {
    demarrage::debut();
    // Une erreur de programme laisse un rapport (`plantage.rs`).
    plantage::install();
    demarrage::marque("rapport de plantage installé");
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
            .with_title("SHOT")
            // Icône de la fenêtre : le « S » du logo entre deux angles (`logo::icone`).
            .with_icon(std::sync::Arc::new(eframe::egui::IconData { rgba: logo::icone(64), width: 64, height: 64 }))
            .with_inner_size([1500.0, 920.0])
            .with_min_inner_size([960.0, 600.0])
            // Ouverture en plein écran (Anthony, 2026-10-18), fenêtre maximisée comme Storyboard Pro.
            .with_maximized(true),
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
        "SHOT",
        options,
        Box::new(move |cc| {
            // (Fenêtre créée, carte graphique choisie et initialisée par eframe : ce temps-là n'est pas à nous.)
            demarrage::marque("fenêtre et carte graphique prêtes (eframe)");
            let mut app = app::App::new(cc);
            demarrage::marque("App::new");
            match initial {
                Some(path) => app.open_path(path),
                // Sans projet demandé : le dernier ouvert (`recents.rs`), si son disque répond.
                None => app.reopen_last_project(),
            }
            demarrage::marque("projet demandé au lancement");
            // La séance d'avant s'est fermée sur une erreur : on le dit, avec où lire le rapport.
            if let Some(report) = plantage::dir().and_then(|d| plantage::take_new(&d)) {
                app.status = format!("Une erreur a été notée pendant la dernière séance : rapport dans {}.", report.display());
            }
            Ok(Box::new(app))
        }),
    )
}
