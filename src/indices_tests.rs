//! Passe de débogage (Anthony, 2026-10-22 : « tu peux me faire une passe de débogage ? ») : les rangs périmés après un
//! changement de structure. Deux plantages du même jour venaient de là (supprimer le dernier plan, réunir un plan à
//! mouvement depuis son clic droit) : un rang de plan, de calque ou de case gardé d'avant un changement du film, puis lu
//! sur le film d'après. Chaque essai rejoue le geste qui plantait (ou qui visait le mauvais plan).

use std::sync::Arc;

use eframe::egui::{epaint::Shape, vec2, Event, Key, Modifiers, PointerButton, Pos2};

use crate::app::Mode;
use crate::cooperation::{Delais, Horloge, Session};
use crate::cooperation_tests::{confirmer, film, partager, Poste};
use crate::depot::DossierPartage;
use crate::document::{Document, Layer, LayerKind};
use crate::equipe::Identite;
use crate::fx::{Effect, EffectKind};
use crate::perf_tests::{banc, banc_taille, Banc};
use crate::project::{self, Loaded};
use crate::verrous::Cible;
use crate::vignettes_camera_tests::{ctrl_glisser, largeur_cadre_ecran, pas, K};
use crate::vignettes_tests::{ouvert, quatre_plans, vignette};
use crate::workspace::Espace;

const DT: f64 = 1.0 / 60.0;

/// Les textes peints à la dernière image, avec leur centre à l'écran.
fn textes(b: &Banc) -> Vec<(String, Pos2)> {
    fn dans(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
        match shape {
            Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0)),
            Shape::Vec(v) => v.iter().for_each(|s| dans(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        dans(&s.shape, &mut out);
    }
    out
}

/// Une image d'écran dont on garde les formes : les textes peints, avec leur centre.
fn textes_a_l_ecran(b: &mut Banc) -> Vec<(String, Pos2)> {
    b.garder_formes = true;
    b.image(Vec::new(), DT);
    let t = textes(b);
    b.garder_formes = false;
    t
}

/// Le centre du texte `texte` le plus à gauche de l'écran (la timeline et la colonne du montage sont à gauche du panneau
/// Calques, qui montre les mêmes noms).
fn le_plus_a_gauche(b: &mut Banc, texte: &str) -> Pos2 {
    let t = textes_a_l_ecran(b);
    t.iter()
        .filter(|(s, _)| s == texte)
        .map(|(_, p)| *p)
        .min_by(|a, b| a.x.total_cmp(&b.x))
        .unwrap_or_else(|| panic!("« {texte} » n'est pas à l'écran : {:?}", t.iter().map(|(s, _)| s).collect::<Vec<_>>()))
}

/// Ouvre le menu du clic droit en `p` et rend le centre de son entrée `entree` (deux images pour qu'il se pose).
fn menu(b: &mut Banc, p: Pos2, entree: &str) -> Pos2 {
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], DT);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], DT);
    b.image(Vec::new(), DT);
    let t = textes_a_l_ecran(b);
    t.iter().find(|(s, _)| s == entree).map(|(_, p)| *p).unwrap_or_else(|| panic!("« {entree} » n'est pas dans le menu : {:?}", t.iter().map(|(s, _)| s).collect::<Vec<_>>()))
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], DT);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], DT);
}

fn touche(b: &mut Banc, key: Key, modifiers: Modifiers) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }], DT);
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers }], DT);
}

// ---------------------------------------------------------------------------------------------- Vignettes

/// Quatre plans (le premier a trois cases), le deuxième et le dernier assez longs pour un mouvement de caméra.
fn deux_longs() -> Document {
    let mut doc = quatre_plans();
    doc.set_duration(1, 25);
    let last = doc.panels.len() - 1;
    doc.set_duration(last, 25);
    doc
}

/// Un mouvement de caméra posé dans la case `k` (Ctrl + Maj + glisser d'une demi-largeur de cadre vers la droite).
fn mouvement(b: &mut Banc, k: usize) {
    let w = largeur_cadre_ecran(b);
    let case = vignette(b, k);
    ctrl_glisser(b, case.center(), &pas(case.center(), vec2(w * 0.5, 0.0), 6), case.center() + vec2(w * 0.5, 0.0), true);
    b.chauffer(3);
}

