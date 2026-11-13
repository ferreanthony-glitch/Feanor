//! Essais des « Clés d'images » d'une trajectoire dans l'espace (`docs/INTERFACE.md`, partie 152). Anthony, 2026-11-13,
//! après son essai : « Clés d'images ne crée pas de clés d'images dans la timeline. »
//!
//! Une carte qui suit un chemin dans l'espace en clés d'images a, à chaque jalon (le départ, chaque marque, la fin), une
//! **clé de dessin** : une copie du dessin du départ, exposée jusqu'au jalon suivant, comme sur la feuille. La copie n'est
//! ni déplacée, ni agrandie, ni tournée : c'est la carte qui porte le mouvement, par sa clé de position tenue du jalon.
//! Les copies sont suivies (`Trajectoire::dessins`) : elles ne sont pas refaites quand une marque glisse ou que le temps
//! est recalé, et ce qu'on y a redessiné reste.

use eframe::egui::{Color32, Modifiers};

use crate::document::{CelId, Document};
use crate::espace3dgizmo::CAMERA_ID;
use crate::espace3dmath as m;
use crate::perf_tests::Banc;
use crate::project;
use crate::raster::TILE;
use crate::trajectoire::Cles;
use crate::trajectoire3d::Sujet;
use crate::trajectoire3d_tests::{a_l_ecran, appuyer, carte, derniere_etape, doc_cartes, geste, lacher, ouvert, proche, tracer_au_sol, trajectoire, AU_BOUT};

/// Un carré noir de `n` pixels de côté, au point (`x0`, `y0`) du dessin `cel`.
fn peindre(doc: &mut Document, cel: CelId, x0: usize, y0: usize, n: usize) {
    let r = &mut doc.cels.get_mut(&cel).expect("le dessin").raster;
    for y in y0..y0 + n {
        for x in x0..x0 + n {
            let idx = r.tile_index_of(x, y);
            r.tile_mut(idx)[(y % TILE) * TILE + (x % TILE)] = Color32::BLACK;
        }
    }
}

/// Le dessin `cel` a de l'encre au point (`x`, `y`).
fn encre(b: &Banc, cel: CelId, x: usize, y: usize) -> bool {
    b.app.doc.cels[&cel].raster.get(x, y).a() > 0
}

/// Un plan en Espace 3D d'une carte (« Carte 0 », avec un carré dessiné en (100, 80)), la vue près du sol, l'outil
/// Trajectoire en main, les clés d'images proposées (celles d'origine). Rend le banc et le dessin du départ.
fn banc_images(cartes: usize) -> (Banc, CelId) {
    let mut doc = doc_cartes(cartes);
    let source = doc.panels[0].layers[0].cel_at(0).expect("un dessin au départ");
    peindre(&mut doc, source, 100, 80, 12);
    let mut b = ouvert(doc, true);
    b.app.traj.cles_propose = Cles::Images;
    let y = b.app.e3_world().sol_y();
    let vue = b.app.espace3d.cam.as_mut().expect("la vue");
    (vue.target, vue.distance) = ([0.0, y, 2.5], vue.distance * 0.4);
    b.chauffer(2);
    (b, source)
}

/// Le point du monde à la fraction `f` du chemin d'essai (de (−2, 1) à (2, 3) sur le sol).
fn sur_le_chemin(b: &Banc, f: f32) -> [f32; 3] {
    [-2.0 + 4.0 * f, b.app.e3_world().sol_y(), 1.0 + 2.0 * f]
}

/// Un petit trait en travers du chemin d'essai, à la fraction `f` : une marque.
fn marquer(b: &mut Banc, f: f32) {
    let (c, sens) = (a_l_ecran(b, sur_le_chemin(b, f)), (a_l_ecran(b, sur_le_chemin(b, f + 0.02)) - a_l_ecran(b, sur_le_chemin(b, f - 0.02))).normalized());
    geste(b, &[c - sens.rot90() * 25.0, c + sens.rot90() * 25.0], Modifiers::NONE);
}

