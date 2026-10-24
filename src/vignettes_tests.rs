//! Essais des « Vignettes simples » (2026-10-21) : la Planche de crayonnés disparaît (ses essais sont
//! partis avec elle), un ancien `crayonnes.json` reste intact ; la vue Vignettes se dessine
//! directement (voir `board.rs` pour la mise en page et `boardui.rs` pour l'écran).

use eframe::egui::{self, pos2, vec2, Event, Key, Modifiers, PointerButton, Pos2, Rect};

use crate::app::{BoardView, Tool};
use crate::board::{self, Hit};
use crate::document::Document;
use crate::history::{Edit, History, Nav};
use crate::perf_tests::{banc_taille, se_reposer, Banc};
use crate::project;

fn dossier(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-vignettes-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    dir
}

/// Un `crayonnes.json` comme l'écrivait la version d'avant : quatre cases, dont deux dessinées et jamais
/// transformées en plans, une peinte et déjà servie, une avec seulement une légende.
const ANCIEN: &str = r#"{
  "format": "shot-crayonnes",
  "version": 1,
  "grille": [4, 3],
  "taille": [640, 360],
  "cases": [
    { "id": 1, "legende": "Léa sort", "traits": [ { "stroke": "illisible pour cet essai" } ] },
    { "id": 2, "utilisee": true, "peinture": "iVBORw0KGgo=" },
    { "id": 3, "peinture": "iVBORw0KGgo=" },
    { "id": 4, "legende": "Plan large" }
  ]
}"#;

#[test]
fn the_planche_de_crayonnes_has_no_command_left() {
    use crate::keymap::{ACTIONS, DEFAULT_KEYMAP};
    let reste: Vec<&str> = ACTIONS.iter().map(|a| a.id).filter(|id| id.starts_with("crayonnes.") || *id == "storyboard.crayonnes").collect();
    assert!(reste.is_empty(), "commandes de la planche de crayonnés encore là : {reste:?}");
    assert!(!DEFAULT_KEYMAP.contains("action = \"crayonnes.") && !DEFAULT_KEYMAP.contains("storyboard.crayonnes"), "keymap.toml : plus de bloc de crayonnés");
    // (« Crayonné multiple » est un effet du trait, une autre chose : il reste.)
    assert!(ACTIONS.iter().any(|a| a.id == "fx.crayonne"));
}

#[test]
fn counting_the_forgotten_boxes_of_an_old_file() {
    assert_eq!(crate::crayonnesheritage::cases_oubliees(ANCIEN), Some(2), "deux cases dessinées jamais servies");
    assert_eq!(crate::crayonnesheritage::cases_oubliees("pas du json"), None);
    assert_eq!(crate::crayonnesheritage::cases_oubliees("{\"cases\": []}"), Some(0));
    assert_eq!(crate::crayonnesheritage::cases_oubliees(&format!("\u{feff}{ANCIEN}")), Some(2), "avec un BOM");
}

#[test]
fn a_project_with_an_old_crayonnes_file_opens_says_so_and_never_loses_the_file() {
    let dir = dossier("ancien");
    let manifest = dir.join("P.pelure");
    let doc = Document::with_frame(160, 90, 24);
    project::save(&doc, &manifest, &mut Default::default()).expect("enregistrement");
    assert!(!dir.join("crayonnes.json").exists(), "un projet neuf n'écrit aucun crayonnes.json");
    let fichier = dir.join("crayonnes.json");
    std::fs::write(&fichier, ANCIEN).unwrap();

    let loaded = project::load(&manifest).expect("le projet s'ouvre");
    let avis: Vec<&String> = loaded.warnings.iter().filter(|w| w.contains("crayonnes.json")).collect();
    assert_eq!(avis.len(), 1, "un seul avertissement : {:?}", loaded.warnings);
    assert!(avis[0].contains("2 cases dessinées") && avis[0].contains("Vignettes"), "{}", avis[0]);
    assert_eq!(std::fs::read_to_string(&fichier).unwrap(), ANCIEN, "ouvrir ne touche pas au fichier");

    // Enregistrer (plusieurs fois, comme un travail de plusieurs jours) ne l'efface pas non plus.
    let mut written = loaded.written.clone();
    project::save(&loaded.doc, &manifest, &mut written).expect("enregistrement");
    project::save(&loaded.doc, &manifest, &mut written).expect("enregistrement");
    assert_eq!(std::fs::read_to_string(&fichier).unwrap(), ANCIEN, "enregistrer ne touche pas au fichier");
    // Et le projet rouvert est le même : la planche n'est nulle part dans le document.
    let again = project::load(&manifest).expect("réouverture");
    assert_eq!(again.doc.panels.len(), doc.panels.len());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unreadable_or_empty_crayonnes_file_is_silent_and_untouched() {
    for (nom, contenu) in [("abime", "{ pas du json"), ("vide", "{\"format\":\"shot-crayonnes\",\"version\":1,\"grille\":[4,3],\"taille\":[640,360],\"cases\":[{\"id\":1,\"legende\":\"Seulement une légende\"}]}")] {
        let dir = dossier(nom);
        let manifest = dir.join("P.pelure");
        project::save(&Document::with_frame(160, 90, 24), &manifest, &mut Default::default()).unwrap();
        let fichier = dir.join("crayonnes.json");
        std::fs::write(&fichier, contenu).unwrap();
        let loaded = project::load(&manifest).expect("le projet s'ouvre quand même");
        assert!(loaded.warnings.iter().all(|w| !w.contains("crayonnes.json")), "{nom} : pas d'avertissement ({:?})", loaded.warnings);
        project::save(&loaded.doc, &manifest, &mut loaded.written.clone()).unwrap();
        assert_eq!(std::fs::read_to_string(&fichier).unwrap(), contenu, "{nom} : le fichier reste tel quel");
        std::fs::remove_dir_all(&dir).ok();
    }
}

// ------------------------------------------------------------------------------------ à l'écran

/// Quatre plans, six vignettes : le plan 1 et ses deux marques d'image (cartes 0, 1, 2), les plans 2, 3 (cartes 3, 4)
/// dans la première séquence, le plan 4 (carte 5) dans une deuxième.
pub(crate) fn quatre_plans() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_panel_after(0);
    doc.add_panel_after(1);
    let p2 = doc.add_scene_after(2);
    let p3 = doc.add_scene_after(p2);
    doc.add_sequence_after(p3);
    doc.into_plans();
    assert_eq!((doc.panels.len(), doc.panels[0].marks.len()), (4, 2));
    doc
}

