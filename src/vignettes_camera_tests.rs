//! Essais des Vignettes : le quatrième onglet « Vignettes » (avant Storyboard) avec sa disposition épurée, et les
//! mouvements de caméra dessinés dans les cases (Ctrl + glisser un cadre : A, B, C…, la case qui grandit).

use eframe::egui::{self, epaint::Shape, pos2, vec2, Event, Key, Modifiers, PointerButton, Pos2, Rect};

use crate::app::{App, Mode, Tool};
use crate::board;
use crate::boardui::Press;
use crate::camera::Camera;
use crate::dock::Pane;
use crate::document::Document;
use crate::perf_tests::{banc_taille, se_reposer, Banc};
use crate::vignettes_tests::{appui, dernier_nom, encre, glisser, ouvert, pas_d_annulation, quatre_plans, vague, vignette};
use crate::workspace::{Espace, Layout};

/// Ctrl + Maj : le geste de caméra des Vignettes (Ctrl seul est le lasso).
pub(crate) const CTRL: Modifiers = Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE };
/// Ctrl seul : le lasso.
pub(crate) const CTRL_SEUL: Modifiers = Modifiers::CTRL;

fn film() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    for _ in 0..3 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    doc
}

fn banc() -> Banc {
    let mut b = banc_taille(film(), false, 1600, 900);
    b.garder_textes = true;
    b.garder_formes = true;
    b.chauffer(6);
    b
}

fn centre_du_texte(b: &Banc, texte: &str) -> Pos2 {
    for clipped in &b.formes {
        if let Shape::Text(t) = &clipped.shape {
            if t.galley.job.text == texte {
                return t.pos + t.galley.size() / 2.0;
            }
        }
    }
    panic!("le texte « {texte} » n'est pas à l'écran : {:?}", b.textes);
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(2);
}

fn touche(b: &mut Banc, key: Key, modifiers: Modifiers) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }], 1.0 / 60.0);
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers }], 1.0 / 60.0);
    b.chauffer(2);
}

fn has(app: &App, pane: Pane) -> bool {
    app.dock_layout().contains(pane)
}

// ------------------------------------------------------------------------------ les quatre onglets

#[test]
fn the_four_tabs_are_drawn_vignettes_first_and_switch_spaces_by_clicks() {
    let mut b = banc();
    let tous = [" Vignettes ", " Storyboard ", " Animation ", " Montage "];
    let xs: Vec<f32> = tous.iter().map(|t| centre_du_texte(&b, t).x).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "Vignettes, Storyboard, Animation, Montage : {xs:?}");
    assert!(tous.iter().all(|t| centre_du_texte(&b, t).y < 40.0), "dans la barre de menus, en haut");
    assert_eq!(Espace::TOUS.map(Espace::nom), ["Vignettes", "Storyboard", "Animation", "Montage"]);
    for (espace, mode) in [(Espace::Vignettes, Mode::Storyboard), (Espace::Animation, Mode::Animation), (Espace::Montage, Mode::Storyboard), (Espace::Storyboard, Mode::Storyboard), (Espace::Vignettes, Mode::Storyboard)] {
        let texte = format!(" {} ", espace.nom());
        let p = centre_du_texte(&b, &texte);
        clic(&mut b, p);
        assert_eq!((b.app.espace, b.app.mode), (espace, mode), "{texte}");
        let apres: Vec<f32> = tous.iter().map(|t| centre_du_texte(&b, t).x).collect();
        assert_eq!(xs, apres, "les onglets ne bougent pas d'un espace à l'autre");
    }
}

#[test]
fn switching_to_the_vignettes_keeps_the_playhead_the_selection_and_the_project() {
    let mut b = banc();
    b.app.select_panel(2);
    b.app.cur_frame = 0;
    b.app.board_sel = vec![b.app.doc.panels[1].id];
    let (panel, frame, global, layer) = (b.app.cur_panel, b.app.cur_frame, b.app.global_frame(), b.app.cur_layer);
    let (total, selection) = (b.app.doc.total_frames(), b.app.board_selection());
    let undo = b.app.history.names().0.len();
    for espace in [Espace::Vignettes, Espace::Animation, Espace::Vignettes, Espace::Montage, Espace::Vignettes, Espace::Storyboard] {
        b.app.set_espace(espace);
        b.chauffer(2);
        assert_eq!((b.app.cur_panel, b.app.cur_layer), (panel, layer), "{espace:?}");
        assert_eq!((b.app.global_frame(), b.app.cur_frame), (global, frame));
        assert_eq!(b.app.board_selection(), selection, "{espace:?} : les plans choisis");
        assert_eq!(b.app.doc.total_frames(), total);
        assert_eq!(b.app.history.names().0.len(), undo, "aucune étape d'annulation");
        assert!(!b.app.playing);
    }
}

#[test]
fn only_the_selected_tab_is_lit() {
    let mut b = banc();
    let couleur = |b: &Banc, texte: &str| -> egui::Color32 {
        for clipped in &b.formes {
            if let Shape::Text(t) = &clipped.shape {
                if t.galley.job.text == texte {
                    return t.galley.job.sections[0].format.color;
                }
            }
        }
        panic!("{texte}");
    };
    let sur_accent = crate::theme::sur_accent();
    for espace in Espace::TOUS {
        b.app.set_espace(espace);
        b.chauffer(3);
        for autre in Espace::TOUS {
            let texte = format!(" {} ", autre.nom());
            assert_eq!(couleur(&b, &texte) == sur_accent, autre == espace, "{} allumé, {texte}", espace.nom());
        }
    }
}

#[test]
fn alt_shift_v_goes_to_the_vignettes_and_nothing_else_uses_it() {
    let b = banc();
    let m = Modifiers { alt: true, shift: true, ..Modifiers::NONE };
    for mode in [Mode::Storyboard, Mode::Animation] {
        assert_eq!(b.app.keymap.find(Key::V, Some(Key::V), m, mode), Some("espace.vignettes"));
    }
    assert!(b.app.keymap.warnings.iter().all(|w| !w.contains("espace.vignettes") && !w.contains("Alt+Maj+V")), "{:?}", b.app.keymap.warnings);
    assert_eq!(b.app.keymap.label("espace.vignettes"), "Maj+Alt+V");
    // Alt+V reste le miroir vertical de la sélection.
    let alt = Modifiers { alt: true, ..Modifiers::NONE };
    assert_eq!(b.app.keymap.find(Key::V, Some(Key::V), alt, Mode::Storyboard), Some("selection.miroir_v"));
    assert!(crate::keymap::action_info("espace.vignettes").is_some_and(|a| a.implemented));
}

#[test]
fn the_shortcut_reaches_the_vignettes_and_tab_leaves_them_for_the_storyboard() {
    let mut b = banc();
    b.crochet = true;
    let m = Modifiers { alt: true, shift: true, ..Modifiers::NONE };
    touche(&mut b, Key::V, m);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Vignettes, Mode::Storyboard), "Alt+Maj+V");
    touche(&mut b, Key::Tab, Modifiers::NONE);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Storyboard, Mode::Storyboard), "Tab revient au storyboard");
    touche(&mut b, Key::Tab, Modifiers::NONE);
    assert_eq!(b.app.mode, Mode::Animation);
    // « Animer cette image » depuis les Vignettes, puis Tab : on revient aux Vignettes.
    b.app.set_espace(Espace::Vignettes);
    b.app.set_mode(Mode::Animation);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Animation, Mode::Animation));
    touche(&mut b, Key::Tab, Modifiers::NONE);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Vignettes, Mode::Storyboard));
}

// ------------------------------------------------------------------------------ la disposition épurée

#[test]
fn the_vignettes_layout_is_bare_the_board_the_brush_the_colour() {
    let layout = Espace::Vignettes.defaut();
    let dock = layout.dock_layout();
    for p in [Pane::Canvas, Pane::ToolOptions, Pane::Color, Pane::Brushes] {
        assert!(dock.contains(p), "{p:?} est dans les Vignettes");
    }
    for p in [Pane::Montage, Pane::Timeline, Pane::Layers, Pane::Effects, Pane::Sheet, Pane::XSheet, Pane::References, Pane::Tools, Pane::Guides, Pane::History, Pane::Comments, Pane::Bruitages, Pane::SuggestionsSonores] {
        assert!(!dock.contains(p), "{p:?} n'est pas dans les Vignettes");
    }
    assert!(layout.vignettes && layout.tool_bar && layout.command_bar);
    // Les Références s'ouvrent d'un clic : le panneau est dans la barre des panneaux, fermé.
    let mut b = banc();
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(3);
    assert!(!has(&b.app, Pane::References));
    b.app.clic_panneau(Pane::References);
    b.chauffer(3);
    assert!(has(&b.app, Pane::References), "un clic ouvre les Références");
    // La couleur et les pinceaux sont en onglets dans une même colonne, les réglages au-dessus.
    assert!(b.app.board_shown() && b.app.layout.vignettes);
    assert_eq!(b.app.pane_title(Pane::Canvas), "Vignettes");
}

#[test]
fn the_character_bar_is_shown_in_the_vignettes_and_hidden_in_the_montage() {
    assert!(Espace::Vignettes.barre_personnages_par_defaut());
    let mut b = banc();
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    assert!(b.app.barre_perso_visible());
    // (Les pastilles sont une colonne au bord droit de la planche, sans texte : « Aucun personnage » est une bulle.)
    assert!(b.app.perso.cases.iter().any(|(_, c)| *c == crate::personnagesui::Case::Plus), "la colonne des pastilles est peinte sur la planche : {:?}", b.app.perso.cases);
    assert!(b.textes.iter().any(|t| t.contains(egui_phosphor::regular::PLUS)), "le « + » de la colonne est peint : {:?}", b.textes);
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    assert!(!b.app.barre_perso_visible());
}

