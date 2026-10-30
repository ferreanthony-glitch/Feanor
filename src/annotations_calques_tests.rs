//! Essais des calques d'annotation de la planche et du lasso d'annotation, à l'écran (`annotationsui.rs`, `vignettespanneau.rs`). Le calque
//! d'annotation d'un plan : `calqueannot_tests.rs`.

use eframe::egui::{self, pos2, vec2, Event, Key, Modifiers, Pos2, Rect};

use crate::annotations::COULEUR_PAR_DEFAUT;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::vector::VPoint;
use crate::vignettes_tests::{appui, dernier_nom, glisser, ouvert, pas_d_annulation, quatre_plans, vignette};

// ------------------------------------------------------------------------------------ la planche

/// Un trait en diagonale, ondulé, de demi-longueur `demi` (planche), centré en `c`.
fn diag(c: Pos2, demi: f32) -> Vec<VPoint> {
    (0..=8)
        .map(|k| {
            let t = k as f32 / 8.0 - 0.5;
            VPoint { x: c.x + t * 2.0 * demi, y: c.y + t * 1.2 * demi + (t * 7.0).sin() * 4.0, r: 1.5 }
        })
        .collect()
}

/// Pose un trait de `demi` de demi-longueur au centre de la case `k`, dans le calque `calque` (le premier, sinon). Rend son identifiant.
pub(crate) fn poser(b: &mut Banc, calque: Option<u64>, k: usize, demi: f32) -> u64 {
    let layout = b.app.board_layout();
    let c = layout.cards[k].thumb.center();
    assert!(b.app.doc.annotations.ajouter_dans(&layout, calque, &diag(c, demi), COULEUR_PAR_DEFAUT));
    b.chauffer(2);
    b.app.doc.annotations.traces.last().expect("un trait").id
}

fn points(b: &Banc, id: u64) -> Vec<VPoint> {
    b.app.doc.annotations.points_planche_de(&b.app.board_layout(), id).expect("trait placé")
}

fn centre(pts: &[VPoint]) -> Pos2 {
    crate::board::points_bounds(pts).center()
}

/// La boîte des traits `ids` telle qu'on la voit (avec l'air que lui laisse le lasso : deux points d'écran), à l'écran.
fn boite(b: &Banc, ids: &[u64]) -> Rect {
    let xf = b.app.vignettes.xf;
    let r = b.app.doc.annotations.bornes(&b.app.board_layout(), ids).expect("des traits placés").expand(2.0 / xf.zoom);
    xf.rect(r)
}

/// L'espace Vignettes (la planche et son panneau Planche à droite), `colonnes` cases par ligne, assez petites pour tenir à l'écran.
pub(crate) fn ouvert_vignettes(doc: Document, colonnes: usize) -> Banc {
    let mut b = banc_taille(doc, false, 1600, 940);
    b.garder_formes = true;
    b.app.set_espace(crate::workspace::Espace::Vignettes);
    b.app.layout.board_columns = colonnes;
    b.app.layout.board_rows = 0;
    b.app.layout.card_size = 170.0;
    b.app.board_zoom = 0.0;
    b.chauffer(6);
    b
}

pub(crate) fn lasso(b: &mut Banc) {
    b.app.run_action("vignettes.annotation_lasso");
    b.chauffer(2);
    assert!(b.app.annot_actif());
}

/// Un lasso qui fait le tour de `r` (écran), en commençant dans le vide.
fn entourer(b: &mut Banc, r: Rect) {
    let (c, m) = (r.center(), r.center());
    let chemin = [r.left_top(), pos2(m.x, r.top()), r.right_top(), pos2(r.right(), c.y), r.right_bottom(), pos2(m.x, r.bottom()), r.left_bottom(), pos2(r.left(), c.y), r.left_top() + vec2(1.0, 1.0)];
    glisser(b, &chemin);
}

/// Un lasso qui fait le tour des traits `ids`, avec `marge` points d'écran d'air.
pub(crate) fn entourer_trait(b: &mut Banc, ids: &[u64], marge: f32) {
    let r = boite(b, ids).expand(marge);
    entourer(b, r);
}

