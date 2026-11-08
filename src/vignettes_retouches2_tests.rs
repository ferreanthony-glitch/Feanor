//! Essais des retouches des Vignettes du 2026-10-22 (deuxième tour) : la barre de coupe qui se décoche, les cases qui suivent un mouvement
//! de caméra, la copie d'un plan (Maj + Alt), le lasso de Ctrl, l'affichage des tuiles.

use eframe::egui::{self, epaint::Shape, pos2, vec2, Event, Key, Modifiers, Pos2, Rect};

use crate::document::Document;
use crate::perf_tests::Banc;
use crate::app::Tool;
use crate::board;
use crate::vignettes_camera_tests::{avec_b_a_droite, cadre_ecran, centre, clefs, ctrl_glisser, glisser_avec, largeur_cadre_ecran, pas, plan_court, CTRL, CTRL_SEUL, K};
use crate::vignettes_tests::{appui, barre, dernier_nom, encre, glisser, ouvert, pas_d_annulation, vague, vignette};
use crate::workspace::Espace;

/// Trois plans d'une même séquence, 1 | 2 | 3, des coupes entre eux.
fn trois_plans() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    let p1 = doc.add_scene_after(0);
    doc.add_scene_after(p1);
    doc.into_plans();
    assert_eq!(doc.panels.len(), 3);
    doc
}