/// Les dessins exposés par le calque `li`, image par image.
fn images(b: &Banc, li: usize) -> Vec<Option<CelId>> {
    (0..b.app.frame_count()).map(|f| b.app.layers()[li].cel_at(f)).collect()
}

/// Les images des clés de position du calque `li`, et si elles sont tenues.
fn cles_de_position(b: &Banc, li: usize) -> Vec<(usize, bool)> {
    b.app.layers()[li].pos_keys.iter().map(|k| (k.frame, k.tenue)).collect()
}

#[test]
fn image_keys_of_a_card_in_space_put_a_drawing_key_at_each_milestone_an_untouched_copy_of_the_start_drawing() {
    let (mut b, source) = banc_images(1);
    let (depart, etapes, dessins_avant) = (carte(&b, 0, 0), b.app.history.undo_len(), b.app.doc.cels.len());
    tracer_au_sol(&mut b);
    let t = trajectoire(&b, 0);
    assert_eq!((t.cles, t.images_des_jalons(24)), (Cles::Images, vec![0, 23]));
    // Un dessin par jalon : au départ le dessin d'origine lui-même, à la fin une copie à elle.
    assert_eq!((t.source, t.dessins.len()), (Some(source), 2), "{}", b.app.status);
    let bout = t.dessins[1].dessin;
    assert_eq!(t.dessins[0].dessin, source);
    assert_ne!(bout, source, "la fin a son propre dessin, qu'on peut redessiner");
    let attendu: Vec<Option<CelId>> = (0..24).map(|f| Some(if f < 23 { source } else { bout })).collect();
    assert_eq!(images(&b, 0), attendu, "le départ est exposé jusqu'à la fin, qui a sa clé de dessin");
    assert_eq!(b.app.doc.cels.len(), dessins_avant + 1, "une seule copie a été faite");
    // La copie n'est ni déplacée, ni agrandie, ni tournée : la même encre, au même endroit du dessin.
    assert!(t.dessins.iter().all(|d| (d.dx, d.dy, d.angle) == (0.0, 0.0, 0.0)));
    assert!(encre(&b, bout, 105, 85) && encre(&b, bout, 100, 80) && encre(&b, bout, 111, 91), "le carré est à sa place dans la copie");
    assert!(!encre(&b, bout, 99, 85) && !encre(&b, bout, 112, 85) && !encre(&b, bout, 105, 79) && !encre(&b, bout, 105, 92), "et rien autour");
    // C'est la carte qui porte le mouvement : une clé de position tenue à chaque jalon.
    assert_eq!(cles_de_position(&b, 0), vec![(0, true), (23, true)]);
    assert!(proche(carte(&b, 0, 22).pos, depart.pos, 1e-3), "tenue au départ jusqu'à la fin");
    assert!(proche(carte(&b, 0, 23).pos, m::add(depart.pos, AU_BOUT), 0.05), "puis au bout du chemin : {:?}", carte(&b, 0, 23).pos);
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Trajectoire".to_string(), etapes + 1), "une seule étape d'annulation");
    assert!(!b.app.status.contains("dessine d'abord"), "{}", b.app.status);

    // Une marque : la timeline se scinde là aussi. Trois jalons, trois dessins distincts.
    marquer(&mut b, 0.15);
    let t = trajectoire(&b, 0);
    assert_eq!((t.marques.len(), t.images_des_jalons(24)), (1, vec![0, 12, 23]), "{}", b.app.status);
    assert_eq!(t.dessins.len(), 3);
    let (d0, d1, d2) = (t.dessins[0].dessin, t.dessins[1].dessin, t.dessins[2].dessin);
    assert!(d0 == source && d1 != d0 && d2 != d0 && d2 != d1, "trois dessins distincts : {d0}, {d1}, {d2}");
    let attendu: Vec<Option<CelId>> = (0..24).map(|f| Some(if f < 12 { d0 } else if f < 23 { d1 } else { d2 })).collect();
    assert_eq!(images(&b, 0), attendu, "une clé de dessin au départ, à la marque et à la fin");
    assert!([d1, d2].iter().all(|&d| encre(&b, d, 105, 85) && !encre(&b, d, 99, 85)), "chaque copie est le dessin du départ, tel quel");
    assert_eq!(cles_de_position(&b, 0), vec![(0, true), (12, true), (23, true)]);
    assert!(proche(carte(&b, 0, 12).pos, m::add(depart.pos, m::scale(AU_BOUT, t.marques[0])), 0.05), "à la marque, la carte est à la marque");
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Marque de trajectoire".to_string(), etapes + 2));
    // Une annulation retire la marque et sa clé de dessin ; une seconde, le chemin et tout ce qu'il a posé.
    b.app.undo();
    assert_eq!(trajectoire(&b, 0).dessins.len(), 2);
    assert_eq!(images(&b, 0)[12], Some(source), "plus de clé de dessin à l'image de la marque");
    b.app.undo();
    assert!(b.app.layers()[0].trajectoire.is_none() && b.app.layers()[0].pos_keys.is_empty());
    assert_eq!(images(&b, 0), vec![Some(source); 24], "le dessin du départ partout, comme avant");
    b.app.redo();
    b.app.redo();
    assert_eq!(images(&b, 0), attendu);
}

