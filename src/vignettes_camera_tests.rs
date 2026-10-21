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
use crate::vignettes_tests::{appui, dernier_nom, encre, ouvert, pas_d_annulation, quatre_plans, vignette};
use crate::workspace::{Espace, Layout};

const CTRL: Modifiers = Modifiers::CTRL;

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
    assert!(b.textes.iter().any(|t| t.contains("Aucun personnage")), "la barre est peinte au-dessus de la planche : {:?}", b.textes);
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
    let s: crate::settings::Settings = toml::from_str("espace_storyboard = \"vignettes\"\n").unwrap();
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

fn plan_court() -> Document {
    let mut doc = quatre_plans();
    doc.set_duration(1, 25);
    doc
}

fn ctrl_glisser(b: &mut Banc, de: Pos2, chemin: &[Pos2], a: Pos2, lacher: bool) {
    b.touches = CTRL;
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![appui(de, true, CTRL)], 1.0 / 60.0);
    for q in chemin {
        b.image(vec![Event::PointerMoved(*q)], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    if lacher {
        b.image(vec![appui(a, false, CTRL)], 1.0 / 60.0);
        b.touches = Modifiers::NONE;
        b.image(Vec::new(), 1.0 / 60.0);
    }
}

/// Les points d'un glissé, un à un (le geste ne saute pas d'un coup).
fn pas(de: Pos2, delta: egui::Vec2, n: usize) -> Vec<Pos2> {
    (1..n).map(|k| de + delta * (k as f32 / n as f32)).collect()
}

const K: usize = 3; // la première image du plan 2 (le plan 1 a deux marques)

fn clefs(b: &Banc) -> Vec<(usize, f32, f32, f32)> {
    b.app.doc.panels[1].camera.iter().map(|k| (k.frame, k.cam.x, k.cam.y, k.cam.zoom)).collect()
}

/// Le pas de l'écran qui vaut une largeur de cadre sur la feuille.
fn largeur_cadre_ecran(b: &Banc) -> f32 {
    b.app.vignettes.xf.zoom * b.app.layout.card_size
}

fn cadre_ecran(b: &Banc, rang: usize) -> [Pos2; 4] {
    let layout = b.app.board_layout();
    let place = layout.cards[K];
    let (quads, _) = b.app.cadres_sur_la_planche(&place);
    quads[rang].map(|p| b.app.vignettes.xf.to_screen(p))
}

fn centre(q: [Pos2; 4]) -> Pos2 {
    pos2((q[0].x + q[2].x) * 0.5, (q[0].y + q[2].y) * 0.5)
}

#[test]
fn ctrl_dragging_the_frame_makes_b_at_the_end_and_the_box_grows_to_two_cells() {
    let mut b = ouvert(plan_court(), 4);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    let a0 = b.app.doc.panel_camera(1, 0);
    assert!(b.app.doc.panels[1].camera.is_empty());
    let (avant, nb) = (b.app.board_layout().cards[K].span, pas_d_annulation(&b));
    assert_eq!(avant, [1, 1]);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 8), case.center() + vec2(w, 0.0), false);
    // En plein geste : les clés sont déjà là et la case a grandi, mais l'étape d'annulation n'est pas encore prise.
    assert_eq!(b.app.doc.panels[1].camera.len(), 2);
    assert_eq!(b.app.board_layout().cards[K].span, [2, 1]);
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
    let big = b.app.board_layout().cards[K];
    assert_eq!(big.span, [2, 1]);
    assert!(big.thumb.width() > 2.0 * b.app.layout.card_size, "deux cases et leur écart");
    // Annuler remet tout : plus de clés, la case redevient une case.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(b.app.doc.panels[1].camera.is_empty());
    assert_eq!(b.app.board_layout().cards[K].span, [1, 1]);
}