/// La vue Vignettes ouverte sur `doc`, `colonnes` vignettes par ligne, une seule page, assez petites pour tenir à l'écran.
pub(crate) fn ouvert(doc: Document, colonnes: usize) -> Banc {
    let mut b = banc_taille(doc, false, 1600, 900);
    b.app.board_view = BoardView::Panels;
    b.app.layout.board_columns = colonnes;
    b.app.layout.board_rows = 0;
    b.app.layout.card_size = 170.0;
    b.app.board_zoom = 0.0;
    b.chauffer(6);
    b
}

/// L'image `k` de la planche, à l'écran.
pub(crate) fn vignette(b: &Banc, k: usize) -> Rect {
    b.app.vignettes.xf.rect(b.app.board_layout().cards[k].thumb)
}

/// L'icône d'écriture de la case k, à l'écran (au bout de sa bande de note).
pub(crate) fn icone_note(b: &Banc, k: usize) -> Rect {
    b.app.note_icone_rect(&b.app.board_layout().cards[k])
}

fn bande(b: &Banc, k: usize) -> Rect {
    b.app.vignettes.xf.rect(b.app.board_layout().cards[k].note)
}

/// La case vide `e`, à l'écran.
pub(crate) fn case_vide(b: &Banc, e: usize) -> Rect {
    b.app.vignettes.xf.rect(b.app.board_layout().empty[e])
}

/// La zone de la barre après l'image `k`, à l'écran.
pub(crate) fn barre(b: &Banc, k: usize) -> Rect {
    b.app.vignettes.xf.rect(b.app.board_layout().bars[k].zone)
}

fn visible(b: &Banc, r: Rect) -> bool {
    b.app.vignettes.view.contains_rect(r)
}

pub(crate) fn appui(pos: Pos2, down: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed: down, modifiers }
}

/// Un clic : la souris arrive, appui, lâcher (une image d'écran chacun).
fn clic(b: &mut Banc, pos: Pos2) {
    clic_avec(b, pos, Modifiers::NONE);
}

pub(crate) fn clic_avec(b: &mut Banc, pos: Pos2, m: Modifiers) {
    b.image(vec![Event::PointerMoved(pos)], 1.0 / 60.0);
    b.image(vec![appui(pos, true, m)], 1.0 / 60.0);
    b.image(vec![appui(pos, false, m)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Un trait à la souris : un point par image d'écran. Rend le temps de construction de chaque image (ms).
pub(crate) fn glisser(b: &mut Banc, path: &[Pos2]) -> Vec<f64> {
    let mut times = Vec::new();
    b.image(vec![Event::PointerMoved(path[0])], 1.0 / 60.0);
    times.push(b.image(vec![appui(path[0], true, Modifiers::NONE)], 1.0 / 60.0).construction);
    for q in &path[1..] {
        times.push(b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0).construction);
    }
    times.push(b.image(vec![appui(*path.last().expect("point"), false, Modifiers::NONE)], 1.0 / 60.0).construction);
    b.image(Vec::new(), 1.0 / 60.0);
    times
}

/// Un trait qui part de `from` et file vers `to`, en vagues.
pub(crate) fn vague(from: Pos2, to: Pos2, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            from + (to - from) * t + vec2(0.0, (t * 9.0).sin() * 6.0)
        })
        .collect()
}

/// Le plan `i` a de l'encre : un pixel peint ou un trait vectoriel dans l'un de ses dessins.
pub(crate) fn encre(doc: &Document, i: usize) -> bool {
    doc.panels[i].layers.iter().flat_map(|l| l.frames.iter().flatten()).any(|id| {
        doc.cels.get(id).is_some_and(|c| c.raster.tiles.iter().flatten().any(|t| t.iter().any(|px| px.a() != 0)) || c.vector.as_ref().is_some_and(|v| !v.elements.is_empty()))
    })
}

pub(crate) fn pas_d_annulation(b: &Banc) -> usize {
    b.app.history.undo_len()
}

