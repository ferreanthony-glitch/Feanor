//! Essais de « la caméra choisie seule » (`docs/INTERFACE.md`, partie 152). Anthony, 2026-11-13, après son essai de la
//! Trajectoire dans l'Espace 3D : « Éditer la trajectoire de la caméra ne fonctionne pas puisqu'un calque est toujours
//! sélectionné en même temps. La partie dédiée à la caméra dans la timeline doit être sélectionnée quand la caméra l'est,
//! et plus un calque. »
//!
//! Choisir la caméra la choisit seule (Ctrl+clic avec l'outil Trajectoire, un clic avec les outils de la vue, « choisir »
//! du panneau Espace 3D) ; ses gestes de trajectoire ne changent alors aucun calque ; la timeline et le montage montrent
//! la piste « Caméra » choisie et plus aucune rangée de calque ; choisir un calque ou une carte la lâche ; Maj (avec Ctrl
//! pour l'outil Trajectoire) l'ajoute exprès aux cartes.

use eframe::egui::{pos2, Color32, Event, Modifiers, PointerButton, Pos2, Rect, Shape, Stroke};

use crate::app::{Mode, Tool};
use crate::camera::CameraKey;
use crate::document::{CelId, Document, LayerTransform};
use crate::espace3d;
use crate::espace3ddessin::Regard3d;
use crate::espace3dgizmo::CAMERA_ID;
use crate::espace3dmath as m;
use crate::perf_tests::Banc;
use crate::poskeys::PosKey;
use crate::theme;
use crate::trajectoire::Trajectoire;
use crate::trajectoire3d::Sujet;
use crate::trajectoire3d_tests::{a_l_ecran, appuyer, carte, derniere_etape, doc_cartes, geste, lacher, ouvert, proche, tracer_au_sol, AU_BOUT};

const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const MAJ_CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: true, mac_cmd: false, command: true };

/// Deux cartes : la carte 1 est derrière la carte 0 (plus loin de la caméra d'origine du plan) et décalée vers la droite :
/// elle dépasse d'elle, et le sol devant les cartes reste libre pour y tracer.
fn doc_deux_cartes() -> Document {
    let mut doc = doc_cartes(2);
    doc.panels[0].layers[1].transform.x = 120.0;
    doc.panels[0].layers[1].depth = 0.4;
    doc
}

/// Les numéros de sélection des cartes du plan, dans l'ordre des calques.
fn cartes(b: &Banc) -> Vec<u64> {
    b.app.layers().iter().map(|l| espace3d::card_selection(l.id)).collect()
}

/// Ce qu'une trajectoire peut changer d'un calque : son chemin, ses clés de position, sa pose fixe, sa profondeur, ses dessins.
type Calque = (Option<Trajectoire>, Vec<PosKey>, LayerTransform, f32, Vec<Option<CelId>>);

fn calques(b: &Banc) -> Vec<Calque> {
    b.app.layers().iter().map(|l| (l.trajectoire.clone(), l.pos_keys.clone(), l.transform, l.depth, l.frames.clone())).collect()
}

/// Le regard de la vue d'édition telle qu'elle est maintenant. (Les outils de la vue ne le notent pas dans `e3d.regard` :
/// seuls le dessin et la Trajectoire le font ; ici il est refait, comme `espace3d_panel` le fait.)
fn regard_de_la_vue(b: &Banc) -> Regard3d {
    let rect = b.app.canvas_rect;
    Regard3d { rect, view: b.app.espace3d.cam.expect("la vue").view(rect.width()) }
}

/// Le point de l'écran où la vue montre ce point du monde ; il doit être dans la vue.
fn voir(b: &Banc, p: [f32; 3]) -> Pos2 {
    let q = regard_de_la_vue(b).ecran(p).unwrap_or_else(|| panic!("le point {p:?} se voit"));
    assert!(b.app.canvas_rect.shrink(4.0).contains(q), "le point {p:?} tombe en {q:?}, hors de la vue {:?}", b.app.canvas_rect);
    q
}

/// Le pointeur, au point `ici` de l'écran, est sur le dessin de la caméra du plan.
fn sur_la_camera(b: &Banc, ici: Pos2) -> bool {
    let r = regard_de_la_vue(b);
    b.app.e3_pick_camera(&r.view, [ici.x - r.rect.left(), ici.y - r.rect.top()], [r.rect.width() / 2.0, r.rect.height() / 2.0])
}

/// La vue regarde les cartes de face, par-dessus la caméra du plan : rend le point de l'écran où est le dessin de la caméra.
fn vue_de_face_sur_la_camera(b: &mut Banc) -> Pos2 {
    let vue = b.app.espace3d.cam.as_mut().expect("la vue");
    (vue.azimuth, vue.elevation, vue.distance) = (0.0, 0.0, vue.distance * 1.5);
    b.chauffer(2);
    let ici = voir(b, b.app.e3_world().scene_cam(&b.app.current_camera()).pos);
    assert!(sur_la_camera(b, ici), "le pointeur est sur le dessin de la caméra");
    ici
}

/// La vue regarde de côté : les cartes sont vues par la tranche, aucune ne passe derrière le dessin de la caméra.
fn vue_de_cote_sur_la_camera(b: &mut Banc) -> Pos2 {
    let vue = b.app.espace3d.cam.as_mut().expect("la vue");
    (vue.azimuth, vue.elevation) = (90.0, 0.0);
    b.chauffer(2);
    let ici = voir(b, b.app.e3_world().scene_cam(&b.app.current_camera()).pos);
    assert!(sur_la_camera(b, ici), "le pointeur est sur le dessin de la caméra");
    let rayon = regard_de_la_vue(b).rayon(ici);
    assert_eq!(b.app.e3_pick(&rayon), None, "aucune carte ne passe derrière elle");
    ici
}