fn clic_barre(b: &mut Banc, k: usize) {
    let p = barre(b, k).center();
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![appui(p, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
}

fn coupes(b: &Banc) -> Vec<bool> {
    let l = b.app.board_layout();
    l.bars.iter().take(l.cards.len() - 1).map(|x| x.cut).collect()
}

#[test]
fn unchecking_a_cut_merges_only_the_two_plans_and_the_next_cut_stays() {
    let mut b = ouvert(trois_plans(), 6);
    assert_eq!(coupes(&b), vec![true, true]);
    let n = pas_d_annulation(&b);
    clic_barre(&mut b, 0);
    assert_eq!(b.app.doc.panels.len(), 2, "1 et 2 ne font qu'un plan : {:?}", coupes(&b));
    assert_eq!(coupes(&b), vec![false, true], "la coupe suivante reste cochée");
    assert_eq!(pas_d_annulation(&b), n + 1);
}


/// Trois plans de trois cases chacun (des marques aux images 10 et 20), des notes à la première case de chacun.
fn trois_plans_a_cases(camera_sur: Option<usize>) -> Document {
    let mut doc = trois_plans();
    for i in 0..3 {
        doc.set_duration(i, 30);
        doc.toggle_mark(i, 10);
        doc.toggle_mark(i, 20);
    }
    if let Some(i) = camera_sur {
        let a = doc.default_camera();
        crate::camera::set_key(&mut doc.panels[i].camera, 0, a);
        crate::camera::set_key(&mut doc.panels[i].camera, 29, a.shifted(80.0, 0.0));
    }
    doc
}

fn formes_du_plan(b: &Banc) -> Vec<(usize, Vec<u64>)> {
    b.app.doc.panels.iter().enumerate().map(|(i, p)| (i, p.marks.iter().map(|m| m.frame as u64).collect())).collect()
}

// ------------------------------------------------------------------------------ la barre de coupe

/// Décocher une coupe ne change que celle-là, quelle que soit la forme des plans : une case, plusieurs, un mouvement de caméra. Recocher la
/// remet exactement où elle était ; chaque geste est une étape d'annulation, nommée.
#[test]
fn unchecking_a_cut_changes_only_that_cut_and_checking_it_again_splits_back_at_the_same_place() {
    for (nom, doc) in [("une case", trois_plans()), ("plusieurs cases", trois_plans_a_cases(None)), ("mouvement sur le plan 1", trois_plans_a_cases(Some(0))), ("mouvement sur le plan 2", trois_plans_a_cases(Some(1)))] {
        let mut b = ouvert(doc, 6);
        let l = b.app.board_layout();
        let coupes_avant = coupes(&b);
        let plans_avant = formes_du_plan(&b);
        for k in 0..l.cards.len() - 1 {
            if !coupes_avant[k] {
                continue;
            }
            let n = pas_d_annulation(&b);
            let layout = b.app.board_layout();
            assert!(b.app.board_toggle_cut(&layout, k), "{nom} : la coupe {k} se décoche");
            b.chauffer(2);
            let mut attendu = coupes_avant.clone();
            attendu[k] = false;
            assert_eq!(coupes(&b), attendu, "{nom} : seule la coupe {k} change, la suivante reste cochée");
            assert_eq!(pas_d_annulation(&b), n + 1, "{nom} : une étape d'annulation");
            assert_eq!(dernier_nom(&b), "Réunir deux vignettes en un plan");
            // La recocher : le plan se coupe au même endroit.
            let layout = b.app.board_layout();
            assert!(b.app.board_toggle_cut(&layout, k));
            b.chauffer(2);
            assert_eq!(coupes(&b), coupes_avant, "{nom} : recochée, comme avant");
            assert_eq!(formes_du_plan(&b), plans_avant, "{nom} : les plans se coupent là où ils étaient joints");
            assert_eq!(pas_d_annulation(&b), n + 2);
            assert_eq!(dernier_nom(&b), "Couper le plan entre deux vignettes");
            // Annuler la coupe, puis la réunion : tout est revenu.
            b.app.run_action("edition.annuler");
            b.app.run_action("edition.annuler");
            b.chauffer(2);
            assert_eq!(coupes(&b), coupes_avant, "{nom} : annulé");
            assert_eq!(pas_d_annulation(&b), n, "{nom}");
        }
    }
}

/// Deux appuis du bouton principal dans la même image d'écran, sans lâcher entre eux (un stylet double l'événement de la souris) : une
/// seule coupe change, jamais la barre d'à côté.
#[test]
fn a_doubled_press_unchecks_one_cut_not_the_next_one() {
    let mut b = ouvert(trois_plans(), 6);
    let p = barre(&b, 0).center();
    let n = pas_d_annulation(&b);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, Modifiers::NONE), appui(p, true, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![appui(p, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(coupes(&b), vec![false, true]);
    assert_eq!(pas_d_annulation(&b), n + 1, "une seule étape");
}

// ------------------------------------------------------------------------------ les notes d'un plan réuni

fn avec_notes() -> Document {
    let mut doc = trois_plans();
    for (i, (action, dialogue, notes)) in [("P avance", "P dit", "P note"), ("Q recule", "Q dit", "Q note"), ("", "", "")].into_iter().enumerate() {
        doc.panels[i].action = action.into();
        doc.panels[i].dialogue = dialogue.into();
        doc.panels[i].notes = notes.into();
    }
    doc
}

/// Réunir deux plans par la barre : leurs notes restent chacune à leur case, dans l'ordre du temps (P avant Q) ; rien ne se perd, et
/// annuler redonne les deux d'origine, exactement.
#[test]
fn merging_two_plans_keeps_both_notes_in_time_order_and_undo_restores_them() {
    let mut b = ouvert(avec_notes(), 6);
    let (p, q) = (b.app.doc.panels[0].id, b.app.doc.panels[1].id);
    let avant = b.app.doc.panels.iter().map(|x| (x.action.clone(), x.dialogue.clone(), x.notes.clone(), x.marks.clone())).collect::<Vec<_>>();
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, 0));
    b.chauffer(2);
    let doc = &b.app.doc;
    assert_eq!(doc.panels.len(), 2);
    let fusion = &doc.panels[0];
    assert_eq!(fusion.id, p);
    // P garde ses textes ; ceux de Q sont à la case qui commence où Q commençait (la marque), après ceux de P.
    assert_eq!((fusion.action.as_str(), fusion.dialogue.as_str(), fusion.notes.as_str()), ("P avance", "P dit", "P note"));
    assert_eq!(fusion.marks.len(), 1);
    assert_eq!((fusion.marks[0].action.as_str(), fusion.marks[0].dialogue.as_str(), fusion.marks[0].notes.as_str()), ("Q recule", "Q dit", "Q note"));
    assert!(fusion.marks[0].frame >= 1, "la marque est après le début de P");
    // Les bandes de note de la planche : P, puis Q, dans l'ordre des cases.
    let l = b.app.board_layout();
    assert_eq!(b.app.card_notes(&l.cards[0]), "P note");
    assert_eq!(b.app.card_notes(&l.cards[1]), "Q note");
    // Un côté vide : pas de saut de ligne perdu, rien d'ajouté.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    let revenu = b.app.doc.panels.iter().map(|x| (x.action.clone(), x.dialogue.clone(), x.notes.clone(), x.marks.clone())).collect::<Vec<_>>();
    assert_eq!(revenu, avant, "annuler redonne les deux plans d'origine");
    assert_eq!(b.app.doc.panels[1].id, q);
    // Réunir Q (qui a des notes) et le plan 3 (sans note), puis les couper : les textes de Q retournent à Q.
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, 1));
    b.chauffer(2);
    let q2 = &b.app.doc.panels[1];
    assert_eq!(q2.notes, "Q note");
    assert_eq!(q2.marks.len(), 1);
    assert!(q2.marks[0].notes.is_empty() && !q2.marks[0].notes.contains('\n'), "un côté vide : rien d'ajouté, pas de saut de ligne");
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, 1));
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[1].notes, "Q note");
    assert_eq!(b.app.doc.panels.len(), 3);
}

