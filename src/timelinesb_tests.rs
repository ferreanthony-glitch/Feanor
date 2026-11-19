//! Partie 178 : la timeline de l'espace Storyboard dans l'organisation dessinée par Anthony (`timelinesb.rs`, `docs/INTERFACE.md`).
//!
//! 1. L'ordre et la hauteur des rangées : séquences (16), plans (26), caméra (22) et ses préréglages (24, dépliée), calques ;
//!    l'espace Montage garde sa disposition.
//! 2. Les icônes des en-têtes des plans et de la caméra, et ce qu'elles font.
//! 3. Une transition : son symbole sur la coupe, son menu au clic, qui la change (annulable) ; un préréglage de transition glissé.
//! 4. La caméra repliée : ses clés sur la ligne ; dépliée : la rangée des préréglages, deux rangs bas quand deux se chevauchent.
//! 5. Poser un préréglage de caméra dans le Storyboard, par glisser (caméra repliée ou dépliée) et par clic : comme dans l'Animation.
//! 6. Un bloc qu'on déplace, étire, enlève (Suppr), convertit en clés (les clés sur la ligne de la caméra).
//! 7. Les calques repliés (une ligne de résumé) et dépliés ; le raccourci ; l'opacité d'un calque.
//! 8. Les traits de coupe traversent la caméra, ses préréglages et les calques.
//! 9. À la plus petite fenêtre, rien ne déborde des en-têtes.

use eframe::egui::{epaint::Shape, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::bulles::vus;
use crate::camera::{Camera, CameraKey};
use crate::camfx::{Params, Preset, Vivant};
use crate::document::{Cel, Document, LayerKind};
use crate::montagerows::{Rows, SB_CALQUES_H, SB_CAM_H, SB_PLAN_H, SB_PREREGLAGES_H, SB_SEQ_H};
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::{clic, doc_with, etapes, losanges, textes};
use crate::storyboard::RULER_H;
use crate::transitions::Kind;
use crate::workspace::Espace;

const DT: f64 = 1.0 / 60.0;
/// La largeur d'une image dans la bande (points).
const PPF: f32 = 8.0;

/// Trois plans de 24 images : trois calques dessinés, deux, un.
pub(crate) fn film() -> Document {
    let mut doc = doc_with(&[&["Décor", "Perso", "Ombre"], &["Seul", "Fond"], &["Trois"]]);
    for pi in 0..doc.panels.len() {
        for li in 0..doc.panels[pi].layers.len() {
            let a = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
            let b = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
            doc.panels[pi].layers[li].frames = (0..24).map(|f| Some(if f < 8 + 2 * li { a } else { b })).collect();
        }
    }
    doc
}

fn effet(preset: Preset, start: usize, duration: usize) -> Vivant {
    Vivant { params: Params { duration, ..Params::new(preset, 24) }, start }.sanitized(24)
}

/// Un banc dans l'espace Storyboard, à la taille `w` × `h`, les formes gardées.
pub(crate) fn banc_sb(doc: Document, w: u32, h: u32, camera_depliee: bool) -> Banc {
    let mut b = banc_taille(doc, false, w, h);
    b.app.set_espace(Espace::Storyboard);
    b.app.settings.storyboard_camera_depliee = camera_depliee;
    b.app.settings.storyboard_calques_replies = false;
    // (Partie 179.10 : ces essais décrivent la piste dépliée du calque actif, que « Déplier le calque actif » montre ; d'origine,
    // elle a la hauteur des autres : `deplier_calque_tests`.)
    b.app.settings.deplier_calque_actif = true;
    b.app.montage_zoom = PPF;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

pub(crate) fn blocs() -> Vec<Rect> {
    crate::storyboard::BLOCS_DU_MONTAGE.with(|b| b.borrow().clone())
}

pub(crate) fn rangees(b: &Banc) -> Rows {
    b.app.montage_rows(300.0)
}

/// Le haut de la bande à l'écran (le bloc d'un plan commence 2 points sous le haut de la rangée des plans).
pub(crate) fn haut(b: &Banc) -> f32 {
    blocs()[0].top() - 2.0 - rangees(b).blocks_top()
}

/// L'abscisse du milieu de l'image `f` du plan `pi`.
fn x_image(pi: usize, f: usize) -> f32 {
    blocs()[pi].left() - 1.0 + (f as f32 + 0.5) * PPF
}

/// Le seul élément à l'écran dont la bulle est `nom`, dans la bande horizontale `y0..y1` de l'écran.
pub(crate) fn ou_dans(nom: &str, y0: f32, y1: f32) -> Rect {
    let v: Vec<Rect> = vus::ou(nom).into_iter().filter(|r| r.center().y > y0 && r.center().y < y1).collect();
    assert_eq!(v.len(), 1, "« {nom} » : un seul entre {y0} et {y1} : {:?} ({:?})", vus::ou(nom), vus::noms());
    v[0]
}

pub(crate) fn colonne() -> Rect {
    let v = vus::ou("Largeur de la colonne");
    assert_eq!(v.len(), 1, "la poignée de la colonne");
    v[0]
}

fn bouton(p: Pos2, pressed: bool, button: PointerButton) -> Event {
    Event::PointerButton { pos: p, button, pressed, modifiers: Modifiers::NONE }
}

fn clic_droit(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![bouton(p, true, PointerButton::Secondary)], DT);
    b.image(vec![bouton(p, false, PointerButton::Secondary)], DT);
    b.chauffer(3);
}

/// Prend en `de`, tire jusqu'en `vers` en huit pas, lâche (comme `blocscamera_tests`).
fn glisser(b: &mut Banc, de: Pos2, vers: Pos2) {
    b.image(vec![Event::PointerMoved(de)], DT);
    b.image(vec![bouton(de, true, PointerButton::Primary)], DT);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(de + (vers - de) * (k as f32 / 8.0))], DT);
    }
    b.image(vec![bouton(vers, false, PointerButton::Primary)], DT);
    b.chauffer(3);
}

