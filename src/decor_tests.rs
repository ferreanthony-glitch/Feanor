//! Essais du décor des Vignettes (2026-10-22) : un calque « Décor » par plan, fait par trois brosses de la colonne des pastilles, partagé par
//! les cases d'une continuité ; et les réglages des Vignettes (Préférences > Vignettes).

use eframe::egui::{self, epaint::Shape, vec2, Color32, Event, Modifiers, Pos2, Rect};

use crate::app::Tool;
use crate::decor::Genre;
use crate::document::{Behaviour, CelId, Document, LayerKind};
use crate::perf_tests::{se_reposer, Banc};
use crate::personnagesui::Case;
use crate::prefsonglets::{Onglet, REGLAGES};
use crate::raster::TILE;
use crate::render;
use crate::settings::Settings;
use crate::vignettes_camera_tests::{avec_b_a_droite, cadre_ecran, centre, clefs, ctrl_glisser, pas};
use crate::vignettes_tests::{appui, case_vide, dernier_nom, glisser, ouvert, pas_d_annulation, quatre_plans, vague, vignette};
use crate::vignettesreglages::{ReglagesVignettes, CADRE_DEFAUT};

const ROUGE: Color32 = Color32::from_rgb(200, 30, 30);
const VERT: Color32 = Color32::from_rgb(30, 160, 60);
const GRIS_BLEU: Color32 = Color32::from_rgb(0x8F, 0xA3, 0xB8);

// ------------------------------------------------------------------------------------------ outils du modèle

fn pixel(doc: &mut Document, cel: CelId, x: usize, y: usize, c: Color32) {
    let c_ = doc.cels.get_mut(&cel).expect("le dessin");
    let i = c_.raster.tile_index_of(x, y);
    c_.raster.tile_mut(i)[(y % TILE) * TILE + x % TILE] = c;
    c_.version += 1;
}

fn a_de_l_encre(doc: &Document, cel: CelId) -> bool {
    doc.cels[&cel].raster.tiles.iter().flatten().any(|t| t.iter().any(|p| p.a() != 0))
}

/// Un personnage (un dessin tenu de `de` à `a` exclu, un pixel `c` en (`x`, 20)) dans le calque 0 du plan `i`.
fn personnage(doc: &mut Document, i: usize, de: usize, a: usize, x: usize, c: Color32) -> CelId {
    let cel = doc.new_cel(LayerKind::Bitmap);
    pixel(doc, cel, x, 20, c);
    let l = &mut doc.panels[i].layers[0];
    for f in de..a {
        l.set(f, Some(cel));
    }
    cel
}

/// Deux plans de six images, un personnage chacun (le rouge au plan 1, le vert au plan 2).
fn deux_plans() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.into_plans();
    for i in 0..2 {
        doc.set_duration(i, 6);
    }
    personnage(&mut doc, 0, 0, 6, 20, ROUGE);
    personnage(&mut doc, 1, 0, 6, 40, VERT);
    doc
}

/// Un plan de six images en deux cases (une marque à l'image 3), un personnage par case.
fn une_continuite() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.set_duration(0, 6);
    doc.toggle_mark(0, 3);
    personnage(&mut doc, 0, 0, 3, 20, ROUGE);
    personnage(&mut doc, 0, 3, 6, 40, VERT);
    doc
}

/// Le décor du plan `i`, peint d'un pixel en (`x`, `y`) à l'image `frame` (comme un trait de la brosse du décor).
fn decorer(doc: &mut Document, i: usize, frame: usize, x: usize, y: usize) -> CelId {
    // (Un décor bitmap, celui d'avant le 2026-10-28 : les essais y posent un pixel à la main ; le décor vectoriel est dans `calques_pastilles_tests.rs`.)
    doc.decor_bitmap = true;
    let (at, _) = doc.assurer_decor(i, frame).expect("le calque Décor");
    let cel = doc.panels[i].layers[at].shown_at(frame).expect("un dessin du décor");
    pixel(doc, cel, x, y, GRIS_BLEU);
    cel
}

fn voit(doc: &Document, i: usize, frame: usize, x: usize, y: usize) -> Color32 {
    let plan = render::panel_plan_at(doc, i, frame, None);
    render::composite_pixel(doc, &plan, x, y)
}

// ------------------------------------------------------------------------------------------ le calque

#[test]
fn the_decor_layer_is_made_at_the_bottom_of_the_plan_with_one_drawing_held_over_all_its_images() {
    let mut doc = une_continuite();
    assert!(doc.decor_de(0).is_none(), "pas de décor avant le premier trait");
    doc.decor_bitmap = true;
    let (at, change) = doc.assurer_decor(0, 4).expect("le plan existe");
    assert_eq!((at, change), (0, true), "créé tout en bas du plan");
    let l = &doc.panels[0].layers[0];
    assert!(l.decor && l.name == "Décor" && l.kind == LayerKind::Bitmap && l.post == Behaviour::Hold && !l.auto_break);
    assert_eq!(doc.panels[0].layers.len(), 2, "sous le calque des personnages");
    assert!(!doc.panels[0].layers[1].decor);
    // Un seul dessin, sur toutes les images du plan (report allumé d'office).
    let cel = l.cel_at(0).expect("un dessin");
    assert_eq!(l.frames, vec![Some(cel); 6]);
    // Un deuxième trait ne refait rien.
    assert_eq!(doc.assurer_decor(0, 1), Some((0, false)));
    assert_eq!(doc.panels[0].layers.len(), 2);
}

#[test]
fn with_the_report_off_a_first_decor_stroke_only_covers_its_case() {
    let mut doc = une_continuite();
    doc.report_decor = false;
    let (at, _) = doc.assurer_decor(0, 4).unwrap();
    let l = &doc.panels[0].layers[at];
    let cel = l.cel_at(4).unwrap();
    assert_eq!(l.frames[..3], [None, None, None], "la première case n'a pas de décor");
    assert_eq!(l.frames[3..], [Some(cel); 3]);
    // (La première case n'a rien, mais le « Maintenir » du calque ne remonte pas le temps.)
    assert!(l.shown_at(1).is_none());
}