pub(crate) fn dernier_nom(b: &Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

#[test]
fn a_stroke_draws_in_any_image_the_moment_the_pen_goes_down_without_a_prior_click() {
    let mut b = ouvert(quatre_plans(), 4);
    assert_eq!(b.app.cur_panel, 0);
    let (a, c) = (vignette(&b, 3), vignette(&b, 4));
    assert!(visible(&b, a) && visible(&b, c));
    // Plein cadre : le plan 2 n'est pas courant, personne ne l'a cliqué.
    b.image(vec![Event::PointerMoved(a.center())], 1.0 / 60.0);
    b.image(vec![appui(a.center(), true, Modifiers::NONE)], 1.0 / 60.0);
    assert_eq!(b.app.cur_panel, 1, "la vignette est courante dès l'appui, dans la même image d'écran");
    // Sans latence : le premier point reçu est déjà dans le dessin à l'image où il arrive.
    b.image(vec![Event::PointerMoved(a.center() + vec2(8.0, 3.0))], 1.0 / 120.0);
    assert!(encre(&b.app.doc, 1), "l'aperçu du trait est déjà dans le dessin");
    for q in vague(a.center(), a.center() + vec2(c.center().x - a.center().x, 0.0), 14) {
        // (Le trait traverse la vignette voisine : il reste dans celle où il a commencé.)
        b.image(vec![Event::PointerMoved(q)], 1.0 / 120.0);
    }
    b.image(vec![appui(c.center(), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(encre(&b.app.doc, 1));
    for i in [0, 2, 3] {
        assert!(!encre(&b.app.doc, i), "plan {} : rien", i + 1);
    }
    assert_eq!(b.app.cur_panel, 1);
    assert!(b.app.modified);
}

#[test]
fn a_stroke_in_an_image_mark_draws_in_that_image_of_the_plan() {
    let mut b = ouvert(quatre_plans(), 4);
    let mark_frame = b.app.doc.panels[0].marks[1].frame;
    let v = vignette(&b, 2);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    assert_eq!((b.app.cur_panel, b.app.cur_frame), (0, mark_frame), "l'image visée est courante");
    assert!(encre(&b.app.doc, 0));
    // Le dessin est à l'image de la marque, pas à la première image du plan.
    let panel = &b.app.doc.panels[0];
    let cel_a = panel.layers.iter().filter_map(|l| l.frames.get(mark_frame).copied().flatten()).next().expect("un dessin à l'image de la marque");
    assert!(b.app.doc.cels[&cel_a].raster.tiles.iter().flatten().any(|t| t.iter().any(|px| px.a() != 0)) || b.app.doc.cels[&cel_a].vector.as_ref().is_some_and(|v| !v.elements.is_empty()));
}

#[test]
fn a_stroke_that_starts_outside_every_image_draws_nothing() {
    let mut b = ouvert(quatre_plans(), 4);
    let (a, c) = (vignette(&b, 3), vignette(&b, 4));
    let before = pas_d_annulation(&b);
    // De la ligne du numéro (au-dessus de l'image) vers l'image 3 et au-delà : rien ne se trace.
    let start = pos2(a.center().x, a.top() - 8.0);
    assert_eq!(board::hit(&b.app.board_layout(), b.app.vignettes.xf.to_board(start), 0.0), Hit::Nothing);
    glisser(&mut b, &vague(start, pos2(c.center().x, c.center().y), 16));
    for i in 0..4 {
        assert!(!encre(&b.app.doc, i), "plan {} : rien", i + 1);
    }
    assert!(b.app.doc.board.strokes.is_empty(), "plus de note libre tracée au pinceau sur la planche");
    assert_eq!(pas_d_annulation(&b), before, "aucune étape d'annulation");
    assert_eq!(b.app.doc.panels.len(), 4);
    // Entre les pages, hors de la page : de même.
    let dehors = pos2(b.app.vignettes.view.left() + 6.0, b.app.vignettes.view.top() + 6.0);
    glisser(&mut b, &vague(dehors, dehors + vec2(80.0, 40.0), 10));
    assert!(b.app.doc.board.strokes.is_empty() && pas_d_annulation(&b) == before);
}

#[test]
fn drawing_in_the_first_empty_box_makes_the_next_plan_in_one_undo_step() {
    let mut b = ouvert(quatre_plans(), 4);
    let (plans, before) = (b.app.doc.panels.len(), pas_d_annulation(&b));
    let e = case_vide(&b, 0);
    assert!(visible(&b, e));
    b.image(vec![Event::PointerMoved(e.center())], 1.0 / 60.0);
    b.image(vec![appui(e.center(), true, Modifiers::NONE)], 1.0 / 60.0);
    assert_eq!(b.app.doc.panels.len(), plans + 1, "le plan est fait à l'appui même");
    assert_eq!(b.app.cur_panel, plans, "il est courant");
    // Le plan est à la place de la case : la vignette est sous la souris.
    assert!(vignette(&b, 6).contains(e.center()));
    for q in vague(e.center(), e.center() + vec2(40.0, 10.0), 12) {
        b.image(vec![Event::PointerMoved(q)], 1.0 / 120.0);
    }
    b.image(vec![appui(e.center(), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(encre(&b.app.doc, plans), "le trait est dans le plan neuf");
    assert_eq!(b.app.doc.panels.len(), plans + 1);
    assert_eq!(pas_d_annulation(&b), before + 1, "la création et le trait ne font qu'une étape");
    assert_eq!(dernier_nom(&b), "Nouveau plan dessiné");
    // Annuler : plus de plan, plus de trait ; rétablir : les deux reviennent.
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.panels.len(), plans);
    b.image(Vec::new(), 1.0 / 60.0);
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.panels.len(), plans + 1);
    assert!(encre(&b.app.doc, plans));
}

#[test]
fn drawing_in_a_later_empty_box_keeps_its_place_with_blank_plans_before_it() {
    let mut b = ouvert(quatre_plans(), 4);
    let e = case_vide(&b, 1);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 0.0), 8));
    assert_eq!(b.app.doc.panels.len(), 6, "la case 2 fait deux plans : un vide, un dessiné");
    assert!(!encre(&b.app.doc, 4) && encre(&b.app.doc, 5));
    assert_eq!(dernier_nom(&b), "Nouveau plan dessiné");
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.panels.len(), 4, "d'un coup");
    // Plus loin encore (la cinquième case pâle, dans la rangée du dessous : un peu plus petit pour qu'elle tienne dans la vue) :
    // quatre plans vides avant elle, toujours en une étape.
    b.app.board_zoom = 0.6;
    b.chauffer(3);
    let e = case_vide(&b, 4);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 0.0), 8));
    assert_eq!(b.app.doc.panels.len(), 9);
    assert!((4..8).all(|i| !encre(&b.app.doc, i)) && encre(&b.app.doc, 8), "l'ordre de lecture est gardé");
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.panels.len(), 4);
}