/// Le pointeur se pose en `p` le temps qu'un sous-menu s'ouvre.
fn survoler(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p - vec2(0.0, 1.0))], DT);
    b.chauffer(3);
    b.image(vec![Event::PointerMoved(p)], DT);
    b.chauffer(12);
}

/// Le texte `mot` peint à l'écran (le plus bas s'il y en a plusieurs : un menu passe par-dessus).
fn texte(b: &Banc, mot: &str) -> Option<Rect> {
    textes(b).into_iter().filter(|(t, _)| t == mot).map(|(_, r)| r).max_by(|a, c| a.top().total_cmp(&c.top()))
}

/// Les blocs de préréglages peints par la dernière image : les rectangles pleins à la couleur de la caméra, d'au moins 8 points de
/// haut (le trait fin de la caméra repliée n'en est pas un).
fn blocs_peints(b: &Banc) -> Vec<Rect> {
    fn chercher(s: &Shape, out: &mut Vec<Rect>) {
        match s {
            Shape::Rect(r) if r.fill == crate::theme::camera() && r.rect.height() >= 8.0 => out.push(r.rect),
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut out);
    }
    out.sort_by(|a, c| a.left().total_cmp(&c.left()));
    out
}

/// Les traits verticaux peints par la dernière image : (x, haut, bas).
fn traits_verticaux(b: &Banc) -> Vec<(f32, f32, f32)> {
    fn chercher(s: &Shape, out: &mut Vec<(f32, f32, f32)>) {
        match s {
            Shape::LineSegment { points, .. } if (points[0].x - points[1].x).abs() < 0.01 => out.push((points[0].x, points[0].y.min(points[1].y), points[0].y.max(points[1].y))),
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut out);
    }
    out
}

fn cameras(doc: &Document, pi: usize) -> Vec<Camera> {
    (0..doc.panels[pi].frame_count()).map(|f| doc.panel_camera(pi, f)).collect()
}

// ------------------------------------------------------------------------------------------------ 1. l'ordre et les hauteurs

#[test]
fn les_rangees_du_storyboard_sont_dans_son_ordre_et_aussi_fines_qu_il_l_a_demande() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    let rows = rangees(&b);
    let sb = rows.sb.expect("la disposition du Storyboard");
    assert_eq!(rows.bands_h(), SB_SEQ_H, "la ligne des séquences, la plus fine");
    // (Partie 179.11 : la ligne des séquences a son en-tête et ses icônes : 20, 16 avant.)
    assert_eq!((SB_SEQ_H, SB_PLAN_H, SB_CAM_H, SB_PREREGLAGES_H), (20.0, 26.0, 22.0, 24.0), "les hauteurs de la maquette");
    assert_eq!(rows.block_h, SB_PLAN_H, "la rangée des plans");
    assert_eq!(rows.cam_h(), SB_CAM_H, "la ligne de la caméra");
    assert_eq!(sb.prereglages_h, 0.0, "repliée d'office : pas de rangée des préréglages");
    // De haut en bas : la règle, les séquences, les plans, la caméra, la ligne « Calques », les calques.
    assert!(rows.regle_h() + rows.bands_h() <= rows.blocks_top() + 0.01 && rows.regle_h() >= RULER_H);
    assert!(rows.blocks_top() + rows.block_h <= rows.camera_top() + 0.01, "la caméra sous les plans");
    assert!(rows.camera_top() + rows.cam_h() <= rows.calques_top() + 0.01);
    assert!(rows.calques_top() + SB_CALQUES_H <= rows.layers_top() + 0.01);
    // Plus contenue qu'avant (la caméra de 16 au-dessus de plans de 56 : les calques commençaient à 111 points).
    // (Partie 179 : plus de seconde ligne de boutons, plus de bande sous la règle.)
    // (Partie 179.11 : la ligne « Séquence » et la barre de lecture en face de la règle : 8 points de plus, 111.)
    assert!(rows.layers_top() <= 111.5, "le haut des calques : {}", rows.layers_top());
    // À l'écran : les plans, leurs en-têtes et celui de la caméra sur ces rangées.
    let h = haut(&b);
    assert!(blocs().iter().all(|r| (r.height() - (SB_PLAN_H - 2.0)).abs() < 0.6), "des plans de 24 points : {:?}", blocs());
    let col = colonne();
    let np = ou_dans("Nouveau plan", h + rows.blocks_top(), h + rows.blocks_top() + rows.block_h);
    assert!(np.right() <= col.right() + 0.5);
    ou_dans("Déplier la caméra", h + rows.camera_top(), h + rows.camera_top() + rows.cam_h());
    ou_dans("Replier la rangée Calques", h + rows.calques_top(), h + rows.calques_top() + SB_CALQUES_H);
    // Dépliée : la rangée des préréglages, entre la caméra et les calques.
    b.app.settings.storyboard_camera_depliee = true;
    b.chauffer(3);
    let rows = rangees(&b);
    assert_eq!(rows.prereglages_h(), SB_PREREGLAGES_H);
    assert!((rows.calques_top() - (rows.camera_top() + SB_CAM_H + SB_PREREGLAGES_H)).abs() < 0.01);
    let h = haut(&b);
    let pr = ou_dans("Préréglages de caméra", h + rows.prereglages_top(), h + rows.prereglages_top() + SB_PREREGLAGES_H);
    assert!(pr.height() >= SB_PREREGLAGES_H - 3.0, "l'en-tête de la rangée des préréglages : {pr:?}");
    // L'espace Montage garde la sienne : la caméra au-dessus des plans, les plans à la hauteur du réglage.
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    let rows = rangees(&b);
    assert!(rows.sb.is_none());
    assert!(rows.camera_top() < rows.blocks_top(), "Montage : la caméra au-dessus des plans");
    assert_eq!(rows.block_h, b.app.settings.montage_plans.points());
    assert!(vus::ou("Déplier la caméra").is_empty() && vus::ou("Replier la rangée Calques").is_empty(), "rien de neuf dans le Montage");
}