#[test]
fn a_decor_stroke_in_any_case_shows_in_every_case_of_the_plan_but_characters_stay_in_their_case() {
    let mut doc = une_continuite();
    // Un trait de décor dans la deuxième case (image 4) : un pixel en (60, 50).
    let cel = decorer(&mut doc, 0, 4, 60, 50);
    for f in [0, 2, 3, 5] {
        let plan = render::panel_plan_at(&doc, 0, f, None);
        assert!(plan.items.iter().any(|i| i.cel == cel), "le décor est dans l'image {f}");
        assert_eq!(voit(&doc, 0, f, 60, 50).a(), 255, "et se voit à l'image {f}");
    }
    // Les personnages : le rouge dans la première case seulement, le vert dans la seconde.
    assert_eq!(voit(&doc, 0, 1, 20, 20), ROUGE);
    assert_eq!(voit(&doc, 0, 4, 20, 20).a(), 0, "le rouge n'est pas reporté dans la deuxième case");
    assert_eq!(voit(&doc, 0, 4, 40, 20), VERT);
    assert_eq!(voit(&doc, 0, 1, 40, 20).a(), 0, "le vert n'est pas dans la première case");
    // Un trait de plus, dans la première case : même dessin, vu dans la seconde.
    let encore = decorer(&mut doc, 0, 1, 80, 60);
    assert_eq!(encore, cel, "un seul dessin pour tout le plan");
    assert_eq!(voit(&doc, 0, 5, 80, 60).a(), 255);
}

#[test]
fn the_cases_after_a_camera_move_see_the_decor_through_the_last_frame() {
    use crate::camera::CameraKey;
    let mut doc = une_continuite();
    let a = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(0, a), CameraKey::new(2, a.shifted(40.0, 0.0))];
    let cel = decorer(&mut doc, 0, 4, 60, 50);
    // La deuxième case (images 3 à 5) voit à travers le dernier cadre (la caméra tient sa dernière clé), et y voit le décor.
    let b = doc.panel_camera(0, 2);
    for f in 3..6 {
        assert_eq!(doc.panel_camera(0, f).x, b.x, "l'image {f} voit à travers B");
        assert!(render::panel_plan_at(&doc, 0, f, None).items.iter().any(|i| i.cel == cel));
    }
}

#[test]
fn the_decor_follows_when_the_plan_gets_longer() {
    let mut doc = une_continuite();
    let cel = decorer(&mut doc, 0, 0, 60, 50);
    doc.set_duration(0, 10);
    for f in 0..10 {
        assert_eq!(doc.panels[0].layers[doc.decor_de(0).unwrap()].shown_at(f), Some(cel), "image {f}");
    }
    // Même sans que ses images aient été allongées (un « Maintenir » de plus).
    let at = doc.decor_de(0).unwrap();
    doc.panels[0].layers[at].frames.truncate(6);
    assert_eq!(doc.panels[0].layers[at].shown_at(9), Some(cel));
}

// ------------------------------------------------------------------------------------------ couper et réunir

#[test]
fn checking_the_cut_again_gives_the_next_plan_an_independent_copy_of_the_decor() {
    let mut doc = une_continuite();
    let cel = decorer(&mut doc, 0, 0, 60, 50);
    let second = doc.split_plan(0, 3).expect("coupé");
    assert_eq!(doc.panels.len(), 2);
    let (a, b) = (doc.decor_de(0).expect("décor du premier"), doc.decor_de(second).expect("décor du second"));
    let (ca, cb) = (doc.panels[0].layers[a].cel_at(0).unwrap(), doc.panels[second].layers[b].cel_at(0).unwrap());
    assert_eq!(ca, cel, "le premier garde le sien");
    assert_ne!(ca, cb, "le second a sa propre copie");
    assert_eq!(voit(&doc, second, 1, 60, 50).a(), 255, "la copie montre le même décor");
    // Éditer l'un ne touche pas l'autre.
    pixel(&mut doc, cb, 100, 70, GRIS_BLEU);
    assert_eq!(voit(&doc, second, 0, 100, 70).a(), 255);
    assert_eq!(voit(&doc, 0, 0, 100, 70).a(), 0, "le premier plan n'a pas le trait du second");
    pixel(&mut doc, ca, 110, 10, GRIS_BLEU);
    assert_eq!(voit(&doc, 0, 1, 110, 10).a(), 255);
    assert_eq!(voit(&doc, second, 0, 110, 10).a(), 0, "ni l'inverse");
    // Le second plan a le calque en bas, marqué.
    assert!(doc.panels[second].layers[b].decor && b == 0);
}

#[test]
fn uniting_two_plans_holds_the_first_decor_over_the_second_with_the_report_but_keeps_the_characters_apart() {
    let mut doc = deux_plans();
    let cel = decorer(&mut doc, 0, 0, 60, 50);
    assert!(doc.report_decor, "le report est allumé d'office");
    doc.join_next_plan(0).expect("réunis");
    assert_eq!(doc.panels.len(), 1);
    let p = &doc.panels[0];
    assert_eq!(p.frame_count(), 12);
    assert_eq!(p.layers.iter().filter(|l| l.decor).count(), 1, "un seul calque Décor");
    let l = &p.layers[doc.decor_de(0).unwrap()];
    assert_eq!(l.frames, vec![Some(cel); 12], "le décor du premier plan est tenu sur toute la suite");
    // La deuxième case (image 6) montre le décor, et son vert, pas le rouge de la première.
    assert_eq!(voit(&doc, 0, 8, 60, 50).a(), 255);
    assert_eq!(voit(&doc, 0, 8, 40, 20), VERT);
    assert_eq!(voit(&doc, 0, 8, 20, 20).a(), 0);
    assert_eq!(voit(&doc, 0, 2, 20, 20), ROUGE);
}

