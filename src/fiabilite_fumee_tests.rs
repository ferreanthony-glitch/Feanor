//! Essai de fumée de toute l'application, sans fenêtre (`App::sans_fenetre`, `App::frame`) : on ouvre
//! le projet d'essai complet (`fiabilite_fixture.rs`) et l'on passe plus de cent images à travers tout
//! ce qu'un lancement un peu sérieux fait — storyboard et animation, tous les panneaux, les réglages de
//! chaque outil, l'écran d'accueil, la fenêtre « Nouveau projet », les Préférences, « À propos », les
//! repères de cadrage, une lecture de 48 images, le défilement de la tête de lecture, un « tout
//! sélectionner » suivi d'un déplacement par la boîte de sélection, un effet de caméra, et
//! l'annulation / le rétablissement de chacun.
//!
//! Chaque image doit : ne pas planter, produire de l'interface (des formes pour egui), et tenir dans un
//! budget de temps large. Un dépassement est signalé (`ATTENTION`) et ne fait échouer l'essai que sur
//! une machine qui n'est pas lente (mesurée par un petit calcul de référence ; `SHOT_FUMEE_SOUPLE=1`
//! force l'indulgence). La carte graphique de la machine est utilisée si elle répond (la composition
//! se fait comme à l'écran) ; sinon le calcul se fait sur le processeur (`calques par le processeur`).

use std::time::Instant;

use eframe::egui::{vec2, Event, Modifiers, PointerButton};

use crate::app::{Mode, Tool};
use crate::dock::Pane;
use crate::fiabilite_fixture::{dossier, projet_complet};
use crate::perf_tests::{banc_taille, Banc};
use crate::selbox::Handle;

/// Une image de plus de ça (millisecondes d'interface) est un défaut sur une machine normale.
const BUDGET_MS: f64 = 15_000.0;
/// Au-delà, une image est signalée, sans plus.
const SIGNALE_MS: f64 = 500.0;

/// La machine est-elle lente ? Un calcul de référence qui prend ~25 ms sur une machine d'aujourd'hui.
fn machine_lente() -> bool {
    if std::env::var("SHOT_FUMEE_SOUPLE").is_ok() {
        return true;
    }
    let t = Instant::now();
    let mut x = 0u64;
    for i in 0..30_000_000u64 {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(i);
    }
    std::hint::black_box(x);
    t.elapsed().as_millis() > 400
}

/// Le banc, les images déjà passées et leurs temps.
struct Fumee {
    b: Banc,
    lent: bool,
    temps: Vec<(String, f64)>,
}

impl Fumee {
    /// Une image : de l'interface est produite, dans le budget de temps.
    fn pas(&mut self, nom: &str, events: Vec<Event>, dt: f64) {
        let m = self.b.image(events, dt);
        assert!(m.formes > 0 && m.sommets > 0, "image « {nom} » : aucune interface produite ({} formes, {} sommets)", m.formes, m.sommets);
        let ms = m.construction;
        if ms > SIGNALE_MS {
            println!("ATTENTION image « {nom} » : {ms:.0} ms d'interface (budget {BUDGET_MS:.0} ms)");
        }
        if ms > BUDGET_MS && !self.lent {
            panic!("image « {nom} » : {ms:.0} ms d'interface, plus que le budget de {BUDGET_MS:.0} ms (machine normale)");
        }
        self.temps.push((nom.to_string(), ms));
    }

    fn pas_simple(&mut self, nom: &str) {
        self.pas(nom, Vec::new(), 1.0 / 60.0);
    }

    fn images(&mut self, nom: &str, n: usize) {
        for k in 0..n {
            self.pas(&format!("{nom} {}/{n}", k + 1), Vec::new(), 1.0 / 60.0);
        }
    }
}