#[test]
fn the_tool_column_of_the_vignettes_is_minimal() {
    use crate::toolbars::{TOOL_GROUPS, TOOL_GROUPS_VIGNETTES};
    let outils: Vec<Tool> = TOOL_GROUPS_VIGNETTES.iter().flat_map(|g| g.iter().flat_map(|s| s.iter().copied())).collect();
    assert_eq!(outils, vec![Tool::Brush, Tool::Pencil, Tool::Eraser]);
    assert!(TOOL_GROUPS.iter().flat_map(|g| g.iter().flat_map(|s| s.iter())).count() > outils.len());
    // Elle se dessine sans broncher dans les deux espaces.
    let mut b = banc();
    for e in [Espace::Vignettes, Espace::Storyboard, Espace::Vignettes] {
        b.app.set_espace(e);
        b.chauffer(3);
    }
}

#[test]
fn the_layout_is_remembered_and_restored() {
    let mut b = banc();
    b.app.set_espace(Espace::Vignettes);
    // Ouvrir l'Historique dans les Vignettes se retient pour elles seules.
    assert!(!has(&b.app, Pane::History));
    b.app.toggle_pane(Pane::History);
    assert!(has(&b.app, Pane::History));
    b.app.set_espace(Espace::Storyboard);
    assert!(!has(&b.app, Pane::History), "le storyboard garde la sienne");
    b.app.set_espace(Espace::Animation);
    b.app.set_espace(Espace::Vignettes);
    assert!(has(&b.app, Pane::History), "les Vignettes retrouvent la leur");
    assert!(b.app.settings.layout_of(Espace::Vignettes).is_some());
    b.app.run_action("espace.retablir");
    assert!(!has(&b.app, Pane::History), "Rétablir la disposition d'origine");
    assert!(has(&b.app, Pane::Color));
    let _: &Layout = b.app.settings.layout_of(Espace::Vignettes).unwrap_or(&b.app.layout);
}

#[test]
fn an_old_settings_file_reads_and_the_vignettes_have_their_own_key() {
    let s: crate::settings::Settings = toml::from_str("espace_storyboard = \"vignettes\"
").unwrap();
    assert_eq!(s.storyboard_space, Espace::Vignettes);
    assert!(s.vignettes_layout.is_none());
    let mut s = crate::settings::Settings::default();
    s.set_layout_of(Espace::Vignettes, Espace::Vignettes.defaut());
    let t = toml::to_string(&s).unwrap();
    assert!(t.contains("disposition_vignettes"));
    let r: crate::settings::Settings = toml::from_str(&t).unwrap();
    assert_eq!(r.layout_of(Espace::Vignettes), s.layout_of(Espace::Vignettes));
}

// ------------------------------------------------------------------------------ les mouvements de caméra

pub(crate) fn plan_court() -> Document {
    let mut doc = quatre_plans();
    doc.set_duration(1, 25);
    doc
}

pub(crate) fn ctrl_glisser(b: &mut Banc, de: Pos2, chemin: &[Pos2], a: Pos2, lacher: bool) {
    glisser_avec(b, CTRL, de, chemin, a, lacher);
}

pub(crate) fn glisser_avec(b: &mut Banc, m: Modifiers, de: Pos2, chemin: &[Pos2], a: Pos2, lacher: bool) {
    b.touches = m;
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![appui(de, true, m)], 1.0 / 60.0);
    for q in chemin {
        b.image(vec![Event::PointerMoved(*q)], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    if lacher {
        b.image(vec![appui(a, false, m)], 1.0 / 60.0);
        b.touches = Modifiers::NONE;
        b.image(Vec::new(), 1.0 / 60.0);
    }
}

/// Les points d'un glissé, un à un (le geste ne saute pas d'un coup).
pub(crate) fn pas(de: Pos2, delta: egui::Vec2, n: usize) -> Vec<Pos2> {
    (1..n).map(|k| de + delta * (k as f32 / n as f32)).collect()
}

pub(crate) const K: usize = 3; // la première image du plan 2 (le plan 1 a deux marques)

pub(crate) fn clefs(b: &Banc) -> Vec<(usize, f32, f32, f32)> {
    b.app.doc.panels[1].camera.iter().map(|k| (k.frame, k.cam.x, k.cam.y, k.cam.zoom)).collect()
}

/// Le pas de l'écran qui vaut une largeur de cadre sur la feuille.
pub(crate) fn largeur_cadre_ecran(b: &Banc) -> f32 {
    b.app.vignettes.xf.zoom * b.app.layout.card_size
}

pub(crate) fn cadre_ecran(b: &Banc, rang: usize) -> [Pos2; 4] {
    let layout = b.app.board_layout();
    let (quads, _) = b.app.cadres_sur_la_planche(&layout, K);
    quads[rang].map(|p| b.app.vignettes.xf.to_screen(p))
}

pub(crate) fn centre(q: [Pos2; 4]) -> Pos2 {
    pos2((q[0].x + q[2].x) * 0.5, (q[0].y + q[2].y) * 0.5)
}

/// Un mouvement posé d'un Ctrl + glisser d'une largeur de cadre vers la droite.
pub(crate) fn avec_b_a_droite() -> Banc {
    let mut b = ouvert(plan_court(), 6);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    b
}

#[test]
fn ctrl_dragging_the_case_makes_b_at_the_end_and_the_case_widens_in_the_direction_of_the_move() {
    let mut b = ouvert(plan_court(), 6);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    let a0 = b.app.doc.panel_camera(1, 0);
    assert!(b.app.doc.panels[1].camera.is_empty());
    let (avant, nb) = (b.app.board_layout().cards[K], pas_d_annulation(&b));
    assert!(!avant.mouvement && avant.foot == avant.thumb);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 8), case.center() + vec2(w, 0.0), false);
    // En plein geste : les clés sont déjà là et l'empreinte aussi, mais l'étape d'annulation n'est pas encore prise.
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    let l = b.app.board_layout();
    assert!(l.cards[K].mouvement);
    assert_eq!(pas_d_annulation(&b), nb);
    b.image(vec![appui(case.center() + vec2(w, 0.0), false, CTRL)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    let n = b.app.doc.panels[1].frame_count();
    let k = clefs(&b);
    assert_eq!(k.iter().map(|c| c.0).collect::<Vec<_>>(), vec![0, n - 1], "A au début du plan, B à la fin");
    assert_eq!((k[0].1, k[0].2), (a0.x, a0.y), "le cadre d'origine reste A");
    assert!((k[1].1 - (a0.x + 160.0)).abs() < 2.0 && (k[1].2 - a0.y).abs() < 2.0, "B est une largeur de cadre à droite : {k:?}");
    assert_eq!(b.app.doc.panels[1].camera[1].ease_in, 0.0, "vitesse constante, comme au montage");
    assert_eq!(pas_d_annulation(&b), nb + 1, "une seule étape");
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
    // La case garde sa taille (même échelle) ; l'empreinte s'élargit d'une case, pas d'un bloc de quatre.
    let l = b.app.board_layout();
    let c = l.cards[K];
    assert_eq!(c.thumb.size(), avant.thumb.size(), "le contenu ne rapetisse jamais");
    let m = l.move_of(K).expect("une empreinte");
    assert!((c.foot.width() - 2.0 * c.thumb.width()).abs() < 1.5, "deux cases de large : {} pour {}", c.foot.width(), c.thumb.width());
    assert!((c.foot.height() - c.thumb.height()).abs() < 1e-3);
    assert_eq!(m.cadres.len(), 2);
    assert!(m.cadres[1].rect.left() >= m.cadres[0].rect.right() - 1.0, "B est à côté de A : {:?}", m.cadres);
    // Annuler remet tout : plus de clés, l'empreinte disparaît.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(b.app.doc.panels[1].camera.is_empty());
    assert!(!b.app.board_layout().cards[K].mouvement);
}

#[test]
fn the_footprint_goes_where_the_move_goes_a_half_case_is_a_half_case() {
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, 80.0, 0.0);
    let s = board::BoardSettings { card_w: 170.0, columns: 6, rows: 0 };
    let l = board::layout(&doc, &s);
    let c = l.cards[K];
    assert!((c.foot.width() - 1.5 * 170.0).abs() <= 1e-2, "une case et demie, exactement (aucun arrondi) : {}", c.foot.width());
    // Vers la gauche : la case s'élargit à gauche de A.
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, -160.0, 0.0);
    let l = board::layout(&doc, &s);
    let c = l.cards[K];
    assert!(c.foot.left() < c.thumb.left() - 100.0 && (c.foot.right() - c.thumb.right()).abs() < 1e-3);
}

#[test]
fn a_zoom_out_is_drawn_around_the_case_and_clamped_to_the_row_height() {
    let mut doc = plan_court();
    let a = doc.default_camera();
    doc.panels[1].camera.clear();
    crate::camera::set_key(&mut doc.panels[1].camera, 0, a);
    let n = doc.panels[1].frame_count();
    // Un très grand zoom arrière : B voudrait trois fois la case.
    crate::camera::set_key(&mut doc.panels[1].camera, n - 1, Camera { zoom: a.zoom / 3.0, ..a });
    let s = board::BoardSettings { card_w: 170.0, columns: 6, rows: 0 };
    let l = board::layout(&doc, &s);
    let c = l.cards[K];
    let m = l.move_of(K).unwrap();
    let hauteur_rangee = c.thumb.height() + board::NOTE_GAP + board::NOTE_LINE_H;
    assert!(m.cadres[1].reduit, "limite de taille");
    assert!((m.cadres[1].rect.height() - hauteur_rangee).abs() < 1e-2, "B est limité à la case et sa bande de note : {} pour {hauteur_rangee}", m.cadres[1].rect.height());
    assert!(m.cadres[1].rect.contains_rect(c.thumb), "B entoure la case");
    assert!(c.foot.height() <= hauteur_rangee + 1e-2);
    assert_eq!(c.thumb.size(), vec2(170.0, 170.0 * 90.0 / 160.0));
    // Sans la limite (un zoom arrière doux) le cadre n'est pas touché.
    let mut doc2 = plan_court();
    doc2.panels[1].camera.clear();
    crate::camera::set_key(&mut doc2.panels[1].camera, 0, a);
    crate::camera::set_key(&mut doc2.panels[1].camera, n - 1, Camera { zoom: a.zoom / 1.05, ..a });
    let l2 = board::layout(&doc2, &s);
    assert!(!l2.move_of(K).unwrap().cadres[1].reduit);
}