#[test]
fn with_the_report_off_the_next_case_starts_with_an_empty_decor() {
    let mut doc = deux_plans();
    let cel = decorer(&mut doc, 0, 0, 60, 50);
    doc.report_decor = false;
    doc.join_next_plan(0).expect("réunis");
    let l = &doc.panels[0].layers[doc.decor_de(0).unwrap()];
    assert_eq!(l.frames[..6], [Some(cel); 6], "la première case garde son décor");
    let vide = l.shown_at(8).expect("un dessin vide pour la suite");
    assert_ne!(vide, cel);
    assert!(!a_de_l_encre(&doc, vide), "la case suivante repart de rien");
    assert_eq!(voit(&doc, 0, 8, 60, 50).a(), 0, "elle ne montre pas le décor d'avant");
    assert_eq!(voit(&doc, 0, 2, 60, 50).a(), 255);
    // Un trait de décor dans la deuxième case n'est que dans la deuxième case.
    let (at, change) = doc.assurer_decor(0, 8).unwrap();
    assert!(!change, "le dessin vide est déjà là");
    let dessin = doc.panels[0].layers[at].shown_at(8).unwrap();
    pixel(&mut doc, dessin, 90, 40, GRIS_BLEU);
    assert_eq!(voit(&doc, 0, 9, 90, 40).a(), 255);
    assert_eq!(voit(&doc, 0, 2, 90, 40).a(), 0);
}

#[test]
fn a_decor_the_next_plan_had_of_its_own_stays_with_the_report_off_and_gives_way_with_it_on() {
    for report in [true, false] {
        let mut doc = deux_plans();
        let a = decorer(&mut doc, 0, 0, 60, 50);
        let b = decorer(&mut doc, 1, 0, 70, 55);
        doc.report_decor = report;
        doc.join_next_plan(0).unwrap();
        let l = &doc.panels[0].layers[doc.decor_de(0).unwrap()];
        assert_eq!(doc.panels[0].layers.iter().filter(|l| l.decor).count(), 1);
        if report {
            assert_eq!(l.frames, vec![Some(a); 12], "le décor du premier plan est partout");
        } else {
            assert_eq!((l.shown_at(1), l.shown_at(8)), (Some(a), Some(b)), "chaque case garde le sien");
        }
    }
    // Le premier plan n'avait pas de décor, le second oui : il reste à sa place dans le temps.
    let mut doc = deux_plans();
    let b = decorer(&mut doc, 1, 0, 70, 55);
    doc.join_next_plan(0).unwrap();
    let l = &doc.panels[0].layers[doc.decor_de(0).unwrap()];
    assert_eq!((l.shown_at(1), l.shown_at(8)), (None, Some(b)));
}

#[test]
fn a_plan_without_decor_joins_as_before() {
    let mut doc = deux_plans();
    doc.join_next_plan(0).unwrap();
    assert!(doc.decor_de(0).is_none());
    assert_eq!(doc.panels[0].layers.len(), 1, "le calque des personnages, réuni par son nom");
    assert_eq!(doc.panels[0].frame_count(), 12);
}

#[test]
fn a_copy_of_the_decor_layer_is_an_ordinary_layer() {
    let mut doc = une_continuite();
    decorer(&mut doc, 0, 0, 60, 50);
    let copie = crate::layerops::duplicate_layer(&mut doc, 0, 0);
    assert!(doc.panels[0].layers[0].decor && !doc.panels[0].layers[copie].decor, "le plan n'a qu'un décor");
}

// ------------------------------------------------------------------------------------------ le fichier

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-decor-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