/// Un point de la carte du calque `li` qui se voit dans la vue, loin du dessin de la caméra, et où cette carte est la
/// première que le regard touche.
fn sur_la_carte(b: &mut Banc, li: usize) -> Pos2 {
    let (r, mx, voulue) = (regard_de_la_vue(b), carte(b, li, 0).matrix(), cartes(b)[li]);
    for (u, v) in [(0.42, 0.3), (0.42, -0.3), (0.44, 0.0), (-0.42, 0.3), (-0.42, -0.3), (0.3, 0.42), (0.3, -0.42), (0.0, 0.4), (0.0, -0.4), (0.2, 0.2)] {
        let Some(ici) = r.ecran(m::point(&mx, [u, v, 0.0])).filter(|q| b.app.canvas_rect.shrink(4.0).contains(*q)) else { continue };
        if !sur_la_camera(b, ici) && b.app.e3_pick(&r.rayon(ici)) == Some(voulue) {
            return ici;
        }
    }
    panic!("aucun point de la carte {li} ne se voit seul dans cette vue");
}

// ------------------------------------------------------------------------------------------------ ce que la bande peint

fn rectangles(forme: &Shape, out: &mut Vec<(Rect, Color32, Stroke)>) {
    match forme {
        Shape::Rect(r) => out.push((r.rect, r.fill, r.stroke)),
        Shape::Vec(v) => v.iter().for_each(|s| rectangles(s, out)),
        _ => {}
    }
}

fn textes(forme: &Shape, out: &mut Vec<(String, Pos2, Color32)>) {
    match forme {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos, t.galley.job.sections.first().map_or(Color32::PLACEHOLDER, |s| s.format.color))),
        Shape::Vec(v) => v.iter().for_each(|s| textes(s, out)),
        _ => {}
    }
}

/// Les couleurs des noms de la piste « Caméra » peints par la dernière image dans la bande du bas (montage ou timeline).
fn encres_de_la_piste_camera(b: &Banc) -> Vec<Color32> {
    let mut tous = Vec::new();
    b.formes.iter().for_each(|f| textes(&f.shape, &mut tous));
    let sous_le_dessin = b.app.canvas_rect.bottom();
    tous.into_iter().filter(|(t, p, _)| p.y > sous_le_dessin && (t == "Caméra" || t.starts_with("Caméra ("))).map(|(_, _, c)| c).collect()
}

/// Les rectangles peints par la dernière image qui passent `garde`.
fn rectangles_peints(b: &Banc, garde: impl Fn(&(Rect, Color32, Stroke)) -> bool) -> Vec<Rect> {
    let mut tous = Vec::new();
    b.formes.iter().for_each(|f| rectangles(&f.shape, &mut tous));
    tous.into_iter().filter(|r| garde(r)).map(|r| r.0).collect()
}

// ------------------------------------------------------------------------------------------------ les essais

#[test]
fn ctrl_click_on_the_camera_releases_the_cards_and_its_path_gestures_change_no_layer() {
    let mut b = ouvert(doc_deux_cartes(), true);
    let c = cartes(&b);
    // Comme après avoir dessiné sur la carte : la carte du calque courant est choisie.
    b.app.espace3d.sel = vec![c[0]];
    let vue_d_avant = b.app.espace3d.cam;
    let ici = vue_de_face_sur_la_camera(&mut b);
    geste(&mut b, &[ici], CTRL);
    assert_eq!(b.app.espace3d.sel, vec![CAMERA_ID], "Ctrl+clic sur la caméra : elle seule, la carte est lâchée ({})", b.app.status);
    assert_eq!(b.app.e3_traj_sujets(), vec![Sujet::Camera], "aucun calque ne s'ajoute à la caméra choisie");
    assert!(b.app.e3_camera_seule());
    assert!(b.app.status.starts_with("La caméra est choisie, seule"), "{}", b.app.status);
    // Un chemin au sol, la vue près du sol : la caméra le suit, aucun calque ne reçoit rien.
    b.app.espace3d.cam = vue_d_avant;
    let y = b.app.e3_world().sol_y();
    let vue = b.app.espace3d.cam.as_mut().expect("la vue");
    (vue.target, vue.distance) = ([0.0, y, 2.5], vue.distance * 0.4);
    b.chauffer(2);
    let (avant, etapes) = (calques(&b), b.app.history.undo_len());
    tracer_au_sol(&mut b);
    let chemin = b.app.doc.panels[0].trajectoire_camera.clone().expect("la caméra suit un chemin");
    assert!(proche(chemin.deplacement_a(1.0).unwrap(), AU_BOUT, 0.05));
    assert_eq!(calques(&b), avant, "aucun calque n'a reçu de chemin, de clé, ni changé de place");
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Trajectoire".to_string(), etapes + 1));
    assert!(b.app.status.contains("la caméra") && !b.app.status.contains("carte"), "{}", b.app.status);
    // Une marque, d'un petit trait en travers du chemin de la caméra.
    let sur = |f: f32| [-2.0 + 4.0 * f, y, 1.0 + 2.0 * f];
    let (centre, sens) = (a_l_ecran(&b, sur(0.2)), (a_l_ecran(&b, sur(0.22)) - a_l_ecran(&b, sur(0.18))).normalized());
    geste(&mut b, &[centre - sens.rot90() * 25.0, centre + sens.rot90() * 25.0], Modifiers::NONE);
    let marque = b.app.doc.panels[0].trajectoire_camera.clone().expect("toujours là");
    assert_eq!(marque.marques.len(), 1, "{}", b.app.status);
    assert_eq!(derniere_etape(&b), "Marque de trajectoire");
    assert_eq!(calques(&b), avant, "la marque n'a touché aucun calque");
    // Tirer le chemin de la caméra : il se tord, les calques ne bougent pas.
    let pris = a_l_ecran(&b, sur(0.6));
    appuyer(&mut b, &[pris, pris + eframe::egui::vec2(10.0, 60.0)], Modifiers::NONE);
    lacher(&mut b, pris + eframe::egui::vec2(10.0, 60.0), Modifiers::NONE);
    let tire = b.app.doc.panels[0].trajectoire_camera.clone().expect("toujours là");
    assert_eq!(derniere_etape(&b), "Tirer la trajectoire");
    assert!(tire.longueur() > marque.longueur() + 2.0, "le chemin de la caméra a une bosse : {} > {}", tire.longueur(), marque.longueur());
    assert_eq!(calques(&b), avant, "tirer le chemin de la caméra n'a touché aucun calque");
    assert!(b.app.doc.panels[0].camera.is_empty(), "et aucune clé de caméra n'est posée");
    assert_eq!(b.app.espace3d.sel, vec![CAMERA_ID], "la caméra est restée seule pendant tous ces gestes");
}