#[test]
fn a_zoom_in_is_drawn_inside_the_case() {
    let mut doc = plan_court();
    let a = doc.default_camera();
    doc.panels[1].camera.clear();
    crate::camera::set_key(&mut doc.panels[1].camera, 0, a);
    let n = doc.panels[1].frame_count();
    crate::camera::set_key(&mut doc.panels[1].camera, n - 1, Camera { zoom: a.zoom * 2.0, ..a });
    let s = board::BoardSettings { card_w: 170.0, columns: 6, rows: 0 };
    let l = board::layout(&doc, &s);
    let m = l.move_of(K).unwrap();
    assert!(l.cards[K].thumb.contains_rect(m.cadres[1].rect) && !m.cadres[1].reduit);
    assert_eq!(l.cards[K].foot, l.cards[K].thumb, "la case ne grandit pas");
}

#[test]
fn a_move_down_sits_between_the_rows_and_the_cases_after_go_around_it_while_a_stays_put() {
    let s = board::BoardSettings { card_w: 170.0, columns: 4, rows: 0 };
    let ordinaire = board::layout(&plan_court(), &s);
    let (th, nl) = (ordinaire.cards[K].thumb.height(), ordinaire.cards.len());
    // B une hauteur de cadre plus bas : sous A, à cheval sur la rangée suivante.
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, 0.0, 90.0);
    let l = board::layout(&doc, &s);
    let (c, m) = (l.cards[K], l.move_of(K).unwrap());
    assert_eq!(c.thumb, ordinaire.cards[K].thumb, "la case A ne bouge pas");
    assert!((m.cadres[1].rect.left() - c.thumb.left()).abs() < 1e-3, "B est sous A");
    assert!((m.cadres[1].rect.top() - (c.thumb.top() + th)).abs() < 1e-2, "B est exactement une hauteur de cadre plus bas : {:?}", m.cadres[1].rect);
    assert_eq!(l.cards.len(), nl, "aucune vignette perdue");
    sans_chevauchement(&l);
    // Un demi-cadre plus bas : B à cheval sur deux rangées, à mi-hauteur de A, et l'empreinte finit à mi-rangée (une case de plus ne
    // se range pas dessous si elle ne tient pas à côté).
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, 0.0, 45.0);
    let l = board::layout(&doc, &s);
    let (c, m) = (l.cards[K], l.move_of(K).unwrap());
    assert_eq!(c.thumb, ordinaire.cards[K].thumb);
    assert!((m.cadres[1].rect.top() - (c.thumb.top() + th * 0.5)).abs() < 1e-2, "à mi-hauteur, sans arrondi à la rangée : {:?}", m.cadres[1].rect);
    assert!((c.foot.bottom() - (c.thumb.top() + th * 1.5)).abs() < 1e-2, "elle finit à mi-rangée");
    assert!(c.foot.bottom() < l.cards.iter().skip(K + 1).map(|n| n.thumb.bottom()).fold(f32::MAX, f32::min) + th, "c'est la planche qui s'écarte, pas le cadre");
    sans_chevauchement(&l);
    // Les cases d'après se rangent dans l'ordre de lecture, dans les places libres.
    let ordre: Vec<(usize, i32)> = l.cards.iter().map(|c| (c.row, (c.thumb.left() * 10.0) as i32)).collect();
    assert!(ordre.windows(2).all(|w| w[0] <= w[1] || w[1].0 > w[0].0), "ordre de lecture : {ordre:?}");
    // Une diagonale : B en bas à droite de A, à cheval aussi.
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, 160.0, 55.0);
    let l = board::layout(&doc, &s);
    assert_eq!(l.cards[K].thumb, ordinaire.cards[K].thumb);
    assert_eq!(l.cards.len(), nl);
    sans_chevauchement(&l);
    // Un cadre qui descend mi-rangée, les notes de trois lignes dans les rangées voisines : toujours aucun recouvrement.
    let lignes: std::collections::HashMap<u64, usize> = doc.panels.iter().map(|p| (p.id, 3)).collect();
    let l = board::layout_with(&doc, &s, &lignes, false, &board::Options::default());
    sans_chevauchement(&l);
    assert_eq!(l.cards.len(), nl);
}

/// Les numéros de plan se lisent dans l'ordre : de gauche à droite, rangée par rangée.
fn ordre_de_lecture(l: &board::BoardLayout) {
    for k in 1..l.cards.len() {
        let (a, b) = (l.cards[k - 1], l.cards[k]);
        assert!(b.row > a.row || (b.row == a.row && b.thumb.left() > a.thumb.left()), "l'ordre de lecture est cassé entre {} et {k} : rangées {} {}, x {} {}", k - 1, a.row, b.row, a.thumb.left(), b.thumb.left());
    }
}

#[test]
fn a_stays_put_when_the_hull_grows_right_or_down_and_moves_once_after_the_previous_case_when_it_needs_room_up_or_left() {
    let s = board::BoardSettings { card_w: 170.0, columns: 4, rows: 0 };
    let ordinaire = board::layout(&plan_court(), &s);
    // Vers la droite, vers le bas, en diagonale : rien ne gêne, A reste où elle est.
    for (dx, dy) in [(160.0, 0.0), (0.0, 55.0), (0.0, 90.0), (500.0, 0.0), (160.0, 55.0), (17.3, 23.9)] {
        let mut doc = plan_court();
        deux_cadres(&mut doc, 1, dx, dy);
        let l = board::layout(&doc, &s);
        assert_eq!(l.cards[K].thumb, ordinaire.cards[K].thumb, "({dx}, {dy}) : A ne bouge pas");
        ordre_de_lecture(&l);
        sans_chevauchement(&l);
        assert_eq!(l.cards.len(), ordinaire.cards.len());
    }
    // Vers le haut ou la gauche, sur des cases d'avant : l'enveloppe ne recouvre jamais une case d'avant, c'est la case A qui passe après la
    // précédente (une fois, au lâcher), sur la même rangée si elle y tient, sinon sur la suivante. L'ordre de lecture tient toujours.
    let mut a_bouge = 0;
    for (dx, dy) in [(-160.0, 0.0), (-120.0, -60.0), (-400.0, 30.0), (-17.3, -23.9), (0.0, -90.0), (-40.0, -200.0)] {
        let mut doc = plan_court();
        deux_cadres(&mut doc, 1, dx, dy);
        let l = board::layout(&doc, &s);
        ordre_de_lecture(&l);
        sans_chevauchement(&l);
        assert_eq!(l.cards.len(), ordinaire.cards.len(), "aucune vignette perdue");
        let (avant, apres) = (ordinaire.cards[K].thumb, l.cards[K].thumb.translate(-l.decalage));
        let plus_tard = l.cards[K].row > ordinaire.cards[K].row || (l.cards[K].row == ordinaire.cards[K].row && apres.left() >= avant.left() - 1e-3);
        assert!(plus_tard, "({dx}, {dy}) : A ne recule jamais avant sa place d'origine : {avant:?} → {apres:?}");
        if apres != avant {
            a_bouge += 1;
        }
        // Les cases d'avant n'ont pas bougé.
        for j in 0..K {
            assert_eq!(l.cards[j].thumb.translate(-l.decalage), ordinaire.cards[j].thumb, "la case {j}, d'avant, reste");
        }
    }
    assert!(a_bouge >= 3, "des enveloppes qui recouvrent des cases d'avant repoussent A : {a_bouge}");
}

#[test]
fn during_a_gesture_the_hold_keeps_a_exactly_where_it_was_even_over_earlier_cases() {
    let s = board::BoardSettings { card_w: 170.0, columns: 4, rows: 0 };
    let ordinaire = board::layout(&plan_court(), &s);
    let pose = board::Reserve::depuis(&ordinaire, K).unwrap();
    for (dx, dy) in [(-160.0, 0.0), (-120.0, -60.0), (0.0, -90.0), (-400.0, 30.0), (200.0, 40.0)] {
        let mut doc = plan_court();
        deux_cadres(&mut doc, 1, dx, dy);
        let l = board::layout_with(&doc, &s, &Default::default(), false, &board::Options { hold: Some(pose), ..Default::default() });
        assert_eq!(l.cards[K].thumb.translate(-l.decalage), ordinaire.cards[K].thumb, "({dx}, {dy}) : A reste pendant le geste");
        assert_eq!(l.cards.len(), ordinaire.cards.len());
        assert!(l.pages.iter().any(|p| p.contains_rect(l.cards[K].foot)), "l'empreinte tient dans la page ({dx}, {dy})");
    }
}
#[test]
fn the_other_cases_reflow_around_a_wide_footprint_and_come_back_when_the_move_goes() {
    let s = board::BoardSettings { card_w: 170.0, columns: 4, rows: 0 };
    let normal = board::layout(&plan_court(), &s);
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, 160.0, 0.0);
    let l = board::layout(&doc, &s);
    let c = l.cards[K];
    // La case A reste à sa place, dans la dernière colonne ; l'empreinte déborde de la rangée sur le papier infini.
    assert_eq!(c.thumb, normal.cards[K].thumb, "A ne bouge pas");
    assert!(c.foot.width() > 2.0 * 170.0 - 1.0 && c.foot.right() > normal.cards[K].thumb.right() + 100.0);
    assert!(l.cards[K + 1].row >= 1, "les voisines suivent, dans l'ordre de lecture, sous la rangée");
    assert_eq!(l.cards.len(), normal.cards.len(), "aucune vignette perdue");
    sans_chevauchement(&l);
    // Les clés supprimées (au montage) : la planche d'avant, à l'identique.
    doc.panels[1].camera.clear();
    let l = board::layout(&doc, &s);
    assert_eq!(l.cards.iter().map(|c| c.thumb).collect::<Vec<_>>(), normal.cards.iter().map(|c| c.thumb).collect::<Vec<_>>());
    // Plus large que la rangée : bornée par les colonnes.
    let petit = board::BoardSettings { columns: 2, ..s };
    deux_cadres(&mut doc, 1, 1000.0, 0.0);
    let l = board::layout(&doc, &petit);
    assert!(l.cards[K].foot.width() <= 2.0 * 170.0 + board::gap_cut(170.0) + 1.0);
    sans_chevauchement(&l);
    // Une empreinte au milieu de la planche, qui recouvrirait des cases d'avant : A passe après la précédente, l'ordre de lecture tient.
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, -200.0, -60.0);
    let milieu = board::BoardSettings { columns: 3, ..s };
    let l = board::layout(&doc, &milieu);
    ordre_de_lecture(&l);
    sans_chevauchement(&l);
}