#[test]
fn the_decor_survives_saving_and_reopening_and_an_old_project_without_decor_opens_as_before() {
    let dir = dossier("fichier");
    let mut doc = une_continuite();
    let cel = decorer(&mut doc, 0, 4, 60, 50);
    let manifeste = dir.join("P.pelure");
    crate::project::save(&doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let texte = std::fs::read_to_string(&manifeste).unwrap();
    assert!(texte.contains("\"decor\""), "le calque Décor est marqué dans le manifeste");
    let relu = crate::project::load(&manifeste).expect("réouverture");
    let at = relu.doc.decor_de(0).expect("le calque Décor est revenu");
    let l = &relu.doc.panels[0].layers[at];
    assert_eq!((l.name.as_str(), l.post), ("Décor", Behaviour::Hold));
    let revenu = l.cel_at(0).expect("son dessin");
    assert_eq!(l.frames, vec![Some(revenu); 6], "toujours un seul dessin pour tout le plan, donc partagé par les cases");
    assert!(a_de_l_encre(&relu.doc, revenu));
    assert_eq!(voit(&relu.doc, 0, 1, 60, 50).a(), 255, "le décor se voit dans la première case");
    let _ = cel;
    // Un projet sans décor n'écrit rien de plus, et s'ouvre sans calque Décor.
    let sans = dir.join("Sans.pelure");
    crate::project::save(&deux_plans(), &sans, &mut Default::default()).unwrap();
    assert!(!std::fs::read_to_string(&sans).unwrap().contains("decor"), "aucun champ de plus pour un projet sans décor");
    let relu = crate::project::load(&sans).expect("un projet d'avant s'ouvre");
    assert!(relu.doc.panels.iter().all(|p| p.layers.iter().all(|l| !l.decor)));
    assert_eq!(relu.doc.decor_de(0), None);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_shared_film_carries_the_decor_layer_in_its_plan_files() {
    use crate::project::partage_io;
    let dir = dossier("partage");
    let mut doc = une_continuite();
    decorer(&mut doc, 0, 4, 60, 50);
    let manifeste = dir.join("Film").join("Film.pelure");
    let signature = crate::project::partage_io::Signature { id: "anthony".into(), nom: "Anthony".into(), le: 1_000 };
    partage_io::partager(&doc, &manifeste, &signature, "p-essai", None, 2).expect("partage");
    let ouvert = crate::project::load(&manifeste).expect("film partagé lisible");
    assert!(ouvert.partage.is_some());
    let at = ouvert.doc.decor_de(0).expect("le calque Décor est dans le fichier du plan");
    assert_eq!(voit(&ouvert.doc, 0, 0, 60, 50).a(), 255);
    assert!(ouvert.doc.panels[0].layers[at].decor);
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------------ à l'écran

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![appui(p, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
}

fn case_decor(b: &Banc, g: Genre) -> Rect {
    b.app.perso.cases.iter().find(|(_, c)| *c == Case::Decor(g)).map(|(r, _)| *r).unwrap_or_else(|| panic!("la pastille du décor {g:?} n'est pas dans la colonne : {:?}", b.app.perso.cases))
}

pub(crate) fn choisir(b: &mut Banc, g: Genre) {
    let p = case_decor(b, g).center();
    clic(b, p);
}

/// Un trait dans la case `k`, au milieu, long de soixante points.
fn trait_dans(b: &mut Banc, k: usize) {
    let v = vignette(b, k);
    glisser(b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
}

fn encre_du_decor(b: &Banc, i: usize) -> bool {
    let doc = &b.app.doc;
    doc.decor_de(i).is_some_and(|at| doc.panels[i].layers[at].frames.iter().flatten().any(|c| a_de_l_encre(doc, *c)))
}

fn encre_ailleurs(b: &Banc, i: usize) -> bool {
    let doc = &b.app.doc;
    doc.panels[i].layers.iter().filter(|l| !l.decor).flat_map(|l| l.frames.iter().flatten()).any(|c| a_de_l_encre(doc, *c))
}

#[test]
fn the_decor_pastilles_sit_after_a_separator_in_the_column_and_choosing_one_takes_its_tool_colour_and_size() {
    let mut b = ouvert(quatre_plans(), 4);
    // Toute la rangée est à portée du pointeur, jusqu'à l'icône du panneau (l'`Area` a grandi avec ses pastilles).
    let colonne = b.ctx.memory(|m| m.area_rect(egui::Id::new("pastilles_rangee_planche"))).expect("l'aire de la rangée");
    for (r, c) in &b.app.perso.cases {
        assert!(colonne.contains(r.center()), "{c:?} est hors de l'aire de la rangée : {r:?} dans {colonne:?}");
        assert_eq!(b.ctx.layer_id_at(r.center()), Some(egui::LayerId::new(egui::Order::Middle, egui::Id::new("pastilles_rangee_planche"))), "{c:?} reçoit les clics");
    }
    let rangs: Vec<Case> = b.app.perso.cases.iter().map(|(_, c)| *c).collect();
    let decor: Vec<usize> = Genre::ALL.iter().map(|g| rangs.iter().position(|c| *c == Case::Decor(*g)).expect("la pastille")).collect();
    let derniere_neutre = rangs.iter().rposition(|c| matches!(c, Case::Choix(_))).unwrap();
    assert!(decor[0] > derniere_neutre && decor.windows(2).all(|w| w[1] == w[0] + 1), "après les couleurs neutres, ensemble : {rangs:?}");
    assert!(decor[2] < rangs.iter().position(|c| *c == Case::Plus).unwrap(), "avant le « + »");
    // Carrés arrondis, de la taille d'une pastille : une colonne, l'une sous l'autre.
    let r = case_decor(&b, Genre::Crayon);
    assert!(r.width() > 20.0 && (r.width() - r.height()).abs() < 1.0);
    let n = pas_d_annulation(&b);
    let (brosse0, crayon0, couleur0, outil0) = (b.app.brush, b.app.pencil, b.app.color, b.app.tool);
    for (g, outil) in [(Genre::Crayon, Tool::Pencil), (Genre::Feutre, Tool::Brush), (Genre::Aplat, Tool::Brush)] {
        choisir(&mut b, g);
        assert_eq!(b.app.perso.decor, Some(g), "{g:?}");
        assert_eq!(b.app.tool, outil);
        assert_eq!(b.app.color, [0x8F, 0xA3, 0xB8], "la couleur du décor");
        let taille = if g == Genre::Crayon { b.app.pencil.size } else { b.app.brush.size };
        assert_eq!(taille, g.taille_defaut());
        assert_eq!(b.app.perso.actif, None);
    }
    assert!((b.app.brush.hardness - 1.0).abs() < 1e-6, "l'aplat est plein");
    assert_eq!(pas_d_annulation(&b), n, "choisir une brosse n'est pas une étape d'annulation");
    // Un clic sur la pastille choisie la lâche : tout revient.
    choisir(&mut b, Genre::Aplat);
    assert_eq!(b.app.perso.decor, None);
    assert_eq!((b.app.brush, b.app.pencil, b.app.color, b.app.tool), (brosse0, crayon0, couleur0, outil0), "l'outil, les pinceaux et la couleur d'avant");
    // Un personnage remplace le décor, « aucun personnage » le lâche aussi.
    choisir(&mut b, Genre::Feutre);
    let perso = b.app.perso.cases.iter().find(|(_, c)| matches!(c, Case::Choix(_))).unwrap().0.center();
    clic(&mut b, perso);
    assert_eq!(b.app.perso.decor, None);
    assert!(b.app.perso.actif.is_some());
    choisir(&mut b, Genre::Feutre);
    assert_eq!(b.app.perso.actif, None, "le décor remplace le personnage");
    let aucun = b.app.perso.cases.iter().find(|(_, c)| *c == Case::Classique).unwrap().0.center();
    clic(&mut b, aucun);
    assert_eq!(b.app.perso.decor, None);
}

#[test]
fn the_first_decor_stroke_makes_the_layer_at_the_bottom_in_one_undo_step_and_the_next_ones_one_step_each() {
    let mut b = ouvert(quatre_plans(), 4);
    let (couches, n) = (b.app.doc.panels[1].layers.len(), pas_d_annulation(&b));
    choisir(&mut b, Genre::Feutre);
    assert_eq!(b.app.doc.decor_de(1), None, "choisir ne crée rien");
    trait_dans(&mut b, 3);
    assert_eq!(b.app.cur_panel, 1);
    let p = &b.app.doc.panels[1];
    assert_eq!(p.layers.len(), couches + 1);
    assert!(p.layers[0].decor && p.layers[0].name == "Décor", "tout en bas du plan");
    assert_eq!(b.app.cur_layer, 0, "le calque Décor est le calque courant");
    assert!(encre_du_decor(&b, 1), "le trait est dans le décor");
    assert!(!encre_ailleurs(&b, 1));
    assert_eq!(pas_d_annulation(&b), n + 1, "le calque et le premier trait ne font qu'une étape");
    assert_eq!(dernier_nom(&b), "Dessiner le décor");
    trait_dans(&mut b, 3);
    assert_eq!(pas_d_annulation(&b), n + 2, "un trait de plus, une étape de plus");
    assert_eq!(b.app.doc.panels[1].layers.len(), couches + 1);
    // Annuler le second : le premier reste ; annuler le premier : plus de calque Décor.
    b.app.run_action("edition.annuler");
    assert!(b.app.doc.decor_de(1).is_some() && encre_du_decor(&b, 1));
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.decor_de(1), None, "le calque part avec son premier trait");
    assert_eq!(b.app.doc.panels[1].layers.len(), couches);
    assert_eq!(pas_d_annulation(&b), n);
    // Rétablir : les deux reviennent.
    b.app.run_action("edition.retablir");
    assert!(b.app.doc.decor_de(1).is_some() && encre_du_decor(&b, 1));
}

#[test]
fn drawing_the_decor_in_an_empty_case_makes_the_plan_and_its_decor_in_one_undo_step() {
    let mut b = ouvert(quatre_plans(), 4);
    let (plans, n) = (b.app.doc.panels.len(), pas_d_annulation(&b));
    choisir(&mut b, Genre::Aplat);
    let e = case_vide(&b, 0);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 5.0), 10));
    assert_eq!(b.app.doc.panels.len(), plans + 1, "le plan est fait à l'appui");
    assert!(encre_du_decor(&b, plans), "le trait est dans le décor du plan neuf");
    assert!(!encre_ailleurs(&b, plans));
    assert_eq!(pas_d_annulation(&b), n + 1, "le plan, le calque Décor et le trait ne font qu'une étape");
    assert_eq!(dernier_nom(&b), "Nouveau plan dessiné");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), plans);
    assert_eq!(pas_d_annulation(&b), n);
}