#[test]
fn the_camera_chosen_alone_is_the_chosen_row_of_the_montage_and_of_the_timeline_and_no_layer_row_is() {
    let mut b = ouvert(doc_deux_cartes(), true);
    b.garder_formes = true;
    let cadre = |r: &(Rect, Color32, Stroke)| r.2.width == 1.5 && r.2.color == theme::contour() && (r.0.height() - (crate::montagerows::CAM_H - 2.0)).abs() < 0.6;

    // Le montage (Storyboard). Un calque est courant : sa rangée est la rangée choisie, pas la piste « Caméra ».
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    b.chauffer(3);
    assert!(b.app.e3_vue_montree(), "la vue Espace 3D est restée ouverte");
    assert!(!b.app.e3_camera_seule() && b.app.calque_montre_courant(0) && !b.app.calque_montre_courant(1));
    let (encres_calque, cadres_calque) = (encres_de_la_piste_camera(&b), rectangles_peints(&b, cadre).len());
    assert!(!encres_calque.is_empty(), "le montage écrit le nom de la piste « Caméra »");
    // La caméra choisie (« choisir » du panneau Espace 3D) : sa piste devient la rangée choisie, plus aucun calque ne l'est.
    b.app.e3_choisir_la_camera(false);
    b.chauffer(2);
    assert!(b.app.e3_camera_seule());
    assert!((0..2).all(|li| !b.app.calque_montre_courant(li)), "aucune rangée de calque n'est montrée choisie");
    assert_eq!(b.app.cur_layer, 0, "le calque courant reste le calque courant (le panneau Calques, le dessin)");
    let (encres_camera, cadres_camera) = (encres_de_la_piste_camera(&b), rectangles_peints(&b, cadre).len());
    assert_ne!(encres_camera, encres_calque, "le nom de la piste « Caméra » passe en texte fort");
    assert!(cadres_camera > cadres_calque, "la piste « Caméra » reçoit le cadre de la rangée choisie : {cadres_camera} cadres au lieu de {cadres_calque}");
    // Un calque choisi de nouveau (son nom, dans le montage) : la piste « Caméra » rend la rangée.
    b.app.click_layer_head(0, false, false);
    b.chauffer(2);
    assert!(!b.app.e3_camera_seule() && b.app.calque_montre_courant(0));
    assert_eq!((encres_de_la_piste_camera(&b), rectangles_peints(&b, cadre).len()), (encres_calque, cadres_calque), "le montage est peint comme avant");

    // La timeline (Animation) : la surbrillance du calque courant passe à la rangée de la caméra, sous les calques.
    b.app.set_mode(Mode::Animation);
    b.chauffer(3);
    assert!(b.app.e3_vue_montree());
    let surbrillance = theme::surface_active(theme::accent(), 0.35);
    let rangees = |b: &Banc| rectangles_peints(b, |r| r.1 == surbrillance && r.0.width() > 200.0);
    let (du_calque, encres_calque) = (rangees(&b), encres_de_la_piste_camera(&b));
    assert_eq!(du_calque.len(), 1, "une rangée en surbrillance : celle du calque courant");
    assert!(!encres_calque.is_empty(), "la timeline écrit le nom de la rangée « Caméra »");
    b.app.e3_choisir_la_camera(false);
    b.chauffer(2);
    let (de_la_camera, encres_camera) = (rangees(&b), encres_de_la_piste_camera(&b));
    assert_eq!(de_la_camera.len(), 1, "une seule rangée en surbrillance : {de_la_camera:?}");
    assert!(de_la_camera[0].top() >= du_calque[0].bottom() - 1.0, "c'est celle de la caméra, sous les calques : {:?} sous {:?}", de_la_camera[0], du_calque[0]);
    assert_ne!(encres_camera, encres_calque, "et son nom est écrit comme celui d'un calque courant");
    // Cliquer dans la rangée d'un calque de la timeline le choisit : la surbrillance lui revient.
    b.app.select_layer(0);
    b.chauffer(2);
    assert_eq!((rangees(&b), encres_de_la_piste_camera(&b)), (du_calque, encres_calque));
}