// ------------------------------------------------------------------------------------------------ 2. les icônes des en-têtes

const ICONES_DES_PLANS: [&str; 6] = ["Nouveau plan", "Dupliquer le plan", "Versions du plan", "Transitions", "Couper le plan à la tête de lecture", "Autres commandes des plans"];
const ICONES_DE_LA_CAMERA: [&str; 5] = ["Déplier la caméra", "Clé de caméra ici", "Préréglages de caméra", "Remettre la caméra à zéro", "Autres commandes de la caméra"];

/// Les icônes `noms` de la rangée `y0..y1`, dans cet ordre de gauche à droite, sans se toucher, dans la colonne.
fn icones_en_ordre(noms: &[&str], y0: f32, y1: f32, quoi: &str) -> Vec<Rect> {
    let col = colonne();
    let v: Vec<Rect> = noms.iter().map(|n| ou_dans(n, y0, y1)).collect();
    for (k, r) in v.iter().enumerate() {
        assert!(r.right() <= col.right() + 0.5 && r.left() >= col.left() - 400.0, "{quoi} : « {} » dans la colonne : {r:?} / {col:?}", noms[k]);
        assert!(r.top() >= y0 - 0.5 && r.bottom() <= y1 + 0.5, "{quoi} : « {} » dans sa rangée : {r:?} / {y0}..{y1}", noms[k]);
        if k > 0 {
            assert!(r.left() >= v[k - 1].right() - 0.5, "{quoi} : « {} » après « {} » sans la toucher : {v:?}", noms[k], noms[k - 1]);
        }
    }
    v
}

#[test]
fn les_icones_des_en_tetes_des_plans_et_de_la_camera_lancent_leurs_actions() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    let rows = rangees(&b);
    let ligne_plans = |b: &Banc| {
        let (h, r) = (haut(b), rangees(b));
        (h + r.blocks_top(), h + r.blocks_top() + r.block_h)
    };
    let ligne_camera = |b: &Banc| {
        let (h, r) = (haut(b), rangees(b));
        (h + r.camera_top(), h + r.camera_top() + r.cam_h())
    };
    let (a, z) = ligne_plans(&b);
    icones_en_ordre(&ICONES_DES_PLANS, a, z, "plans");
    let (a, z) = ligne_camera(&b);
    icones_en_ordre(&ICONES_DE_LA_CAMERA, a, z, "caméra");
    assert!(rows.sb.is_some());
    // Nouveau plan, dupliquer : un plan de plus, une étape chacun.
    let n = b.app.doc.panels.len();
    let e = etapes(&b);
    let (a, z) = ligne_plans(&b);
    clic(&mut b, ou_dans("Nouveau plan", a, z).center(), Modifiers::NONE);
    assert_eq!((b.app.doc.panels.len(), etapes(&b)), (n + 1, e + 1), "nouveau plan : {}", b.app.status);
    let (a, z) = ligne_plans(&b);
    clic(&mut b, ou_dans("Dupliquer le plan", a, z).center(), Modifiers::NONE);
    assert_eq!((b.app.doc.panels.len(), etapes(&b)), (n + 2, e + 2), "dupliquer : {}", b.app.status);
    // Couper le plan à la tête de lecture.
    b.app.goto_global(10);
    b.chauffer(2);
    let (a, z) = ligne_plans(&b);
    clic(&mut b, ou_dans("Couper le plan à la tête de lecture", a, z).center(), Modifiers::NONE);
    assert_eq!(b.app.doc.panels.len(), n + 3, "couper : {}", b.app.status);
    assert_eq!(b.app.doc.panels[0].frame_count(), 10, "le plan coupé à l'image 10");
    // Transitions : le panneau Préréglages, onglet Montage.
    let (a, z) = ligne_plans(&b);
    clic(&mut b, ou_dans("Transitions", a, z).center(), Modifiers::NONE);
    assert!(b.app.pane_shown(crate::dock::Pane::Prereglages), "le panneau Préréglages");
    assert_eq!(crate::panneauprereglages::onglet(&b.app.ctx), crate::panneauprereglages::Onglet::Montage);
    // Les menus : « ⋯ » des plans, les versions.
    let (a, z) = ligne_plans(&b);
    let menu = ou_dans("Autres commandes des plans", a, z).center();
    clic(&mut b, menu, Modifiers::NONE);
    for mot in ["Durée du plan…", "Nom et fiche du plan…", "Réunir le plan et le suivant", "Réglages du projet…", "Verrouiller le storyboard"] {
        assert!(texte(&b, mot).is_some(), "« {mot} » dans le menu des plans : {:?}", textes(&b).into_iter().map(|t| t.0).collect::<Vec<_>>());
    }
    clic(&mut b, menu, Modifiers::NONE);
    let (a, z) = ligne_plans(&b);
    let menu = ou_dans("Versions du plan", a, z).center();
    clic(&mut b, menu, Modifiers::NONE);
    assert!(textes(&b).iter().any(|(t, _)| t.starts_with("Version du plan")), "le menu des versions");
    clic(&mut b, menu, Modifiers::NONE);
    // La caméra : une clé ici, sur la ligne de la caméra.
    b.app.goto_global(3);
    b.chauffer(2);
    let e = etapes(&b);
    let (a, z) = ligne_camera(&b);
    clic(&mut b, ou_dans("Clé de caméra ici", a, z).center(), Modifiers::NONE);
    let pi = b.app.cur_panel;
    assert!(b.app.doc.panels[pi].camera.iter().any(|k| k.frame == 3), "une clé de caméra à l'image 3 : {}", b.app.status);
    assert_eq!(etapes(&b), e + 1);
    let (a, z) = ligne_camera(&b);
    let losange = losanges(&b, crate::theme::camera());
    assert!(losange.iter().any(|l| l.center().y > a && l.center().y < z && (l.center().x - x_image(pi, 3)).abs() < 1.0), "son losange sur la ligne : {losange:?}");
    // Remettre la caméra à zéro : les clés s'en vont, une étape.
    clic(&mut b, ou_dans("Remettre la caméra à zéro", a, z).center(), Modifiers::NONE);
    assert!(b.app.doc.panels[pi].camera.is_empty(), "{}", b.app.status);
    assert_eq!(etapes(&b), e + 2);
    // Le « ⋯ » de la caméra.
    let (a, z) = ligne_camera(&b);
    let menu = ou_dans("Autres commandes de la caméra", a, z).center();
    clic(&mut b, menu, Modifiers::NONE);
    for mot in ["Convertir les préréglages en clés", "Copier les clés de caméra choisies", "Coller les clés de caméra"] {
        assert!(texte(&b, mot).is_some(), "« {mot} » dans le menu de la caméra");
    }
    clic(&mut b, menu, Modifiers::NONE);
    // Préréglages : le panneau, onglet Caméra, et la caméra se déplie.
    let (a, z) = ligne_camera(&b);
    clic(&mut b, ou_dans("Préréglages de caméra", a, z).center(), Modifiers::NONE);
    assert_eq!(crate::panneauprereglages::onglet(&b.app.ctx), crate::panneauprereglages::Onglet::Camera);
    assert!(b.app.settings.storyboard_camera_depliee, "la caméra dépliée pour poser des préréglages");
    assert_eq!(rangees(&b).prereglages_h(), SB_PREREGLAGES_H);
}