#[test]
fn a_case_added_after_the_last_plan_shares_the_decor_with_the_report_and_starts_empty_without_it() {
    for report in [true, false] {
        let mut b = ouvert(quatre_plans(), 4);
        b.app.settings.vignettes.report_decor = report;
        choisir(&mut b, Genre::Aplat);
        trait_dans(&mut b, 5);
        let dernier = b.app.doc.panels.len() - 1;
        let d = b.app.doc.panels[dernier].layers[b.app.doc.decor_de(dernier).unwrap()].cel_at(0).unwrap();
        // La barre grise devant la première case vide : la case de plus est une image de plus du dernier plan.
        b.app.vignettes.empty_joins = true;
        let e = case_vide(&b, 0);
        glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 5.0), 10));
        assert_eq!(b.app.doc.panels.len(), dernier + 1, "une image de plus, pas un plan de plus");
        let l = &b.app.doc.panels[dernier].layers[b.app.doc.decor_de(dernier).unwrap()];
        let fin = b.app.doc.panels[dernier].frame_count() - 1;
        assert!(fin > 0 && !b.app.doc.panels[dernier].marks.is_empty(), "le plan a maintenant une marque d'image");
        let nouveau = l.shown_at(fin).expect("un décor à la dernière image");
        if report {
            assert_eq!(nouveau, d, "avec le report, la case de plus partage le décor");
            assert_eq!(l.frames.iter().flatten().collect::<std::collections::BTreeSet<_>>().len(), 1);
        } else {
            assert_ne!(nouveau, d, "sans report, la case de plus a son propre décor");
            assert!(a_de_l_encre(&b.app.doc, nouveau), "où le trait a été fait");
            assert_eq!(l.shown_at(0), Some(d), "la première case garde le sien");
        }
    }
}

#[test]
fn a_decor_brush_draws_in_the_decor_layer_whatever_the_current_layer_and_gives_the_layer_back_when_let_go() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.select_panel(1);
    b.app.add_layer(LayerKind::Bitmap);
    let (perso_id, perso_rang) = (b.app.layer().id, b.app.cur_layer);
    assert_eq!(perso_rang, 1);
    b.chauffer(2);
    choisir(&mut b, Genre::Crayon);
    trait_dans(&mut b, 3);
    assert!(encre_du_decor(&b, 1), "le trait est dans le décor");
    assert!(!encre_ailleurs(&b, 1), "ni dans le calque courant d'avant, ni dans l'autre");
    assert!(b.app.layer().decor);
    // Lâcher le décor : le calque d'avant (par son numéro, il a changé de rang) redevient courant.
    choisir(&mut b, Genre::Crayon);
    assert_eq!(b.app.perso.decor, None);
    assert_eq!(b.app.layer().id, perso_id);
    assert_eq!(b.app.cur_layer, 2, "le décor a pris le rang 0");
    // Un trait normal va dans ce calque, pas dans le décor.
    let avant = pas_d_annulation(&b);
    trait_dans(&mut b, 3);
    assert_eq!(pas_d_annulation(&b), avant + 1);
    assert!(encre_ailleurs(&b, 1));
    // Choisir un calque à la main pendant le décor le lâche : on dessine où l'on a cliqué.
    choisir(&mut b, Genre::Feutre);
    b.app.select_layer(1);
    assert_eq!(b.app.perso.decor, None);
    assert_eq!(b.app.cur_layer, 1);
}