/// « Réunir avec le plan précédent » depuis le clic droit d'un plan à mouvement, quand le DERNIER plan a aussi un mouvement :
/// les tuiles des empreintes se peignent avant les cases, et la suivante lisait encore le rang d'avant la réunion (le dernier
/// plan n'existait plus à ce rang : « index out of bounds » dans `render::panel_plan_at`).
#[test]
fn joining_a_moving_plan_when_the_last_plan_moves_too_does_not_crash() {
    let mut b = ouvert(deux_longs(), 6);
    // Le dernier plan d'abord (sa case ne bouge pas quand celle du deuxième grandit ensuite).
    let derniere = b.app.board_layout().cards.len() - 1;
    mouvement(&mut b, derniere);
    mouvement(&mut b, K);
    let n = b.app.doc.panels.len();
    assert!(b.app.doc.panels[1].camera.len() >= 2 && b.app.doc.panels[n - 1].camera.len() >= 2, "deux plans bougent");
    let case = vignette(&b, K);
    let p = menu(&mut b, case.center(), "Réunir avec le plan précédent");
    clic(&mut b, p);
    b.chauffer(4);
    assert_eq!(b.app.doc.panels.len(), n - 1, "les deux plans sont réunis");
    assert!(b.app.cur_panel < b.app.doc.panels.len());
}

/// Le menu du clic droit d'une case reste ouvert pendant un Ctrl+Z qui décale les plans : il vise toujours le plan sur lequel
/// on l'a ouvert (il gardait son rang, que l'annulation donnait au plan suivant : « Animer cette image » animait un autre plan).
#[test]
fn the_case_menu_keeps_its_plan_when_an_undo_shifts_the_plans() {
    let mut b = ouvert(quatre_plans(), 6);
    // Une copie du premier plan (une étape d'annulation) : les plans d'après sont décalés d'un rang.
    b.app.select_panel(0);
    b.app.duplicate_panel();
    b.chauffer(3);
    let n = b.app.doc.panels.len();
    let vise = b.app.doc.panels[3].id;
    let k = b.app.board_layout().cards.iter().position(|c| c.panel == 3).expect("la case du plan");
    let case = vignette(&b, k);
    let p = menu(&mut b, case.center(), "Animer cette image");
    // Ctrl+Z, menu ouvert : la copie s'en va, le plan visé recule d'un rang.
    touche(&mut b, Key::Z, Modifiers::COMMAND);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), n - 1, "l'annulation a retiré la copie");
    let encore = textes_a_l_ecran(&mut b).iter().any(|(s, _)| s == "Animer cette image");
    if encore {
        clic(&mut b, p);
        b.chauffer(2);
    }
    if b.app.mode == Mode::Animation {
        assert_eq!(b.app.doc.panels[b.app.cur_panel].id, vise, "« Animer cette image » anime le plan du clic droit, pas son voisin");
    }
}

/// Réunir le plan courant et le suivant (la barre noire entre leurs cases) quand le calque du bas du suivant ne ressemble à aucun
/// des siens : ce calque se pose tout en bas du plan réuni, et le calque courant gardait son rang, devenu celui du calque du dessous
/// (le trait suivant allait dans un autre calque que celui qu'on avait choisi).
#[test]
fn joining_the_next_plan_keeps_the_current_layer() {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.panels[0].layers = vec![Layer::new("Calque A0", LayerKind::Bitmap), Layer::new("Calque A1", LayerKind::Bitmap)];
    let mut fond = Layer::new("Fond B", LayerKind::Bitmap);
    fond.opacity = 0.5;
    doc.panels[1].layers = vec![fond];
    let mut b = ouvert(doc, 6);
    b.app.select_panel(0);
    b.app.cur_layer = 1;
    b.chauffer(3);
    let choisi = b.app.layer().id;
    let layout = b.app.board_layout();
    let k = layout.bars.iter().position(|r| r.after == 0 && r.cut).expect("la barre noire entre les deux plans");
    let barre = b.app.vignettes.xf.rect(layout.bars[k].zone);
    clic(&mut b, barre.center());
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), 1, "les deux plans sont réunis");
    assert_eq!(b.app.layers().len(), 3, "le calque du second plan s'est ajouté");
    assert_eq!(b.app.layer().id, choisi, "le calque courant est toujours celui qu'on avait choisi");
}

// ---------------------------------------------------------------------------------------------- calques