#[test]
fn a_click_that_draws_nothing_in_an_empty_box_leaves_no_trace() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.tool = Tool::Line;
    let before = pas_d_annulation(&b);
    let e = case_vide(&b, 0).center();
    clic(&mut b, e);
    assert_eq!(b.app.doc.panels.len(), 4, "pas de plan vide laissé par un simple clic");
    assert_eq!(pas_d_annulation(&b), before);
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.panels.len(), 4, "rien à rétablir non plus");
    // Un outil qui ne dessine pas ne fait pas de plan non plus.
    b.app.tool = Tool::Select;
    glisser(&mut b, &vague(e - vec2(20.0, 0.0), e + vec2(20.0, 0.0), 6));
    assert_eq!(b.app.doc.panels.len(), 4);
    b.app.tool = Tool::Eraser;
    glisser(&mut b, &vague(e - vec2(20.0, 0.0), e + vec2(20.0, 0.0), 6));
    assert_eq!(b.app.doc.panels.len(), 4);
}

#[test]
fn a_grey_bar_before_the_first_empty_box_makes_it_an_image_of_the_last_plan() {
    let mut b = ouvert(quatre_plans(), 4);
    let last = b.app.doc.panels.len() - 1;
    let (frames, marks) = (b.app.doc.panels[last].frame_count(), b.app.doc.panels[last].marks.len());
    let before = pas_d_annulation(&b);
    // La barre est noire (un plan) ; un clic la rend grise (une image du plan).
    assert!(b.app.board_layout().bars.last().unwrap().cut);
    let zone = barre(&b, 5);
    clic(&mut b, zone.center());
    assert!(!b.app.board_layout().bars.last().unwrap().cut, "barre grise");
    assert_eq!(pas_d_annulation(&b), before, "ce n'est qu'un réglage de l'écran, pas une étape");
    let e = case_vide(&b, 0);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 0.0), 8));
    assert_eq!(b.app.doc.panels.len(), 4, "pas de plan de plus");
    let p = &b.app.doc.panels[last];
    assert_eq!(p.marks.len(), marks + 1, "une image de plus");
    assert_eq!(p.marks.last().unwrap().frame, frames);
    assert!(p.frame_count() > frames, "le plan s'allonge de la durée de son image");
    assert_eq!((b.app.cur_panel, b.app.cur_frame), (last, frames));
    assert!(encre(&b.app.doc, last));
    assert_eq!(dernier_nom(&b), "Nouvelle image dessinée");
    assert_eq!(pas_d_annulation(&b), before + 1);
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.doc.panels[last].marks.len(), b.app.doc.panels[last].frame_count()), (marks, frames));
}

#[test]
fn clicking_a_grey_bar_cuts_the_plan_and_a_black_bar_joins_two_plans_each_one_named_step() {
    let mut b = ouvert(quatre_plans(), 6);
    let (plans, before) = (b.app.doc.panels.len(), pas_d_annulation(&b));
    let (mark0, mark1) = (b.app.doc.panels[0].marks[0].frame, b.app.doc.panels[0].marks[1].frame);
    assert!(!b.app.board_layout().bars[0].cut);
    // Grise → noire : la vignette de droite commence un plan neuf.
    { let p = barre(&b, 0).center(); clic(&mut b, p); }
    assert_eq!(b.app.doc.panels.len(), plans + 1, "le plan est coupé");
    assert!(b.app.doc.panels[0].marks.is_empty() && b.app.doc.panels[0].frame_count() == mark0);
    assert_eq!(b.app.doc.panels[1].marks.len(), 1, "la marque suivante suit la partie coupée");
    assert_eq!(b.app.doc.panels[1].marks[0].frame, mark1 - mark0);
    assert!(b.app.board_layout().bars[0].cut, "la barre est noire");
    assert_eq!(b.app.board_layout().cards.len(), 6, "autant de vignettes qu'avant");
    assert_eq!(pas_d_annulation(&b), before + 1, "une seule étape");
    assert_eq!(dernier_nom(&b), "Couper le plan entre deux vignettes");
    // Noire → grise : les deux plans n'en font plus qu'un, la vignette de droite devient une image du plan.
    { let p = barre(&b, 0).center(); clic(&mut b, p); }
    assert_eq!(b.app.doc.panels.len(), plans);
    assert_eq!(b.app.doc.panels[0].marks.len(), 2);
    assert!(!b.app.board_layout().bars[0].cut);
    assert_eq!(pas_d_annulation(&b), before + 2);
    assert_eq!(dernier_nom(&b), "Réunir deux vignettes en un plan");
    // Annuler redonne la coupe, puis le plan d'avant.
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.panels.len(), plans + 1);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.panels.len(), plans);
    assert_eq!(b.app.doc.panels[0].marks.len(), 2);
    // Deux plans de séquences différentes ne se réunissent pas : un message, pas d'étape.
    let n = pas_d_annulation(&b);
    { let p = barre(&b, 4).center(); clic(&mut b, p); }
    assert_eq!(b.app.doc.panels.len(), plans);
    assert_eq!(pas_d_annulation(&b), n);
    assert!(b.app.status.contains("séquence"), "{}", b.app.status);
}