#[test]
fn repeating_ctrl_drag_from_the_last_frame_adds_c_and_d_spread_over_the_plan() {
    let mut b = ouvert(plan_court(), 4);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    // C : depuis l'intérieur du dernier cadre (B), une largeur de plus à droite.
    let n = b.app.doc.panels[1].frame_count();
    let b_centre = centre(cadre_ecran(&b, 1));
    ctrl_glisser(&mut b, b_centre, &pas(b_centre, vec2(w, 0.0), 6), b_centre + vec2(w, 0.0), true);
    let k = clefs(&b);
    assert_eq!(k.len(), 3);
    assert_eq!(k.iter().map(|c| c.0).collect::<Vec<_>>(), vec![0, (n - 1) / 2, n - 1], "réparties sur le plan : {k:?}");
    assert!((k[2].1 - (k[1].1 + 160.0)).abs() < 2.0, "C est une largeur de cadre après B : {k:?}");
    assert_eq!(dernier_nom(&b), "Cadre C");
    assert_eq!(b.app.board_layout().cards[K].span, [3, 1]);
    // D : un cran plus bas.
    let c_centre = centre(cadre_ecran(&b, 2));
    ctrl_glisser(&mut b, c_centre, &pas(c_centre, vec2(0.0, w * 0.56), 6), c_centre + vec2(0.0, w * 0.5625), true);
    let k = clefs(&b);
    assert_eq!(k.len(), 4);
    assert_eq!(k.iter().map(|c| c.0).collect::<Vec<_>>(), (0..4).map(|i| crate::vignettecam::repartition(4, n)[i]).collect::<Vec<_>>());
    assert_eq!(dernier_nom(&b), "Cadre D");
    assert_eq!(b.app.board_layout().cards[K].span[1], 2, "un vertical de plus : deux rangées");
}

#[test]
fn dragging_the_edge_of_an_existing_frame_moves_that_key_and_the_corner_zooms_it() {
    let mut b = ouvert(plan_court(), 4);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    let avant = clefs(&b);
    let nb = pas_d_annulation(&b);
    // Le bord bas de B (loin de A) le déplace vers le bas.
    let q = cadre_ecran(&b, 1);
    let bord = pos2((q[2].x + q[3].x) * 0.5, q[2].y);
    ctrl_glisser(&mut b, bord, &pas(bord, vec2(0.0, 20.0), 5), bord + vec2(0.0, 20.0), true);
    let apres = clefs(&b);
    assert_eq!(apres[0], avant[0], "A ne bouge pas");
    assert!(apres[1].2 > avant[1].2 + 5.0 && (apres[1].1 - avant[1].1).abs() < 0.5, "B descend : {avant:?} {apres:?}");
    assert_eq!(pas_d_annulation(&b), nb + 1);
    assert_eq!(dernier_nom(&b), "Cadre B");
    // Un coin de B tiré vers l'extérieur : son zoom baisse (le cadre s'élargit), son centre reste.
    let q = cadre_ecran(&b, 1);
    let coin = q[1];
    let c = centre(q);
    let dehors = (coin - c).normalized() * 30.0;
    ctrl_glisser(&mut b, coin, &pas(coin, dehors, 5), coin + dehors, true);
    let zoome = clefs(&b);
    assert!(zoome[1].3 < apres[1].3, "le zoom de B baisse : {apres:?} {zoome:?}");
    assert!((zoome[1].1 - apres[1].1).abs() < 0.5 && (zoome[1].2 - apres[1].2).abs() < 0.5);
    assert_eq!(zoome[0], avant[0]);
    assert_eq!(dernier_nom(&b), "Zoom du cadre B");
    assert_eq!(pas_d_annulation(&b), nb + 2);
}

#[test]
fn dragging_a_corner_of_a_plain_case_makes_b_zoomed_and_a_stays() {
    let mut b = ouvert(plan_court(), 4);
    let case = vignette(&b, K);
    let a0 = b.app.doc.panel_camera(1, 0);
    let coin = pos2(case.right() - 3.0, case.bottom() - 3.0);
    let dehors = vec2(36.0, 20.0);
    ctrl_glisser(&mut b, coin, &pas(coin, dehors, 5), coin + dehors, true);
    let k = b.app.doc.panels[1].camera.clone();
    assert_eq!(k.len(), 2);
    assert_eq!(k[0].cam, a0, "A reste");
    assert!(k[1].cam.zoom < 1.0 && (k[1].cam.x - a0.x).abs() < 0.01, "B est un zoom arrière centré comme A : {:?}", k[1].cam);
    assert!(b.app.board_layout().cards[K].span[0] >= 2, "un cadre plus large : la case grandit");
    assert_eq!(dernier_nom(&b), "Mouvement de caméra");
}