#[test]
fn choosing_a_layer_or_a_card_again_releases_the_camera_and_gives_the_row_back() {
    let mut b = ouvert(doc_deux_cartes(), true);
    let c = cartes(&b);
    // Le même calque, recliqué par son nom (panneau Calques, timeline, montage) : la caméra est lâchée, sa carte est choisie.
    b.app.e3_choisir_la_camera(false);
    assert!(b.app.e3_camera_seule() && !b.app.calque_montre_courant(0));
    b.app.click_layer_head(0, false, false);
    assert_eq!(b.app.espace3d.sel, vec![c[0]]);
    assert!(!b.app.e3_camera_seule() && b.app.calque_montre_courant(0));
    // Un autre calque : il devient courant, sa carte est choisie, la caméra lâchée ; la vue ne défait rien ensuite.
    b.app.e3_choisir_la_camera(false);
    b.app.click_layer_head(1, false, false);
    assert_eq!((b.app.espace3d.sel.clone(), b.app.cur_layer), (vec![c[1]], 1));
    b.chauffer(2);
    assert_eq!(b.app.espace3d.sel, vec![c[1]]);
    assert!(b.app.calque_montre_courant(1));
    // Une carte cliquée dans la vue, avec l'outil Trajectoire (Ctrl+clic) : elle seule, et son calque devient courant.
    b.app.click_layer_head(0, false, false);
    b.app.e3_choisir_la_camera(false);
    b.chauffer(2);
    let ici = sur_la_carte(&mut b, 1);
    geste(&mut b, &[ici], CTRL);
    assert_eq!((b.app.espace3d.sel.clone(), b.app.cur_layer), (vec![c[1]], 1), "{}", b.app.status);
    assert!(!b.app.e3_camera_seule() && b.app.calque_montre_courant(1));
    assert_eq!(b.app.e3_traj_sujets(), vec![Sujet::Carte(b.app.layers()[1].id)]);
    // Une carte cliquée dans la vue avec l'outil « choisir » de la vue.
    b.app.run_action("espace3d.outil_choisir");
    b.app.click_layer_head(0, false, false);
    b.app.e3_choisir_la_camera(false);
    b.chauffer(2);
    assert!(b.app.e3_camera_seule());
    let ici = sur_la_carte(&mut b, 1);
    geste(&mut b, &[ici], Modifiers::NONE);
    assert_eq!((b.app.espace3d.sel.clone(), b.app.cur_layer), (vec![c[1]], 1), "{}", b.app.status);
    assert!(!b.app.e3_camera_seule());
    // La vue fermée, la caméra restée choisie : un calque choisi la lâche aussi (rien n'est choisi au retour dans la vue).
    b.app.e3_choisir_la_camera(false);
    b.app.run_action("espace3d.basculer");
    assert!(!b.app.espace3d.on && !b.app.e3_camera_seule(), "vue fermée : la timeline montre le calque courant");
    assert!(b.app.calque_montre_courant(1));
    b.app.click_layer_head(1, false, false);
    assert!(!b.app.espace3d.sel.contains(&CAMERA_ID));
}

#[test]
fn the_camera_joins_the_cards_only_on_purpose_with_shift() {
    let mut b = ouvert(doc_deux_cartes(), true);
    let c = cartes(&b);
    let id0 = b.app.layers()[0].id;
    // Rien de choisi : le repli sur la carte du calque courant. La caméra choisie : elle seule, jamais ce repli.
    assert!(b.app.espace3d.sel.is_empty());
    assert_eq!(b.app.e3_traj_sujets(), vec![Sujet::Carte(id0)]);
    b.app.e3_choisir_la_camera(false);
    assert_eq!(b.app.e3_traj_sujets(), vec![Sujet::Camera]);
    // Avec l'outil Trajectoire : Maj+Ctrl+clic sur la caméra l'ajoute à la carte choisie, un second la lâche.
    b.app.espace3d.sel = vec![c[0]];
    let ici = vue_de_face_sur_la_camera(&mut b);
    geste(&mut b, &[ici], MAJ_CTRL);
    assert_eq!(b.app.espace3d.sel, vec![c[0], CAMERA_ID], "{}", b.app.status);
    assert_eq!(b.app.e3_traj_sujets(), vec![Sujet::Carte(id0), Sujet::Camera]);
    assert!(!b.app.e3_camera_seule() && b.app.calque_montre_courant(0), "avec une carte, la rangée reste à son calque");
    assert_eq!(b.app.status, "Le chemin déplacera 1 carte et la caméra.");
    geste(&mut b, &[ici], MAJ_CTRL);
    assert_eq!(b.app.espace3d.sel, vec![c[0]], "{}", b.app.status);
    // Maj seule, avec cet outil, n'ajoute rien : elle reste au chemin (le déplacer en bloc).
    geste(&mut b, &[ici], Modifiers::SHIFT);
    assert_eq!(b.app.espace3d.sel, vec![c[0]], "{}", b.app.status);
    // Ctrl+clic, sans Maj : la caméra seule.
    geste(&mut b, &[ici], CTRL);
    assert_eq!(b.app.espace3d.sel, vec![CAMERA_ID], "{}", b.app.status);
    // « choisir » du panneau Espace 3D : seule ; Maj+clic : avec les cartes, ou lâchée, sans jamais de doublon.
    b.app.espace3d.sel = vec![c[0], c[1]];
    b.app.e3_choisir_la_camera(true);
    assert_eq!(b.app.espace3d.sel, vec![c[0], c[1], CAMERA_ID]);
    b.app.e3_choisir_la_camera(true);
    assert_eq!(b.app.espace3d.sel, vec![c[0], c[1]]);
    b.app.e3_choisir_la_camera(false);
    assert_eq!(b.app.espace3d.sel, vec![CAMERA_ID]);
    // Avec les outils de la vue : un clic sur la caméra la choisit seule, Maj+clic l'ajoute aux cartes ou la lâche.
    b.app.run_action("espace3d.outil_choisir");
    let ici = vue_de_cote_sur_la_camera(&mut b);
    b.app.espace3d.sel = vec![c[0]];
    geste(&mut b, &[ici], Modifiers::NONE);
    assert_eq!(b.app.espace3d.sel, vec![CAMERA_ID], "{}", b.app.status);
    b.app.espace3d.sel = vec![c[0]];
    geste(&mut b, &[ici], Modifiers::SHIFT);
    assert_eq!(b.app.espace3d.sel, vec![c[0], CAMERA_ID]);
    geste(&mut b, &[ici], Modifiers::SHIFT);
    assert_eq!(b.app.espace3d.sel, vec![c[0]], "un second Maj+clic la lâche, sans doublon");
}