// ------------------------------------------------------------------------------ le montage ne montre pas les limites des cases

fn marqueurs_orange(b: &Banc) -> usize {
    let c = crate::theme::marque();
    b.formes.iter().filter(|s| matches!(&s.shape, Shape::Path(p) if p.fill == c)).count()
}

#[test]
fn the_montage_paints_no_case_boundary_marker_but_the_data_keeps_the_case() {
    let mut b = ouvert(trois_plans(), 6);
    b.garder_formes = true;
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, 0));
    assert_eq!(b.app.doc.panels.len(), 2);
    assert_eq!(b.app.doc.panels[0].marks.len(), 1, "la limite de la case est dans les données : les Vignettes en ont besoin");
    assert_eq!(b.app.board_layout().cards.len(), 3, "deux cases pour le plan réuni");
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    b.image(Vec::new(), 1.0 / 60.0);
    let c = crate::theme::marque();
    let dans_le_montage = b.formes.iter().filter(|s| matches!(&s.shape, Shape::Path(p) if p.fill == c && p.points.len() == 3)).count();
    assert_eq!(dans_le_montage, 0, "aucun repère de limite de case peint dans les blocs du montage ({} triangles de la couleur des marques)", marqueurs_orange(&b));
    assert_eq!(b.app.doc.panels[0].marks.len(), 1);
}

// ------------------------------------------------------------------------------ les cases qui suivent un mouvement de caméra

/// Un plan court dont le plan 1 (K) a une marque d'image : sa deuxième case suit le mouvement de la première.
fn plan_a_deux_cases() -> Document {
    let mut doc = plan_court();
    doc.set_duration(1, 24);
    doc.toggle_mark(1, 12);
    doc
}