#[test]
fn sliding_a_mark_or_retiming_it_moves_its_key_and_keeps_what_was_redrawn_on_its_copy() {
    let (mut b, source) = banc_images(1);
    tracer_au_sol(&mut b);
    marquer(&mut b, 0.15);
    let depart = carte(&b, 0, 0);
    let t = trajectoire(&b, 0);
    let (d1, d2) = (t.dessins[1].dessin, t.dessins[2].dessin);
    // On redessine la pose de la marque : un second carré sur sa copie.
    peindre(&mut b.app.doc, d1, 300, 200, 10);
    let (dessins, tous, etapes) = (t.dessins.clone(), b.app.doc.cels.len(), b.app.history.undo_len());
    // Glisser la marque le long du chemin, dans la vue. Pendant le geste : la carte suit, aucun dessin n'est refait.
    let (prise, plus_loin) = (a_l_ecran(&b, sur_le_chemin(&b, t.marques[0])), a_l_ecran(&b, sur_le_chemin(&b, 0.6)));
    appuyer(&mut b, &[prise, plus_loin], Modifiers::NONE);
    assert_eq!((trajectoire(&b, 0).dessins, b.app.doc.cels.len()), (dessins.clone(), tous), "pendant le geste, les dessins ne sont pas refaits");
    lacher(&mut b, plus_loin, Modifiers::NONE);
    let glissee = trajectoire(&b, 0);
    assert!((glissee.marques[0] - 0.6).abs() < 0.03, "{}", glissee.marques[0]);
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Glisser une marque".to_string(), etapes + 1));
    assert_eq!((glissee.dessins.clone(), b.app.doc.cels.len()), (dessins.clone(), tous), "au lâcher non plus : les mêmes dessins, aucune copie de plus");
    assert_eq!((images(&b, 0)[11], images(&b, 0)[12], images(&b, 0)[23]), (Some(source), Some(d1), Some(d2)));
    assert!(encre(&b, d1, 305, 205), "ce qu'on a redessiné sur la pose de la marque est toujours là");
    assert!(proche(carte(&b, 0, 12).pos, m::add(depart.pos, m::scale(AU_BOUT, glissee.marques[0])), 0.05), "sa clé de position tenue a suivi la marque");
    // Sur la timeline (le montage des clés), glisser la clé de dessin de la marque de l'image 12 à l'image 6 la recale.
    let (plan, calque) = (b.app.doc.panels[0].id, b.app.layers()[0].id);
    b.app.key_sel = vec![crate::montagekeys::KeyRef { panel: plan, layer: calque, start: 12 }];
    assert!(b.app.begin_keys_drag(12));
    b.app.drag_keys_mode(6, crate::handles::KeyMove::Single);
    b.app.end_keys_drag();
    let recalee = trajectoire(&b, 0);
    assert_eq!(recalee.images_des_jalons(24), vec![0, 6, 23], "{}", b.app.status);
    assert_eq!((recalee.dessins.clone(), b.app.doc.cels.len()), (dessins.clone(), tous), "la clé a bougé, son dessin est le même");
    assert_eq!((images(&b, 0)[5], images(&b, 0)[6], images(&b, 0)[22], images(&b, 0)[23]), (Some(source), Some(d1), Some(d1), Some(d2)));
    assert_eq!(cles_de_position(&b, 0), vec![(0, true), (6, true), (23, true)], "sa clé de position tenue aussi");
    assert!(encre(&b, d1, 305, 205));
    // Glisser sa clé de position sur la timeline fait de même (`trajectoire_recaler`).
    assert!(b.app.trajectoire_recaler(0, 6, 9));
    assert_eq!(trajectoire(&b, 0).images_des_jalons(24), vec![0, 9, 23]);
    assert_eq!((images(&b, 0)[8], images(&b, 0)[9], b.app.doc.cels.len()), (Some(source), Some(d1), tous));
    // « Finir à la tête de lecture » recale la fin : sa clé de dessin la suit.
    assert!(b.app.trajectoire_recaler(0, 23, 20));
    assert_eq!((images(&b, 0)[19], images(&b, 0)[20], images(&b, 0)[23]), (Some(d1), Some(d2), Some(d2)));
    // La marque arrachée : deux jalons, deux clés de dessin, sans copie de plus.
    let prise = a_l_ecran(&b, sur_le_chemin(&b, trajectoire(&b, 0).marques[0]));
    geste(&mut b, &[prise, prise + eframe::egui::vec2(0.0, -90.0)], Modifiers::NONE);
    let sans = trajectoire(&b, 0);
    assert!(sans.marques.is_empty(), "{}", b.app.status);
    assert_eq!((sans.dessins.len(), b.app.doc.cels.len()), (2, tous));
    assert_eq!(derniere_etape(&b), "Retirer une marque");
    let exposes = images(&b, 0);
    assert!((0..20).all(|f| exposes[f] == Some(source)) && (20..24).all(|f| exposes[f] == Some(sans.dessins[1].dessin)), "{exposes:?}");
}