/// Les outils de la vue (« choisir ») : un clic simple sur le dessin de la caméra la prend, même avec une carte derrière
/// (Anthony, 2026-11-13 : « règle ce qui reste imparfait » ; avant, la carte de derrière l'emportait) ; Maj ou Ctrl
/// l'ajoute ou la lâche ; tirer son dessin déplace la caméra, pas la carte de derrière.
#[test]
fn with_the_view_tools_a_click_on_the_camera_takes_it_even_with_a_card_behind_and_dragging_it_moves_the_camera() {
    let mut b = ouvert(doc_deux_cartes(), true);
    let c = cartes(&b);
    b.app.run_action("espace3d.outil_choisir");
    b.chauffer(2);
    assert!(!b.app.e3_trace() && !b.app.e3_dessine(), "les outils de la vue ont le pointeur");
    let ici = vue_de_face_sur_la_camera(&mut b);
    // Une carte passe bien derrière le dessin de la caméra : c'est elle qu'un clic simple prenait.
    let derriere = b.app.e3_pick(&regard_de_la_vue(&b).rayon(ici));
    assert!(derriere.is_some_and(|id| c.contains(&id)), "une carte est sous le pointeur, derrière la caméra : {derriere:?}");
    b.app.espace3d.sel = vec![c[0]];
    geste(&mut b, &[ici], Modifiers::NONE);
    assert_eq!(b.app.espace3d.sel, vec![CAMERA_ID], "un clic simple : la caméra, seule ({})", b.app.status);
    assert!(b.app.e3_camera_seule() && (0..2).all(|li| !b.app.calque_montre_courant(li)), "la piste « Caméra » est la rangée choisie");
    assert_eq!(b.app.cur_layer, 0, "le calque courant ne change pas");
    // Maj+clic, Ctrl+clic : elle rejoint les cartes choisies, ou les quitte, sans doublon.
    b.app.espace3d.sel = vec![c[0]];
    geste(&mut b, &[ici], Modifiers::SHIFT);
    assert_eq!(b.app.espace3d.sel, vec![c[0], CAMERA_ID]);
    geste(&mut b, &[ici], Modifiers::SHIFT);
    assert_eq!(b.app.espace3d.sel, vec![c[0]]);
    geste(&mut b, &[ici], CTRL);
    assert_eq!(b.app.espace3d.sel, vec![c[0], CAMERA_ID]);
    geste(&mut b, &[ici], CTRL);
    assert_eq!(b.app.espace3d.sel, vec![c[0]]);
    // Loin du dessin de la caméra, un clic sur une carte la prend toujours, et son calque devient courant.
    let sur = sur_la_carte(&mut b, 1);
    geste(&mut b, &[sur], Modifiers::NONE);
    assert_eq!((b.app.espace3d.sel.clone(), b.app.cur_layer), (vec![c[1]], 1), "{}", b.app.status);
    // Tirer le dessin de la caméra : la caméra se déplace (seule choisie), la carte de derrière ne bouge pas.
    let (avant, camera_avant, etapes) = (calques(&b), b.app.e3_world().scene_cam(&b.app.current_camera()).pos, b.app.history.undo_len());
    let loin = ici + eframe::egui::vec2(80.0, 0.0);
    appuyer(&mut b, &[ici, loin], Modifiers::NONE);
    lacher(&mut b, loin, Modifiers::NONE);
    let camera_apres = b.app.e3_world().scene_cam(&b.app.current_camera()).pos;
    assert_eq!(b.app.espace3d.sel, vec![CAMERA_ID], "{}", b.app.status);
    assert!(m::length(m::sub(camera_apres, camera_avant)) > 0.05, "la caméra a glissé : {camera_avant:?} → {camera_apres:?}");
    assert_eq!(calques(&b), avant, "aucune carte n'a bougé");
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Déplacer la caméra".to_string(), etapes + 1));
    b.app.undo();
    assert!(proche(b.app.e3_world().scene_cam(&b.app.current_camera()).pos, camera_avant, 1e-4), "une annulation la remet");
}

// ------------------------------------------------------------------------------------------------ la piste « Caméra » cliquée

/// Un clic de souris au point `p` de l'écran (`mods` tenues) : le pointeur s'y pose d'abord (l'appui d'un pointeur qui vient
/// d'arriver se perd), et une demi-seconde le sépare du clic d'avant (sinon egui y voit un double-clic).
fn cliquer(b: &mut Banc, p: Pos2, mods: Modifiers) {
    b.image(Vec::new(), 0.5);
    b.touches = mods;
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.chauffer(2);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: mods }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: mods }], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(2);
}