#[test]
fn the_footprints_hold_their_width_during_a_gesture_so_the_other_cases_do_not_jitter() {
    let s = board::BoardSettings { card_w: 170.0, columns: 6, rows: 0 };
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, 200.0, 0.0);
    let large = board::layout(&doc, &s);
    let e = large.cards[K];
    let reserve = board::Reserve::depuis(&large, K).unwrap();
    // Le cadre revient en deçà : sans réserve les suivantes remontent, avec elle elles restent où elles étaient.
    deux_cadres(&mut doc, 1, 40.0, 0.0);
    let libre = board::layout(&doc, &s);
    let retenu = board::layout_with(&doc, &s, &Default::default(), false, &board::Options { hold: Some(reserve), ..Default::default() });
    assert!(libre.cards[K + 1].thumb.left() < large.cards[K + 1].thumb.left() - 50.0, "sans réserve, elles reviennent");
    assert_eq!(retenu.cards[K + 1].thumb, large.cards[K + 1].thumb, "avec la réserve, les autres cases ne bougent pas");
    assert_eq!(retenu.cards[K].reach, e.reach);
    // Elle ne rétrécit pas, elle grandit : une réserve plus petite que ce que le cadre couvre ne retient rien.
    assert_eq!(retenu.cards[K].thumb, large.cards[K].thumb, "et A ne bouge jamais");
    // Un tout petit changement du cadre ne déplace jamais la case A.
    deux_cadres(&mut doc, 1, 20.0, 0.0);
    let avant = board::layout(&doc, &s).cards[K].thumb;
    deux_cadres(&mut doc, 1, 21.0, 0.0);
    assert_eq!(board::layout(&doc, &s).cards[K].thumb, avant);
}

pub(crate) fn deux_cadres(doc: &mut Document, plan: usize, dx: f32, dy: f32) {
    let a = doc.default_camera();
    doc.panels[plan].camera.clear();
    crate::camera::set_key(&mut doc.panels[plan].camera, 0, a);
    let n = doc.panels[plan].frame_count();
    crate::camera::set_key(&mut doc.panels[plan].camera, n - 1, a.shifted(dx, dy));
}

fn sans_chevauchement(l: &board::BoardLayout) {
    use crate::vignettecam::{coins_de, se_recouvrent};
    for (i, a) in l.cards.iter().enumerate() {
        let za = l.zones_of(i);
        for j in (i + 1)..l.cards.len() {
            for p in &za {
                for q in l.zones_of(j) {
                    assert!(!se_recouvrent(p, &q, 0.5), "les images {i} et {j} se chevauchent : {p:?} {q:?}");
                }
            }
        }
        // Les bandes de note ne recouvrent pas non plus l'image ou l'enveloppe d'une autre case.
        for j in (0..l.cards.len()).filter(|&j| j != i) {
            for q in l.zones_of(j) {
                assert!(!se_recouvrent(&coins_de(a.note).to_vec(), &q, 0.5), "la note de {i} recouvre l'image de {j}");
            }
        }
        for e in &l.empty {
            for p in &za {
                assert!(!se_recouvrent(p, &coins_de(*e).to_vec(), 0.5), "l'image {i} recouvre une case pâle");
            }
        }
        assert!(l.pages.iter().any(|p| p.contains_rect(a.card)), "l'image {i} sort de la page");
    }
    for (i, e) in l.empty.iter().enumerate() {
        for f in l.empty.iter().skip(i + 1) {
            assert!(!e.shrink(0.5).intersects(*f), "deux cases pâles se chevauchent");
        }
    }
}
// ------------------------------------------------------------------------------ le geste : déplacer, zoomer, ajouter, retirer

#[test]
fn dragging_inside_b_after_a_move_moves_b_and_keeps_the_frame_count() {
    let mut b = avec_b_a_droite();
    let avant = clefs(&b);
    let nb = pas_d_annulation(&b);
    let w = largeur_cadre_ecran(&b);
    // Dedans B (loin de ses bords) : un Ctrl + glisser le déplace, il ne naît rien.
    let c = centre(cadre_ecran(&b, 1));
    ctrl_glisser(&mut b, c, &pas(c, vec2(0.0, 0.1 * w), 6), c + vec2(0.0, 0.1 * w), true);
    let apres = clefs(&b);
    assert_eq!(apres.len(), 2, "pas de cadre C : {apres:?}");
    assert_eq!(apres[0], avant[0], "A ne bouge pas");
    assert!(apres[1].2 > avant[1].2 + 10.0 && (apres[1].1 - avant[1].1).abs() < 1.0, "B descend : {avant:?} {apres:?}");
    assert_eq!(pas_d_annulation(&b), nb + 1);
    assert_eq!(dernier_nom(&b), "Cadre B");
    // Et encore une fois, sans rien créer.
    let c = centre(cadre_ecran(&b, 1));
    ctrl_glisser(&mut b, c, &pas(c, vec2(-0.25 * w, 0.0), 6), c + vec2(-0.25 * w, 0.0), true);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert!(clefs(&b)[1].1 < apres[1].1 - 20.0);
}

#[test]
fn dragging_inside_a_outside_b_moves_a_and_the_smallest_frame_wins_when_b_is_inside_a() {
    let mut b = avec_b_a_droite();
    let w = largeur_cadre_ecran(&b);
    let avant = clefs(&b);
    // A est la case de gauche : un point dedans A (loin de B) déplace A.
    let qa = cadre_ecran(&b, 0);
    let p = pos2(qa[0].x + (qa[2].x - qa[0].x) * 0.3, centre(qa).y);
    ctrl_glisser(&mut b, p, &pas(p, vec2(0.0, 0.3 * w), 6), p + vec2(0.0, 0.3 * w), true);
    let apres = clefs(&b);
    assert_eq!(apres.len(), 2);
    assert!(apres[0].2 > avant[0].2 + 10.0, "A descend : {avant:?} {apres:?}");
    assert_eq!(apres[1].1, avant[1].1, "B ne bouge pas");
    assert_eq!(dernier_nom(&b), "Cadre A");

    // B dans A (un zoom avant) : dedans B on déplace B, dans A hors de B on déplace A.
    let mut b = ouvert(plan_court(), 4);
    let (case, w) = (vignette(&b, K), largeur_cadre_ecran(&b));
    let coin = pos2(case.right() - 3.0, case.bottom() - 3.0);
    let dedans = vec2(-0.45 * w, -0.25 * w);
    ctrl_glisser(&mut b, coin, &pas(coin, dedans, 6), coin + dedans, true);
    let k = clefs(&b);
    assert!(k[1].3 > 1.0, "B est un zoom avant : {k:?}");
    let cb = centre(cadre_ecran(&b, 1));
    ctrl_glisser(&mut b, cb, &pas(cb, vec2(10.0, 0.0), 4), cb + vec2(10.0, 0.0), true);
    assert_eq!(dernier_nom(&b), "Cadre B", "dedans B : B");
    let qa = cadre_ecran(&b, 0);
    let hors_b = pos2(qa[0].x + 6.0, qa[0].y + 6.0 + (qa[3].y - qa[0].y) * 0.15);
    ctrl_glisser(&mut b, hors_b, &pas(hors_b, vec2(0.0, 6.0), 3), hors_b + vec2(0.0, 6.0), true);
    assert_eq!(dernier_nom(&b), "Cadre A", "dans A hors de B : A");
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
}

#[test]
fn a_corner_of_any_frame_zooms_that_key_and_never_creates_one() {
    let mut b = avec_b_a_droite();
    let avant = clefs(&b);
    let nb = pas_d_annulation(&b);
    let q = cadre_ecran(&b, 1);
    let c = centre(q);
    let dehors = (q[1] - c).normalized() * 30.0;
    ctrl_glisser(&mut b, q[1], &pas(q[1], dehors, 5), q[1] + dehors, true);
    let zoome = clefs(&b);
    assert_eq!(zoome.len(), 2);
    assert!(zoome[1].3 < avant[1].3, "le zoom de B baisse : {avant:?} {zoome:?}");
    assert!((zoome[1].1 - avant[1].1).abs() < 0.5 && (zoome[1].2 - avant[1].2).abs() < 0.5);
    assert_eq!(zoome[0], avant[0]);
    assert_eq!(dernier_nom(&b), "Zoom du cadre B");
    assert_eq!(pas_d_annulation(&b), nb + 1);
    // Un coin de A, lui aussi.
    let qa = cadre_ecran(&b, 0);
    let ca = centre(qa);
    let dehors = (qa[0] - ca).normalized() * 25.0;
    ctrl_glisser(&mut b, qa[0], &pas(qa[0], dehors, 5), qa[0] + dehors, true);
    assert_eq!(dernier_nom(&b), "Zoom du cadre A");
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
}