// ------------------------------------------------------------------------------------------------ 3. les transitions

#[test]
fn une_transition_se_voit_sur_sa_coupe_et_son_menu_la_change() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    b.app.set_transition(1, Kind::Dissolve);
    b.chauffer(4);
    let rows = rangees(&b);
    let h = haut(&b);
    let (a, z) = (h + rows.blocks_top(), h + rows.blocks_top() + rows.block_h);
    let nom = format!("Transition : {}", Kind::Dissolve.name());
    let s = ou_dans(&nom, a, z);
    let coupe = blocs()[1].left() - 1.0;
    assert!((s.center().x - coupe).abs() < 1.0, "le symbole sur la coupe : {s:?} / {coupe}");
    assert!(s.width() <= 20.0 && s.height() <= z - a, "un petit symbole, dans la rangée des plans : {s:?}");
    // Un clic sur le plan, tout près du symbole : le plan, pas la transition.
    clic(&mut b, pos2(s.left() - 12.0, s.center().y), Modifiers::NONE);
    assert_eq!(b.app.cur_panel, 0, "le plan d'avant la coupe");
    assert_eq!(b.app.trans.sel, None, "la transition n'a pas pris le clic");
    // Un clic sur le symbole : son menu ; la sorte se change, une étape d'annulation.
    clic(&mut b, s.center(), Modifiers::NONE);
    let sorte = texte(&b, &format!("Sorte : {}", Kind::Dissolve.name())).expect("le menu de la transition s'ouvre au clic");
    assert!(texte(&b, "Supprimer la transition").is_some());
    survoler(&mut b, sorte.center());
    let noir = texte(&b, Kind::Black.name()).expect("les sortes, en sous-menu");
    let e = etapes(&b);
    clic(&mut b, noir.center(), Modifiers::NONE);
    assert_eq!(b.app.doc.transitions.len(), 1);
    assert_eq!(b.app.doc.transitions[0].kind, Kind::Black, "{}", b.app.status);
    assert_eq!(etapes(&b), e + 1, "une étape");
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(b.app.doc.transitions[0].kind, Kind::Dissolve, "Ctrl+Z la rend");
    // Un préréglage de transition glissé sur une autre coupe (depuis le panneau Préréglages) : elle s'y pose.
    let p = pos2(blocs()[2].left() + 3.0, blocs()[2].center().y);
    b.image_avec(|ctx| eframe::egui::DragAndDrop::set_payload(ctx, crate::transitionsui::TransitionDrag { kind: Kind::Push }));
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![Event::PointerMoved(p + vec2(1.0, 0.0))], DT);
    b.image(vec![bouton(p + vec2(1.0, 0.0), false, PointerButton::Primary)], DT);
    b.chauffer(3);
    assert!(b.app.doc.transitions.iter().any(|t| t.kind == Kind::Push), "la poussée posée sur la coupe 2 : {:?}", b.app.doc.transitions);
    ou_dans(&format!("Transition : {}", Kind::Push.name()), a, z);
}

// ------------------------------------------------------------------------------------------------ 4. la caméra repliée, dépliée