#[test]
fn switching_kinds_detaching_and_erasing_treat_the_drawing_keys_as_on_the_sheet() {
    let (mut b, source) = banc_images(1);
    let id = b.app.layers()[0].id;
    tracer_au_sol(&mut b);
    marquer(&mut b, 0.15);
    let en_images = (trajectoire(&b, 0), images(&b, 0), cles_de_position(&b, 0));
    let etapes = b.app.history.undo_len();
    // Clés d'interpolation : les dessins posés s'en vont, le dessin du départ tient partout, une clé de position par image.
    b.app.e3_traj_sorte(&[Sujet::Carte(id)], Cles::Interpolation);
    let t = trajectoire(&b, 0);
    assert!(t.cles == Cles::Interpolation && t.dessins.is_empty() && t.source.is_none());
    assert_eq!(images(&b, 0), vec![Some(source); 24]);
    assert_eq!(b.app.layers()[0].pos_keys.len(), 24);
    assert!(b.app.layers()[0].pos_keys.iter().all(|k| !k.tenue));
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Sorte des clés de la trajectoire".to_string(), etapes + 1));
    assert_eq!(b.app.traj.cles_propose, Cles::Interpolation, "la sorte choisie est proposée au chemin suivant");
    // Une annulation rend les clés d'images, leurs dessins et leurs clés tenues.
    b.app.undo();
    assert_eq!((trajectoire(&b, 0), images(&b, 0), cles_de_position(&b, 0)), en_images);
    // Et de nouveau des clés d'images, depuis les clés d'interpolation : les dessins sont posés aux jalons.
    b.app.e3_traj_sorte(&[Sujet::Carte(id)], Cles::Interpolation);
    b.app.e3_traj_sorte(&[Sujet::Carte(id)], Cles::Images);
    let t = trajectoire(&b, 0);
    assert_eq!((t.dessins.len(), t.source), (3, Some(source)));
    let (d1, d2) = (t.dessins[1].dessin, t.dessins[2].dessin);
    assert!(d1 != source && d2 != source && d1 != d2);
    assert_eq!((images(&b, 0)[11], images(&b, 0)[12], images(&b, 0)[23]), (Some(source), Some(d1), Some(d2)));
    assert_eq!(cles_de_position(&b, 0), vec![(0, true), (12, true), (23, true)]);
    // « Détacher » : le chemin s'en va, ce qu'il a posé reste (les clés de dessin et les clés de position).
    let poses = (images(&b, 0), cles_de_position(&b, 0));
    b.app.e3_traj_retirer(&[Sujet::Carte(id)], false);
    assert!(b.app.layers()[0].trajectoire.is_none());
    assert_eq!((images(&b, 0), cles_de_position(&b, 0)), poses);
    b.app.undo();
    // « Effacer » : le chemin et ce qu'il a posé s'en vont, en une étape ; le dessin du départ tient partout.
    let etapes = b.app.history.undo_len();
    b.app.e3_traj_retirer(&[Sujet::Carte(id)], true);
    assert!(b.app.layers()[0].trajectoire.is_none() && b.app.layers()[0].pos_keys.is_empty());
    assert_eq!(images(&b, 0), vec![Some(source); 24]);
    assert_eq!((derniere_etape(&b), b.app.history.undo_len()), ("Effacer la trajectoire".to_string(), etapes + 1));
    b.app.undo();
    assert_eq!((images(&b, 0), cles_de_position(&b, 0)), poses, "une annulation rend tout");
}