#[test]
fn the_bar_at_the_end_of_a_row_joins_the_last_image_of_the_row_to_the_first_of_the_next() {
    let mut b = ouvert(quatre_plans(), 3);
    let l = b.app.board_layout();
    assert_eq!((l.cards[2].row, l.cards[3].row), (0, 1));
    let zone = barre(&b, 2);
    assert!(visible(&b, zone));
    assert!(zone.left() >= vignette(&b, 2).right() - 0.01, "après la dernière image de la ligne");
    assert!(zone.center().y < vignette(&b, 3).top(), "à la hauteur de la ligne qu'elle termine");
    let plans = b.app.doc.panels.len();
    clic(&mut b, zone.center());
    assert_eq!(b.app.doc.panels.len(), plans - 1, "plan 1 et plan 2 réunis");
    assert_eq!(b.app.doc.panels[0].marks.len(), 3);
    assert_eq!(dernier_nom(&b), "Réunir deux vignettes en un plan");
}

#[test]
fn the_notes_strip_writes_the_notes_of_the_plan_or_of_the_image_mark() {
    let mut b = ouvert(quatre_plans(), 4);
    let mark = b.app.doc.panels[0].marks[0].id;
    let before = pas_d_annulation(&b);
    // Une bande vide : rien d'écrit, un filet très pâle (pas de texte).
    assert!(b.app.doc.panels[0].notes.is_empty());
    let strip = bande(&b, 1);
    assert!(visible(&b, strip));
    { let p = icone_note(&b, 1).center(); clic(&mut b, p); }
    assert!(b.app.vignettes.note.as_ref().is_some_and(|e| e.id == mark), "la bande de la marque est ouverte");
    assert_eq!(b.app.cur_panel, 0);
    b.chauffer(3);
    b.image(vec![Event::Text("Plan large, la voiture arrive".into())], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.vignettes.note.as_ref().map(|e| e.buffer.as_str()), Some("Plan large, la voiture arrive"));
    let enter = |pressed| Event::Key { key: Key::Enter, physical_key: Some(Key::Enter), pressed, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![enter(true)], 1.0 / 60.0);
    b.image(vec![enter(false)], 1.0 / 60.0);
    b.chauffer(2);
    assert!(b.app.vignettes.note.is_none(), "Entrée valide");
    assert_eq!(b.app.doc.panels[0].marks[0].notes, "Plan large, la voiture arrive", "dans la marque d'image");
    assert!(b.app.doc.panels[0].notes.is_empty() && b.app.doc.panels[0].marks[1].notes.is_empty(), "pas dans le plan ni dans l'autre marque");
    assert_eq!(pas_d_annulation(&b), before + 1);
    assert_eq!(dernier_nom(&b), "Note de vignette");
    // La première image d'un plan : les notes du plan lui-même.
    { let p = icone_note(&b, 3).center(); clic(&mut b, p); }
    b.chauffer(3);
    b.image(vec![Event::Text("Contre-champ".into())], 1.0 / 60.0);
    b.chauffer(2);
    // Un clic ailleurs (sur une autre vignette) pose la note, sans autre geste (l'outil Sélection : le pinceau y ferait un point).
    b.app.tool = Tool::Select;
    { let p = vignette(&b, 4).center(); clic(&mut b, p); }
    b.app.tool = Tool::Brush;
    assert!(b.app.vignettes.note.is_none());
    assert_eq!(b.app.doc.panels[1].notes, "Contre-champ", "dans le plan");
    // Échap abandonne ; annuler rend l'état d'avant.
    { let p = icone_note(&b, 3).center(); clic(&mut b, p); }
    b.chauffer(3);
    b.image(vec![Event::Text(" et plus".into())], 1.0 / 60.0);
    b.chauffer(2);
    let esc = |pressed| Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![esc(true)], 1.0 / 60.0);
    b.image(vec![esc(false)], 1.0 / 60.0);
    b.chauffer(2);
    assert!(b.app.vignettes.note.is_none());
    assert_eq!(b.app.doc.panels[1].notes, "Contre-champ", "Échap abandonne la saisie");
    b.app.run_action("edition.annuler");
    assert!(b.app.doc.panels[1].notes.is_empty(), "annuler efface la note du plan");
}