#[test]
fn camera_repliee_ses_cles_sur_la_ligne_et_depliee_la_rangee_des_preregalges() {
    let mut doc = film();
    let c = Camera::centered(64, 48);
    doc.panels[1].camera = vec![CameraKey::new(3, c), CameraKey::new(15, Camera { x: 40.0, ..c })];
    doc.panels[1].effets_camera = vec![effet(Preset::ZoomIn, 2, 10), effet(Preset::LightShake, 6, 10)];
    let mut b = banc_sb(doc, 1920, 1080, false);
    let rows = rangees(&b);
    let h = haut(&b);
    let ligne = (h + rows.camera_top(), h + rows.camera_top() + SB_CAM_H);
    let sur_la_ligne = |b: &Banc| {
        let mut v: Vec<f32> = losanges(b, crate::theme::camera()).into_iter().filter(|l| l.center().y > ligne.0 && l.center().y < ligne.1).map(|l| l.center().x).collect();
        v.sort_by(f32::total_cmp);
        v
    };
    let attendus = [x_image(1, 3), x_image(1, 15)];
    let vus_ = sur_la_ligne(&b);
    assert_eq!(vus_.len(), 2, "les deux clés sur la ligne de la caméra : {vus_:?}");
    assert!(vus_.iter().zip(attendus).all(|(a, e)| (a - e).abs() < 1.0), "{vus_:?} / {attendus:?}");
    assert!(blocs_peints(&b).is_empty(), "repliée : pas de blocs");
    // Déplier : la rangée des préréglages, deux rangs bas (les deux préréglages se chevauchent) ; les clés restent sur la ligne.
    clic(&mut b, ou_dans("Déplier la caméra", ligne.0, ligne.1).center(), Modifiers::NONE);
    assert!(b.app.settings.storyboard_camera_depliee);
    assert!(crate::settings::Settings::load().storyboard_camera_depliee, "gardé dans les réglages du poste");
    let rows = rangees(&b);
    let sb = rows.sb.unwrap();
    assert_eq!((sb.prereglages_h, sb.rang_h), (SB_PREREGLAGES_H, SB_PREREGLAGES_H / 2.0), "deux rangs de 12 dans 24 points");
    let haut_pr = haut(&b) + rows.prereglages_top();
    let peints = blocs_peints(&b);
    assert_eq!(peints.len(), 2, "{peints:?}");
    assert!(peints.iter().all(|r| r.top() >= haut_pr && r.bottom() <= haut_pr + SB_PREREGLAGES_H && (r.height() - 10.0).abs() < 0.6), "dans la rangée, 10 points de haut : {peints:?}");
    assert!((peints[0].top() - peints[1].top()).abs() > 5.0, "sur deux rangs");
    assert!((peints[0].left() - (blocs()[1].left() - 1.0 + 2.0 * PPF)).abs() < 1.0, "le zoom part de l'image 2 du plan 2 : {peints:?}");
    assert_eq!(sur_la_ligne(&b).len(), 2, "les clés restent sur la ligne de la caméra");
    // Un troisième qui chevauche les deux : la rangée grandit d'un rang bas.
    b.app.doc.panels[1].effets_camera.push(effet(Preset::Jolt, 7, 4));
    b.chauffer(3);
    assert_eq!(rangees(&b).prereglages_h(), 36.0);
    // Replier.
    let rows = rangees(&b);
    let h = haut(&b);
    clic(&mut b, ou_dans("Replier la caméra", h + rows.camera_top(), h + rows.camera_top() + SB_CAM_H).center(), Modifiers::NONE);
    assert!(!b.app.settings.storyboard_camera_depliee);
    assert!(!crate::settings::Settings::load().storyboard_camera_depliee);
    assert!(blocs_peints(&b).is_empty());
}

// ------------------------------------------------------------------------------------------------ 5. poser un préréglage

/// Le point d'une vignette du panneau Préréglages, d'après son étiquette (comme `blocscamera_tests`).
fn vignette(b: &Banc, preset: Preset) -> Pos2 {
    let e = textes(b).into_iter().find(|(t, _)| t == preset.label()).map(|(_, r)| r).unwrap_or_else(|| panic!("la vignette « {} »", preset.label()));
    pos2(e.left() + 60.0, e.center().y - 46.0)
}

#[test]
fn un_prereglage_se_pose_dans_le_storyboard_par_glisser_et_par_clic_comme_dans_l_animation() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    crate::panneauprereglages::choisir_onglet(&b.app.ctx, crate::panneauprereglages::Onglet::Camera);
    b.chauffer(8);
    // La caméra repliée : glissé sur la ligne de la caméra du plan 2, à son image 5 (la rangée des préréglages s'ouvre pendant le glisser).
    let rows = rangees(&b);
    let cible = pos2(x_image(1, 5), haut(&b) + rows.camera_top() + SB_CAM_H * 0.5);
    let e = etapes(&b);
    let v = vignette(&b, Preset::ZoomOut);
    glisser(&mut b, v, cible);
    // Le même geste que la timeline de l'Animation : `ajouter_effet_camera` du plan, à l'image visée.
    let mut temoin = crate::pistes_tests::app_with("prereglage178", film());
    temoin.set_panel(1);
    temoin.ajouter_effet_camera(Preset::ZoomOut, 5);
    assert_eq!(b.app.doc.panels[1].effets_camera, temoin.doc.panels[1].effets_camera, "le même bloc qu'en Animation : {}", b.app.status);
    assert_eq!(b.app.cur_panel, 1, "le plan visé devient le plan courant");
    assert_eq!(etapes(&b), e + 1, "une étape d'annulation");
    assert_eq!(b.app.bloc_camera_choisi(), Some(0), "le bloc posé est choisi");
    assert!(b.app.doc.panels[1].camera.is_empty(), "aucune clé");
    assert!(b.app.doc.panels[0].effets_camera.is_empty() && b.app.doc.panels[2].effets_camera.is_empty(), "les autres plans ne bougent pas");
    assert!(!b.app.settings.storyboard_camera_depliee, "la caméra reste repliée après le glisser");
    // Un clic sur une vignette : à la tête de lecture du plan courant.
    b.app.cur_frame = 12;
    b.chauffer(2);
    let p = vignette(&b, Preset::CrashZoom);
    clic(&mut b, p, Modifiers::NONE);
    let effets = &b.app.doc.panels[1].effets_camera;
    assert_eq!(effets.len(), 2);
    assert_eq!((effets[1].params.preset, effets[1].start), (Preset::CrashZoom, 12));
    // La caméra dépliée : glissé sur la rangée des préréglages du plan 3, à son image 4.
    b.app.settings.storyboard_camera_depliee = true;
    b.chauffer(3);
    let rows = rangees(&b);
    let cible = pos2(x_image(2, 4), haut(&b) + rows.prereglages_top() + SB_PREREGLAGES_H * 0.5);
    let v = vignette(&b, Preset::ZoomIn);
    glisser(&mut b, v, cible);
    let mut temoin = crate::pistes_tests::app_with("prereglage178b", film());
    temoin.set_panel(2);
    temoin.ajouter_effet_camera(Preset::ZoomIn, 4);
    assert_eq!(b.app.doc.panels[2].effets_camera, temoin.doc.panels[2].effets_camera);
    assert_eq!(b.app.cur_panel, 2);
}