fn echap() -> Event {
    Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

fn proches(a: &[VPoint], b: &[VPoint], tol: f32) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p.x - q.x).abs() < tol && (p.y - q.y).abs() < tol && (p.r - q.r).abs() < tol)
}

/// Le texte `t` dans ce que la dernière image a peint : le centre de sa zone, à l'écran (il faut `garder_formes`).
pub(crate) fn texte_en(b: &Banc, t: &str) -> Option<Pos2> {
    b.formes.iter().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(x) if x.galley.job.text == t => Some(x.pos + x.galley.size() * 0.5),
        _ => None,
    })
}

/// Le texte `t` le plus proche, à l'horizontale, de `x` (plusieurs textes de l'écran peuvent s'écrire pareil : on prend celui de la colonne).
fn texte_pres_de(b: &Banc, t: &str, x: f32) -> Option<Pos2> {
    textes_en(b, t).into_iter().min_by(|p, q| (p.x - x).abs().total_cmp(&(q.x - x).abs()))
}

fn textes_en(b: &Banc, t: &str) -> Vec<Pos2> {
    let mut v: Vec<Pos2> = b.formes.iter().filter_map(|s| match &s.shape {
        egui::epaint::Shape::Text(x) if x.galley.job.text == t => Some(x.pos + x.galley.size() * 0.5),
        _ => None,
    }).collect();
    v.sort_by(|a, b| a.y.total_cmp(&b.y));
    v
}