fn zones_des_textes(forme: &Shape, out: &mut Vec<(String, Rect)>) {
    match forme {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| zones_des_textes(s, out)),
        _ => {}
    }
}

/// La zone du nom de la piste « Caméra » peint par la dernière image sous le dessin : celui de la timeline (« Caméra (2
/// clés) » : le plan d'essai en a deux), ou celui du montage (« Caméra »).
fn nom_de_la_piste_camera(b: &Banc, timeline: bool) -> Rect {
    let mut tous = Vec::new();
    b.formes.iter().for_each(|f| zones_des_textes(&f.shape, &mut tous));
    let sous_le_dessin = b.app.canvas_rect.bottom();
    let noms: Vec<Rect> = tous.into_iter().filter(|(t, z)| z.top() > sous_le_dessin && if timeline { t.starts_with("Caméra (") } else { t == "Caméra" }).map(|(_, z)| z).collect();
    assert_eq!(noms.len(), 1, "un seul nom de la piste Caméra ({}) sous le dessin : {noms:?}", if timeline { "timeline" } else { "montage" });
    noms[0]
}

/// La rangée de la piste « Caméra », hors de la colonne des noms, à la hauteur de son nom `nom` : le long rectangle sombre
/// qu'elle peint (la caméra n'étant pas choisie).
fn rangee_de_la_piste_camera(b: &Banc, nom: Rect) -> Rect {
    let y = nom.center().y;
    let rangees = rectangles_peints(b, |(r, fill, _)| *fill == theme::rangee_sombre() && r.width() > 300.0 && r.left() > nom.right() && r.top() <= y && r.bottom() >= y);
    let a_cette_hauteur = rectangles_peints(b, |(r, _, _)| r.width() > 100.0 && r.top() <= y && r.bottom() >= y);
    assert!(!rangees.is_empty(), "la rangée de la caméra est peinte à la hauteur de son nom {nom:?} : {a_cette_hauteur:?}");
    rangees[0]
}

/// La caméra est choisie seule, sa piste est la rangée choisie (aucune rangée de calque), et le calque courant est toujours `li`.
fn camera_seule_choisie(b: &Banc, li: usize) -> bool {
    b.app.espace3d.sel == vec![CAMERA_ID] && b.app.e3_camera_seule() && b.app.cur_layer == li && (0..b.app.layers().len()).all(|k| !b.app.calque_montre_courant(k))
}

/// Anthony, 2026-11-13 (« règle ce qui reste imparfait ») : cliquer la piste « Caméra » de la timeline ou du montage ne
/// choisissait pas la caméra. Un clic sur son nom, dans le vide de sa rangée ou sur une de ses clés la choisit maintenant
/// seule, comme « choisir » du panneau Espace 3D ; ses clés se choisissent toujours comme avant ; vue Espace 3D fermée, rien
/// de plus ne change.
#[test]
fn a_click_on_the_camera_track_of_the_montage_or_of_the_timeline_chooses_the_camera_alone_while_the_view_is_shown() {
    let mut doc = doc_deux_cartes();
    let camera = doc.default_camera();
    doc.panels[0].camera = vec![CameraKey::new(4, camera), CameraKey::new(16, camera)];
    let mut b = ouvert(doc, true);
    b.garder_formes = true;
    let c = cartes(&b);
    let etapes = b.app.history.undo_len();

    // Le montage (Storyboard) : le nom « Caméra », puis le vide de sa piste.
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    b.chauffer(3);
    assert!(b.app.e3_vue_montree());
    b.app.espace3d.sel = vec![c[0]];
    let nom = nom_de_la_piste_camera(&b, false);
    cliquer(&mut b, nom.center(), Modifiers::NONE);
    assert!(camera_seule_choisie(&b, 0), "clic sur « Caméra » au montage : {:?} ({})", b.app.espace3d.sel, b.app.status);
    b.app.click_layer_head(0, false, false);
    assert_eq!(b.app.espace3d.sel, vec![c[0]], "un calque choisi lâche la caméra");
    b.chauffer(2);
    let piste = rangee_de_la_piste_camera(&b, nom);
    cliquer(&mut b, pos2(piste.left() + 30.0, piste.center().y), Modifiers::NONE);
    assert!(camera_seule_choisie(&b, 0), "clic dans le vide de la piste Caméra au montage : {:?} ({})", b.app.espace3d.sel, b.app.status);
    b.app.click_layer_head(0, false, false);

    // La timeline (Animation) : le nom, le vide de la rangée (la tête de lecture y va, comme avant), une clé.
    b.app.set_mode(Mode::Animation);
    b.chauffer(3);
    assert!(b.app.e3_vue_montree() && !b.app.e3_camera_seule());
    let nom = nom_de_la_piste_camera(&b, true);
    cliquer(&mut b, nom.center(), Modifiers::NONE);
    assert!(camera_seule_choisie(&b, 0), "clic sur le nom de la rangée Caméra de la timeline : {:?}", b.app.espace3d.sel);
    b.app.click_layer_head(0, false, false);
    assert!(!b.app.e3_camera_seule());
    // (Une image de plus : la rangée de la caméra choisie a la surbrillance, rendue maintenant au calque.)
    b.chauffer(2);
    let rangee = rangee_de_la_piste_camera(&b, nom);
    let cellule = b.app.timeline_cell;
    let image = |f: f32| pos2(rangee.left() + (f + 0.5) * cellule, rangee.center().y);
    cliquer(&mut b, image(10.0), Modifiers::NONE);
    assert!(camera_seule_choisie(&b, 0), "clic dans le vide de la rangée Caméra : {:?}", b.app.espace3d.sel);
    assert_eq!(b.app.cur_frame, 10, "la tête de lecture va à l'image cliquée, comme avant");
    assert!(b.app.cam_key_sel.is_empty());
    b.app.click_layer_head(0, false, false);
    // Une clé de caméra : elle est choisie (comme avant), la tête de lecture y va, et la caméra est choisie seule.
    cliquer(&mut b, image(16.0), Modifiers::NONE);
    assert_eq!(b.app.cam_key_sel.len(), 1, "la clé est choisie ({})", b.app.status);
    assert_eq!(b.app.cur_frame, 16);
    assert!(camera_seule_choisie(&b, 0), "{:?}", b.app.espace3d.sel);
    // Ctrl+clic sur l'autre clé : elle s'ajoute au choix des clés, comme avant.
    cliquer(&mut b, image(4.0), CTRL);
    assert_eq!(b.app.cam_key_sel.len(), 2, "{}", b.app.status);
    assert!(camera_seule_choisie(&b, 0));
    assert_eq!(b.app.history.undo_len(), etapes, "choisir n'est pas une étape d'annulation");

    // La vue Espace 3D fermée : un clic sur la piste ne change rien de plus qu'avant, la rangée reste au calque courant.
    b.app.click_layer_head(1, false, false);
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(!b.app.e3_vue_montree());
    let avant = b.app.espace3d.sel.clone();
    let nom = nom_de_la_piste_camera(&b, true);
    cliquer(&mut b, nom.center(), Modifiers::NONE);
    let rangee = rangee_de_la_piste_camera(&b, nom);
    cliquer(&mut b, pos2(rangee.left() + 10.5 * cellule, rangee.center().y), Modifiers::NONE);
    assert_eq!(b.app.espace3d.sel, avant, "vue fermée, la caméra ne se choisit pas");
    assert!(!b.app.e3_camera_seule() && b.app.calque_montre_courant(1) && b.app.cur_layer == 1);
    assert_eq!(b.app.cur_frame, 10, "la tête de lecture va toujours à l'image cliquée");
    // Rouverte, la vue montre la carte du calque courant choisie, pas la caméra.
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(b.app.e3_vue_montree() && !b.app.espace3d.sel.contains(&CAMERA_ID), "{:?}", b.app.espace3d.sel);
}