// ------------------------------------------------------------------------------------------------ 6. déplacer, étirer, Suppr, convertir

#[test]
fn un_bloc_se_deplace_s_etire_s_enleve_et_se_convertit_en_cles_sur_la_ligne_de_la_camera() {
    let mut doc = film();
    doc.panels[1].effets_camera = vec![effet(Preset::ZoomIn, 4, 12)];
    let mut b = banc_sb(doc, 1920, 1080, true);
    let r = blocs_peints(&b)[0];
    assert!((r.width() - 12.0 * PPF).abs() < 1.0, "douze images : {r:?}");
    assert!((r.height() - (SB_PREREGLAGES_H - 2.0)).abs() < 0.6, "un seul rang : toute la rangée : {r:?}");
    // Le bord droit tiré de 4 images : la durée passe à 16, le départ reste ; une étape ; le plan devient courant, le bloc choisi.
    let e = etapes(&b);
    glisser(&mut b, pos2(r.right() - 2.0, r.center().y), pos2(r.right() - 2.0 + 4.0 * PPF, r.center().y));
    let v = b.app.doc.panels[1].effets_camera[0];
    assert_eq!((v.start, v.params.duration), (4, 16), "{}", b.app.status);
    assert_eq!(etapes(&b), e + 1);
    assert_eq!((b.app.cur_panel, b.app.bloc_camera_choisi()), (1, Some(0)));
    // Le corps tiré de 2 images vers la gauche : il glisse.
    let r = blocs_peints(&b)[0];
    glisser(&mut b, r.center(), r.center() - vec2(2.0 * PPF, 0.0));
    let v = b.app.doc.panels[1].effets_camera[0];
    assert_eq!((v.start, v.params.duration), (2, 16));
    // Un clic le choisit ; Suppr l'enlève ; Ctrl+Z le rend.
    let r = blocs_peints(&b)[0];
    b.app.choisir_bloc_camera(None);
    clic(&mut b, r.center(), Modifiers::NONE);
    assert_eq!(b.app.bloc_camera_choisi(), Some(0));
    b.app.run_action("edition.supprimer");
    assert!(b.app.doc.panels[1].effets_camera.is_empty(), "Suppr : {}", b.app.status);
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(b.app.doc.panels[1].effets_camera.len(), 1);
    // Clic droit : « Convertir les effets en clés » ; les clés, avec la même image, sur la ligne de la caméra.
    let avant = cameras(&b.app.doc, 1);
    let r = blocs_peints(&b)[0];
    clic_droit(&mut b, r.center());
    let convertir = texte(&b, "Convertir les effets en clés").expect("le menu du bloc");
    let e = etapes(&b);
    clic(&mut b, convertir.center(), Modifiers::NONE);
    assert!(b.app.doc.panels[1].effets_camera.is_empty() && !b.app.doc.panels[1].camera.is_empty(), "{}", b.app.status);
    assert_eq!(cameras(&b.app.doc, 1), avant, "la même image");
    assert_eq!(etapes(&b), e + 1);
    let rows = rangees(&b);
    let ligne = (haut(&b) + rows.camera_top(), haut(&b) + rows.camera_top() + SB_CAM_H);
    let (x0, x1) = (blocs()[1].left(), blocs()[1].right());
    let n = losanges(&b, crate::theme::camera()).into_iter().filter(|l| l.center().y > ligne.0 && l.center().y < ligne.1 && l.center().x > x0 && l.center().x < x1).count();
    assert_eq!(n, b.app.doc.panels[1].camera.len(), "chaque clé sur la ligne de la caméra du plan 2");
    assert!(blocs_peints(&b).is_empty());
}

// ------------------------------------------------------------------------------------------------ 7. les calques

#[test]
fn les_calques_se_replient_en_une_ligne_de_resume_et_se_deplient() {
    let mut doc = film();
    doc.panels[0].layers[1].opacity = 0.5;
    let mut b = banc_sb(doc, 1920, 1080, false);
    assert!(!rangees(&b).lanes.is_empty());
    assert_eq!(vus::ou("Mode de fusion").len(), 1, "la piste du calque actif, comme avant");
    // Un calque à 50 % : son opacité, en petit, sur sa ligne.
    assert!(texte(&b, "50 %").is_some(), "l'opacité d'un calque à moins de 100 %");
    let rows = rangees(&b);
    let h = haut(&b);
    let ligne = (h + rows.calques_top(), h + rows.calques_top() + SB_CALQUES_H);
    // Dépliée : les commandes des calques sur la ligne « Calques ». (Partie 179.5 : les icônes des sept colonnes de la partie 174 ont
    // quitté la ligne ; la piste du calque actif montre ses réglages en icônes.)
    for nom in crate::barrecalques::NOMS {
        assert!(vus::ou(nom).iter().any(|r| r.center().y > ligne.0 && r.center().y < ligne.1), "« {nom} » sur la ligne Calques");
    }
    for col in crate::layerheads::Colonne::TOUTES {
        assert!(!vus::ou(col.nom()).iter().any(|r| r.center().y > ligne.0 && r.center().y < ligne.1), "« {} » n'est plus sur la ligne Calques", col.nom());
    }
    let hauteur_avant = rows.height(0);
    clic(&mut b, ou_dans("Replier la rangée Calques", ligne.0, ligne.1).center(), Modifiers::NONE);
    assert!(b.app.settings.storyboard_calques_replies);
    assert!(crate::settings::Settings::load().storyboard_calques_replies, "gardé dans les réglages du poste");
    let rows = rangees(&b);
    assert!(rows.lanes.is_empty(), "plus de piste");
    assert!(rows.height(0) < hauteur_avant - 60.0, "la timeline rétrécit : {} / {hauteur_avant}", rows.height(0));
    assert!(vus::ou("Mode de fusion").is_empty(), "plus de piste du calque actif");
    assert!(texte(&b, "Calques · 3").is_some(), "le résumé : le nombre de calques du plan");
    // Sur la ligne de résumé : les clés de chaque plan, en petits traits (le contour sombre de `clesplan`).
    let (y0, y1) = (haut(&b) + rows.calques_top(), haut(&b) + rows.calques_top() + SB_CALQUES_H);
    let traits = b.formes.iter().filter(|s| matches!(&s.shape, Shape::Rect(r) if r.fill == Color32::from_black_alpha(150) && r.rect.center().y > y0 && r.rect.center().y < y1)).count();
    assert!(traits >= 3, "des traits de clés sur la ligne de résumé : {traits}");
    // Le raccourci de la partie 174 (Alt+Maj+C) : dans le Storyboard, cette rangée.
    b.app.run_action(crate::layerheads::ACTION_REPLIER);
    assert!(!b.app.settings.storyboard_calques_replies);
    b.chauffer(3);
    assert!(!rangees(&b).lanes.is_empty());
    // Dans l'Animation, il replie toujours la colonne de la timeline, sans toucher au Storyboard.
    b.app.set_mode(crate::app::Mode::Animation);
    // (Partie 179.10 : c'est le réglage « Déplier le calque actif ».)
    let avant = b.app.calques_tout_replies();
    b.app.run_action(crate::layerheads::ACTION_REPLIER);
    assert_eq!((b.app.calques_tout_replies(), b.app.settings.storyboard_calques_replies), (!avant, false));
}