#[test]
fn ctrl_alt_drag_adds_the_next_frame_from_the_last_one() {
    let mut b = avec_b_a_droite();
    let w = largeur_cadre_ecran(&b);
    let n = b.app.doc.panels[1].frame_count();
    let m = Modifiers { ctrl: true, shift: true, alt: true, command: true, ..Modifiers::NONE };
    let c = centre(cadre_ecran(&b, 1));
    glisser_avec(&mut b, m, c, &pas(c, vec2(w, 0.0), 6), c + vec2(w, 0.0), true);
    let k = clefs(&b);
    assert_eq!(k.len(), 3, "C est née : {k:?}");
    assert_eq!(k.iter().map(|c| c.0).collect::<Vec<_>>(), vec![0, (n - 1) / 2, n - 1], "réparties sur le plan");
    assert!((k[2].1 - (k[1].1 + 160.0)).abs() < 2.0, "C est une largeur de cadre après B : {k:?}");
    assert_eq!(dernier_nom(&b), "Cadre C");
    // Avec Alt, même un appui dans A (loin du dernier cadre) fait un cadre de plus, il ne déplace pas A.
    let qa = cadre_ecran(&b, 0);
    let p = pos2(qa[0].x + (qa[2].x - qa[0].x) * 0.3, centre(qa).y);
    glisser_avec(&mut b, m, p, &pas(p, vec2(0.0, 0.3 * w), 4), p + vec2(0.0, 0.3 * w), true);
    assert_eq!(b.app.doc.panels[1].camera.len(), 4);
    assert_eq!(dernier_nom(&b), "Cadre D");
    assert_eq!(clefs(&b)[0].2, k[0].2, "A n'a pas bougé");
}

#[test]
fn the_right_click_menu_adds_a_frame_and_removes_the_one_under_the_pointer() {
    let mut b = ouvert(plan_court(), 6);
    // « Ajouter un cadre » sur une case sans mouvement : un B à côté, tiré ensuite où l'on veut.
    let nb = pas_d_annulation(&b);
    b.app.cam_ajouter_cadre(1);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert!(clefs(&b)[1].1 > clefs(&b)[0].1, "B à droite de A");
    assert_eq!(pas_d_annulation(&b), nb + 1);
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
    b.app.cam_ajouter_cadre(1);
    assert_eq!(b.app.doc.panels[1].camera.len(), 3);
    assert_eq!(dernier_nom(&b), "Cadre C");
    // « Retirer ce cadre » : le clic droit sur un cadre le vise.
    let layout = b.app.board_layout();
    let (quads, simple) = b.app.cadres_sur_la_planche(&layout, K);
    assert!(!simple);
    let sous = crate::vignettecam::cadre_sous(&quads, centre_quad(&quads[2]));
    assert_eq!(sous, Some(2), "le dernier cadre est sous le pointeur");
    b.app.cam_retirer_cadre(1, 2);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert_eq!(dernier_nom(&b), "Retirer le cadre C");
    // Retirer B d'un mouvement à deux cadres enlève le mouvement.
    b.app.cam_retirer_cadre(1, 1);
    assert!(b.app.doc.panels[1].camera.len() < 2);
    assert!(!b.app.board_layout().cards[K].mouvement, "plus de mouvement : la case est une case");
    // Annuler les rend un par un.
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert!(b.app.board_layout().cards[K].mouvement);
}

fn centre_quad(q: &[Pos2; 4]) -> Pos2 {
    pos2((q[0].x + q[2].x) * 0.5, (q[0].y + q[2].y) * 0.5)
}

#[test]
fn the_right_click_menu_lists_the_camera_entries_on_the_frames_that_can_carry_them() {
    let mut b = ouvert(plan_court(), 6);
    b.garder_textes = true;
    let case = vignette(&b, K);
    // Un clic droit sur une case : la cible retient le cadre visé (aucun sur une case sans mouvement).
    b.image(vec![Event::PointerMoved(case.center())], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: case.center(), button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    assert!(matches!(b.app.vignettes.menu, Some(crate::boardui::MenuTarget::Card { cadre: None, .. })), "{:?}", b.app.vignettes.menu);
    b.image(vec![Event::PointerButton { pos: case.center(), button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.textes.iter().any(|t| t == "Ajouter un cadre"), "{:?}", b.textes);
    assert!(!b.textes.iter().any(|t| t == "Retirer ce cadre"), "pas de cadre sous le pointeur");
}

#[test]
fn a_ctrl_shift_click_that_does_not_move_does_nothing_and_escape_abandons_a_drag() {
    let mut b = ouvert(plan_court(), 6);
    let case = vignette(&b, K);
    let id = b.app.doc.panels[1].id;
    // Ctrl + Maj + clic qui ne bouge pas : ni plan choisi, ni plan déplacé, ni clé posée.
    b.touches = CTRL;
    b.image(vec![Event::PointerMoved(case.center())], 1.0 / 60.0);
    b.image(vec![appui(case.center(), true, CTRL)], 1.0 / 60.0);
    b.image(vec![appui(case.center(), false, CTRL)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!b.app.board_sel.contains(&id), "Ctrl + Maj + clic ne choisit pas le plan");
    assert!(b.app.vignettes.moving.is_none());
    assert!(b.app.doc.panels[1].camera.is_empty());
    // Échap en plein geste : les clés d'avant reviennent, aucune étape.
    let nb = pas_d_annulation(&b);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), false);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    b.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: CTRL }], 1.0 / 60.0);
    assert!(b.app.doc.panels[1].camera.is_empty());
    b.image(vec![appui(case.center(), false, CTRL)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.doc.panels[1].camera.is_empty());
    assert_eq!(pas_d_annulation(&b), nb, "rien à annuler");
    assert!(!b.app.board_layout().cards[K].mouvement);
}

#[test]
fn a_locked_storyboard_refuses_the_camera_drag() {
    let mut doc = plan_court();
    doc.storyboard_locked = true;
    let mut b = ouvert(doc, 4);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    assert!(b.app.doc.panels[1].camera.is_empty());
    assert!(b.app.status.contains("verrouillé"), "{}", b.app.status);
}

#[test]
fn a_one_frame_plan_cannot_carry_a_movement() {
    let mut doc = quatre_plans();
    doc.set_duration(1, 1);
    let mut b = ouvert(doc, 4);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    assert!(b.app.doc.panels[1].camera.is_empty());
}

/// Le geste comme la main le fait : Ctrl est tenu un bon moment AVANT l'appui (le pointeur survole déjà la case courante),
/// puis appui, glissé, lâcher de la souris, et seulement ensuite Ctrl est relâché. Chaque événement porte les touches tenues,
/// comme ceux d'egui-winit.
fn ctrl_tenu_puis_glisser(b: &mut Banc, survol: Pos2, de: Pos2, delta: egui::Vec2) {
    b.touches = CTRL;
    b.image(vec![Event::PointerMoved(survol)], 1.0 / 60.0);
    for _ in 0..30 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![appui(de, true, CTRL)], 1.0 / 60.0);
    for q in pas(de, delta, 10) {
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerMoved(de + delta)], 1.0 / 60.0);
    b.image(vec![appui(de + delta, false, CTRL)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    b.chauffer(2);
}

#[test]
fn holding_ctrl_before_the_press_draws_the_move_whatever_the_tool_and_never_takes_the_lasso() {
    for tool in [Tool::Brush, Tool::Pencil, Tool::Eraser, Tool::Select, Tool::Line] {
        let mut b = ouvert(plan_court(), 6);
        b.app.set_tool(tool);
        let w = largeur_cadre_ecran(&b);
        let (courante, case) = (vignette(&b, 0), vignette(&b, K));
        // (Ctrl tenu sur la case courante : c'est là que l'outil Sélection se prenait.)
        b.touches = CTRL;
        b.image(vec![Event::PointerMoved(courante.center())], 1.0 / 60.0);
        for _ in 0..30 {
            b.image(Vec::new(), 1.0 / 60.0);
        }
        assert_eq!(b.app.tool, tool, "{tool:?} : Ctrl tenu sur la case ne change pas d'outil dans les Vignettes");
        b.touches = Modifiers::NONE;
        b.image(Vec::new(), 1.0 / 60.0);
        ctrl_tenu_puis_glisser(&mut b, courante.center(), case.center(), vec2(w, 0.0));
        assert_eq!(b.app.doc.panels[1].camera.len(), 2, "{tool:?} : le mouvement est posé : {}", b.app.status);
        assert_eq!(b.app.tool, tool, "{tool:?} : l'outil est resté");
        assert!(b.app.board_layout().cards[K].mouvement, "{tool:?} : la case a son empreinte");
        assert_eq!(dernier_nom(&b), "Mouvement de caméra");
        assert!(!encre(&b.app.doc, 1), "{tool:?} : aucun trait n'a été dessiné");
    }
}

/// Un Ctrl + appui qui rate tout cadre, puis un glissé : un geste de caméra, jamais un plan qui bouge ni un lasso. (La cause du
/// « souvent ça déplace le plan » : l'appui tombait hors de la zone des cadres et retombait sur le choix de plans.)
#[test]
fn a_ctrl_press_that_misses_every_frame_is_still_a_camera_drag_and_never_moves_a_plan() {
    let ordre = |b: &Banc| b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>();
    // 1. Sur une case ordinaire, juste hors de l'image (la marge qui garde le trait) : B naît quand même.
    let mut b = ouvert(plan_court(), 6);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    let ids = ordre(&b);
    let marge = pos2(case.right() + 2.0, case.center().y);
    ctrl_tenu_puis_glisser(&mut b, case.center(), marge, vec2(0.6 * w, 0.0));
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "un appui de caméra sur la marge : {}", b.app.status);
    assert_eq!(ordre(&b), ids, "aucun plan n'a bougé");
    assert!(b.app.vignettes.moving.is_none() && b.app.board_drag.is_none());

    // 2. Sur une case qui a une diagonale, dans le creux de son empreinte (hors des deux cadres) : un cadre bouge, rien d'autre.
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, 160.0, 90.0);
    let mut b = ouvert(doc, 6);
    let l = b.app.board_layout();
    let c = l.cards[K];
    let (quads, _) = b.app.cadres_sur_la_planche(&l, K);
    let a_droite_de_a = pos2(c.thumb.right() + 0.3 * c.thumb.width(), c.thumb.center().y);
    assert!(quads.iter().all(|q| !(q[0].x <= a_droite_de_a.x && a_droite_de_a.x <= q[2].x && q[0].y <= a_droite_de_a.y && a_droite_de_a.y <= q[2].y)), "le point est hors des cadres");
    assert!(c.foot.contains(a_droite_de_a), "mais dans l'empreinte");
    let ids = ordre(&b);
    let avant = clefs(&b);
    let p = b.app.vignettes.xf.to_screen(a_droite_de_a);
    let w = largeur_cadre_ecran(&b);
    ctrl_tenu_puis_glisser(&mut b, p, p, vec2(0.0, -0.15 * w));
    assert_ne!(clefs(&b), avant, "une clé a bougé : {}", b.app.status);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "rien ne naît");
    assert_eq!(ordre(&b), ids, "aucun plan n'a bougé");
    assert!(b.app.vignettes.moving.is_none());
}