#[test]
fn the_case_after_a_camera_move_shows_the_last_frame_and_moving_b_updates_it() {
    // (Un mouvement créé sur une case qui en a derrière elle coupe le plan : voir _camera_move_created_on_a_case_with_continuity_cases_splits_them_off. La règle case qui suit = dernier cadre vaut pour les cases faites APRÈS le mouvement : ici, la case de continuité est ajoutée une fois le mouvement posé.)
    let mut doc = plan_court();
    doc.set_duration(1, 12);
    let mut b = ouvert(doc, 6);
    let (case, w) = (vignette(&b, K), largeur_cadre_ecran(&b));
    ctrl_glisser(&mut b, case.center(), &pas(case.center(), vec2(w, 0.0), 6), case.center() + vec2(w, 0.0), true);
    b.app.doc.set_duration(1, 24);
    b.app.doc.toggle_mark(1, 12);
    b.chauffer(3);
    assert_eq!(b.app.board_layout().cards[K + 1].frame, 12);
    let k = &b.app.doc.panels[1].camera;
    assert_eq!(k.len(), 2);
    // Le mouvement tient dans la première case : B est à sa dernière image (11), A à la première.
    assert_eq!((k[0].frame, k[1].frame), (0, 11), "les clés se rangent sur la case qui porte le mouvement");
    let b_cam = k[1].cam;
    for f in [12, 18, 23] {
        let c = b.app.doc.panel_camera(1, f);
        assert_eq!((c.x, c.y, c.zoom), (b_cam.x, b_cam.y, b_cam.zoom), "l'image {f} (case suivante) voit à travers le dernier cadre");
    }
    // Les vignettes : la case suivante est composée avec la caméra de B ; B déplacé, elle se refait.
    b.chauffer(3);
    let lire = |b: &mut Banc| {
        for _ in 0..60 {
            b.image(Vec::new(), 1.0 / 60.0);
            if !b.app.thumbs.busy() {
                break;
            }
        }
        let id = b.app.board_layout().cards[K + 1].id;
        let pref = crate::thumbs::width_for(vignette(b, K + 1).width());
        std::iter::once(pref).chain((1..=8).map(|m| m * 128)).filter_map(|w| b.app.thumbs.case_image_of(id, w).map(|(i, _)| i.pixels.clone())).next()
    };
    // (Du trait dans la feuille, que B déplacé change de place dans la vignette suivante.)
    b.app.select_panel(1);
    // (Un trait qui traverse A et B : ce que la case suivante voit dépend de la place de B.)
    glisser(&mut b, &vague(case.center() - vec2(30.0, 10.0), case.center() + vec2(1.4 * w, 10.0), 14));
    let avant = lire(&mut b).expect("la vignette de la case suivante");
    let c = centre(cadre_ecran(&b, 1));
    ctrl_glisser(&mut b, c, &pas(c, vec2(0.0, 0.2 * w), 6), c + vec2(0.0, 0.2 * w), true);
    let apres = lire(&mut b).expect("la vignette refaite");
    assert_ne!(avant, apres, "B déplacé, la case qui la suit montre le nouveau dernier cadre");
    let b2 = b.app.doc.panels[1].camera[1].cam;
    assert_eq!(b.app.doc.panel_camera(1, 20).y, b2.y);
    // Une case d'avant le mouvement n'est pas touchée.
    assert!(b.app.doc.panel_camera(0, 5).y == b.app.doc.default_camera().y);
}

#[test]
fn a_move_made_on_keys_spread_over_the_whole_plan_is_settled_on_the_first_case_the_next_time_it_is_touched() {
    use crate::camera::CameraKey;
    let mut doc = plan_a_deux_cases();
    let a = doc.default_camera();
    doc.panels[1].camera = vec![CameraKey::new(0, a), CameraKey::new(23, a.shifted(160.0, 0.0))];
    let mut b = ouvert(doc, 6);
    let w = largeur_cadre_ecran(&b);
    let c = centre(cadre_ecran(&b, 1));
    ctrl_glisser(&mut b, c, &pas(c, vec2(0.0, 0.1 * w), 6), c + vec2(0.0, 0.1 * w), true);
    let frames: Vec<usize> = clefs(&b).iter().map(|k| k.0).collect();
    assert_eq!(frames, vec![0, 11], "les clés posées sur tout le plan se rangent sur sa première case");
}

#[test]
fn settling_pure_functions() {
    use crate::camera::CameraKey;
    use crate::vignettecam::{etendue_du_mouvement, recale};
    assert_eq!((etendue_du_mouvement(12, 24), etendue_du_mouvement(1, 24), etendue_du_mouvement(30, 24)), (12, 24, 24));
    let a = crate::camera::Camera::centered(160, 90);
    let tout = vec![CameraKey::new(0, a), CameraKey::new(23, a)];
    assert_eq!(recale(&tout, 12, 24).iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 11]);
    // Des clés posées à la main restent.
    let main = vec![CameraKey::new(0, a), CameraKey::new(7, a), CameraKey::new(23, a)];
    assert_eq!(recale(&main, 12, 24), main);
    // Un plan d'une seule case : rien ne change.
    assert_eq!(recale(&tout, 24, 24), tout);
}

// ------------------------------------------------------------------------------ Maj + Alt : plus de copie d'un plan (partie 113, décision 5)

const MAJ_ALT: Modifiers = Modifiers { alt: true, shift: true, ..Modifiers::NONE };