// ------------------------------------------------------------------------------------------------ 8. les coupes

#[test]
fn les_traits_de_coupe_traversent_la_camera_ses_prereglages_et_les_calques() {
    let mut doc = film();
    doc.panels[1].effets_camera = vec![effet(Preset::ZoomIn, 4, 12)];
    let b = banc_sb(doc, 1920, 1080, true);
    let rows = rangees(&b);
    let h = haut(&b);
    let (y0, y1) = (h + rows.camera_top(), h + rows.background_top());
    assert!(y1 - y0 > SB_CAM_H + SB_PREREGLAGES_H + SB_CALQUES_H, "des calques sous la caméra");
    let traits = traits_verticaux(&b);
    let coupes = crate::timelinesb::abscisses_des_coupes(&blocs());
    assert_eq!(coupes.len(), 4, "trois plans : quatre coupes, bords compris");
    for x in coupes {
        assert!(traits.iter().any(|&(tx, a, z)| (tx - x).abs() < 0.5 && (a - y0).abs() < 0.5 && (z - y1).abs() < 0.5), "le trait de la coupe {x} de {y0} à {y1} : {traits:?}");
    }
}

// ------------------------------------------------------------------------------------------------ 9. la plus petite fenêtre

#[test]
fn a_la_plus_petite_fenetre_rien_ne_deborde_des_en_tetes() {
    let [w, h] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let doc = || {
        let mut doc = film();
        doc.panels[0].effets_camera = vec![effet(Preset::ZoomIn, 0, 8), effet(Preset::LightShake, 2, 8)];
        doc
    };
    for largeur in [None, Some(crate::montagehead::largeur_min_colonne(crate::barres::Barre::Montage))] {
        let mut b = banc_sb(doc(), w as u32, h as u32, true);
        if let Some(l) = largeur {
            b.app.montage_head_w = l;
            b.chauffer(4);
        }
        let quoi = format!("{w}×{h}, colonne de {}", b.app.montage_head_w);
        let rows = rangees(&b);
        let hh = haut(&b);
        icones_en_ordre(&ICONES_DES_PLANS, hh + rows.blocks_top(), hh + rows.blocks_top() + rows.block_h, &quoi);
        icones_en_ordre(&["Replier la caméra", "Clé de caméra ici", "Préréglages de caméra", "Remettre la caméra à zéro", "Autres commandes de la caméra"], hh + rows.camera_top(), hh + rows.camera_top() + SB_CAM_H, &quoi);
        let (a, z) = (hh + rows.calques_top(), hh + rows.calques_top() + SB_CALQUES_H);
        // (Partie 179.5 : la ligne « Calques » porte ⊖ et les commandes des calques, plus les icônes des colonnes ; « + » et « ⋯ » y
        // sont toujours, les autres selon la place.)
        let mut ligne: Vec<Rect> = std::iter::once(ou_dans("Replier la rangée Calques", a, z))
            .chain([crate::barrecalques::NOMS[0], crate::barrecalques::NOMS[5]].iter().map(|n| ou_dans(n, a, z)))
            .chain(crate::barrecalques::NOMS[1..5].iter().flat_map(|n| vus::ou(n).into_iter().filter(|r| r.center().y > a && r.center().y < z)))
            .collect();
        ligne.sort_by(|p, q| p.left().total_cmp(&q.left()));
        let col = colonne();
        for (k, r) in ligne.iter().enumerate() {
            assert!(r.right() <= col.right() + 0.5, "{quoi} : la ligne Calques dans la colonne : {r:?} / {col:?}");
            if k > 0 {
                assert!(r.left() >= ligne[k - 1].right() - 0.5, "{quoi} : deux icônes de la ligne Calques se touchent : {ligne:?}");
            }
        }
        // Le mot de chaque rangée ne passe pas sous ses icônes.
        for (mot, premiere) in [("Plans", "Nouveau plan"), ("Caméra", "Clé de caméra ici")] {
            let t = textes(&b).into_iter().filter(|(t, r)| t == mot && r.right() < col.right()).map(|(_, r)| r).find(|r| r.top() > hh + rows.blocks_top() - 1.0).unwrap_or_else(|| panic!("{quoi} : « {mot} »"));
            let i = vus::ou(premiere).into_iter().find(|r| (r.center().y - t.center().y).abs() < 8.0).unwrap_or_else(|| panic!("{quoi} : « {premiere} »"));
            assert!(t.right() <= i.left() + 0.5, "{quoi} : « {mot} » touche « {premiere} » : {t:?} / {i:?}");
        }
    }
}

// ------------------------------------------------------------------------------------------------ captures