/// Un plan de quatre calques : A et B rangés dans le dossier F, et D au-dessus.
fn avec_dossier() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    let dossier = Layer::new_folder("Dossier F");
    let f = dossier.id;
    let mut a = Layer::new("Calque A", LayerKind::Bitmap);
    a.parent = Some(f);
    let mut bb = Layer::new("Calque B", LayerKind::Bitmap);
    bb.parent = Some(f);
    doc.panels[0].layers = vec![a, bb, dossier, Layer::new("Calque D", LayerKind::Bitmap)];
    doc.panels[0].length = 12;
    doc
}

/// Supprimer un dossier depuis le clic droit de sa ligne dans la timeline : ses calques partent avec lui, et les lignes d'après
/// (peintes dans la même image, juste après le menu) lisaient des rangs de calques qui n'existaient plus.
#[test]
fn deleting_a_folder_from_its_timeline_row_does_not_crash() {
    let mut b = banc_taille(avec_dossier(), false, 1600, 900);
    b.app.set_espace(Espace::Animation);
    b.app.cur_layer = 3;
    b.chauffer(6);
    let p = le_plus_a_gauche(&mut b, "Dossier F");
    let entree = menu(&mut b, p, "Supprimer");
    clic(&mut b, entree);
    b.chauffer(4);
    assert_eq!(b.app.layers().len(), 1, "le dossier et ses deux calques sont partis");
    assert!(b.app.cur_layer < b.app.layers().len());
}

/// La même chose depuis la colonne des pistes du montage.
#[test]
fn deleting_a_folder_from_its_montage_row_does_not_crash() {
    let mut b = banc_taille(avec_dossier(), false, 1600, 900);
    b.app.set_espace(Espace::Montage);
    b.app.cur_layer = 3;
    b.chauffer(6);
    let p = le_plus_a_gauche(&mut b, "Dossier F");
    let entree = menu(&mut b, p, "Supprimer");
    clic(&mut b, entree);
    b.chauffer(4);
    assert_eq!(b.app.layers().len(), 1, "le dossier et ses deux calques sont partis");
    assert!(b.app.cur_layer < b.app.layers().len());
}

// ---------------------------------------------------------------------------------------------- effets

/// « Remplacer les effets » depuis le clic droit du premier effet d'une pile de trois, par un seul effet copié : la pile se
/// peignait encore avec trois effets (« index out of bounds » sur le deuxième).
#[test]
fn replacing_the_effects_from_an_effect_menu_does_not_crash() {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.panels[0].layers = vec![Layer::new("Calque", LayerKind::Bitmap)];
    doc.panels[0].layers[0].fx = vec![Effect::new(EffectKind::Invert), Effect::new(EffectKind::Posterize), Effect::new(EffectKind::Threshold)];
    doc.panels[0].length = 12;
    // (Un écran haut : le panneau Effets partage sa colonne avec d'autres, la pile doit y tenir.)
    let mut b = banc_taille(doc, false, 1600, 2400);
    b.app.set_espace(Espace::Animation);
    b.chauffer(4);
    b.app.show_pane(crate::dock::Pane::Effects);
    b.chauffer(6);
    // Un seul effet copié : le dernier.
    b.app.fx_copy_effect(0, 2);
    let p = le_plus_a_gauche(&mut b, EffectKind::Invert.label());
    let entree = menu(&mut b, p, "Remplacer les effets");
    clic(&mut b, entree);
    b.chauffer(4);
    let kinds: Vec<EffectKind> = b.app.layers()[0].fx.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EffectKind::Threshold], "la pile est remplacée par l'effet copié");
}

// ---------------------------------------------------------------------------------------------- son

/// « Supprimer la piste » depuis le clic droit du nom de la première de deux pistes son : la colonne des noms peignait encore la
/// seconde à son ancien rang (« index out of bounds »).
#[test]
fn deleting_a_sound_track_from_its_name_does_not_crash() {
    let mut doc = quatre_plans();
    doc.audio_tracks.clear();
    doc.add_audio_track();
    doc.add_audio_track();
    let mut b = banc_taille(doc, false, 1600, 900);
    b.app.set_espace(Espace::Montage);
    b.chauffer(6);
    let p = le_plus_a_gauche(&mut b, "Son 1");
    let entree = menu(&mut b, p, "Supprimer la piste");
    clic(&mut b, entree);
    b.chauffer(4);
    assert_eq!(b.app.doc.audio_tracks.len(), 1);
    assert_eq!(b.app.doc.audio_tracks[0].name, "Son 2");
}