#[test]
fn shift_alt_dragging_a_plan_no_longer_duplicates_it_but_moves_it_like_shift_alone() {
    let mut b = ouvert(crate::vignettes_tests::quatre_plans(), 4);
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let n = pas_d_annulation(&b);
    let case = vignette(&b, 3);
    let cadre = pos2(case.left() + 2.0, case.center().y);
    // Les repères de pose : devant chaque plan ; on vise celui d'avant le plan 4 (index 3).
    let layout = b.app.board_layout();
    let slot = board::drop_slots(&layout).into_iter().find(|s| s.before == Some(ids[3])).expect("un repère devant le plan 4");
    let cible = b.app.vignettes.xf.to_screen(pos2(slot.x, (slot.top + slot.bottom) * 0.5));
    glisser_avec(&mut b, MAJ_ALT, cadre, &pas(cadre, cible - cadre, 8), cible, true);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 4, "aucun plan de plus : {}", b.app.status);
    assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[0], ids[2], ids[1], ids[3]], "le plan 2 est déplacé devant le plan 4, comme avec Maj seule");
    assert_eq!(pas_d_annulation(&b), n + 1, "une seule étape d'annulation");
    assert_eq!(dernier_nom(&b), "Déplacer le plan");
}

// ------------------------------------------------------------------------------ Ctrl : le lasso ; Ctrl + Maj ou T : la caméra