#[test]
fn a_decor_stroke_in_case_two_of_a_continuity_shows_in_case_one_and_its_tile_is_remade() {
    let mut b = ouvert(quatre_plans(), 4);
    // Le plan 1 a trois cases (cartes 0, 1, 2) : un seul plan.
    let cartes = b.app.board_layout().cards.clone();
    assert!(cartes[0].plan == cartes[1].plan && cartes[1].plan == cartes[2].plan);
    se_reposer(&mut b, 400);
    let (id0, largeur0) = (cartes[0].id, crate::thumbs::width_for(vignette(&b, 0).width() * b.ctx.pixels_per_point()));
    let blanche = |b: &Banc| -> bool {
        let (img, _) = b.app.thumbs.case_image_of(id0, largeur0).expect("la vignette de la case 1");
        img.pixels.iter().all(|p| p.a() == 0 || *p == Color32::WHITE)
    };
    assert!(blanche(&b), "la case 1 est blanche");
    choisir(&mut b, Genre::Aplat);
    trait_dans(&mut b, 1);
    // Le décor est un seul dessin, sur toutes les images du plan : la case 1 le montre.
    let at = b.app.doc.decor_de(0).expect("le calque Décor du plan 1");
    let l = &b.app.doc.panels[0].layers[at];
    let cel = l.cel_at(0).unwrap();
    assert_eq!(l.frames, vec![Some(cel); b.app.doc.panels[0].frame_count()]);
    for c in &cartes[..3] {
        let plan = render::panel_plan_at(&b.app.doc, 0, c.frame, None);
        assert!(plan.items.iter().any(|i| i.cel == cel), "la case de l'image {} montre le décor", c.frame);
    }
    // La vignette de la case 1 est refaite, par la clé de son cache (la version du dessin du décor y est).
    se_reposer(&mut b, 400);
    assert!(!blanche(&b), "la vignette de la case 1 montre le trait fait dans la case 2");
    // Et le plan voisin n'a rien.
    assert!(b.app.doc.decor_de(1).is_none());
}

#[test]
fn unchecking_and_checking_the_cut_shares_then_separates_the_decor_in_one_undo_step_each() {
    let mut b = ouvert(quatre_plans(), 4);
    choisir(&mut b, Genre::Feutre);
    // Le plan 1 (cartes 0 à 2) et le plan 2 (carte 3) : une coupe entre les cartes 2 et 3. Un décor au plan 2, un autre au plan 1.
    trait_dans(&mut b, 3);
    trait_dans(&mut b, 0);
    let (d1, d2) = (b.app.doc.panels[0].layers[b.app.doc.decor_de(0).unwrap()].cel_at(0).unwrap(), b.app.doc.panels[1].layers[b.app.doc.decor_de(1).unwrap()].cel_at(0).unwrap());
    assert_ne!(d1, d2);
    // Décocher la coupe : le décor du plan 1 est tenu sur la case suivante (report allumé), celui du plan 2 s'efface au profit du premier.
    let n = pas_d_annulation(&b);
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, 2));
    b.chauffer(2);
    assert_eq!(pas_d_annulation(&b), n + 1, "une étape d'annulation par coupe");
    assert_eq!(b.app.doc.panels.len(), 3);
    let l = &b.app.doc.panels[0].layers[b.app.doc.decor_de(0).unwrap()];
    assert_eq!(l.shown_at(l.frames.len() - 1), Some(d1), "la case réunie montre le décor du premier plan");
    assert_eq!(l.frames.iter().flatten().collect::<std::collections::BTreeSet<_>>().len(), 1);
    // Un trait dans la case réunie va dans ce décor-là.
    let cartes = b.app.board_layout().cards.clone();
    let derniere = cartes.iter().rposition(|c| c.plan == b.app.doc.panels[0].id).unwrap();
    trait_dans(&mut b, derniere);
    assert!(encre_du_decor(&b, 0));
    // Recocher la coupe : le plan suivant a sa propre copie, indépendante.
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, derniere - 1));
    b.chauffer(2);
    assert_eq!(pas_d_annulation(&b), n + 3, "la réunion, le trait, la coupe : trois étapes");
    assert_eq!(b.app.doc.panels.len(), 4);
    let copies: Vec<CelId> = (0..2).filter_map(|i| b.app.doc.decor_de(i).and_then(|at| b.app.doc.panels[i].layers[at].cel_at(0))).collect();
    assert_eq!(copies.len(), 2);
    assert_ne!(copies[0], copies[1], "chacun son dessin");
    assert_eq!(copies[0], d1, "le premier plan garde son décor, le suivant a une copie");
    // Annuler la coupe, le trait, puis la réunion : on retrouve les deux décors d'avant.
    for _ in 0..3 {
        b.app.run_action("edition.annuler");
    }
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 4);
    let cels: Vec<CelId> = (0..2).map(|i| b.app.doc.panels[i].layers[b.app.doc.decor_de(i).unwrap()].cel_at(0).unwrap()).collect();
    assert_eq!(cels, vec![d1, d2], "les deux décors d'avant, chacun le sien");
    assert_eq!(pas_d_annulation(&b), n);
}

#[test]
fn with_the_report_off_the_united_case_starts_empty_on_the_board() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.settings.vignettes.report_decor = false;
    choisir(&mut b, Genre::Feutre);
    trait_dans(&mut b, 0);
    let d1 = b.app.doc.panels[0].layers[b.app.doc.decor_de(0).unwrap()].cel_at(0).unwrap();
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, 2));
    b.chauffer(2);
    let l = &b.app.doc.panels[0].layers[b.app.doc.decor_de(0).unwrap()];
    let fin = b.app.doc.panels[0].frame_count() - 1;
    let suite = l.shown_at(fin).expect("un dessin pour la case réunie");
    assert_ne!(suite, d1, "la case réunie ne tient pas le décor d'avant");
    assert!(!a_de_l_encre(&b.app.doc, suite));
    assert_eq!(l.shown_at(0), Some(d1));
}

// ------------------------------------------------------------------------------------------ les réglages