#[test]
fn the_held_camera_key_turns_any_drag_on_a_case_into_a_camera_gesture_and_gives_the_tool_back() {
    let mut b = ouvert(plan_court(), 6);
    b.app.set_tool(Tool::Brush);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    assert_eq!(b.app.keymap.label("vignettes.camera"), "T");
    // T tenue (pas de Ctrl) : un appui sur la case ne dessine pas, il tire la caméra.
    let t = |pressed: bool| Event::Key { key: Key::T, physical_key: Some(Key::T), pressed, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![Event::PointerMoved(case.center())], 1.0 / 60.0);
    b.image(vec![t(true)], 1.0 / 60.0);
    for _ in 0..5 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    assert!(b.app.vignettes.camera_tenue, "la touche est tenue");
    b.image(vec![appui(case.center(), true, Modifiers::NONE)], 1.0 / 60.0);
    for q in pas(case.center(), vec2(w, 0.0), 8) {
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerMoved(case.center() + vec2(w, 0.0))], 1.0 / 60.0);
    b.image(vec![appui(case.center() + vec2(w, 0.0), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![t(false)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!b.app.vignettes.camera_tenue, "relâchée");
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "le mouvement est posé : {}", b.app.status);
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
    assert!(!encre(&b.app.doc, 1), "aucun trait n'a été dessiné");
    assert_eq!(b.app.tool, Tool::Brush, "l'outil d'avant est là : on n'en a jamais changé");
    // Relâchée, le même glissé redessine.
    let case0 = vignette(&b, 0);
    glisser(&mut b, &vague(case0.center() - vec2(20.0, 0.0), case0.center() + vec2(20.0, 0.0), 8));
    assert!(encre(&b.app.doc, 0), "sans la touche, le pinceau dessine");
}

#[test]
fn the_camera_key_with_alt_adds_a_frame_and_the_key_is_free_in_every_keymap() {
    let mut b = avec_b_a_droite();
    let w = largeur_cadre_ecran(&b);
    let c = centre(cadre_ecran(&b, 1));
    let t = |pressed: bool, m: Modifiers| Event::Key { key: Key::T, physical_key: Some(Key::T), pressed, repeat: false, modifiers: m };
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.image(vec![t(true, Modifiers::NONE)], 1.0 / 60.0);
    b.touches = Modifiers::ALT;
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![appui(c, true, Modifiers::ALT)], 1.0 / 60.0);
    for q in pas(c, vec2(w, 0.0), 6) {
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerMoved(c + vec2(w, 0.0))], 1.0 / 60.0);
    b.image(vec![appui(c + vec2(w, 0.0), false, Modifiers::ALT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(vec![t(false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.doc.panels[1].camera.len(), 3, "T + Alt : un cadre de plus : {}", b.app.status);
    // T n'est prise par aucune autre action, dans les deux modes, ni dans le jeu « Façon Storyboard Pro ».
    for mode in [Mode::Storyboard, Mode::Animation] {
        assert_eq!(b.app.keymap.find(Key::T, Some(Key::T), Modifiers::NONE, mode), Some("vignettes.camera"));
    }
    assert!(b.app.keymap.warnings.iter().all(|w| !w.contains("vignettes.camera")), "{:?}", b.app.keymap.warnings);
    let pro = crate::keymap::Keymap::from_layers(crate::keymap::DEFAULT_KEYMAP, Some((crate::keymap::PresetId::StoryboardPro, crate::keymap::STORYBOARD_PRO_KEYMAP)), None);
    assert_eq!(pro.find(Key::T, Some(Key::T), Modifiers::NONE, Mode::Storyboard), Some("vignettes.camera"));
    assert!(crate::keymap::action_info("vignettes.camera").is_some_and(|a| a.implemented));
    let fois = crate::keymap::DEFAULT_KEYMAP.matches("touches = [\"T\"]").count();
    assert_eq!(fois, 1, "une seule action sur T dans keymap.toml");
}

#[test]
fn the_cursor_shows_a_camera_while_ctrl_or_the_key_is_held_over_a_case() {
    let mut b = avec_b_a_droite();
    let c = centre(cadre_ecran(&b, 1));
    b.touches = CTRL;
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.curseur, egui::CursorIcon::Move, "dedans un cadre : le déplacer");
    let q = cadre_ecran(&b, 1);
    b.image(vec![Event::PointerMoved(q[2])], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(matches!(b.curseur, egui::CursorIcon::ResizeNwSe | egui::CursorIcon::ResizeNeSw), "à un coin : changer sa taille ({:?})", b.curseur);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    assert_ne!(b.curseur, egui::CursorIcon::Move, "sans Ctrl : le curseur du dessin");
}

// ------------------------------------------------------------------------------ dessiner, montage, annulation

fn avec_mouvement(dx: f32, dy: f32) -> Banc {
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, dx, dy);
    ouvert(doc, 4)
}

#[test]
fn drawing_in_the_footprint_reaches_beyond_the_sheet() {
    let mut b = avec_mouvement(160.0, 0.0);
    b.app.set_tool(Tool::Brush);
    b.app.color = [255, 0, 255];
    let l = b.app.board_layout();
    let tuile = b.app.vignettes.xf.rect(l.move_of(K).unwrap().tuile.rect);
    // Le quart droit de l'empreinte est hors de la feuille (le cadre B est à droite d'elle).
    let a = pos2(tuile.left() + tuile.width() * 0.78, tuile.center().y);
    let chemin: Vec<Pos2> = (1..10).map(|k| a + vec2(k as f32 * 3.0, (k as f32 * 0.7).sin() * 4.0)).collect();
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![appui(a, true, Modifiers::NONE)], 1.0 / 60.0);
    for q in &chemin {
        b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0);
    }
    b.image(vec![appui(*chemin.last().unwrap(), false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.cur_panel, 1);
    assert!(encre(&b.app.doc, 1), "le trait est dans le plan");
    let plan = crate::render::panel_plan_at(&b.app.doc, 1, 0, None);
    let sheet_w = b.app.doc.width as i64;
    let vecteur_dehors = b.app.doc.panels[1].layers.iter().flat_map(|l| l.frames.iter().flatten()).any(|id| {
        b.app.doc.cels.get(id).and_then(|c| c.vector.as_ref()).is_some_and(|v| v.elements.iter().any(|e| matches!(e, crate::vector::Element::Stroke { points, .. } if points.iter().any(|p| p.x > sheet_w as f32 + 4.0))))
    });
    assert!(crate::render::plan_bounds(&b.app.doc, &plan).x1 > sheet_w || vecteur_dehors, "le trait déborde de la feuille (toile infinie)");
    // Les cadres sont restés ce qu'ils étaient.
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
}

#[test]
fn the_other_footprints_are_tile_thumbnails_and_the_current_one_is_the_drawing() {
    let mut b = avec_mouvement(160.0, 90.0);
    b.garder_formes = true;
    // Le plan 2 est courant (il est vivant) ; un autre plan courant, il devient une vignette de tuiles.
    b.app.select_panel(0);
    for _ in 0..40 {
        b.image(Vec::new(), 1.0 / 60.0);
        if !b.app.thumbs.busy() {
            break;
        }
    }
    let l = b.app.board_layout();
    let m = l.move_of(K).unwrap();
    let r = b.app.vignettes.xf.rect(m.tuile.rect);
    let images = b.formes.iter().filter(|s| matches!(&s.shape, Shape::Mesh(m) if m.texture_id != egui::TextureId::default() && Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()).intersects(r))).count();
    assert!(images >= 1, "la tuile se peint comme une vignette");
    b.app.select_panel(1);
    b.chauffer(3);
    assert!(b.app.board_layout().cards[K].mouvement);
}

#[test]
fn the_montage_shows_the_keys_and_removing_them_there_shrinks_the_footprint() {
    let mut b = ouvert(plan_court(), 6);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    let n = b.app.doc.panels[1].frame_count();
    let (a, fin, milieu) = (b.app.doc.panel_camera(1, 0), b.app.doc.panel_camera(1, n - 1), b.app.doc.panel_camera(1, (n - 1) / 2));
    assert!(fin.x > a.x + 150.0, "la caméra du film va de A à B");
    assert!((milieu.x - (a.x + fin.x) * 0.5).abs() < 1.5, "à vitesse constante : à mi-plan, à mi-chemin");
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "les clés sont sur la piste Caméra du montage");
    // Supprimer les clés au montage.
    b.app.doc.panels[1].camera.clear();
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(4);
    assert!(!b.app.board_layout().cards[K].mouvement, "l'empreinte a rétréci");
}

#[test]
fn shift_dragging_the_frame_of_a_footprint_still_moves_the_plan() {
    let mut b = avec_mouvement(160.0, 0.0);
    let case = vignette(&b, K);
    let cadre = pos2(case.left() + 2.0, case.center().y);
    b.touches = Modifiers::SHIFT;
    b.image(vec![Event::PointerMoved(cadre)], 1.0 / 60.0);
    b.image(vec![appui(cadre, true, Modifiers::SHIFT)], 1.0 / 60.0);
    assert_eq!(b.app.vignettes.press, Press::Move);
    assert!(b.app.vignettes.moving.is_some());
    assert!(b.app.vignettes.cam.is_none(), "Ctrl seul distingue le cadre de caméra");
    b.image(vec![Event::PointerMoved(cadre + vec2(40.0, 40.0))], 1.0 / 60.0);
    b.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::SHIFT }], 1.0 / 60.0);
    b.image(vec![appui(cadre, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "les clés suivent le plan, intactes");
}

#[test]
fn the_vignettes_rest_after_a_camera_drag() {
    let mut b = ouvert(plan_court(), 6);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT se repose après un mouvement de caméra");
    let repos: Vec<_> = (0..6).map(|_| b.image(Vec::new(), 0.5)).collect();
    let pire = repos.iter().map(|m| m.reveil).min().unwrap();
    assert!(pire >= std::time::Duration::from_secs(1), "au repos, SHOT demande à être repeint dans {pire:?}");
    let _ = Camera::centered(1, 1);
}

// ------------------------------------------------------------------------------ la case A ne bouge pas, l'aimant est doux

#[test]
fn the_case_a_does_not_move_on_screen_during_a_b_drag_nor_after_when_b_goes_right_or_down_and_reading_order_always_holds() {
    // (En largeurs de cadre : vers la droite, vers le bas, en bas à droite : A ne bouge jamais, ni pendant ni après. Vers le haut, en
    // haut à gauche, vers la gauche : A ne bouge pas pendant le geste ; au lâcher, si l'enveloppe recouvre des cases d'avant, elle passe
    // après la précédente, une fois.)
    for (dx, dy, apres_identique) in [(1.0_f32, 0.0_f32, true), (0.0, 0.7, true), (0.4, 0.7, true), (0.0, -0.8, false), (-0.9, -0.5, false), (-1.2, 0.0, false)] {
        let mut b = ouvert(plan_court(), 6);
        let (case, w) = (vignette(&b, K), largeur_cadre_ecran(&b));
        let de = case.center();
        let delta = vec2(dx * w, dy * w);
        ctrl_glisser(&mut b, de, &pas(de, delta, 8), de + delta, false);
        b.image(Vec::new(), 1.0 / 60.0);
        assert_eq!(b.app.doc.panels[1].camera.len(), 2, "({dx}, {dy}) : B est née");
        let pendant = vignette(&b, K);
        assert!((pendant.min - case.min).length() < 0.01 && (pendant.size() - case.size()).length() < 0.01, "({dx}, {dy}) : A a bougé pendant le geste : {case:?} → {pendant:?}");
        // (Pendant le geste, rien ne se range sous la main : les cases d'avant peuvent être recouvertes, A ne bouge pas.)
        b.image(vec![appui(de + delta, false, CTRL)], 1.0 / 60.0);
        b.touches = Modifiers::NONE;
        b.image(Vec::new(), 1.0 / 60.0);
        b.chauffer(2);
        let apres = vignette(&b, K);
        let l = b.app.board_layout();
        ordre_de_lecture(&l);
        sans_chevauchement(&l);
        if apres_identique {
            assert!((apres.min - case.min).length() < 0.01, "({dx}, {dy}) : A a bougé au lâcher alors que B va à droite ou en bas : {case:?} → {apres:?}");
        } else {
            let ordinaire = board::layout(&plan_court(), &board::BoardSettings { card_w: 170.0, columns: 6, rows: 0 });
            let (r0, x0) = (ordinaire.cards[K].row, ordinaire.cards[K].thumb.left());
            let c = l.cards[K];
            assert!(c.row > r0 || (c.row == r0 && c.thumb.left() - l.decalage.x >= x0 - 1e-3), "({dx}, {dy}) : A ne recule jamais : rangée {} x {}", c.row, c.thumb.left());
        }
        // Et quand on retouche B (le déplacer plus haut, à gauche), A reste pendant le geste.
        let avant = vignette(&b, K);
        let c = centre(cadre_ecran(&b, 1));
        let delta = vec2(-0.5 * w, -0.4 * w);
        ctrl_glisser(&mut b, c, &pas(c, delta, 8), c + delta, false);
        b.image(Vec::new(), 1.0 / 60.0);
        let encore = vignette(&b, K);
        assert!((encore.min - avant.min).length() < 0.01, "({dx}, {dy}) : A a bougé pendant qu'on retouche B : {avant:?} → {encore:?}");
        b.image(vec![appui(c + delta, false, CTRL)], 1.0 / 60.0);
        b.touches = Modifiers::NONE;
        b.image(Vec::new(), 1.0 / 60.0);
        ordre_de_lecture(&b.app.board_layout());
    }
}
#[test]
fn dragging_b_near_an_alignment_snaps_softly_within_the_threshold_and_follows_the_pointer_outside_it() {
    let mut b = avec_b_a_droite();
    let zoom = b.app.vignettes.xf.zoom;
    // B est à droite de A, haut contre haut : le bord haut de B est sur celui de A. À 3 points d'écran de là : attiré (pas lâché
    // pour autant) ; à 20 : le pointeur, exactement.
    let echelle = zoom * 170.0 / 160.0; // points d'écran par point de la feuille
    let y0 = clefs(&b)[1].2;
    for (pixels, libre) in [(3.0_f32, false), (20.0, true), (14.0, true)] {
        let c = centre(cadre_ecran(&b, 1));
        ctrl_glisser(&mut b, c, &pas(c, vec2(0.0, pixels), 6), c + vec2(0.0, pixels), true);
        let dy = (clefs(&b)[1].2 - y0) * echelle;
        if libre {
            assert!((dy - pixels).abs() < 0.05, "à {pixels} points : le cadre suit le pointeur ({dy})");
        } else {
            assert!(dy.abs() < pixels - 0.5 && dy >= 0.0, "à {pixels} points : attiré vers l'alignement ({dy})");
        }
        // On remet B à sa place pour le tour suivant.
        let c = centre(cadre_ecran(&b, 1));
        ctrl_glisser(&mut b, c, &pas(c, vec2(0.0, -dy), 6), c + vec2(0.0, -dy), true);
        let reste = (clefs(&b)[1].2 - y0) * echelle;
        assert!(reste.abs() < 4.0, "{reste}");
        // (Retour exact : la clé d'origine.)
        let k = clefs(&b);
        b.app.doc.panels[1].camera[1].cam.y = y0;
        let _ = k;
    }
}
#[test]
fn the_layout_of_200_plans_with_mid_row_footprints_and_tall_notes_stays_fast_and_clear() {
    let mut doc = Document::with_frame(64, 48, 24);
    for _ in 0..199 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    let a = doc.default_camera();
    for (k, i) in (0..200).step_by(5).enumerate() {
        doc.set_duration(i, 25);
        let n = doc.panels[i].frame_count();
        // Des décalages quelconques, vers le bas, le haut, la gauche, la droite : à mi-rangée, à cheval.
        let (dx, dy) = [(37.0, 21.0), (-23.0, -17.0), (11.0, 41.0), (-45.0, 9.0), (64.0, -30.0)][k % 5];
        doc.panels[i].camera.clear();
        crate::camera::set_key(&mut doc.panels[i].camera, 0, a);
        crate::camera::set_key(&mut doc.panels[i].camera, n - 1, a.shifted(dx, dy));
    }
    let lignes: std::collections::HashMap<u64, usize> = doc.panels.iter().enumerate().map(|(i, p)| (p.id, 1 + i % 3)).collect();
    let s = board::BoardSettings { card_w: 170.0, columns: 6, rows: 0 };
    let opt = board::Options::pale(3);
    let l = board::layout_with(&doc, &s, &lignes, false, &opt);
    assert_eq!(l.cards.len(), 200);
    assert!(l.cards.iter().filter(|c| c.mouvement).count() >= 40);
    let mut pire = std::time::Duration::ZERO;
    for _ in 0..20 {
        let t = std::time::Instant::now();
        let l = board::layout_with(&doc, &s, &lignes, false, &opt);
        pire = pire.max(t.elapsed());
        assert_eq!(l.cards.len(), 200);
    }
    eprintln!("mise en page de 200 plans (40 mouvements à mi-rangée, notes de 1 à 3 lignes) : au pire {pire:?}");
    assert!(pire < std::time::Duration::from_millis(5), "{pire:?} par disposition");
    // Aucun recouvrement, tout est dans l'ordre de lecture.
    use crate::vignettecam::{se_recouvrent, coins_de};
    let zones: Vec<Vec<Vec<Pos2>>> = (0..l.cards.len()).map(|k| l.zones_of(k)).collect();
    for i in 0..l.cards.len() {
        for j in (i + 1)..l.cards.len() {
            if l.cards[i].row.abs_diff(l.cards[j].row) > 5 {
                continue;
            }
            for p in &zones[i] {
                for q in &zones[j] {
                    assert!(!se_recouvrent(p, q, 0.5), "les cases {i} et {j} se chevauchent");
                }
            }
        }
        for q in l.empty.iter() {
            for p in &zones[i] {
                assert!(!se_recouvrent(p, &coins_de(*q).to_vec(), 0.5), "la case {i} recouvre une case pâle");
            }
        }
    }
}
#[test]
fn a_hand_held_camera_with_dozens_of_keys_stacks_no_letters_and_the_tile_is_not_veiled() {
    // Des dizaines de clés presque au même endroit (le tremblé d'une caméra à la main) : leurs lettres s'empilaient en un petit carré
    // bleu dans le coin de la case (Anthony, 2026-10-22). Une seule lettre, A, se peint ; et aucun voile blanc sur le dessin.
    let mut doc = plan_court();
    let a = doc.default_camera();
    doc.panels[1].camera.clear();
    doc.set_duration(1, 60);
    let n = doc.panels[1].frame_count();
    for k in 0..30 {
        crate::camera::set_key(&mut doc.panels[1].camera, k * (n - 1) / 29, a.shifted((k as f32 * 0.9).sin() * 0.6, (k as f32 * 1.3).cos() * 0.5));
    }
    let mut b = ouvert(doc, 6);
    b.garder_formes = true;
    b.chauffer(6);
    assert_eq!(b.app.doc.panels[1].camera.len(), 30);
    let mut lettres: Vec<String> = Vec::new();
    for clipped in &b.formes {
        if let Shape::Text(t) = &clipped.shape {
            let texte = t.galley.job.text.clone();
            if texte.len() == 1 && texte.chars().all(|c| c.is_ascii_uppercase()) {
                lettres.push(texte);
            }
        }
    }
    assert_eq!(lettres, vec!["A".to_string()], "une seule lettre : les cadres qui se confondent ne se redessinent pas");
    // Pas de voile blanc translucide sur la tuile (le dessin reste net partout dans l'enveloppe).
    let l = b.app.board_layout();
    let tuile = b.app.vignettes.xf.rect(l.move_of(K).unwrap().tuile.rect);
    let voile = |c: egui::Color32| c.a() > 0 && c.a() < 200 && c.r() == c.g() && c.g() == c.b() && c.r() > 0 && c.r() >= c.a();
    let voiles = b
        .formes
        .iter()
        .filter(|s| match &s.shape {
            Shape::Rect(r) => voile(r.fill) && r.rect.intersects(tuile),
            Shape::Mesh(m) => m.texture_id == egui::TextureId::default() && m.vertices.first().is_some_and(|v| voile(v.color)) && Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()).intersects(tuile),
            _ => false,
        })
        .count();
    assert_eq!(voiles, 0, "aucun voile sur le dessin");
}
// ------------------------------------------------------------------------------ dessiner entre A et B, saisir B près d'un coin de A