// ---------------------------------------------------------------------------------------------- travail à plusieurs

/// Une application qui a ouvert un film partagé comme Anthony, et un poste d'essai pour Léa (comme `cooperation_ui_tests`).
fn app_partagee(nom: &str, n: usize) -> (Banc, Poste, Horloge) {
    let racine = crate::fiabilite_fixture::dossier(&format!("coop-rangs-{nom}"));
    let manifeste = partager(&film(n), &racine);
    let h = Horloge::reglable(1_760_000_000_000);
    let Loaded { doc, partage, written, .. } = project::load(&manifeste).expect("projet partagé");
    let mut b = banc(doc, false);
    b.app.path = Some(manifeste.clone());
    b.app.written = written;
    let moi = Identite { id: "anthony".into(), nom: "Anthony".into(), couleur: [220, 60, 60], machine: "poste".into(), seance: "s1".into() };
    let depot = Arc::new(DossierPartage::nouveau(&manifeste));
    let s = Session::rejoindre(depot, moi, partage.expect("partagé"), &mut b.app.doc, Delais::default(), h.clone(), false, None).expect("session");
    b.app.partage = Some(s);
    let lea = Poste::ouvrir(&manifeste, "Lea", &h);
    (b, lea, h)
}

/// Léa déplace un plan du montage pendant qu'Anthony regarde le dernier : quand la structure arrive chez Anthony, son plan courant
/// reste le plan qu'il regardait (il gardait son rang, qui était devenu celui d'un autre plan).
#[test]
fn the_current_plan_stays_the_same_plan_when_a_teammate_reorders_the_montage() {
    let (mut b, mut lea, h) = app_partagee("ordre", 3);
    b.app.select_panel(2);
    b.image(Vec::new(), 0.05);
    let vu = b.app.doc.panels[2].id;
    assert!(lea.s.prendre(&[Cible::Montage], false).remove(0).1.is_ok());
    confirmer(&h);
    lea.tour();
    let premier = lea.doc.panels[0].id;
    lea.doc.move_plans(&[premier], None).expect("déplacer");
    lea.s.apres_structure(&lea.doc);
    lea.enregistrer();
    for _ in 0..6 {
        lea.tour();
        b.app.partage.as_ref().expect("session").pomper();
        b.image(Vec::new(), 0.05);
    }
    assert_eq!(b.app.doc.panels.last().map(|p| p.id), Some(premier), "la structure de Léa est arrivée");
    assert_eq!(b.app.doc.panels[b.app.cur_panel].id, vu, "le plan courant est toujours celui qu'Anthony regardait");
}

// ---------------------------------------------------------------------------------------------- toutes les vues, toutes les opérations

/// Un film qui a de tout : quatre plans (le premier en trois cases, le deuxième et le dernier avec un mouvement de caméra), des
/// calques rangés dans un dossier, deux pistes son, des effets.
fn film_complet() -> Document {
    let mut doc = deux_longs();
    for i in [1, doc.panels.len() - 1] {
        let a = doc.panel_camera(i, 0);
        let n = doc.panels[i].frame_count();
        doc.panels[i].camera = vec![crate::camera::CameraKey::new(0, a), crate::camera::CameraKey::new(n - 1, a.shifted(40.0, 0.0))];
    }
    let dossier = Layer::new_folder("Dossier F");
    let f = dossier.id;
    let mut a = Layer::new("Calque A", LayerKind::Bitmap);
    a.parent = Some(f);
    let mut bb = Layer::new("Calque B", LayerKind::Vector);
    bb.parent = Some(f);
    let mut d = Layer::new("Calque D", LayerKind::Bitmap);
    d.fx = vec![Effect::new(EffectKind::Invert), Effect::new(EffectKind::Posterize)];
    doc.panels[1].layers = vec![a, bb, dossier, d];
    doc.audio_tracks.clear();
    doc.add_audio_track();
    doc.add_audio_track();
    doc
}