#[test]
fn a_ctrl_click_that_does_not_move_still_chooses_plans_and_escape_abandons_a_drag() {
    let mut b = ouvert(plan_court(), 4);
    let case = vignette(&b, K);
    let id = b.app.doc.panels[1].id;
    // Ctrl + clic : le choix de plans d'avant, fait au lâcher.
    b.touches = CTRL;
    b.image(vec![Event::PointerMoved(case.center())], 1.0 / 60.0);
    b.image(vec![appui(case.center(), true, CTRL)], 1.0 / 60.0);
    b.image(vec![appui(case.center(), false, CTRL)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.board_sel.contains(&id), "Ctrl + clic choisit le plan");
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
    assert_eq!(b.app.board_layout().cards[K].span, [1, 1]);
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

// ------------------------------------------------------------------------------ la mise en page

fn deux_cadres(doc: &mut Document, plan: usize, dx: f32, dy: f32) {
    let a = doc.default_camera();
    doc.panels[plan].camera.clear();
    crate::camera::set_key(&mut doc.panels[plan].camera, 0, a);
    let n = doc.panels[plan].frame_count();
    crate::camera::set_key(&mut doc.panels[plan].camera, n - 1, a.shifted(dx, dy));
}

fn sans_chevauchement(l: &board::BoardLayout) {
    for (i, a) in l.cards.iter().enumerate() {
        for (j, c) in l.cards.iter().enumerate().skip(i + 1) {
            assert!(!a.thumb.intersects(c.thumb), "les images {i} et {j} se chevauchent : {:?} {:?}", a.thumb, c.thumb);
        }
        for e in &l.empty {
            assert!(!a.thumb.intersects(*e), "l'image {i} recouvre une case vide");
        }
        assert!(l.pages.iter().any(|p| p.contains_rect(a.card)), "l'image {i} sort de la page");
    }
}

#[test]
fn the_box_size_comes_from_the_union_of_the_frames_and_neighbours_reflow() {
    let mut doc = plan_court();
    let s = board::BoardSettings { card_w: 170.0, columns: 4, rows: 0 };
    let normal = board::layout(&doc, &s);
    assert!(normal.cards.iter().all(|c| c.span == [1, 1] && !c.mouvement));
    // Un panoramique d'une largeur de cadre : 2 × 1. Le plan 2 est la quatrième vignette (colonne 3), une case de large
    // ne tient plus dans la rangée : elle passe tout entière à la suivante.
    deux_cadres(&mut doc, 1, 160.0, 0.0);
    let l = board::layout(&doc, &s);
    assert_eq!(l.cards[K].span, [2, 1]);
    assert!(l.cards[K].mouvement);
    assert_eq!((l.cards[K].row, l.cards[K].col), (1, 0), "elle ne se coupe pas : rangée suivante");
    assert!((l.cards[K].thumb.width() - (2.0 * 170.0 + board::GAP_SAME)).abs() < 0.01);
    assert!((l.cards[K].thumb.height() - normal.cards[K].thumb.height()).abs() < 0.01);
    assert_eq!(l.cards.len(), normal.cards.len(), "aucune vignette perdue");
    assert_eq!((l.cards[K + 1].row, l.cards[K + 1].col), (1, 2), "les voisines suivent, dans l'ordre de lecture");
    sans_chevauchement(&l);
    // Un vertical : 1 × 2 (les colonnes s'alignent, les rangées d'en dessous ne se chevauchent pas).
    deux_cadres(&mut doc, 1, 0.0, 90.0);
    let l = board::layout(&doc, &s);
    assert_eq!(l.cards[K].span, [1, 2]);
    assert!(l.cards[K].thumb.height() > 2.0 * normal.cards[K].thumb.height());
    sans_chevauchement(&l);
    // 2 × 2.
    deux_cadres(&mut doc, 1, 160.0, 90.0);
    let l = board::layout(&doc, &s);
    assert_eq!(l.cards[K].span, [2, 2]);
    sans_chevauchement(&l);
    assert!(l.cards.iter().skip(K + 1).all(|c| c.span == [1, 1]));
    // Plus large que la rangée : la largeur est bornée par les colonnes.
    let petit = board::BoardSettings { columns: 2, ..s };
    deux_cadres(&mut doc, 1, 1000.0, 0.0);
    assert_eq!(board::layout(&doc, &petit).cards[K].span[0], 2);
    // Les clés supprimées (au montage) : la case rétrécit.
    doc.panels[1].camera.clear();
    let l = board::layout(&doc, &s);
    assert_eq!(l.cards[K].span, [1, 1]);
    assert_eq!(l.cards.iter().map(|c| c.thumb).collect::<Vec<_>>(), normal.cards.iter().map(|c| c.thumb).collect::<Vec<_>>(), "la planche d'avant, à l'identique");
}

#[test]
fn a_big_box_wraps_whole_on_a_paged_board_and_the_hits_and_slots_still_work() {
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, 160.0, 90.0);
    let s = board::BoardSettings { card_w: 170.0, columns: 4, rows: 2 };
    let l = board::layout(&doc, &s);
    sans_chevauchement(&l);
    let big = l.cards[K];
    assert_eq!(big.span, [2, 2]);
    // Une page de deux rangées : elle ne se coupe pas entre deux pages.
    let page = |r: usize| r / 2;
    assert_eq!(page(big.row), page(big.row + 1));
    assert_eq!(board::hit(&l, big.thumb.center(), 14.0), board::Hit::Thumb(K));
    // Les repères de pose ne sont qu'entre deux plans, jamais dans la grande case.
    for slot in board::drop_slots(&l) {
        assert!(!big.thumb.shrink(1.0).contains(pos2(slot.x, (slot.top + slot.bottom) * 0.5)), "{slot:?}");
    }
    // Au moins une case vide après la dernière vignette.
    assert!(!l.empty.is_empty());
}

// ------------------------------------------------------------------------------ dessiner, montage, annulation

fn avec_mouvement(dx: f32, dy: f32) -> Banc {
    let mut doc = plan_court();
    deux_cadres(&mut doc, 1, dx, dy);
    ouvert(doc, 4)
}

#[test]
fn drawing_in_the_big_box_reaches_beyond_the_sheet() {
    let mut b = avec_mouvement(160.0, 0.0);
    b.app.set_tool(Tool::Brush);
    b.app.color = [255, 0, 255];
    let case = vignette(&b, K);
    assert!(case.width() > 2.0 * 160.0 * b.app.vignettes.xf.zoom.min(1.0) * 0.5);
    // Le quart droit de la grande case est hors de la feuille (le cadre B est à droite d'elle).
    let a = pos2(case.left() + case.width() * 0.78, case.center().y);
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
fn the_other_big_box_is_a_zone_thumbnail_and_the_current_one_is_the_drawing() {
    let mut b = avec_mouvement(160.0, 90.0);
    b.garder_formes = true;
    // Le plan 2 est courant (il est vivant) ; un autre plan courant, il devient une vignette de zone.
    b.app.select_panel(0);
    for _ in 0..40 {
        b.image(Vec::new(), 1.0 / 60.0);
        if !b.app.thumbs.busy() {
            break;
        }
    }
    let case = vignette(&b, K);
    let images = b.formes.iter().filter(|s| matches!(&s.shape, Shape::Mesh(m) if m.texture_id != egui::TextureId::default() && Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()).intersects(case))).count();
    assert!(images >= 1, "la zone se peint comme une vignette");
    b.app.select_panel(1);
    b.chauffer(3);
    assert_eq!(b.app.board_layout().cards[K].span, [2, 2]);
}

#[test]
fn the_montage_shows_the_keys_and_removing_them_there_shrinks_the_box() {
    let mut b = ouvert(plan_court(), 4);
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
    assert_eq!(b.app.board_layout().cards[K].span, [1, 1], "la case a rétréci");
}

#[test]
fn shift_dragging_the_frame_of_a_big_box_still_moves_the_plan() {
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
    let mut b = ouvert(plan_court(), 4);
    let w = largeur_cadre_ecran(&b);
    let case = vignette(&b, K);
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT se repose après un mouvement de caméra");
    let repos: Vec<_> = (0..6).map(|_| b.image(Vec::new(), 0.5)).collect();
    let pire = repos.iter().map(|m| m.reveil).min().unwrap();
    assert!(pire >= std::time::Duration::from_secs(1), "au repos, SHOT demande à être repeint dans {pire:?}");
    let _ = Camera::centered(1, 1);
}