pub(crate) fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![appui(p, false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(3);
}

#[test]
fn the_lasso_picks_strokes_and_moves_them_in_one_undo_step_that_undoes_and_redoes() {
    let mut b = ouvert(quatre_plans(), 3);
    let (id0, id3) = (poser(&mut b, None, 0, 40.0), poser(&mut b, None, 3, 40.0));
    lasso(&mut b);
    let (avant0, avant3) = (points(&b, id0), points(&b, id3));
    let etapes = pas_d_annulation(&b);
    // Un lasso autour du trait de la case 0 : choisi, et seulement lui. Choisir ne laisse aucune étape.
    entourer_trait(&mut b, &[id0], 14.0);
    assert_eq!(b.app.vignettes.annot.selection, vec![id0]);
    assert_eq!(pas_d_annulation(&b), etapes);
    // Glisser dans la boîte (hors du pivot, des poignées et des bords) : déplacer.
    let r = boite(&b, &[id0]);
    let prise = r.center() + vec2(r.width() * 0.25, 0.0);
    let zoom = b.app.vignettes.xf.zoom;
    glisser(&mut b, &[prise, prise + vec2(10.0, 4.0), prise + vec2(30.0, 12.0), prise + vec2(50.0, 20.0)]);
    assert_eq!(pas_d_annulation(&b), etapes + 1, "une seule étape pour le geste");
    assert_eq!(dernier_nom(&b), "Déplacer des annotations");
    let apres0 = points(&b, id0);
    let d = centre(&apres0) - centre(&avant0);
    assert!((d.x - 50.0 / zoom).abs() < 0.5 && (d.y - 20.0 / zoom).abs() < 0.5, "déplacé de (50, 20) points d'écran : {d:?} (zoom {zoom})");
    for (p, q) in avant0.iter().zip(&apres0) {
        assert!((q.r - p.r).abs() < 1e-3, "l'épaisseur ne change pas en déplaçant");
    }
    assert!(proches(&points(&b, id3), &avant3, 1e-3), "l'autre trait n'a pas bougé");
    assert_eq!(b.app.vignettes.annot.selection, vec![id0], "il reste choisi");
    // Ctrl+Z le remet, Ctrl+Maj+Z le redéplace.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(proches(&points(&b, id0), &avant0, 1e-2), "annuler remet le trait où il était");
    b.app.run_action("edition.retablir");
    b.chauffer(2);
    assert!(proches(&points(&b, id0), &apres0, 1e-2));
}

#[test]
fn a_click_picks_the_stroke_under_it_and_shift_adds_or_removes() {
    let mut b = ouvert(quatre_plans(), 3);
    let (id0, id3) = (poser(&mut b, None, 0, 40.0), poser(&mut b, None, 3, 40.0));
    lasso(&mut b);
    let xf = b.app.vignettes.xf;
    let sur = |b: &Banc, id: u64| xf.to_screen(points(b, id)[4].pos());
    let etapes = pas_d_annulation(&b);
    let p = sur(&b, id0);
    clic(&mut b, p);
    assert_eq!(b.app.vignettes.annot.selection, vec![id0]);
    // Maj + clic sur l'autre : il s'ajoute ; Maj + clic dessus de nouveau : il se retire.
    b.touches = Modifiers::SHIFT;
    let p = sur(&b, id3);
    clic_avec_maj(&mut b, p);
    assert_eq!(b.app.vignettes.annot.selection, vec![id0, id3]);
    clic_avec_maj(&mut b, p);
    assert_eq!(b.app.vignettes.annot.selection, vec![id0]);
    b.touches = Modifiers::NONE;
    // Un clic dans le vide lève le choix.
    let vide = b.app.vignettes.view.right_bottom() - vec2(8.0, 3.0);
    clic(&mut b, vide);
    assert!(b.app.vignettes.annot.selection.is_empty());
    assert_eq!(pas_d_annulation(&b), etapes, "choisir ne laisse aucune étape");
}

fn clic_avec_maj(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true, Modifiers::SHIFT)], 1.0 / 60.0);
    b.image(vec![appui(p, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.chauffer(2);
}

#[test]
fn a_corner_scales_and_the_outside_of_a_corner_rotates_each_in_one_undo_step() {
    let mut b = ouvert(quatre_plans(), 3);
    let id = poser(&mut b, None, 1, 40.0);
    lasso(&mut b);
    entourer_trait(&mut b, &[id], 14.0);
    assert_eq!(b.app.vignettes.annot.selection, vec![id]);
    let avant = points(&b, id);
    let etapes = pas_d_annulation(&b);
    // Le coin haut gauche, tiré vers l'extérieur : le trait grandit (le coin bas droit reste en place).
    let r = boite(&b, &[id]);
    glisser(&mut b, &[r.left_top(), r.left_top() + vec2(-8.0, -8.0), r.left_top() + vec2(-20.0, -20.0), r.left_top() + vec2(-30.0, -30.0)]);
    assert_eq!(pas_d_annulation(&b), etapes + 1);
    assert_eq!(dernier_nom(&b), "Mettre à l'échelle des annotations");
    let grand = points(&b, id);
    let (l0, l1) = (crate::board::points_bounds(&avant).size(), crate::board::points_bounds(&grand).size());
    assert!(l1.x > l0.x * 1.2 && l1.y > l0.y * 1.2, "le trait a grandi : {l0:?} -> {l1:?}");
    assert!(grand[3].r > avant[3].r * 1.2, "l'épaisseur suit l'échelle");
    // Hors d'un coin : tourner autour du centre de la boîte (de 60°).
    let r = boite(&b, &[id]);
    let (c, coin) = (r.center(), r.left_top());
    let dehors = coin + (coin - c).normalized() * 12.0;
    let v = dehors - c;
    let tourne = |a: f32| c + vec2(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos());
    let (a1, a2, a3) = (20f32.to_radians(), 40f32.to_radians(), 60f32.to_radians());
    glisser(&mut b, &[dehors, tourne(a1), tourne(a2), tourne(a3)]);
    assert_eq!(pas_d_annulation(&b), etapes + 2);
    assert_eq!(dernier_nom(&b), "Tourner des annotations");
    let apres = points(&b, id);
    let corde = |p: &[VPoint]| p[8].pos() - p[0].pos();
    let (u, w) = (corde(&grand), corde(&apres));
    assert!((u.length() - w.length()).abs() < 0.5, "tourner ne change pas les distances : {} contre {}", u.length(), w.length());
    let angle = (u.x * w.y - u.y * w.x).atan2(u.dot(w)).to_degrees();
    assert!((angle - 60.0).abs() < 3.0, "tourné de 60° : {angle}");
    // Annuler deux fois : le trait est revenu où il était.
    b.app.run_action("edition.annuler");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert!(proches(&points(&b, id), &avant, 1e-2));
}

#[test]
fn escape_abandons_a_gesture_without_changing_anything_then_clears_the_selection() {
    let mut b = ouvert(quatre_plans(), 3);
    let id = poser(&mut b, None, 2, 40.0);
    lasso(&mut b);
    entourer_trait(&mut b, &[id], 14.0);
    assert_eq!(b.app.vignettes.annot.selection, vec![id]);
    let (avant, etapes) = (points(&b, id), pas_d_annulation(&b));
    // Un déplacement commencé : Échap, puis on lâche. Rien n'a bougé, aucune étape.
    let r = boite(&b, &[id]);
    let prise = r.center() + vec2(r.width() * 0.25, 0.0);
    b.image(vec![Event::PointerMoved(prise)], 1.0 / 60.0);
    b.image(vec![appui(prise, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=4 {
        b.image(vec![Event::PointerMoved(prise + vec2(8.0 * k as f32, 3.0 * k as f32))], 1.0 / 120.0);
    }
    b.image(vec![echap()], 1.0 / 60.0);
    b.image(vec![Event::PointerMoved(prise + vec2(60.0, 30.0))], 1.0 / 120.0);
    b.image(vec![appui(prise + vec2(60.0, 30.0), false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(2);
    assert!(proches(&points(&b, id), &avant, 1e-3), "rien n'a bougé");
    assert_eq!(pas_d_annulation(&b), etapes, "aucune étape d'annulation");
    assert_eq!(b.app.vignettes.annot.selection, vec![id], "le trait est toujours choisi");
    // Un lasso en cours : Échap l'abandonne, la sélection d'avant reste.
    let vide = b.app.vignettes.view.right_bottom() - vec2(8.0, 3.0);
    b.image(vec![Event::PointerMoved(vide)], 1.0 / 60.0);
    b.image(vec![appui(vide, true, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![Event::PointerMoved(vide + vec2(-60.0, 10.0))], 1.0 / 120.0);
    b.image(vec![echap()], 1.0 / 60.0);
    b.image(vec![appui(vide + vec2(-60.0, 10.0), false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.vignettes.annot.selection, vec![id], "le lasso abandonné n'a rien changé au choix");
    // Échap sans geste : la sélection est levée.
    b.image(vec![echap()], 1.0 / 60.0);
    b.chauffer(2);
    assert!(b.app.vignettes.annot.selection.is_empty());
    assert_eq!(pas_d_annulation(&b), etapes);
}

#[test]
fn the_tools_act_on_the_targeted_layer_only() {
    let mut b = ouvert(quatre_plans(), 3);
    let a = b.app.doc.annotations.calque_ou_premier(None);
    let c = b.app.doc.annotations.ajouter_calque().unwrap();
    // Un trait par calque, au même endroit (la case 1).
    let ta = poser(&mut b, Some(a), 1, 40.0);
    let tc = poser(&mut b, Some(c), 1, 40.0);
    // Viser le calque `a` : le lasso ne prend que son trait.
    b.app.annot_viser_le_calque(a);
    b.chauffer(2);
    assert_eq!(b.app.vignettes.annot.outil, Some(crate::annotationsui::Outil::Stylo), "viser un calque prend le stylo");
    lasso(&mut b);
    entourer_trait(&mut b, &[ta], 20.0);
    assert_eq!(b.app.vignettes.annot.selection, vec![ta]);
    // Viser `c` : ce qui était choisi est lâché, et le lasso ne prend que le trait de `c`.
    b.app.annot_viser_le_calque(c);
    b.chauffer(2);
    assert!(b.app.vignettes.annot.selection.is_empty());
    assert_eq!(b.app.vignettes.annot.outil, Some(crate::annotationsui::Outil::Lasso), "l'outil reste le même");
    entourer_trait(&mut b, &[tc], 20.0);
    assert_eq!(b.app.vignettes.annot.selection, vec![tc]);
    // La gomme n'efface que dans le calque visé : sur les deux traits superposés, seul celui de `c` part.
    b.app.run_action("vignettes.annotation_gomme");
    b.chauffer(2);
    let milieu = b.app.vignettes.xf.to_screen(points(&b, tc)[4].pos());
    glisser(&mut b, &[milieu - vec2(6.0, 0.0), milieu, milieu + vec2(6.0, 0.0)]);
    assert_eq!(b.app.doc.annotations.traces.len(), 1);
    assert_eq!(b.app.doc.annotations.traces[0].calque, a, "le trait de l'autre calque est resté");
    // Le stylo pose son trait dans le calque visé.
    b.app.annot_viser_le_calque(c);
    b.app.run_action("vignettes.annotation_stylo");
    b.app.annot_viser_le_calque(c);
    b.chauffer(2);
    assert_eq!(b.app.vignettes.annot.outil, Some(crate::annotationsui::Outil::Stylo));
    let (p, q) = (vignette(&b, 4).center(), vignette(&b, 5).center());
    glisser(&mut b, &[p, p + vec2(20.0, 5.0), q]);
    let dernier = b.app.doc.annotations.traces.last().unwrap();
    assert_eq!(dernier.calque, c, "tracé dans le calque visé");
    // Viser « Dessin » range les outils d'annotation : le pinceau dessine de nouveau dans la case.
    b.app.annot_viser_le_dessin();
    assert!(!b.app.annot_actif());
    // (La case 3 est la vignette du plan 1.)
    let (n, case) = (b.app.doc.annotations.traces.len(), vignette(&b, 3).center());
    glisser(&mut b, &[case - vec2(20.0, 0.0), case, case + vec2(20.0, 6.0)]);
    assert_eq!(b.app.doc.annotations.traces.len(), n, "aucune annotation de plus");
    assert!(crate::vignettes_tests::encre(&b.app.doc, 1), "le pinceau a dessiné dans la case");
}

#[test]
fn a_layer_can_be_shown_or_hidden_alone_and_nothing_is_erased() {
    // (On compte les formes peintes : les vignettes des plans, calculées sur des fils de fond communs à tous les essais, s'y
    // ajoutent en arrivant, et un compte pris avant leur arrivée et un autre après ne se comparent pas — « 553 contre 553 ».
    // État commun pour cet essai seul, et on attend les vignettes avant chaque compte.)
    crate::fxcache::monde::ecrire();
    let mut b = ouvert(quatre_plans(), 3);
    let a = b.app.doc.annotations.calque_ou_premier(None);
    let c = b.app.doc.annotations.ajouter_calque().unwrap();
    poser(&mut b, Some(a), 0, 40.0);
    poser(&mut b, Some(c), 4, 40.0);
    b.garder_formes = true;
    b.attendre_vignettes();
    let deux = b.formes.len();
    b.app.annot_calque_basculer(c);
    b.attendre_vignettes();
    let un = b.formes.len();
    assert!(un < deux, "un calque caché n'est pas peint ({un} contre {deux})");
    b.app.annot_calque_basculer(a);
    b.attendre_vignettes();
    assert!(b.formes.len() < un, "les deux cachés : moins encore ({} contre {un})", b.formes.len());
    assert_eq!(b.app.doc.annotations.traces.len(), 2, "rien n'est effacé");
    assert!(b.app.doc.annotations.calques.iter().all(|k| !k.visible));
    // Viser un calque caché le remontre : on ne trace pas sur ce qu'on ne voit pas.
    b.app.annot_viser_le_calque(a);
    b.chauffer(2);
    assert!(b.app.doc.annotations.calque(a).unwrap().visible);
    assert!(!b.app.doc.annotations.calque(c).unwrap().visible, "l'autre reste caché");
    // Cacher n'est pas une étape d'annulation (c'est un affichage), mais le projet est marqué modifié : l'œil est enregistré.
    let etapes = pas_d_annulation(&b);
    b.app.modified = false;
    b.app.annot_calque_basculer(c);
    assert_eq!(pas_d_annulation(&b), etapes);
    assert!(b.app.modified);
    // Un calque caché n'est ni dans l'export de la planche, ni dans son cadrage.
    let layout = b.app.board_layout();
    assert!(b.app.doc.annotations.etendue(&layout).is_some());
    b.app.doc.annotations.regler_visible(a, false);
    b.app.doc.annotations.regler_visible(c, false);
    assert!(b.app.doc.annotations.etendue(&layout).is_none());
    assert!(b.app.doc.annotations.ordre_de_peinture().is_empty());
}

#[test]
fn layers_are_added_renamed_deleted_and_reordered_each_in_one_undo_step() {
    let mut b = ouvert(quatre_plans(), 3);
    let a = b.app.doc.annotations.calque_ou_premier(None);
    let t = poser(&mut b, Some(a), 2, 40.0);
    let etapes = pas_d_annulation(&b);
    // Ajouter : un calque de plus, visé, qui prend le stylo.
    b.app.run_action("vignettes.annotation_calque_nouveau");
    b.chauffer(2);
    assert_eq!((pas_d_annulation(&b), dernier_nom(&b)), (etapes + 1, "Ajouter un calque d'annotation".to_string()));
    assert_eq!(b.app.doc.annotations.calques.len(), 2);
    let c = b.app.doc.annotations.calques[1].id;
    assert_eq!(b.app.annot_calque_actif(), Some(c));
    assert!(b.app.annot_actif());
    // Renommer.
    b.app.annot_calque_renommer(c, "Corrections de Sam");
    assert_eq!(dernier_nom(&b), "Renommer un calque d'annotation");
    assert_eq!(b.app.doc.annotations.calque(c).unwrap().nom, "Corrections de Sam");
    let apres_renommer = pas_d_annulation(&b);
    b.app.annot_calque_renommer(c, "Corrections de Sam");
    b.app.annot_calque_renommer(c, "   ");
    assert_eq!(pas_d_annulation(&b), apres_renommer, "même nom ou nom vide : aucune étape");
    // Monter / descendre : au bord, aucune étape.
    b.app.annot_calque_deplacer(c, true);
    assert_eq!(pas_d_annulation(&b), apres_renommer, "déjà au-dessus");
    b.app.annot_calque_deplacer(c, false);
    assert_eq!((pas_d_annulation(&b), dernier_nom(&b)), (apres_renommer + 1, "Descendre un calque d'annotation".to_string()));
    assert_eq!(b.app.doc.annotations.calques.iter().map(|k| k.id).collect::<Vec<_>>(), [c, a]);
    // Supprimer le calque `a` : son trait part avec lui.
    b.app.annot_calque_supprimer(a);
    assert_eq!(dernier_nom(&b), "Supprimer un calque d'annotation");
    assert!(b.app.doc.annotations.traces.is_empty() && b.app.doc.annotations.calques.len() == 1);
    // Ctrl+Z, un par un : le calque et son trait reviennent, puis l'ordre, le nom, le calque ajouté.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!((b.app.doc.annotations.calques.len(), b.app.doc.annotations.traces.len()), (2, 1), "le calque supprimé revient avec son trait");
    assert_eq!(b.app.doc.annotations.traces[0].id, t);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.annotations.calques.iter().map(|k| k.id).collect::<Vec<_>>(), [a, c]);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.annotations.calque(c).unwrap().nom, "Annotations 2");
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.annotations.calques.len(), 1, "le calque ajouté est parti");
    assert_eq!(pas_d_annulation(&b), etapes);
    // Rétablir les rejoue.
    for _ in 0..4 {
        b.app.run_action("edition.retablir");
    }
    b.chauffer(2);
    assert_eq!(b.app.doc.annotations.calques.len(), 1);
    assert_eq!(b.app.doc.annotations.calques[0].id, c);
}

#[test]
fn the_planche_panel_lists_the_layers_with_the_drawing_below_and_one_click_switches_the_target() {
    let mut b = ouvert_vignettes(quatre_plans(), 3);
    assert!(b.app.dock_layout().contains(crate::dock::Pane::Vignettes), "le panneau Planche est à l'écran avec l'onglet");
    let a = b.app.doc.annotations.calque_ou_premier(None);
    let c = b.app.doc.annotations.ajouter_calque().unwrap();
    b.app.doc.annotations.renommer_calque(c, "Reprises");
    poser(&mut b, Some(a), 0, 40.0);
    b.garder_formes = true;
    b.chauffer(3);
    let reprises = texte_en(&b, "Reprises").expect("le calque « Reprises » est listé");
    let annotations = texte_en(&b, "Annotations").expect("le calque « Annotations » est listé");
    let dessin = texte_pres_de(&b, "Dessin", annotations.x).expect("« Dessin » est listé");
    assert!(texte_en(&b, "Calques").is_some());
    assert!(reprises.y < annotations.y && annotations.y < dessin.y, "le dernier calque est dessus, « Dessin » tout en bas");
    // Au départ aucun outil d'annotation : le dessin de la case est visé.
    assert!(!b.app.annot_actif());
    // Un clic sur « Reprises » le vise : le stylo est pris.
    clic(&mut b, reprises);
    assert_eq!(b.app.annot_calque_actif(), Some(c));
    assert!(b.app.annot_actif(), "le stylo est pris");
    // Un clic sur « Annotations » : l'autre calque, l'outil ne change pas.
    b.app.run_action("vignettes.annotation_lasso");
    b.chauffer(2);
    let annotations = texte_en(&b, "Annotations").unwrap();
    clic(&mut b, annotations);
    assert_eq!(b.app.annot_calque_actif(), Some(a));
    assert_eq!(b.app.vignettes.annot.outil, Some(crate::annotationsui::Outil::Lasso));
    // Un clic sur « Dessin » range les outils d'annotation.
    let ax = texte_en(&b, "Annotations").unwrap().x;
    let dessin = texte_pres_de(&b, "Dessin", ax).unwrap();
    clic(&mut b, dessin);
    assert!(!b.app.annot_actif());
    // L'œil d'un calque : le premier œil de la liste est celui du calque du dessus.
    let yeux = textes_en(&b, egui_phosphor::regular::EYE);
    assert_eq!(yeux.len(), 2, "un œil par calque");
    clic(&mut b, yeux[0]);
    assert!(!b.app.doc.annotations.calque(c).unwrap().visible, "l'œil du dessus cache « Reprises »");
    assert!(b.app.doc.annotations.calque(a).unwrap().visible);
    assert_eq!(b.app.doc.annotations.traces.len(), 1, "rien n'est effacé");
    // Le panneau n'est que dans l'espace Vignettes : on ne le voit pas dans une autre disposition.
    assert!(b.app.vignettes.annot.renomme.is_none());
}

#[test]
fn a_double_click_renames_a_layer_in_the_panel() {
    let mut b = ouvert_vignettes(quatre_plans(), 3);
    let a = b.app.doc.annotations.calque_ou_premier(None);
    poser(&mut b, Some(a), 0, 40.0);
    b.garder_formes = true;
    b.chauffer(3);
    let p = texte_en(&b, "Annotations").expect("listé");
    clic(&mut b, p);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    for _ in 0..2 {
        b.image(vec![appui(p, true, Modifiers::NONE)], 1.0 / 60.0);
        b.image(vec![appui(p, false, Modifiers::NONE)], 1.0 / 60.0);
    }
    b.chauffer(2);
    assert_eq!(b.app.vignettes.annot.renomme.as_ref().map(|r| r.0), Some(a), "le champ de renommage est ouvert");
    // On valide un nom : Entrée.
    b.app.vignettes.annot.renomme = Some((a, "Notes de Léa".into()));
    let entree = Event::Key { key: Key::Enter, physical_key: Some(Key::Enter), pressed: true, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![entree.clone()], 1.0 / 60.0);
    b.image(vec![entree], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.doc.annotations.calque(a).unwrap().nom, "Notes de Léa");
    assert!(b.app.vignettes.annot.renomme.is_none());
    assert_eq!(dernier_nom(&b), "Renommer un calque d'annotation");
}

#[test]
fn annotation_layers_and_the_lasso_save_and_reopen_as_they_were() {
    let mut b = ouvert(quatre_plans(), 3);
    let a = b.app.doc.annotations.calque_ou_premier(None);
    let c = b.app.doc.annotations.ajouter_calque().unwrap();
    b.app.doc.annotations.renommer_calque(c, "Reprises");
    let (ta, tc) = (poser(&mut b, Some(a), 0, 40.0), poser(&mut b, Some(c), 4, 40.0));
    b.app.doc.annotations.regler_visible(c, false);
    // Un trait déplacé au lasso est enregistré où il a été posé.
    b.app.annot_viser_le_calque(a);
    lasso(&mut b);
    entourer_trait(&mut b, &[ta], 14.0);
    let r = boite(&b, &[ta]);
    let prise = r.center() + vec2(r.width() * 0.25, 0.0);
    glisser(&mut b, &[prise, prise + vec2(20.0, 20.0), prise + vec2(60.0, 40.0)]);
    let (deplace, autre) = (points(&b, ta), points(&b, tc));
    let dir = std::env::temp_dir().join(format!("shot-test-annot-calques-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join("P.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let texte = std::fs::read_to_string(dir.join(crate::annotations::FILE)).unwrap();
    assert!(texte.contains("\"version\": 2") && texte.contains("Reprises"), "{texte}");
    let relu = crate::project::load(&manifeste).expect("ouverture");
    let (x, y) = (&b.app.doc.annotations, &relu.doc.annotations);
    // (Le fichier arrondit les points à quatre décimales : proches ; tout le reste est identique.)
    assert_eq!(x.calques, y.calques, "les calques, leur ordre, leur nom, leur œil");
    assert_eq!(x.traces.len(), y.traces.len());
    assert!(x.traces.iter().zip(&y.traces).all(|(p, q)| p.calque == q.calque && p.anchor == q.anchor && p.color == q.color), "les traits dans l'ordre, chacun dans son calque");
    assert!(!y.calque(c).unwrap().visible, "l'œil fermé est gardé");
    // Les traits sont toujours aux mêmes places sur la planche.
    let layout = b.app.board_layout();
    for (i, t) in x.traces.iter().enumerate() {
        let (p, q) = (x.points_planche(&layout, i).unwrap(), y.points_planche(&layout, i).unwrap());
        assert!(proches(&p, &q, 1e-2), "le trait {i} de {:?} est au même endroit", t.calque);
    }
    assert!(proches(&y.points_planche_de(&layout, y.traces[0].id).unwrap(), &deplace, 1e-2));
    assert!(proches(&y.points_planche_de(&layout, y.traces[1].id).unwrap(), &autre, 1e-2));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_lasso_and_the_layers_cost_nothing_at_rest_and_the_board_goes_idle() {
    let mut b = ouvert(quatre_plans(), 3);
    let a = b.app.doc.annotations.calque_ou_premier(None);
    let ids: Vec<u64> = (0..6).map(|k| poser(&mut b, Some(a), k, 30.0)).collect();
    lasso(&mut b);
    entourer_trait(&mut b, &ids[..2], 40.0);
    assert!(!b.app.vignettes.annot.selection.is_empty());
    assert!(crate::perf_tests::se_reposer(&mut b, 60) < 60, "l'écran se repose avec une sélection et sa boîte");
}