// ------------------------------------------------------------------------------------------------ le panneau Calques

/// Le nom `nom` écrit par la dernière image dans le panneau Calques (à côté du dessin, au-dessus de la bande du bas, qui
/// écrit les mêmes noms ; le panneau Espace 3D, qui écrit ceux des cartes, est derrière Réglages) : sa zone à l'écran.
fn nom_dans_le_panneau_calques(b: &Banc, nom: &str) -> Rect {
    let mut tous = Vec::new();
    b.formes.iter().for_each(|f| zones_des_textes(&f.shape, &mut tous));
    let dessin = b.app.canvas_rect;
    let zones: Vec<Rect> = tous.iter().filter(|(t, z)| t == nom && !dessin.contains(z.center()) && z.center().y < dessin.bottom()).map(|(_, z)| *z).collect();
    // (S'il y en a plusieurs : celui du panneau Calques a sa vignette, encadrée, juste à sa gauche.)
    let mut cadres = Vec::new();
    b.formes.iter().for_each(|f| rectangles(&f.shape, &mut cadres));
    let a_sa_vignette = |z: &Rect| cadres.iter().any(|(r, _, s)| s.width > 0.0 && r.height() >= 12.0 && r.right() <= z.left() + 1.0 && r.right() >= z.left() - 40.0 && r.top() <= z.center().y && r.bottom() >= z.center().y);
    let zones: Vec<Rect> = if zones.len() > 1 { zones.into_iter().filter(a_sa_vignette).collect() } else { zones };
    let partout: Vec<&Rect> = tous.iter().filter(|(t, _)| t == nom).map(|(_, z)| z).collect();
    assert_eq!(zones.len(), 1, "le nom « {nom} » est écrit une fois dans le panneau Calques : {zones:?} (partout : {partout:?} ; le dessin : {dessin:?})");
    zones[0]
}

/// Ce que le panneau Calques peint pour le rang du calque `nom` : les fonds pleins de la taille d'un rang sous son nom, et
/// combien de cadres épais (celui de la vignette du calque courant) il a à gauche de son nom.
fn rang_peint(b: &Banc, nom: &str) -> (Vec<Color32>, usize) {
    let z = nom_dans_le_panneau_calques(b, nom);
    let mut tous = Vec::new();
    b.formes.iter().for_each(|f| rectangles(&f.shape, &mut tous));
    let fonds = tous.iter().filter(|(r, fill, _)| r.contains(z.center()) && r.height() < 80.0 && r.width() < 600.0 && *fill != Color32::TRANSPARENT).map(|r| r.1).collect();
    let cadres = tous.iter().filter(|(r, _, s)| s.width >= 2.0 && r.top() <= z.center().y && r.bottom() >= z.center().y && r.right() <= z.left() + 1.0 && r.left() > z.left() - 200.0).count();
    (fonds, cadres)
}