/// Tout ce qui est peint dans les dessins du plan `i`, en points de la feuille : les pixels de ses bitmaps et les points de ses traits.
fn encre_en_feuille(doc: &Document, i: usize) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    for id in doc.panels[i].layers.iter().flat_map(|l| l.frames.iter().flatten()) {
        let Some(c) = doc.cels.get(id) else { continue };
        for y in 0..c.raster.height {
            for x in 0..c.raster.width {
                if c.raster.get(x, y).a() != 0 {
                    out.push((x as i64, y as i64));
                }
            }
        }
        if let Some(v) = &c.vector {
            for e in &v.elements {
                if let crate::vector::Element::Stroke { points, .. } = e {
                    out.extend(points.iter().map(|p| (p.x.round() as i64, p.y.round() as i64)));
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn encre_pres(encre: &[(i64, i64)], p: Pos2, rayon: f32) -> bool {
    encre.iter().any(|&(x, y)| (pos2(x as f32 + 0.5, y as f32 + 0.5) - p).length() <= rayon)
}

#[test]
fn a_stroke_between_a_and_b_lands_at_the_right_sheet_position_and_moving_b_afterwards_leaves_it_there() {
    let mut b = ouvert(plan_court(), 6);
    b.app.set_tool(Tool::Brush);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    // B : un cadre vers la droite de 120 points de la feuille et vers le bas de 70 (le cadre fait 160 × 90).
    let delta = vec2(120.0, 70.0) * (w / 160.0);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), delta, 8), case.center() + delta, true);
    let k = clefs(&b);
    assert_eq!(k.len(), 2);
    assert!(((k[1].1 - k[0].1) - 120.0).abs() < 6.0 && ((k[1].2 - k[0].2) - 70.0).abs() < 6.0, "{k:?}");
    let a0 = pos2(k[0].1, k[0].2);
    // Deux points dans la zone entre A et B (ni dans A, ni dans B, mais dans l'enveloppe), en points de la feuille.
    let (s0, s1) = (a0 + vec2(110.0, -10.0), a0 + vec2(140.0, 15.0));
    let l = b.app.board_layout();
    let m = l.move_of(K).unwrap();
    for s in [s0, s1, s0.lerp(s1, 0.5)] {
        let (dans_a, dans_b) = ((s.x - a0.x).abs() <= 80.0 && (s.y - a0.y).abs() <= 45.0, (s.x - k[1].1).abs() <= 80.0 && (s.y - k[1].2).abs() <= 45.0);
        assert!(!dans_a && !dans_b, "{s:?} est entre A et B");
    }
    // La feuille → l'écran : la tuile montre la feuille à l'échelle des cases, centrée sur sa zone.
    let ecran = |s: Pos2| b.app.vignettes.xf.to_screen(m.tuile.rect.center() + (s - m.tuile.zone.centre) * m.tuile.zone.echelle);
    assert!(m.contient(b.app.vignettes.xf.to_board(ecran(s0))) && m.contient(b.app.vignettes.xf.to_board(ecran(s1))), "les deux points sont dans l'enveloppe");
    let chemin: Vec<Pos2> = (0..=10).map(|j| ecran(s0.lerp(s1, j as f32 / 10.0))).collect();
    assert!(!encre(&b.app.doc, 1));
    glisser(&mut b, &chemin);
    assert_eq!(b.app.cur_panel, 1);
    assert!(encre(&b.app.doc, 1), "le trait est dans le dessin du plan");
    let trait_ = encre_en_feuille(&b.app.doc, 1);
    for (nom, s) in [("début", s0), ("milieu", s0.lerp(s1, 0.5)), ("fin", s1)] {
        assert!(encre_pres(&trait_, s, 4.0), "le {nom} du trait est à sa place sur la feuille ({s:?})");
    }
    // Et rien de plus loin que le trait : la boîte de l'encre est celle du trait (au rayon du pinceau près).
    let (x0, x1) = (trait_.iter().map(|p| p.0).min().unwrap() as f32, trait_.iter().map(|p| p.0).max().unwrap() as f32);
    let (y0, y1) = (trait_.iter().map(|p| p.1).min().unwrap() as f32, trait_.iter().map(|p| p.1).max().unwrap() as f32);
    assert!(x0 > s0.x - 8.0 && x1 < s1.x + 8.0 && y0 > s0.y - 8.0 && y1 < s1.y + 8.0, "({x0}, {y0})-({x1}, {y1}) pour {s0:?}-{s1:?}");
    // Déplacer B ensuite : le dessin ne bouge pas, seule la fenêtre.
    let avant = clefs(&b);
    let c = centre(cadre_ecran(&b, 1)) + vec2(0.2 * w, 0.1 * w);
    let d = vec2(-0.3 * w, 0.15 * w);
    ctrl_glisser(&mut b, c, &pas(c, d, 8), c + d, true);
    let apres = clefs(&b);
    assert_ne!(apres[1], avant[1], "B a bougé : {avant:?} {apres:?}");
    assert_eq!(apres[0], avant[0]);
    assert_eq!(encre_en_feuille(&b.app.doc, 1), trait_, "le dessin est resté où il était sur la feuille");
    assert_eq!(dernier_nom(&b), "Cadre B");
}

#[test]
fn grabbing_near_a_corner_of_a_from_inside_b_moves_b() {
    let mut b = ouvert(plan_court(), 6);
    let (case, w) = (vignette(&b, K), largeur_cadre_ecran(&b));
    // B décalé d'une demi-largeur et d'une demi-hauteur : le centre de B est le coin bas-droit de A.
    let delta = vec2(0.5 * w, 0.5 * w * 9.0 / 16.0);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), delta, 8), case.center() + delta, true);
    let avant = clefs(&b);
    assert_eq!(avant.len(), 2);
    let coin_a = cadre_ecran(&b, 0)[2];
    let centre_b = centre(cadre_ecran(&b, 1));
    assert!((coin_a - centre_b).length() < 3.0, "le coin bas-droit de A est le centre de B : {coin_a:?} {centre_b:?}");
    let d = vec2(0.1 * w, 0.08 * w);
    ctrl_glisser(&mut b, coin_a, &pas(coin_a, d, 6), coin_a + d, true);
    let apres = clefs(&b);
    assert_eq!(dernier_nom(&b), "Cadre B", "dans B : B se déplace, le coin de A ne prend pas");
    assert_eq!(apres[0], avant[0], "A ne bouge pas");
    assert!(apres[1].1 > avant[1].1 + 10.0 && (apres[1].3 - avant[1].3).abs() < 1e-6, "B glisse sans changer de zoom : {avant:?} {apres:?}");
}