#[test]
fn a_long_note_shows_one_line_and_an_ellipsis_but_is_stored_in_full() {
    let mut b = ouvert(quatre_plans(), 4);
    let long = "Elle traverse la cour sans se retourner, le sac contre la hanche, et le gardien lève la tête un instant trop tard : la grille est déjà refermée derrière elle, et personne ne l'a vue partir ce matin-là, pas même le chien.";
    b.app.doc.panels[1].notes = long.into();
    b.chauffer(3);
    let l = b.app.board_layout();
    assert_eq!(l.cards[3].note.height(), board::NOTE_LINE_H, "une seule ligne pour le champ Notes, quelle que soit la longueur");
    assert_eq!(b.app.doc.panels[1].notes, long, "le texte entier est gardé");
    // À l'écran : une ligne, qui finit par « … ».
    b.garder_formes = true;
    b.image(Vec::new(), 1.0 / 60.0);
    let galley = b
        .formes
        .iter()
        .filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some(t.galley.clone()) } else { None })
        .find(|g| g.job.text == long)
        .expect("la note est peinte");
    assert_eq!(galley.rows.len(), 1);
    assert!(galley.rows.last().unwrap().text().ends_with('…'), "« … » au bout de la ligne : {:?}", galley.rows.last().unwrap().text());
    // Pendant la saisie, tout est là : le tampon a le texte entier.
    { let p = icone_note(&b, 3).center(); clic(&mut b, p); }
    assert_eq!(b.app.vignettes.note.as_ref().map(|e| e.buffer.as_str()), Some(long));
    b.chauffer(3);
    assert!(b.app.vignettes.note.is_some());
    // Une note plus courte ne change pas la hauteur : la ligne reste une ligne, la planche ne bouge pas.
    { let p = vignette(&b, 0).center(); clic(&mut b, p); }
    let avant = b.app.board_layout();
    b.app.doc.panels[1].notes = "Contre-champ serré sur son visage, elle ne dit rien.".into();
    b.chauffer(3);
    assert_eq!(b.app.board_layout(), avant);
}

#[test]
fn the_note_font_shrinks_a_little_with_the_length() {
    assert_eq!(board::note_font_size(5), 11.0);
    assert_eq!(board::note_font_size(40), 11.0);
    assert_eq!(board::note_font_size(60), 10.0);
    assert_eq!(board::note_font_size(200), 9.5);
    assert!(board::note_font_size(300) <= board::note_font_size(10));
}

#[test]
fn only_the_plan_number_is_written_above_the_first_image_of_each_plan() {
    let mut b = ouvert(quatre_plans(), 4);
    b.garder_formes = true;
    b.image(Vec::new(), 1.0 / 60.0);
    let view = b.app.vignettes.view;
    let mut textes: Vec<String> = b
        .formes
        .iter()
        .filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some((t.pos, t.galley.job.text.clone())) } else { None })
        .filter(|(pos, _)| view.contains(*pos))
        // (La colonne des pastilles des personnages, au bord droit de la planche : ses icônes ne sont pas des numéros.)
        .filter(|(pos, _)| !b.app.perso.cases.iter().any(|(r, _)| r.expand(8.0).contains(*pos)))
        .map(|(_, t)| t)
        .filter(|t| t != egui_phosphor::regular::PLUS) // (le « + » discret de la case vide)
        .collect();
    textes.sort();
    textes.dedup();
    assert_eq!(textes, vec!["1", "2", "3", "4"], "les numéros des quatre plans, rien d'autre (ni « Plan », ni durée)");
}

#[test]
fn shift_click_chooses_plans_instead_of_drawing_and_ctrl_click_no_longer_does() {
    let mut b = ouvert(quatre_plans(), 4);
    let (ids, before) = (b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), pas_d_annulation(&b));
    // (Ctrl seul est le lasso des Vignettes, comme partout : il ne choisit plus de plans.)
    { let p = vignette(&b, 4).center(); clic_avec(&mut b, p, Modifiers::COMMAND); }
    assert_eq!((b.app.cur_panel, b.app.board_sel.clone()), (2, vec![ids[2]]), "Ctrl + clic prend la case comme un clic (le lasso), il n'ajoute rien au choix");
    b.app.set_tool(crate::app::Tool::Brush);
    b.app.select_panel(0);
    b.app.board_sel = vec![ids[0]];
    b.app.vignettes.press = crate::boardui::Press::None;
    { let p = vignette(&b, 3).center(); clic_avec(&mut b, p, Modifiers::SHIFT); }
    assert_eq!(b.app.board_selection(), vec![ids[0], ids[1]], "Maj + clic prend de la dernière vignette courante à celle-ci");
    for i in 0..4 {
        assert!(!encre(&b.app.doc, i));
    }
    assert_eq!(pas_d_annulation(&b), before);
    assert!(b.app.panel_focus, "les plans choisis ont la main (copier, coller, supprimer)");
}

#[test]
fn the_context_menu_of_an_image_targets_its_plan() {
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 4).center();
    b.image(vec![Event::PointerMoved(v)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: v, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    assert_eq!(b.app.cur_panel, 2, "le clic droit choisit la vignette");
    let plan = b.app.doc.panels[2].id;
    assert_eq!(b.app.vignettes.menu, Some(crate::boardui::MenuTarget::Card { plan, case: plan, cadre: None }));
    b.image(vec![Event::PointerButton { pos: v, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
}

#[test]
fn a_locked_storyboard_refuses_new_plans_and_cuts() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.doc.storyboard_locked = true;
    let before = pas_d_annulation(&b);
    let e = case_vide(&b, 0);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 0.0), 8));
    assert_eq!(b.app.doc.panels.len(), 4);
    { let p = barre(&b, 0).center(); clic(&mut b, p); }
    assert_eq!(b.app.doc.panels.len(), 4);
    assert_eq!(pas_d_annulation(&b), before);
}