/// Le dessin du plan 1 / calque « Personnage » / image 0 : trois éléments vectoriels dont deux traits
/// texturés. Sa version JSON, pour voir si une opération l'a changé.
fn dessin_courant(f: &Fumee) -> String {
    let id = f.b.app.cel_here().expect("un dessin sous la tête de lecture");
    serde_json::to_string(f.b.app.doc.cels[&id].vector.as_ref().expect("dessin vectoriel")).expect("écriture du dessin")
}

/// Avec la carte graphique de la machine (si elle répond : sinon le calcul se fait sur le processeur,
/// comme le second essai).
#[test]
fn the_whole_application_runs_a_hundred_and_twenty_frames_on_a_full_project() {
    passer(true);
}

/// La composition sur le processeur (le moteur de secours, et la machine sans carte graphique).
#[test]
fn the_whole_application_runs_a_hundred_and_twenty_frames_on_the_cpu_path() {
    passer(false);
    // (Le banc sans carte graphique compose sur le processeur : `calques par le processeur`.)
}

fn passer(avec_gpu: bool) {
    let travail = dossier(if avec_gpu { "fumee-gpu" } else { "fumee-cpu" });
    let fixture = projet_complet(&travail);
    // Le dossier de réglages de l'essai est celui du fil (`banc_taille`) : rien n'est écrit chez Anthony.
    let mut f = Fumee { b: banc_taille(fixture.doc, avec_gpu, 1280, 720), lent: machine_lente(), temps: Vec::new() };
    assert!(avec_gpu || f.b.app.gpu.is_none(), "sans carte graphique, le calcul se fait sur le processeur");
    println!("FIABILITE fumée : composition {} ; machine {}", f.b.app.display_info, if f.lent { "lente (budget indulgent)" } else { "normale" });
    // Ni son (la carte son n'est pas l'objet), ni rejeu de sauvegarde automatique.
    f.b.app.sound_on = false;
    f.b.app.sound_scrub = false;
    f.b.app.doc.storyboard_locked = false;
    let t0 = Instant::now();

    // ------------------------------------------------------------ l'accueil et les fenêtres
    f.b.app.welcome = true;
    f.images("accueil", 3);
    f.b.app.open_new_project();
    f.images("fenêtre Nouveau projet", 3);
    f.b.app.new_dialog = None;
    f.b.app.welcome = false;
    f.b.app.prefs_window = true;
    f.images("Préférences", 3);
    f.b.app.prefs_window = false;
    f.b.app.about_window = true;
    f.images("À propos", 3);
    f.b.app.about_window = false;

    // ------------------------------------------------------------ storyboard : montage et panneaux
    f.b.app.set_mode(Mode::Storyboard);
    f.images("storyboard", 3);
    for pane in Pane::ALL {
        f.b.app.show_pane(pane);
        f.pas_simple(&format!("storyboard, panneau {}", pane.name()));
    }
    // Le montage lit l'image de chaque plan, effets compris (vignettes en attente de leurs effets).
    f.images("storyboard, vignettes", 6);

    // ------------------------------------------------------------ animation
    f.b.app.enter_animation(0);
    f.b.app.cur_layer = 2; // « Personnage » : vectoriel, avec ses effets et son fond sous le trait
    f.b.app.cur_frame = 0;
    for pane in Pane::ALL {
        f.b.app.show_pane(pane);
        f.pas_simple(&format!("animation, panneau {}", pane.name()));
    }
    f.b.app.cadrage.thirds = true;
    f.b.app.cadrage.phi_grid = true;
    f.b.app.cadrage.spiral = true;
    f.b.app.cadrage.diagonals = true;
    f.b.app.cadrage.cross = true;
    f.images("repères de cadrage", 3);
    // Les réglages de chaque outil.
    f.b.app.show_pane(Pane::ToolOptions);
    for tool in Tool::ALL {
        f.b.app.set_tool(tool);
        f.pas_simple(&format!("réglages de l'outil {}", tool.label()));
    }
    f.b.app.cadrage.thirds = false;
    f.b.app.cadrage.phi_grid = false;
    f.b.app.cadrage.spiral = false;
    f.b.app.cadrage.diagonals = false;
    f.b.app.cadrage.cross = false;
    f.b.app.set_tool(Tool::Brush);

    // ------------------------------------------------------------ lecture et défilement
    f.b.app.cur_frame = 0;
    f.b.app.toggle_play_scope(false);
    assert!(f.b.app.playing, "la lecture démarre");
    for k in 0..48 {
        f.pas(&format!("lecture {}/48", k + 1), Vec::new(), 1.0 / 24.0);
    }
    f.b.app.toggle_play_scope(false);
    assert!(!f.b.app.playing);
    for k in (0..24).chain((0..24).rev()) {
        f.b.app.goto_frame(k);
        f.pas(&format!("défilement, image {k}"), Vec::new(), 1.0 / 60.0);
    }
    f.b.app.goto_frame(0);
    f.images("repos", 2);

    // ------------------------------------------------------------ tout sélectionner, déplacer par la boîte
    let avant = dessin_courant(&f);
    let n_avant = f.b.app.history.names().0.len();
    f.b.app.select_all();
    assert!(f.b.app.vsel.is_some(), "tout est sélectionné");
    f.images("sélection", 2);
    let cadre = f.b.app.sel_frame().expect("la boîte de sélection");
    let pivot = f.b.app.layer_to_screen(cadre.pivot_world());
    // Un point dedans, qui n'est ni une poignée ni le pivot : déplacer.
    let depart = (-6..=6)
        .flat_map(|i| (-6..=6).map(move |j| vec2(i as f32 * 12.0, j as f32 * 12.0)))
        .map(|d| pivot + d)
        .find(|p| f.b.app.box_hit(*p) == Some(Handle::Move))
        .expect("un point de la boîte qui la déplace");
    let tire = |k: usize, n: usize| depart + vec2(70.0, 40.0) * (k as f32 / n as f32);
    f.pas("déplacement : survol", vec![Event::PointerMoved(depart)], 1.0 / 60.0);
    f.pas("déplacement : appui", vec![Event::PointerButton { pos: depart, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    for k in 1..=6 {
        f.pas(&format!("déplacement {k}/6"), vec![Event::PointerMoved(tire(k, 6))], 1.0 / 60.0);
    }
    f.pas("déplacement : lâcher", vec![Event::PointerButton { pos: tire(6, 6), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    f.images("après le déplacement", 3);
    let apres = dessin_courant(&f);
    assert_ne!(apres, avant, "le déplacement par la boîte a changé le dessin");
    assert!(f.b.app.history.names().0.len() > n_avant, "le déplacement est une étape d'annulation");
    f.b.app.undo();
    f.pas_simple("annuler le déplacement");
    assert_eq!(dessin_courant(&f), avant, "annuler remet le dessin d'avant");
    f.b.app.redo();
    f.pas_simple("rétablir le déplacement");
    assert_eq!(dessin_courant(&f), apres, "rétablir remet le déplacement");
    f.b.app.undo();
    f.pas_simple("annuler de nouveau");
    f.b.app.vsel = None;

    // ------------------------------------------------------------ un effet de caméra
    f.b.app.cam_fx = crate::camfx::Params::new(crate::camfx::Preset::Jolt, 24);
    f.b.app.cur_frame = 2;
    let cles_avant = format!("{:?}", f.b.app.doc.panels[0].camera);
    f.b.app.apply_camera_effect();
    f.images("effet de caméra", 2);
    let cles_apres = format!("{:?}", f.b.app.doc.panels[0].camera);
    assert_ne!(cles_apres, cles_avant, "l'effet de caméra pose des clés ({})", f.b.app.status);
    f.b.app.undo();
    f.pas_simple("annuler l'effet de caméra");
    assert_eq!(format!("{:?}", f.b.app.doc.panels[0].camera), cles_avant);
    f.b.app.redo();
    f.pas_simple("rétablir l'effet de caméra");
    assert_eq!(format!("{:?}", f.b.app.doc.panels[0].camera), cles_apres);

    // ------------------------------------------------------------ retour au storyboard, plans suivants
    f.b.app.set_mode(Mode::Storyboard);
    for plan in 0..f.b.app.doc.panels.len() {
        f.b.app.select_panel(plan);
        f.pas_simple(&format!("storyboard, plan {}", plan + 1));
    }
    f.images("fin", 3);

    let total = f.temps.len();
    let pire = f.temps.iter().max_by(|a, b| a.1.total_cmp(&b.1)).expect("des images");
    let moyenne = f.temps.iter().map(|t| t.1).sum::<f64>() / total as f64;
    println!("FIABILITE fumée : {total} images en {:.1} s, interface {moyenne:.1} ms en moyenne, la plus lente « {} » ({:.0} ms)", t0.elapsed().as_secs_f64(), pire.0, pire.1);
    assert!(total >= 120, "au moins 120 images ({total})");
    let _ = std::fs::remove_dir_all(&travail);
}

/// Les vignettes d'un calque à effets pendant que ses effets se calculent : le dessin servi est le
/// dernier résultat gardé, qui peut être un aperçu réduit (`fxproxy.rs`). C'est exactement ce qui a
/// fermé SHOT au lancement le 2026-10-18 (`raster.rs:57`) : toutes les vignettes de tous les plans du
/// projet d'essai, sans résultat gardé, puis avec un aperçu réduit gardé pour chaque dessin à effets.
#[test]
fn thumbnails_of_effect_layers_do_not_panic_while_the_effects_compute() {
    use crate::document::Cel;
    use crate::render;
    // (Elle vide et remplit le cache des effets, commun à tous les essais : `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let travail = dossier("fumee-vignettes");
    let f = projet_complet(&travail);
    let doc = f.doc;
    let cam = doc.default_camera();
    crate::fx::forget_all();
    let mut avec_effets = 0;
    let mut passes = 0;
    for tour in 0..2 {
        for pi in 0..doc.panels.len() {
            for frame in [0, doc.panels[pi].frame_count() / 2, doc.panels[pi].frame_count() - 1] {
                let plan = render::panel_plan_at(&doc, pi, frame, Some(doc.paper));
                if tour == 1 {
                    // Un aperçu réduit (moitié, quart) gardé comme dernier résultat de chaque dessin à effets.
                    for item in plan.items.iter().filter(|i| !i.fx.is_empty()) {
                        let Some(src) = doc.cels.get(&item.cel) else { continue };
                        for level in [1u8, 2] {
                            let petit = crate::fxproxy::downscale(&src.raster, level);
                            let id = item.cel | (1 << 61) | ((level as u64) << 56);
                            crate::fxcache::get_or_compute(id, src.version, Some(item.cel), || Cel::bitmap(petit));
                            let (image, _) = crate::fxcache::soft_scope(|| render::thumbnail(&doc, &plan, &cam, 96, 54));
                            assert_eq!(image.pixels.len(), 96 * 54);
                            passes += 1;
                        }
                    }
                }
                avec_effets += plan.items.iter().filter(|i| !i.fx.is_empty()).count();
                let (image, _) = crate::fxcache::soft_scope(|| render::thumbnail(&doc, &plan, &cam, 96, 54));
                assert_eq!(image.pixels.len(), 96 * 54);
                // Le même plan en pleine composition (export, secours) sur un aperçu gardé.
                if tour == 1 {
                    let tout = render::composite_full(&doc, &plan);
                    assert_eq!(tout.len(), doc.width * doc.height);
                }
            }
        }
    }
    assert!(avec_effets > 0 && passes > 0, "le projet d'essai a des dessins à effets ({avec_effets}, {passes})");
    crate::fx::forget_all();
    let _ = std::fs::remove_dir_all(&travail);
}