#[test]
fn ctrl_alone_dragging_on_a_case_is_the_lasso_and_never_the_camera() {
    let mut b = ouvert(plan_court(), 6);
    b.app.set_tool(Tool::Brush);
    let case = vignette(&b, K);
    let (de, a) = (case.left_top() + vec2(8.0, 8.0), case.right_bottom() - vec2(8.0, 8.0));
    b.touches = CTRL_SEUL;
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    for _ in 0..30 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    b.image(vec![appui(de, true, CTRL_SEUL)], 1.0 / 60.0);
    for q in pas(de, a - de, 6) {
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    // (Depuis la partie 125, sur un calque vectoriel, le lasso de Ctrl est celui de la planche : il prend les traits entiers, sans changer d'outil.)
    let planche = b.app.vignettes.selp.as_ref().is_some_and(|s| s.lasso.is_some());
    assert!(planche || b.app.tool == Tool::Select, "Ctrl + appui sur une case : le lasso");
    assert!(planche || b.app.vdrag.is_some() || b.app.bdrag.is_some(), "un lasso est en cours");
    assert!(b.app.vignettes.cam.is_none() && b.app.vignettes.moving.is_none(), "ni caméra ni plan déplacé");
    b.image(vec![appui(a, false, CTRL_SEUL)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.doc.panels[1].camera.is_empty(), "aucun mouvement de caméra");
    assert_eq!(b.app.tool, Tool::Brush, "Ctrl relâché : l'outil d'avant revient");
    assert!(!encre(&b.app.doc, 1), "et rien n'est dessiné");
}

/// Le geste comme la main le fait, Ctrl et Maj dans un ordre ou dans l'autre, tenus un bon moment avant l'appui : la caméra, jamais le lasso.
fn deux_touches_puis_glisser(b: &mut Banc, premiere: Modifiers, de: Pos2, delta: egui::Vec2) {
    b.touches = premiere;
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    for _ in 0..30 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    b.touches = CTRL;
    for _ in 0..4 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    b.image(vec![appui(de, true, CTRL)], 1.0 / 60.0);
    for q in pas(de, delta, 8) {
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerMoved(de + delta)], 1.0 / 60.0);
    b.image(vec![appui(de + delta, false, CTRL)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    b.chauffer(2);
}

#[test]
fn ctrl_then_shift_or_shift_then_ctrl_is_the_camera_and_never_arms_the_lasso() {
    for (nom, premiere) in [("Ctrl puis Maj", CTRL_SEUL), ("Maj puis Ctrl", Modifiers::SHIFT)] {
        let mut b = ouvert(plan_court(), 6);
        b.app.set_tool(Tool::Brush);
        let (case, w) = (vignette(&b, K), largeur_cadre_ecran(&b));
        deux_touches_puis_glisser(&mut b, premiere, case.center(), vec2(w, 0.0));
        assert_eq!(b.app.doc.panels[1].camera.len(), 2, "{nom} : B est née : {}", b.app.status);
        assert_eq!(b.app.tool, Tool::Brush, "{nom} : le lasso ne s'est jamais pris");
        assert!(b.app.vdrag.is_none() && b.app.bdrag.is_none() && b.app.vsel.is_none());
        assert!(!encre(&b.app.doc, 1) && b.app.vignettes.moving.is_none());
        assert_eq!(dernier_nom(&b), "Mouvement de caméra");
    }
}

#[test]
fn t_held_is_the_camera_and_t_with_alt_adds_a_frame() {
    let mut b = ouvert(plan_court(), 6);
    let (case, w) = (vignette(&b, K), largeur_cadre_ecran(&b));
    let t = |pressed: bool| Event::Key { key: Key::T, physical_key: Some(Key::T), pressed, repeat: false, modifiers: Modifiers::NONE };
    let tirer = |b: &mut Banc, m: Modifiers, de: Pos2, delta: egui::Vec2| {
        b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
        b.image(vec![t(true)], 1.0 / 60.0);
        b.touches = m;
        for _ in 0..4 {
            b.image(Vec::new(), 1.0 / 60.0);
        }
        b.image(vec![appui(de, true, m)], 1.0 / 60.0);
        for q in pas(de, delta, 8) {
            b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
        }
        b.image(vec![appui(de + delta, false, m)], 1.0 / 60.0);
        b.touches = Modifiers::NONE;
        b.image(vec![t(false)], 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
    };
    tirer(&mut b, Modifiers::NONE, case.center(), vec2(w, 0.0));
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "T + glisser : B");
    // (Partie 113, décision 5 : Alt n'ajoute plus de cadre ; T + Alt et Ctrl + Maj + Alt déplacent le cadre pris, comme sans Alt.
    // « Ajouter un cadre » est au clic droit de la case.)
    let c = centre(cadre_ecran(&b, 1));
    tirer(&mut b, Modifiers::ALT, c, vec2(w, 0.0));
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "T + Alt + glisser : B déplacé, pas de C : {}", b.app.status);
    let c = centre(cadre_ecran(&b, 1));
    let m = Modifiers { ctrl: true, shift: true, alt: true, command: true, ..Modifiers::NONE };
    glisser_avec(&mut b, m, c, &pas(c, vec2(w, 0.0), 6), c + vec2(w, 0.0), true);
    assert_eq!(b.app.doc.panels[1].camera.len(), 2, "Ctrl + Maj + Alt + glisser : pas de D");
}

// ------------------------------------------------------------------------------ l'affichage des tuiles

/// Attend la fin des vignettes de fond.
fn attendre(b: &mut Banc) {
    for _ in 0..80 {
        b.image(Vec::new(), 1.0 / 60.0);
        if !b.app.thumbs.busy() {
            break;
        }
    }
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Les rectangles des images de tuile (maillages texturés) qui touchent `zone`.
fn images_dans(b: &Banc, zone: Rect) -> Vec<Rect> {
    b.formes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Mesh(m) if m.texture_id != egui::TextureId::default() && !m.vertices.is_empty() => Some(Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>())),
            _ => None,
        })
        .filter(|r| r.intersects(zone))
        .collect()
}