/// Chaque opération qui change la structure (plans, calques, images, onglets, projet), dans chaque vue, depuis l'état le plus « au
/// bord » (dernier plan, dernier calque, dernière image, tout choisi), puis Ctrl+Z et Ctrl+Y : aucune image d'écran ne plante, et le
/// plan et le calque courants restent dans le film.
#[test]
fn every_structure_change_in_every_view_keeps_the_app_standing() {
    type Op = (&'static str, fn(&mut crate::app::App));
    let ops: &[Op] = &[
        ("supprimer le dernier plan", |a| {
            let id = a.doc.panels[a.cur_panel].id;
            a.delete_panels(&[id]);
        }),
        ("supprimer tous les plans sauf le premier", |a| {
            let ids: Vec<u64> = a.doc.panels.iter().skip(1).map(|p| p.id).collect();
            a.delete_panels(&ids);
        }),
        ("réunir avec le plan précédent", |a| {
            a.select_panel(2);
            a.join_previous_scene();
        }),
        ("réunir avec le plan suivant", |a| {
            a.select_panel(0);
            a.merge_next_panel();
        }),
        ("fusionner les plans choisis", |a| a.merge_chosen_panels()),
        ("couper le plan", |a| {
            a.select_panel(1);
            a.cur_frame = 7;
            a.split_current_panel();
        }),
        ("dupliquer le plan", |a| a.duplicate_panel()),
        ("nouveau plan, nouvelle séquence", |a| {
            a.new_panel();
            a.new_sequence();
        }),
        ("déplacer le dernier plan au début", |a| {
            let n = a.doc.panels.len();
            a.move_panel(n - 1, 0);
        }),
        ("copier, coller des plans", |a| {
            a.copy_panels();
            a.paste_panels();
        }),
        ("calques : ajouter, dupliquer, fusionner, déplacer", |a| {
            a.select_panel(1);
            a.add_layer(LayerKind::Bitmap);
            a.duplicate_layer();
            a.merge_layer_down();
            a.move_layer(false);
        }),
        ("supprimer le dossier et ses calques", |a| {
            a.select_panel(1);
            a.select_layer(2);
            a.layer_sel.clear();
            a.delete_chosen_layers();
        }),
        ("supprimer tous les calques choisis", |a| {
            a.select_panel(1);
            a.layer_sel = a.layers().iter().map(|l| l.id).collect();
            a.delete_chosen_layers();
        }),
        ("images : nouvelle, supprimer, marque", |a| {
            a.new_image(false);
            a.delete_image();
            a.toggle_mark();
        }),
        ("un autre projet", |a| a.load_doc(quatre_plans())),
        ("onglet neuf puis retour", |a| {
            a.nouvel_onglet();
            a.activer_onglet(0);
        }),
    ];
    let vues: [(&str, Espace, crate::app::BoardView); 4] = [
        ("Vignettes", Espace::Vignettes, crate::app::BoardView::Panels),
        ("Storyboard", Espace::Storyboard, crate::app::BoardView::Scene),
        ("Montage", Espace::Montage, crate::app::BoardView::Scene),
        ("Animation", Espace::Animation, crate::app::BoardView::Scene),
    ];
    let mut echecs: Vec<String> = Vec::new();
    for (nom_vue, espace, board_view) in vues {
        for (nom, op) in ops {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut b = banc_taille(film_complet(), false, 1600, 900);
                b.app.set_espace(espace);
                b.app.board_view = board_view;
                b.chauffer(3);
                // Tout au bord : le dernier plan, son dernier calque, sa dernière image ; tout choisi.
                let n = b.app.doc.panels.len();
                b.app.select_panel(n - 1);
                b.app.cur_layer = b.app.layers().len() - 1;
                b.app.cur_frame = b.app.frame_count() - 1;
                b.app.board_sel = b.app.doc.panels.iter().map(|p| p.id).collect();
                b.app.frame_sel = Some((0, b.app.frame_count()));
                b.chauffer(2);
                op(&mut b.app);
                b.chauffer(3);
                for _ in 0..4 {
                    touche(&mut b, Key::Z, Modifiers::COMMAND);
                    b.chauffer(1);
                }
                for _ in 0..4 {
                    touche(&mut b, Key::Y, Modifiers::COMMAND);
                    b.chauffer(1);
                }
                b.chauffer(2);
                assert!(b.app.cur_panel < b.app.doc.panels.len() && b.app.cur_layer < b.app.layers().len(), "plan et calque courants dans le film");
            }));
            if let Err(e) = r {
                let m = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                echecs.push(format!("{nom_vue} / {nom} : {m}"));
            }
        }
    }
    assert!(echecs.is_empty(), "{} plantage(s) :\n{}", echecs.len(), echecs.join("\n"));
}