#[test]
fn image_keys_in_space_need_a_start_drawing_leave_the_camera_alone_and_survive_a_save() {
    // Une carte et la caméra ensemble, en clés d'images : la carte a ses dessins, la caméra n'en a pas (elle n'a pas de dessin).
    let (mut b, source) = banc_images(2);
    let id = b.app.layers()[0].id;
    b.app.espace3d.sel = vec![crate::espace3d::card_selection(id), CAMERA_ID];
    tracer_au_sol(&mut b);
    marquer(&mut b, 0.15);
    let t = trajectoire(&b, 0);
    assert_eq!((t.dessins.len(), t.source), (3, Some(source)));
    let camera = b.app.doc.panels[0].trajectoire_camera.clone().expect("la caméra suit le chemin");
    assert!(camera.cles == Cles::Images && camera.dessins.is_empty() && camera.source.is_none());
    assert!(b.app.doc.panels[0].camera.is_empty(), "et toujours aucune clé de caméra");
    assert!(b.app.layers()[1].trajectoire.is_none(), "la carte qui n'est pas choisie ne reçoit rien");
    // Enregistré, relu : les dessins des jalons sont toujours suivis (un calcul de plus n'en referait pas).
    let dir = std::env::temp_dir().join(format!("feanor-trajectoire3d-images-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    let manifest = dir.join("I.shot");
    project::save(&b.app.doc, &manifest, &mut Default::default()).expect("enregistré");
    let relu = project::load(&manifest).expect("relu").doc;
    assert_eq!(relu.panels[0].layers[0].trajectoire.as_ref(), Some(&t), "la trajectoire revient avec ses dessins");
    assert_eq!(relu.panels[0].layers[0].frames, b.app.layers()[0].frames);
    std::fs::remove_dir_all(&dir).ok();

    // Sans dessin au départ : rien à copier, la barre d'état le dit ; la carte, elle, saute quand même de jalon en jalon.
    let mut doc = doc_cartes(1);
    doc.panels[0].layers[0].frames = Vec::new();
    let mut b = ouvert(doc, true);
    b.app.traj.cles_propose = Cles::Images;
    tracer_au_sol(&mut b);
    let t = trajectoire(&b, 0);
    assert!(t.cles == Cles::Images && t.dessins.is_empty());
    assert!(b.app.status.contains("dessine d'abord"), "{}", b.app.status);
    assert_eq!(cles_de_position(&b, 0), vec![(0, true), (23, true)]);
    assert!(images(&b, 0).iter().all(Option::is_none), "aucune clé de dessin n'est inventée");
}