#[test]
fn the_vignettes_settings_persist_have_clean_defaults_and_keep_old_files_readable() {
    let defaut = Settings::default();
    assert_eq!(defaut.vignettes, ReglagesVignettes::default());
    assert_eq!(defaut.vignettes.debord, 12);
    assert_eq!(defaut.vignettes.aimant, 6);
    assert_eq!(defaut.vignettes.couleur_cadre, CADRE_DEFAUT);
    assert!(defaut.vignettes.report_decor);
    assert_eq!(defaut.vignettes.couleur_decor, [0x8F, 0xA3, 0xB8]);
    assert!(!toml::to_string(&defaut).unwrap().contains("[vignettes]"), "des réglages d'origine n'écrivent rien de plus");
    let mut s = Settings::default();
    s.vignettes = ReglagesVignettes { debord: 20, aimant: 0, couleur_cadre: [200, 10, 10], report_decor: false, couleur_decor: [10, 20, 30], tailles_decor: [3.0, 30.0, 120.0], decor_bitmap: true };
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("[vignettes]") && texte.contains("report_decor = false") && texte.contains("aimant_points = 0"), "{texte}");
    let relu: Settings = toml::from_str(&texte).unwrap();
    assert_eq!(relu.vignettes, s.vignettes);
    // Des réglages d'avant : ceux d'origine ; une clé qui manque dans la table : sa valeur d'origine.
    let ancien: Settings = toml::from_str("copie_secours_minutes = 3").unwrap();
    assert_eq!(ancien.vignettes, ReglagesVignettes::default());
    let partiel: Settings = toml::from_str("[vignettes]\nreport_decor = false").unwrap();
    assert_eq!((partiel.vignettes.report_decor, partiel.vignettes.debord, partiel.vignettes.aimant), (false, 12, 6));
    // Des valeurs absurdes (un fichier modifié à la main) sont ramenées dans leurs bornes.
    let propre = ReglagesVignettes { debord: 200, aimant: 99, tailles_decor: [f32::NAN, -4.0, 9000.0], ..Default::default() }.nettoyes();
    assert_eq!((propre.debord, propre.aimant), (40, 30));
    assert_eq!(propre.tailles_decor, [Genre::Crayon.taille_defaut(), 0.5, 400.0]);
    assert_eq!((ReglagesVignettes { aimant: 0, ..Default::default() }).aimant_points(), None, "0 : pas d'aimant");
    assert_eq!(ReglagesVignettes::default().aimant_points(), Some(6.0));
}

#[test]
fn the_overflow_margin_setting_changes_the_margin_of_the_tiles() {
    let mut b = ouvert(quatre_plans(), 4);
    se_reposer(&mut b, 400);
    let (id, largeur) = (b.app.board_layout().cards[3].id, crate::thumbs::width_for(vignette(&b, 3).width() * b.ctx.pixels_per_point()));
    let (_, marge0) = b.app.thumbs.case_image_of(id, largeur).expect("la vignette");
    assert_eq!(b.app.thumbs.debord, Some(0.12));
    b.app.settings.vignettes.debord = 24;
    se_reposer(&mut b, 400);
    let (img, marge1) = b.app.thumbs.case_image_of(id, largeur).expect("la vignette refaite");
    assert_eq!(b.app.thumbs.debord, Some(0.24));
    assert!(marge1 > marge0 && (marge1 as f32 / marge0 as f32 - 2.0).abs() < 0.2, "le débord de 24 % double la marge de 12 % : {marge0} → {marge1}");
    let (tw, th) = crate::thumbs::thumb_size_of(b.app.doc.frame, largeur, crate::thumbs::scale_pct());
    assert_eq!(img.size, [tw + 2 * marge1, th + 2 * marge1]);
    assert_eq!(marge1, crate::thumbs::marge_px_de(tw, 0.24));
}

#[test]
fn the_camera_magnet_setting_zero_means_off() {
    // B est à droite de A, haut contre haut : son bord haut est sur un alignement. On le tire de `pixels` points d'écran vers le bas (au-delà
    // du seuil de 5 points qui fait un geste) et on lit de combien il a bougé, en points d'écran. L'aimant est doux : il tire en arrière de
    // d × (1 − d / seuil), d étant l'écart à l'alignement.
    let mesure = |aimant: u8, pixels: f32| -> f32 {
        let mut b = avec_b_a_droite();
        b.app.settings.vignettes.aimant = aimant;
        let echelle = b.app.vignettes.xf.zoom * 170.0 / 160.0;
        let y0 = clefs(&b)[1].2;
        let c = centre(cadre_ecran(&b, 1));
        ctrl_glisser(&mut b, c, &pas(c, vec2(0.0, pixels), 6), c + vec2(0.0, pixels), true);
        (clefs(&b)[1].2 - y0) * echelle
    };
    // Éteint (0) : le cadre suit le pointeur exactement.
    assert!((mesure(0, 5.5) - 5.5).abs() < 0.05, "éteint : le cadre suit le pointeur");
    // Par défaut (6 points) : à 5,5 points d'un alignement, il est attiré (5,5² / 6 ≈ 5,04).
    let attire = mesure(6, 5.5);
    assert!(attire > 0.0 && attire < 5.3, "attiré à 6 points : {attire}");
    // Hors du seuil, le pointeur ; plus fort, l'aimant attire de plus loin.
    assert!((mesure(6, 8.0) - 8.0).abs() < 0.05, "à 8 points, hors de l'aimant de 6 points : le pointeur");
    assert!(mesure(12, 8.0) < 7.0, "dans l'aimant de 12 points : attiré (8² / 12 ≈ 5,3)");
    assert!((mesure(0, 8.0) - 8.0).abs() < 0.05);
}

/// Les couleurs des traits et des textes peints par la dernière image (le bleu des cadres de caméra et leurs lettres).
fn couleurs_peintes(b: &Banc) -> Vec<Color32> {
    fn marche(s: &Shape, out: &mut Vec<Color32>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| marche(s, out)),
            Shape::LineSegment { stroke, .. } => out.push(stroke.color),
            Shape::Path(p) => {
                if let egui::epaint::ColorMode::Solid(c) = p.stroke.color {
                    out.push(c);
                }
            }
            Shape::Text(t) => out.push(t.galley.job.sections.first().map_or(Color32::TRANSPARENT, |s| s.format.color)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        marche(&s.shape, &mut out);
    }
    out
}