#[test]
fn after_a_camera_drag_the_tile_matches_the_new_geometry_and_a_stale_one_is_never_placed_as_if_it_were_right() {
    let mut b = avec_b_a_droite();
    attendre(&mut b);
    let w = largeur_cadre_ecran(&b);
    let geometrie = |b: &Banc| {
        let l = b.app.board_layout();
        let m = l.move_of(K).unwrap().clone();
        (m.tuile.zone, m.tuile.masque.clone())
    };
    let (zone0, masque0) = geometrie(&b);
    let g0 = crate::thumbs::Geom::de(&zone0, &masque0);
    // Déplacer B : la géométrie change.
    let c = centre(cadre_ecran(&b, 1));
    ctrl_glisser(&mut b, c, &pas(c, vec2(0.3 * w, 0.5 * w), 6), c + vec2(0.3 * w, 0.5 * w), true);
    let (zone1, masque1) = geometrie(&b);
    let g1 = crate::thumbs::Geom::de(&zone1, &masque1);
    assert_ne!(g0, g1, "les cadres ont bougé : la géométrie de la tuile n'est plus la même");
    // Sitôt lâchée : la tuile qu'on a est celle d'avant, elle n'est pas prise pour la bonne ; elle est posée à sa place d'avant.
    let l = b.app.board_layout();
    let place = l.cards[K];
    let id = place.id;
    // Une fois les fils de fond finis : la texture est celle de la nouvelle géométrie, sans masque d'avant.
    attendre(&mut b);
    let texs = b.app.thumbs.case_texs_of(id);
    let tex = *texs.iter().find(|t| t.geom == Some(g1)).unwrap_or_else(|| panic!("aucune tuile composée pour la géométrie d'après le geste : {:?}", texs.iter().map(|t| t.geom).collect::<Vec<_>>()));
    // Posée sur sa propre géométrie, elle tombe pile sur le rectangle de la tuile.
    let r = b.app.vignettes.xf.rect(l.move_of(K).unwrap().tuile.rect);
    assert_eq!(tex.rect_tuile(r, &zone1, b.app.vignettes.xf.zoom * zone1.echelle), tex.rect(r));
    // Si la géométrie avait changé sans que la texture se refasse, elle est posée là où est la feuille qu'elle montre, pas étirée sur la nouvelle tuile.
    let ancienne = crate::thumbs::CaseTex { geom: Some(g0), ..tex };
    let placee = ancienne.rect_tuile(r, &zone1, b.app.vignettes.xf.zoom * zone1.echelle);
    let attendu_centre = r.center() + (zone0.centre - zone1.centre) * (b.app.vignettes.xf.zoom * zone1.echelle);
    let sans_marge = Rect::from_center_size(attendu_centre, zone0.taille * (b.app.vignettes.xf.zoom * zone1.echelle));
    assert!((placee.center() - ancienne.rect(sans_marge).center()).length() < 0.01);
}

#[test]
fn every_plan_shows_its_margin_on_the_paper_whether_it_is_current_or_not() {
    let mut doc = plan_court();
    deux_cadres_et_un_autre(&mut doc);
    let mut b = ouvert(doc, 6);
    b.garder_formes = true;
    b.app.select_panel(0);
    attendre(&mut b);
    let l = b.app.board_layout();
    let tuile = b.app.vignettes.xf.rect(l.move_of(K).unwrap().tuile.rect);
    let hull = vignette_bbox(&b, K);
    // Un autre plan que le courant : sa marge se voit (partie 111 : c'est ce qui laissait un trou entre deux plans).
    let serre = images_dans(&b, tuile.expand(200.0)).into_iter().filter(|r| r.width() > 20.0 && r.height() > 20.0).collect::<Vec<_>>();
    assert!(!serre.is_empty(), "la tuile est peinte");
    let limite = hull.expand(4.6);
    assert!(serre.iter().any(|r| !limite.contains_rect(*r)), "plan non courant : la marge est ouverte aussi : {serre:?}");
    // Le plan courant : la marge de 12 % est ouverte, l'image dépasse l'enveloppe de bien plus.
    b.app.select_panel(1);
    attendre(&mut b);
    b.app.select_panel(0);
    b.app.select_panel(1);
    b.image(Vec::new(), 1.0 / 60.0);
    let ouverte = images_dans(&b, tuile.expand(200.0)).into_iter().filter(|r| r.width() > 20.0).collect::<Vec<_>>();
    assert!(ouverte.iter().any(|r| !limite.contains_rect(*r)), "plan courant : la marge est ouverte : {ouverte:?}");
}

fn deux_cadres_et_un_autre(doc: &mut Document) {
    crate::vignettes_camera_tests::deux_cadres(doc, 1, 160.0, 0.0);
}

fn vignette_bbox(b: &Banc, k: usize) -> Rect {
    let l = b.app.board_layout();
    b.app.vignettes.xf.rect(crate::vignettecam::boite(&l.move_of(k).unwrap().enveloppe))
}