#[test]
fn merging_the_last_undo_steps_makes_one_step_and_forgetting_one_leaves_no_redo() {
    let mut doc = Document::with_frame(160, 90, 24);
    let mut nav = Nav { panel: 0, layer: 0 };
    let mut history = History::default();
    let snap = |doc: &Document| Edit::Structure { structure: doc.structure(), nav: Nav { panel: 0, layer: 0 } };
    history.context = Some("Premier".into());
    history.push(snap(&doc));
    doc.add_panel_after(0);
    history.context = Some("Deuxième".into());
    history.push(snap(&doc));
    doc.add_panel_after(1);
    history.context = Some("Troisième".into());
    history.push(snap(&doc));
    doc.add_panel_after(2);
    assert_eq!((history.undo_len(), doc.panels.len()), (3, 4));
    // Les deux dernières n'en font qu'une, qui garde le nom de la plus ancienne des deux.
    history.merge_last(2);
    assert_eq!(history.undo_len(), 2);
    assert_eq!(history.names().0, vec!["Premier", "Deuxième"]);
    history.merge_last(1);
    history.merge_last(5);
    assert_eq!(history.undo_len(), 2, "moins de deux, ou plus qu'il n'y en a : rien");
    assert!(history.undo(&mut doc, &mut nav));
    assert_eq!(doc.panels.len(), 2, "annuler défait les deux d'un coup");
    assert!(history.redo(&mut doc, &mut nav));
    assert_eq!(doc.panels.len(), 4);
    assert!(history.undo_and_forget(&mut doc, &mut nav));
    assert_eq!(doc.panels.len(), 2);
    assert!(!history.redo(&mut doc, &mut nav), "oubliée : rien à rétablir");
    assert_eq!(history.names().1.len(), 0);
}

#[test]
fn drawing_in_the_vignettes_costs_about_what_it_costs_in_the_drawing_view() {
    // Un trait de 60 points, quatre événements par image, dans une vignette puis dans la vue Dessin.
    let trait_de = |origine: Pos2| -> Vec<Pos2> { (0..60).map(|k| origine + vec2(k as f32 * 1.4, (k as f32 * 0.3).sin() * 9.0)).collect() };
    let run = |b: &mut Banc, path: &[Pos2]| -> Vec<f64> {
        let mut times = Vec::new();
        b.image(vec![Event::PointerMoved(path[0])], 1.0 / 60.0);
        times.push(b.image(vec![appui(path[0], true, Modifiers::NONE)], 1.0 / 60.0).construction);
        for chunk in path[1..].chunks(4) {
            times.push(b.image(chunk.iter().map(|q| Event::PointerMoved(*q)).collect(), 1.0 / 60.0).construction);
        }
        b.image(vec![appui(*path.last().unwrap(), false, Modifiers::NONE)], 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
        times
    };
    let stats = |v: &[f64]| (v.iter().sum::<f64>() / v.len() as f64, v.iter().cloned().fold(0.0, f64::max));
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 3);
    let dans_vignette = run(&mut b, &trait_de(v.center() - vec2(40.0, 0.0)));
    assert!(encre(&b.app.doc, 1));
    // La vue Dessin : le même trait au milieu du dessin.
    b.app.board_view = BoardView::Scene;
    b.chauffer(6);
    b.app.select_panel(2);
    b.chauffer(3);
    let r = b.app.canvas_rect;
    let dans_dessin = run(&mut b, &trait_de(r.center() - vec2(40.0, 0.0)));
    assert!(encre(&b.app.doc, 2));
    let ((vm, vp), (dm, dp)) = (stats(&dans_vignette), stats(&dans_dessin));
    println!("VIGNETTES : trait de 60 points : image moyenne {vm:.2} ms, pire {vp:.2} ms | vue Dessin : moyenne {dm:.2} ms, pire {dp:.2} ms");
    // (Large : pas de carte graphique ici et la machine peut être chargée. Les chiffres sont affichés.)
    assert!(vm < dm * 4.0 + 80.0, "image moyenne {vm:.1} ms dans les vignettes contre {dm:.1} ms dans la vue Dessin");
    assert!(vp < 3000.0, "pire image {vp:.1} ms");
}