fn proche(c: Color32, rvb: [u8; 3]) -> bool {
    let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs();
    d(c.r(), rvb[0]) <= 3 && d(c.g(), rvb[1]) <= 3 && d(c.b(), rvb[2]) <= 3 && c.a() > 100
}

#[test]
fn the_camera_frame_colour_setting_colours_the_frames_and_defaults_to_the_old_blue() {
    for (reglage, attendu) in [(None, CADRE_DEFAUT), (Some([210, 40, 60]), [210, 40, 60])] {
        let mut b = avec_b_a_droite();
        b.garder_formes = true;
        if let Some(c) = reglage {
            b.app.settings.vignettes.couleur_cadre = c;
        }
        b.chauffer(3);
        let peintes = couleurs_peintes(&b);
        // (Les couleurs prémultipliées d'un trait à 215 ou 245 d'opacité : à quelques unités près.)
        let sur_papier = |c: Color32| {
            let [r, g, bl, a] = c.to_srgba_unmultiplied();
            a > 100 && proche(Color32::from_rgba_unmultiplied(r, g, bl, 255), attendu)
        };
        assert!(peintes.iter().any(|c| sur_papier(*c)), "les cadres de caméra sont de la couleur réglée {attendu:?}");
        if reglage.is_some() {
            assert!(!peintes.iter().any(|c| proche(*c, CADRE_DEFAUT)), "plus de bleu d'origine quand une autre couleur est réglée");
        }
    }
}

#[test]
fn the_export_of_the_board_uses_the_same_frame_colour() {
    let o = crate::planche::Options::default();
    assert_eq!(o.couleur_cadre, CADRE_DEFAUT);
    // (`App::planche_job` pose la couleur des Préférences dans les options de l'export.)
    let src = include_str!("plancheui.rs");
    assert!(src.contains("couleur_cadre: self.settings.vignettes.couleur_cadre"));
}

// ------------------------------------------------------------------------------------------ la fenêtre Préférences

fn ouvrir_preferences(b: &mut Banc, onglet: Onglet) {
    b.garder_formes = true;
    b.app.settings.prefs_onglet = onglet;
    b.app.run_action("fichier.preferences");
    b.chauffer(6);
}

fn textes(b: &Banc) -> Vec<String> {
    fn marche(s: &Shape, out: &mut Vec<String>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| marche(s, out)),
            Shape::Text(t) => out.push(t.galley.job.text.clone()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        marche(&s.shape, &mut out);
    }
    out
}

#[test]
fn the_vignettes_tab_holds_only_the_vignettes_settings_and_the_pale_rows_moved_out_of_the_montage_tab() {
    let mut b = ouvert(quatre_plans(), 4);
    ouvrir_preferences(&mut b, Onglet::Vignettes);
    let vus = textes(&b);
    let contient = |vus: &[String], m: &str| vus.iter().any(|t| t.contains(m));
    for r in REGLAGES.iter().filter(|r| r.onglet == Onglet::Vignettes) {
        assert!(contient(&vus, r.libelle), "« {} » manque dans l'onglet Vignettes :\n{vus:#?}", r.libelle);
    }
    // Ce qui y est : les rangées pâles, les colonnes, le débord, l'aimant, la couleur des cadres, le report, le décor.
    for m in ["Rangées de cases pâles", "Cases par rangée", "Débord du trait hors des cases", "Aimant des cadres de caméra", "Couleur des cadres de caméra", "Report du décor", "Couleur du décor", "Taille des brosses du décor"] {
        assert!(contient(&vus, m), "{m}");
    }
    // Ce qui n'y est pas : rien du montage, du dessin ou de l'équipe.
    for m in ["Repliage des pistes", "Opacité du passe-partout", "Mon nom"] {
        assert!(!contient(&vus, m), "{m} n'est pas un réglage des Vignettes");
    }
    // L'onglet Montage n'a plus les rangées pâles.
    b.app.prefs_window = false;
    b.chauffer(2);
    ouvrir_preferences(&mut b, Onglet::Montage);
    let vus = textes(&b);
    assert!(!contient(&vus, "Rangées de cases pâles"), "déménagées dans l'onglet Vignettes");
    assert!(contient(&vus, "Repliage des pistes"));
}

#[test]
fn changing_the_pale_rows_and_columns_from_the_vignettes_tab_changes_the_board_and_persists() {
    let mut b = ouvert(quatre_plans(), 4);
    let avant = b.app.board_layout().empty.len();
    b.app.layout.board_min_rows = 5;
    b.chauffer(3);
    assert!(b.app.board_layout().empty.len() > avant, "plus de rangées pâles");
    // Les deux réglages sont ceux de la disposition, retenus dans les réglages de l'application comme avant.
    b.app.layout.board_columns = 3;
    let texte = toml::to_string(&crate::workspace::Layout { board_min_rows: 5, board_columns: 3, ..Default::default() }).unwrap();
    assert!(texte.contains("rangees_pales = 5") && texte.contains("colonnes_vignettes = 3"), "{texte}");
    assert_eq!(b.app.layout.board_settings().columns, 3);
}

#[test]
fn the_decor_pastilles_exist_only_on_the_storyboard_side_and_leaving_it_lets_the_decor_go() {
    // (Depuis le 2026-10-23, elles sont aussi dans le dessin du Storyboard : `pastilles_storyboard_tests.rs` ; pas en animation.)
    use crate::workspace::Espace;
    let mut b = ouvert(quatre_plans(), 4);
    let brosse0 = b.app.brush;
    choisir(&mut b, Genre::Feutre);
    assert_eq!(b.app.perso.decor, Some(Genre::Feutre));
    b.app.set_espace(Espace::Animation);
    b.chauffer(4);
    assert!(!b.app.board_shown(), "l'animation n'est pas la planche");
    assert_eq!(b.app.perso.decor, None, "le décor est lâché en passant à l'animation");
    assert_eq!(b.app.brush, brosse0, "et le pinceau d'avant est revenu");
    assert!(b.app.perso.cases.iter().all(|(_, c)| !matches!(c, Case::Decor(_))), "pas de pastille de décor en animation");
}