/// Comme `ouvert` (plan en Espace 3D, vue ouverte et cadrée, outil Trajectoire), dans une fenêtre plus haute (1600 × 1200) :
/// le panneau Calques, qui partage sa colonne avec les effets au-dessus de la bande du montage, y montre tous ses rangs (à
/// 1280 × 720, le second rang est sous le bord de la liste, et rien n'y est peint).
fn ouvert_grand(doc: Document) -> Banc {
    let mut b = crate::perf_tests::banc_taille(doc, false, 1600, 1200);
    b.chauffer(2);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.run_action("espace3d.basculer");
    b.app.run_action("espace3d.plan_en_espace");
    assert!(b.app.doc.plan_en_espace(0), "{}", b.app.status);
    b.chauffer(2);
    b.app.run_action("espace3d.cadrer");
    b.app.run_action("outil.trajectoire");
    b.chauffer(3);
    assert!(b.app.e3_trace(), "l'outil a le pointeur dans la vue");
    b
}

/// Le pointeur quitte le panneau (rien n'y est survolé), une image ou deux passent.
fn pointeur_ailleurs(b: &mut Banc) {
    b.image(vec![Event::PointerMoved(pos2(4.0, 4.0))], 1.0 / 60.0);
    b.chauffer(2);
}

/// Anthony, 2026-11-13 (« règle ce qui reste imparfait ») : la caméra choisie seule, le panneau Calques montrait encore le
/// calque courant comme choisi. Plus maintenant ; un clic sur un calque (même le courant), un outil de dessin pris dans la
/// vue ou un trait dans la vue rendent le calque courant et lâchent la caméra. L'outil Trajectoire, lui, la garde.
#[test]
fn while_the_camera_is_chosen_alone_the_layers_panel_shows_no_layer_as_chosen_and_drawing_or_a_click_on_a_layer_gives_it_back() {
    let mut b = ouvert_grand(doc_deux_cartes());
    b.garder_formes = true;
    let c = cartes(&b);
    // (Le panneau Calques n'est pas de l'espace Montage : on le regarde dans le Storyboard. Réglages devant le panneau Espace 3D,
    // qui écrit lui aussi les noms des cartes.)
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    b.app.show_pane(crate::dock::Pane::Layers);
    b.app.show_pane(crate::dock::Pane::ToolOptions);
    b.chauffer(8);
    assert!(b.app.pane_shown(crate::dock::Pane::Layers) && b.app.e3_vue_montree());
    let (courant, autre) = (rang_peint(&b, "Carte 0"), rang_peint(&b, "Carte 1"));
    assert_ne!(courant, autre, "le panneau Calques montre le calque courant");
    // La caméra choisie seule : le rang du calque courant est peint comme les autres.
    b.app.e3_choisir_la_camera(false);
    b.chauffer(2);
    assert_eq!(rang_peint(&b, "Carte 0"), autre, "caméra seule : aucun rang du panneau Calques n'est montré choisi");
    assert_eq!(rang_peint(&b, "Carte 1"), autre);
    // Un clic sur le nom du calque courant, dans le panneau : il redevient le calque choisi, la caméra est lâchée.
    let nom = nom_dans_le_panneau_calques(&b, "Carte 0");
    cliquer(&mut b, nom.center(), Modifiers::NONE);
    pointeur_ailleurs(&mut b);
    assert_eq!((b.app.espace3d.sel.clone(), b.app.cur_layer), (vec![c[0]], 0), "{}", b.app.status);
    assert_eq!(rang_peint(&b, "Carte 0"), courant);
    // Prendre un outil de dessin dans la vue (sa case, sa touche) : de même, pour chacun.
    for outil in [Tool::Brush, Tool::Pencil, Tool::Eraser, Tool::Mixer, Tool::Select] {
        b.app.set_tool(Tool::Trajectoire);
        b.app.e3_choisir_la_camera(false);
        assert!(b.app.e3_camera_seule());
        b.app.set_tool(outil);
        b.chauffer(2);
        assert_eq!(b.app.espace3d.sel, vec![c[0]], "{outil:?} : la carte du calque courant, plus la caméra");
        assert!(!b.app.e3_camera_seule() && b.app.calque_montre_courant(0), "{outil:?}");
    }
    // Les cartes cachées : prendre le pinceau lâche quand même la caméra (aucune carte n'est alors choisie).
    b.app.set_tool(Tool::Trajectoire);
    b.app.e3_choisir_la_camera(false);
    b.app.espace3d.show_cards = false;
    b.app.set_tool(Tool::Brush);
    assert!(!b.app.espace3d.sel.contains(&CAMERA_ID) && b.app.calque_montre_courant(0), "{:?}", b.app.espace3d.sel);
    b.app.espace3d.show_cards = true;
    b.chauffer(2);
    // L'outil Trajectoire garde la caméra choisie : c'est elle qui suivra le chemin.
    b.app.e3_choisir_la_camera(false);
    b.app.set_tool(Tool::Trajectoire);
    b.chauffer(2);
    assert!(b.app.e3_camera_seule(), "{:?}", b.app.espace3d.sel);
    // Le pinceau en main, la caméra choisie ensuite (« choisir » du panneau Espace 3D, ou sa piste) : un trait dans la vue va
    // sur le calque courant, qui redevient le calque choisi.
    b.app.set_tool(Tool::Brush);
    b.app.e3_choisir_la_camera(false);
    b.chauffer(2);
    assert!(b.app.e3_camera_seule());
    let sur = sur_la_carte(&mut b, 0);
    geste(&mut b, &[sur, sur + eframe::egui::vec2(30.0, 12.0), sur + eframe::egui::vec2(50.0, 30.0)], Modifiers::NONE);
    pointeur_ailleurs(&mut b);
    assert_eq!(b.app.espace3d.sel, vec![c[0]], "{}", b.app.status);
    assert!(!b.app.e3_camera_seule() && b.app.calque_montre_courant(0));
    assert_eq!(rang_peint(&b, "Carte 0"), courant);
}