#[test]
fn the_vignettes_view_is_idle_when_nothing_happens() {
    // (Le moindre calcul d'effet d'un autre essai réveille cette application : état commun pour elle seule.)
    crate::fxcache::monde::ecrire();
    let mut b = ouvert(quatre_plans(), 4);
    b.app.doc.panels[1].notes = "Une note, une seule.".into();
    let n = se_reposer(&mut b, 80);
    assert!(n < 80, "la vue Vignettes se repose ({n} images)");
    let m = b.image(Vec::new(), 0.5);
    assert!(m.reveil >= std::time::Duration::from_secs(1), "aucune image demandée avant une seconde : {:?}", m.reveil);
}
/// Des images de la vue Vignettes (dessins, notes de une à trois lignes, coupes, cases vides) dans le dossier
/// `SHOT_CAPTURE_DIR`, pour en vérifier la mise en page à l'œil. Sans carte graphique : rien.
#[test]
#[ignore]
fn vignettes_capture() {
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let mut b = crate::perf_tests::banc(quatre_plans(), true);
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("VIGNETTES pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("VIGNETTES {nom}");
    };
    b.app.board_view = BoardView::Panels;
    b.app.layout.board_columns = 4;
    b.app.layout.board_rows = 0;
    b.app.layout.card_size = 300.0;
    b.app.board_zoom = 0.0;
    b.chauffer(6);
    for k in [0usize, 1, 3, 4] {
        let v = vignette(&b, k);
        let (c, rx, ry) = (v.center(), v.width() * 0.3, v.height() * 0.28);
        let ellipse: Vec<Pos2> = (0..=40).map(|i| c + vec2((i as f32 / 40.0 * 6.2).cos() * rx, (i as f32 / 40.0 * 6.2).sin() * ry)).collect();
        glisser(&mut b, &ellipse);
        glisser(&mut b, &[c - vec2(rx, 0.0), c + vec2(rx * 0.8, ry * 0.2 * (k as f32 - 2.0) / 2.0)]);
    }
    b.app.doc.panels[0].notes = "Plan large, la rue".into();
    b.app.doc.panels[0].marks[0].notes = "Il entre dans le champ par la gauche, sans se presser, et regarde les fenêtres une à une.".into();
    b.app.doc.panels[1].notes = "Contrechamp".into();
    b.app.doc.panels[2].notes = "Elle traverse la cour sans se retourner, le sac contre la hanche, et le gardien lève la tête un instant trop tard : la grille est déjà refermée derrière elle, et personne ne l'a vue partir ce matin-là, pas même le chien.".into();
    b.chauffer(4);
    sauver(&b, "vignettes-1-dessins-et-notes.png");
    // Une coupe de plus (barre noire) et la prochaine case en image du plan (barre grise).
    let p = barre(&b, 0).center();
    clic(&mut b, p);
    let p = barre(&b, 5).center();
    clic(&mut b, p);
    b.chauffer(4);
    sauver(&b, "vignettes-2-coupes.png");
    b.app.layout.board_columns = 3;
    b.chauffer(4);
    sauver(&b, "vignettes-3-trois-colonnes.png");
}
/// Le coût d'une image d'écran en traçant dans une vignette et dans la vue Dessin, avec la carte graphique (`--ignored`) :
/// un trait de 120 points, huit événements par image, trois traits de suite.
#[test]
#[ignore]
fn perf_vignettes_drawing() {
    let mut b = crate::perf_tests::banc(quatre_plans(), true);
    b.app.board_view = BoardView::Panels;
    b.app.layout.board_columns = 4;
    b.app.layout.board_rows = 0;
    b.app.layout.card_size = 300.0;
    b.app.board_zoom = 0.0;
    b.chauffer(8);
    let trait_de = |origine: Pos2, k: usize| -> Vec<Pos2> { (0..120).map(|i| origine + vec2(i as f32 * 1.2, (i as f32 * 0.25 + k as f32).sin() * 12.0 + k as f32 * 9.0)).collect() };
    let run = |b: &mut Banc, path: &[Pos2]| -> Vec<f64> {
        let mut totals = Vec::new();
        b.image(vec![Event::PointerMoved(path[0])], 1.0 / 60.0);
        totals.push(b.image(vec![appui(path[0], true, Modifiers::NONE)], 1.0 / 60.0).total());
        for chunk in path[1..].chunks(8) {
            totals.push(b.image(chunk.iter().map(|q| Event::PointerMoved(*q)).collect(), 1.0 / 60.0).total());
        }
        b.image(vec![appui(*path.last().unwrap(), false, Modifiers::NONE)], 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
        totals
    };
    let resume = |nom: &str, v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        let moy = v.iter().sum::<f64>() / v.len() as f64;
        println!("PERF {nom} : {} images, moyenne {moy:.2} ms, médiane {:.2}, 95e {:.2}, pire {:.2}", v.len(), v[v.len() / 2], v[v.len() * 95 / 100], v[v.len() - 1]);
    };
    let (mut dans_vignette, mut dans_dessin) = (Vec::new(), Vec::new());
    let v = vignette(&b, 0);
    for k in 0..3 {
        dans_vignette.extend(run(&mut b, &trait_de(v.min + vec2(20.0, 25.0), k)));
    }
    resume("trait dans une vignette de la planche", &mut dans_vignette);
    b.app.board_view = BoardView::Scene;
    b.chauffer(8);
    let r = b.app.canvas_rect;
    for k in 0..3 {
        dans_dessin.extend(run(&mut b, &trait_de(r.min + vec2(r.width() * 0.3, r.height() * 0.3), k)));
    }
    resume("trait dans la vue Dessin", &mut dans_dessin);
}

/// Supprimer le plan courant quand c'est le dernier, ou plusieurs plans dont le courant, ne plante pas (Anthony,
/// 2026-10-22 : « index out of bounds » dans `sync_couleurs`, appelé avant que le plan courant soit remis).
#[test]
fn deleting_the_current_last_plans_does_not_crash() {
    let mut b = ouvert(quatre_plans(), 3);
    let dernier = b.app.doc.panels.len() - 1;
    b.app.select_panel(dernier);
    let id = b.app.doc.panels[dernier].id;
    b.app.delete_panels(&[id]);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 3);
    assert!(b.app.cur_panel < b.app.doc.panels.len());
    // Deux plans d'un coup, dont le courant, en fin de film.
    let n = b.app.doc.panels.len();
    b.app.select_panel(n - 1);
    let ids = [b.app.doc.panels[n - 2].id, b.app.doc.panels[n - 1].id];
    b.app.delete_panels(&ids);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 1);
    assert_eq!(b.app.cur_panel, 0);
    // Et l'annulation rend les plans.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 3);
    assert!(b.app.cur_panel < b.app.doc.panels.len());
}