/// Les images de contrôle (partie 178), dans `SHOT_CAPTURE_DIR`, à taille réelle : la timeline entière caméra repliée ; dépliée avec
/// deux préréglages qui se chevauchent ; le menu d'une transition ouvert sur sa coupe ; les calques repliés ; la plus petite fenêtre ;
/// gros plan des en-têtes Plan et Caméra. `SHOT_CAPTURE_PROJET` : le projet à ouvrir (une copie) ; sinon le film de ces essais.
#[test]
#[ignore = "écrit des captures : SHOT_CAPTURE_DIR"]
fn timeline_storyboard_178_captures() {
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let charger = || match std::env::var_os("SHOT_CAPTURE_PROJET") {
        Some(p) => crate::project::load(std::path::Path::new(&p)).expect("le projet").doc,
        None => film(),
    };
    let preparer = |doc: &mut Document| {
        // (Au moins trois plans ; une transition sur la première coupe ; deux préréglages qui se chevauchent dans le deuxième plan, et
        // des clés de caméra.)
        while doc.panels.len() < 3 {
            let last = doc.panels.len() - 1;
            doc.add_scene_after(last);
        }
        let pi = 1.min(doc.panels.len() - 1);
        let len = doc.panels[pi].frame_count().max(24);
        doc.panels[pi].effets_camera = vec![effet(Preset::ZoomIn, len / 8, len / 2), effet(Preset::LightShake, len / 4, len / 2)];
        let c = doc.default_camera();
        if doc.panels[pi].camera.is_empty() {
            doc.panels[pi].camera = vec![CameraKey::new(0, c), CameraKey::new(len - 1, Camera { zoom: c.zoom * 1.2, ..c })];
        }
        if doc.transitions.is_empty() {
            doc.set_transition(1, Kind::Dissolve);
        }
    };
    let sauver = |b: &mut Banc, nom: &str, zone: Rect, taille: [u32; 2]| {
        b.chauffer(4);
        let Some(rgba) = b.capturer() else { return println!("CAPTURE pas de carte graphique : {nom}") };
        let img = image::RgbaImage::from_raw(taille[0], taille[1], rgba).expect("image");
        let z = zone.intersect(Rect::from_min_size(Pos2::ZERO, vec2(taille[0] as f32, taille[1] as f32)));
        let crop = image::imageops::crop_imm(&img, z.left() as u32, z.top() as u32, z.width() as u32, z.height() as u32).to_image();
        crop.save(dossier.join(nom)).expect("png");
        println!("CAPTURE {nom} {}x{}", crop.width(), crop.height());
    };
    let timeline = |b: &Banc, w: f32, h: f32| {
        let rows = rangees(b);
        let haut = (haut(b) - 4.0).max(0.0);
        Rect::from_min_max(pos2(0.0, haut), pos2(w, (haut + rows.height(b.app.doc.audio_tracks.len()) + 30.0).min(h)))
    };
    let mut doc = charger();
    preparer(&mut doc);
    let mut b = crate::perf_tests::banc_taille(doc, true, 1920, 1080);
    b.app.set_espace(Espace::Storyboard);
    b.app.settings.storyboard_camera_depliee = false;
    b.app.settings.storyboard_calques_replies = false;
    b.app.montage_fit = true;
    b.app.cur_panel = 1.min(b.app.doc.panels.len() - 1);
    b.garder_formes = true;
    b.chauffer(12);
    let z = timeline(&b, 1920.0, 1080.0);
    sauver(&mut b, "178-1-timeline-camera-repliee.png", z, [1920, 1080]);
    b.app.settings.storyboard_camera_depliee = true;
    b.chauffer(4);
    let z = timeline(&b, 1920.0, 1080.0);
    sauver(&mut b, "178-2-camera-depliee-deux-prereglages.png", z, [1920, 1080]);
    // Gros plan des en-têtes Plan et Caméra (la colonne, de la règle aux calques).
    let rows = rangees(&b);
    let col = colonne();
    let h = haut(&b);
    let z = Rect::from_min_max(pos2(0.0, h + rows.blocks_top() - 4.0), pos2(col.right() + 120.0, h + rows.calques_top() + SB_CALQUES_H + 4.0));
    sauver(&mut b, "178-6-en-tetes-plan-et-camera.png", z, [1920, 1080]);
    // Le menu de la transition, ouvert sur sa coupe.
    let nom = b.app.doc.transitions.first().map(|t| format!("Transition : {}", t.kind.name()));
    if let Some(s) = nom.and_then(|n| vus::ou(&n).first().copied()) {
        clic(&mut b, s.center(), Modifiers::NONE);
        let z = Rect::from_min_max(pos2((s.left() - 300.0).max(0.0), (s.top() - 380.0).max(0.0)), pos2(s.right() + 420.0, (s.bottom() + 120.0).min(1080.0)));
        sauver(&mut b, "178-3-menu-de-transition.png", z, [1920, 1080]);
        clic(&mut b, s.center(), Modifiers::NONE);
    }
    b.app.settings.storyboard_camera_depliee = false;
    b.app.settings.storyboard_calques_replies = true;
    b.chauffer(4);
    let z = timeline(&b, 1920.0, 1080.0);
    sauver(&mut b, "178-4-calques-replies.png", z, [1920, 1080]);
    // La plus petite fenêtre, toute l'image.
    let [w, hh] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let mut doc = charger();
    preparer(&mut doc);
    let mut p = crate::perf_tests::banc_taille(doc, true, w as u32, hh as u32);
    p.app.set_espace(Espace::Storyboard);
    p.app.settings.storyboard_camera_depliee = true;
    p.app.settings.storyboard_calques_replies = false;
    p.app.montage_fit = true;
    p.app.cur_panel = 1.min(p.app.doc.panels.len() - 1);
    p.garder_formes = true;
    p.chauffer(12);
    sauver(&mut p, "178-5-plus-petite-fenetre.png", Rect::from_min_size(Pos2::ZERO, vec2(w, hh)), [w as u32, hh as u32]);
}
