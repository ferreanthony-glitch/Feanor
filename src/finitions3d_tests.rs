//! Essais des finitions 3D (partie 201, `docs/INTERFACE.md`) : le zoom, le défilement et « Ajuster » de la timeline de la séquence
//! (`timelineseq.rs`) ; le sélecteur de séquence et les objets du décor copiés ou déplacés vers une autre séquence, par le menu et en
//! glissant (`seqdecor3d.rs`) ; qui a la main dans la vue (`quialamain.rs`) ; un calque neuf devant la caméra (`calquedevant.rs`).

use std::sync::Arc;

use eframe::egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::bulles::vus;
use crate::decor3d::{Decor3d, Modele3d, Objet3d, Place3d};
use crate::decor3drendu::Aspect;
use crate::dock::Pane;
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d::Monde;
use crate::espace3dmath as m;
use crate::miseenscene3d::CleObjet;
use crate::onglet3d::Ligne3d;
use crate::onglet3dui::{GRADUATIONS_SEQ, LIGNES_S3D, TIMELINE_SEQ};
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::clic;
use crate::poignees3d::objet_selection;
use crate::timelineseq;
use crate::workspace::Espace;

// ------------------------------------------------------------------------------------------------ les aides

fn modele_du_carre() -> Arc<Modele3d> {
    let octets = crate::decor3d_tests::carre(true, None, |_| {});
    let (scene, _) = crate::decor3dgltf::lire(&octets, None).unwrap();
    Arc::new(Modele3d::new(Arc::new(scene), Arc::new(octets)))
}

/// Un film : une séquence de trois plans de 24 images, puis une autre de deux plans ; la première a un décor de deux carrés
/// (« Maison », « Arbre »), la seconde aucun.
fn film() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    doc.add_sequence_after(2);
    doc.add_scene_after(3);
    for (k, p) in doc.panels.iter_mut().enumerate() {
        p.layers = vec![Layer::new(&format!("Fond {k}"), LayerKind::Vector), Layer::new(&format!("Perso {k}"), LayerKind::Vector)];
        p.set_length(24);
    }
    let seq = doc.sequence_of(0);
    let modele = modele_du_carre();
    let mut d = Decor3d::new(Aspect { ombres: false, ..Default::default() });
    for (nom, x) in [("Maison", -1.0), ("Arbre", 1.5)] {
        let id = doc.alloc();
        d.objets.push(Objet3d { id, nom: nom.into(), source: "carre.glb".into(), allege: Vec::new(), place: Place3d { position: [x, 0.0, -2.0], rotation_y: 0.0, echelle: 1.0 }, origine: [0.0; 3], aimante: false, modele: modele.clone(), couleur: None, cache: false, verrou: false, cles: Vec::new(), caches_dans: Vec::new() });
    }
    doc.decors3d.par_sequence.insert(seq, d);
    doc
}

fn banc_3d(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1600, 1800);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    b.app.set_espace(Espace::TroisD);
    if let Some(k) = b.app.colonne.rang_de(Pane::Scene3d) {
        b.app.colonne.panneaux[k].hauteur = Some(420.0);
    }
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn ou(nom: &str) -> Rect {
    *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

fn ids(doc: &Document, seq: u64) -> Vec<u64> {
    doc.decors3d.par_sequence.get(&seq).map_or_else(Vec::new, |d| d.objets.iter().map(|o| o.id).collect())
}

fn etapes(b: &Banc) -> Vec<String> {
    b.app.history.names().0.into_iter().map(String::from).collect()
}

fn ligne(l: Ligne3d) -> Rect {
    LIGNES_S3D.with(|v| v.borrow().iter().find(|x| x.0 == l).map(|x| x.1)).unwrap_or_else(|| panic!("{l:?}"))
}

fn fenetre(b: &Banc) -> timelineseq::Fenetre {
    b.app.o3.fenetre_seq.expect("la timeline de la séquence")
}

fn seqs(doc: &Document) -> (u64, u64) {
    let o = doc.sequence_order();
    (o[0], o[1])
}

// ------------------------------------------------------------------------------------------------ 1. la timeline de la séquence

#[test]
fn the_ruler_step_and_labels_follow_the_zoom() {
    assert_eq!(timelineseq::pas_de_la_regle(0.5, 24), 120, "loin : toutes les cinq secondes");
    assert_eq!(timelineseq::pas_de_la_regle(3.0, 24), 24, "une seconde");
    assert_eq!(timelineseq::pas_de_la_regle(12.0, 24), 5, "de près : des images");
    assert_eq!(timelineseq::pas_de_la_regle(80.0, 24), 1);
    assert_eq!(timelineseq::chiffre(48, 24, 24), "2 s");
    assert_eq!(timelineseq::chiffre(53, 24, 5), "2 s 05");
    assert_eq!(timelineseq::chiffre(24 * 150, 24, 24 * 30), "2 min 30");
    let piste = Rect::from_min_size(pos2(100.0, 0.0), vec2(720.0, 30.0));
    let f = timelineseq::Fenetre::de(piste, 48, 72, 4.0, 30.0);
    assert_eq!((f.visibles, f.depart), (18.0, 30.0));
    assert_eq!(f.f_de(f.x_de(48 + 35) + 1.0), 48 + 35);
    let f = timelineseq::Fenetre::de(piste, 48, 72, 4.0, 70.0);
    assert_eq!(f.depart, 54.0, "la fin de la séquence au bord droit, pas au-delà");
}

#[test]
fn the_sequence_timeline_zooms_scrolls_fits_and_keeps_the_playhead_in_view() {
    let mut b = banc_3d(film());
    let blocs = |_: &Banc| TIMELINE_SEQ.with(|t| t.borrow().clone()).expect("la timeline").1;
    let largeur0 = blocs(&b)[0].1.width();
    assert_eq!(b.app.o3.zoom_seq, 1.0);
    let pas0 = GRADUATIONS_SEQ.with(|g| g.borrow().0);
    // Les raccourcis du zoom de la bande : autour de la tête de lecture (au début).
    for _ in 0..4 {
        b.app.run_action("bande.zoom_avant");
    }
    b.chauffer(2);
    let z = b.app.o3.zoom_seq;
    assert!((z - 1.25f32.powi(4)).abs() < 1e-3, "{z}");
    assert!(blocs(&b)[0].1.width() > largeur0 * 2.0, "les plans s'élargissent");
    assert!(GRADUATIONS_SEQ.with(|g| g.borrow().0) < pas0, "la règle se gradue plus fin");
    let f = fenetre(&b);
    assert!(f.visibles < f.n as f32 && f.depart < 1.0, "la tête de lecture (au début) reste à sa place : {f:?}");
    // La molette fait défiler ; Ctrl + molette zoome.
    let (piste, _) = TIMELINE_SEQ.with(|t| t.borrow().clone()).unwrap();
    let dessus = piste.center();
    b.image(vec![Event::PointerMoved(dessus)], 1.0 / 60.0);
    let ev = b.molette(vec2(0.0, -120.0), Modifiers::NONE);
    b.image(ev, 1.0 / 60.0);
    b.chauffer(3);
    assert!(b.app.o3.debut_seq > 0.0, "la bande a défilé : {}", b.app.o3.debut_seq);
    let avant = b.app.o3.zoom_seq;
    b.touches = Modifiers::COMMAND;
    let ev = b.molette(vec2(0.0, 120.0), Modifiers::COMMAND);
    b.image(ev, 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
    assert!(b.app.o3.zoom_seq > avant, "Ctrl + molette : {} → {}", avant, b.app.o3.zoom_seq);
    // La réglette de zoom et « Ajuster », en tête de la bande.
    assert!(!vus::ou("Zoom de la timeline").is_empty());
    clic(&mut b, ou("Ajuster la timeline").center(), Modifiers::NONE);
    assert_eq!((b.app.o3.zoom_seq, b.app.o3.debut_seq), (1.0, 0.0));
    assert!((blocs(&b)[0].1.width() - largeur0).abs() < 1.0);
    // `bande.tout_voir` aussi.
    b.app.run_action("bande.zoom_avant");
    b.app.run_action("bande.tout_voir");
    assert_eq!(b.app.o3.zoom_seq, 1.0);
    // Pendant la lecture, la tête de lecture reste dans la vue : la bande tourne la page.
    b.app.o3.zoom_seq = 8.0;
    b.app.o3.debut_seq = 0.0;
    b.chauffer(2);
    b.app.goto_global(60);
    b.app.playing = true;
    b.chauffer(2);
    b.app.playing = false;
    let f = fenetre(&b);
    let tete = b.app.global_frame() as f32;
    assert!(tete >= f.depart && tete < f.depart + f.visibles, "la tête {tete} dans [{}, {})", f.depart, f.depart + f.visibles);
}

// ------------------------------------------------------------------------------------------------ 3. d'une séquence à l'autre

#[test]
fn the_sequence_switcher_lists_the_sequences_and_goes_to_the_first_plan() {
    let mut b = banc_3d(film());
    let (s1, s2) = seqs(&b.app.doc);
    clic(&mut b, ou("Séquence de l'onglet 3D").center(), Modifiers::NONE);
    let liste = crate::seqdecor3d::SEQUENCES_LISTE.with(|l| l.borrow().clone());
    assert_eq!(liste.iter().map(|x| x.0).collect::<Vec<_>>(), vec![s1, s2]);
    clic(&mut b, liste[1].1.center(), Modifiers::NONE);
    assert_eq!(b.app.cur_panel, 3, "le premier plan de la seconde séquence");
    assert_eq!(b.app.doc.sequence_of(b.app.cur_panel), s2);
    assert!(!b.app.o3.sequences_ouvertes);
    assert!(b.app.o3_nom_de_sequence(s2).starts_with("Séquence "));
    b.app.doc.sequences.iter_mut().find(|s| s.id == s1).unwrap().name = "La cuisine".into();
    assert!(b.app.o3_nom_de_sequence(s1).ends_with(" · La cuisine"), "{}", b.app.o3_nom_de_sequence(s1));
}

#[test]
fn copying_and_moving_objects_to_another_sequence_by_the_menu_is_one_step_and_keeps_keys_and_colours() {
    let mut b = banc_3d(film());
    let (s1, s2) = seqs(&b.app.doc);
    let i = ids(&b.app.doc, s1);
    let plan0 = b.app.doc.panels[0].id;
    {
        let d = b.app.doc.decors3d.par_sequence.get_mut(&s1).unwrap();
        let o = d.objet_mut(i[0]).unwrap();
        o.couleur = Some(3);
        o.cles = vec![CleObjet { image: 0, place: o.place }, CleObjet { image: 30, place: Place3d { position: [0.0, 0.0, -3.0], ..o.place } }];
        o.caches_dans = vec![plan0];
    }
    // Le clic droit d'une ligne d'objet ouvre son menu.
    b.chauffer(2);
    let r = ligne(Ligne3d::Objet(i[0]));
    let p = pos2(r.left() + 60.0, r.center().y);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(2);
    let textes = textes(&b);
    assert!(textes.iter().any(|t| t == "Copier vers la séquence") && textes.iter().any(|t| t == "Déplacer vers la séquence"), "{textes:?}");
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    // Ses deux gestes : le chemin de la partie 190 (une étape).
    let n = etapes(&b).len();
    b.app.decor3d_vers_sequence(&[i[0]], s2, false);
    assert_eq!(etapes(&b).len(), n + 1);
    let copie = b.app.doc.decors3d.par_sequence.get(&s2).unwrap().objets[0].clone();
    assert_eq!((copie.couleur, copie.cles.len()), (Some(3), 2), "la couleur et les clés suivent");
    assert!(copie.caches_dans.is_empty(), "« Caché dans le plan » visait un plan de l'autre séquence");
    assert_eq!(ids(&b.app.doc, s1).len(), 2, "copier : l'original reste");
    // Déplacer : copier puis enlever, une étape ; deux objets choisis ensemble.
    b.app.e3_choisir_objet(i[0], false);
    b.app.e3_choisir_objet(i[1], true);
    assert_eq!(b.app.o3_objets_du_geste(i[1]), vec![i[0], i[1]]);
    let n = etapes(&b).len();
    b.app.decor3d_vers_sequence(&b.app.o3_objets_du_geste(i[1]), s2, true);
    assert_eq!(etapes(&b).len(), n + 1);
    assert!(ids(&b.app.doc, s1).is_empty() && ids(&b.app.doc, s2).len() == 3);
    b.app.run_action("edition.annuler");
    assert_eq!((ids(&b.app.doc, s1).len(), ids(&b.app.doc, s2).len()), (2, 1), "Ctrl+Z : tout revient en une fois");
}

/// Les textes peints à la dernière image.
fn textes(b: &Banc) -> Vec<String> {
    fn chercher(s: &eframe::egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            eframe::egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            eframe::egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut out);
    }
    out
}

/// Glisse la ligne `de` jusqu'à la ligne de la séquence `vers` de la liste du sélecteur (ouverte par le glisser), lâche avec `mods`.
fn glisser_vers_la_sequence(b: &mut Banc, de: Pos2, vers: u64, mods: Modifiers) {
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: de, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    for k in 1..=4 {
        b.image(vec![Event::PointerMoved(de + vec2(0.0, -6.0 * k as f32))], 1.0 / 60.0);
    }
    let cible = crate::seqdecor3d::SEQUENCES_LISTE.with(|l| l.borrow().iter().find(|x| x.0 == vers).map(|x| x.1)).expect("la liste s'ouvre pendant le glisser");
    let a = de + vec2(0.0, -24.0);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(a + (cible.center() - a) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    b.touches = mods;
    b.image(vec![Event::PointerMoved(cible.center())], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: cible.center(), button: PointerButton::Primary, pressed: false, modifiers: mods }], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.chauffer(3);
}

#[test]
fn dragging_object_rows_onto_a_sequence_moves_them_and_alt_copies_in_one_step() {
    let mut b = banc_3d(film());
    let (s1, s2) = seqs(&b.app.doc);
    let i = ids(&b.app.doc, s1);
    b.app.doc.decors3d.par_sequence.get_mut(&s1).unwrap().objet_mut(i[1]).unwrap().couleur = Some(6);
    b.chauffer(2);
    // Alt : copier.
    let n = etapes(&b).len();
    let r = ligne(Ligne3d::Objet(i[1]));
    glisser_vers_la_sequence(&mut b, pos2(r.left() + 60.0, r.center().y), s2, Modifiers::ALT);
    assert_eq!(ids(&b.app.doc, s1).len(), 2, "Alt : l'original reste");
    assert_eq!(b.app.doc.decors3d.par_sequence.get(&s2).map(|d| d.objets.iter().map(|o| o.couleur).collect::<Vec<_>>()), Some(vec![Some(6)]));
    assert_eq!(etapes(&b).len(), n + 1);
    // Sans Alt : déplacer.
    let n = etapes(&b).len();
    let r = ligne(Ligne3d::Objet(i[0]));
    glisser_vers_la_sequence(&mut b, pos2(r.left() + 60.0, r.center().y), s2, Modifiers::NONE);
    assert_eq!(ids(&b.app.doc, s1), vec![i[1]], "lâcher sans Alt : il part");
    assert_eq!(ids(&b.app.doc, s2).len(), 2);
    assert_eq!(etapes(&b).len(), n + 1, "une étape");
    assert!(etapes(&b).last().is_some_and(|e| e.contains("Envoyer")), "{:?}", etapes(&b));
}

// ------------------------------------------------------------------------------------------------ 4. qui a la main

fn ecrit() -> (Option<String>, Option<String>) {
    crate::quialamain::ECRIT.with(|e| e.borrow().clone())
}

#[test]
fn the_view_says_who_has_the_hand_decor_card_or_none_and_what_a_click_would_take() {
    let mut b = banc_3d(film());
    let i = ids(&b.app.doc, seqs(&b.app.doc).0);
    b.app.espace3d.sel.clear();
    b.chauffer(2);
    assert_eq!(ecrit().0, None, "rien de choisi : pas d'étiquette");
    b.app.e3_choisir_objet(i[0], false);
    b.chauffer(2);
    assert_eq!(ecrit().0.as_deref(), Some("Décor : Maison"));
    // Sa couleur d'identification : l'étiquette et le carré du manipulateur.
    b.app.couleur_d_identification(&[i[0]], Some(5));
    assert_eq!(b.app.e3_qui_a_la_main().map(|m| m.1), Some(crate::couleurs3d::color32(crate::couleurs3d::PALETTE[5])));
    assert_eq!(b.app.e3_couleur_de_la_main(), Some(crate::couleurs3d::color32(crate::couleurs3d::PALETTE[5])));
    b.app.e3_choisir_objet(i[1], true);
    b.chauffer(2);
    assert_eq!(ecrit().0.as_deref(), Some("Décor : Maison et 1 autre"));
    // Une carte.
    let id = b.app.layers()[1].id;
    b.app.espace3d.sel = vec![crate::espace3d::card_selection(id)];
    b.chauffer(2);
    assert_eq!(ecrit().0.as_deref(), Some("Carte : Perso 0"));
    // La caméra.
    b.app.espace3d.sel = vec![crate::espace3dgizmo::CAMERA_ID];
    b.chauffer(2);
    assert_eq!(ecrit().0.as_deref(), Some("Caméra du plan"));
    // Rien de choisi, le pointeur sur un objet : ce qu'un clic prendrait.
    b.app.espace3d.sel.clear();
    b.chauffer(1);
    let o = b.app.e3_objet(i[0]).unwrap().clone();
    let (lo, hi) = o.bornes_monde();
    let c = m::scale(m::add(lo, hi), 0.5);
    let draw = b.app.espace3d.rect;
    let view = b.app.e3_view(draw);
    let q = view.project(c, [draw.width() / 2.0, draw.height() / 2.0]).unwrap();
    b.image(vec![Event::PointerMoved(pos2(draw.left() + q[0], draw.top() + q[1]))], 1.0 / 60.0);
    b.chauffer(1);
    assert_eq!(ecrit(), (None, Some("Décor : Maison".into())));
    // Dans le Storyboard : la vue à côté le dit aussi.
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(3);
    if !b.app.espace3d_a_cote_visible() {
        b.app.run_action("espace3d.a_cote");
    }
    b.app.e3_choisir_objet(i[1], false);
    b.chauffer(4);
    assert!(b.app.espace3d_a_cote_visible());
    assert_eq!(ecrit().0.as_deref(), Some("Décor : Arbre"));
}

// ------------------------------------------------------------------------------------------------ 5. un calque neuf devant la caméra

/// Un plan dont la caméra, tirée dans la vue, est haut au-dessus de la feuille et plonge (le « Plan 5 » de la partie 200.7) : le
/// chemin de toujours ne la filme pas.
fn plan_camera_libre() -> Document {
    let mut doc = film();
    let [fw, _] = doc.frame;
    let (cx, cy) = (doc.width as f32 / 2.0, doc.height as f32 / 2.0);
    let d0 = crate::camera::ref_distance(doc.frame);
    let cam = crate::camera::Camera { x: cx + 0.17 * fw as f32, y: cy - 1.97 * fw as f32, zoom: 1.0, angle: -5.8, dolly: 0.95 * d0, tilt: -44.0, pan: -36.6 };
    doc.panels[0].camera = vec![crate::camera::CameraKey::new(0, cam)];
    doc.panels[0].camera_libre = true;
    doc
}

/// Le calque `li` du plan courant est devant la caméra : au milieu de son cadre (le rayon du centre de la vue le touche en son
/// centre), face à elle, et il remplit le cadre (le cadre entier tombe sur sa carte).
fn devant_la_camera(b: &Banc, li: usize) -> bool {
    let doc = &b.app.doc;
    let world = Monde::du_plan(doc, b.app.cur_panel);
    let cam = doc.panel_camera_reelle(b.app.cur_panel, b.app.cur_frame);
    let sc = world.scene_cam(&cam);
    let v = sc.view(doc.frame[0] as f32);
    let xf = world.carte(&doc.panels[b.app.cur_panel].layers[li], b.app.cur_frame);
    let coins = world.card_corners(&xf);
    let n = m::normalize(m::cross(m::sub(coins[1], coins[0]), m::sub(coins[3], coins[0])));
    let face = m::dot(n, m::normalize(v.forward)).abs() > 0.999;
    // Le rayon du centre de la vue : où il rencontre la carte.
    let t = m::dot(m::sub(xf.pos, sc.pos), n) / m::dot(m::normalize(v.forward), n);
    let p = m::add(sc.pos, m::scale(m::normalize(v.forward), t));
    let centre = m::length(m::sub(p, xf.pos)) < 1e-3 * world.d0.max(1.0);
    // Le cadre sur la carte : ses coins, vus de la caméra, entourent tout le cadre.
    let c = [doc.frame[0] as f32 / 2.0, doc.frame[1] as f32 / 2.0];
    let proj: Vec<[f32; 2]> = coins.iter().filter_map(|q| v.project(*q, c)).collect();
    let (x0, x1) = proj.iter().fold((f32::MAX, f32::MIN), |(a, z), q| (a.min(q[0]), z.max(q[0])));
    let (y0, y1) = proj.iter().fold((f32::MAX, f32::MIN), |(a, z), q| (a.min(q[1]), z.max(q[1])));
    let remplit = proj.len() == 4 && x0 <= 0.0 && y0 <= 0.0 && x1 >= doc.frame[0] as f32 && y1 >= doc.frame[1] as f32;
    face && centre && remplit && t > 0.0
}

#[test]
fn a_new_layer_with_a_free_plunging_camera_sits_in_front_of_it_by_every_path() {
    let mut b = banc_3d(plan_camera_libre());
    b.app.select_panel(0);
    b.chauffer(2);
    assert!(b.app.doc.repere_du_plan(0, 0).is_some(), "la caméra n'est pas filmée par le chemin de toujours");
    assert!(!devant_la_camera(&b, 0), "un calque d'avant est sur la feuille, hors du cadre");
    // Le « + » de la timeline, les menus, la palette, coller une image comme nouveau calque : `calque.nouveau_*`, `add_layer`.
    let n = etapes(&b).len();
    b.app.run_action("calque.nouveau_vectoriel");
    let li = b.app.cur_layer;
    assert!(devant_la_camera(&b, li), "un calque neuf vectoriel devant la caméra");
    assert_eq!(etapes(&b).len(), n + 1, "dans l'étape de sa création");
    b.app.run_action("calque.nouveau_bitmap");
    assert!(devant_la_camera(&b, b.app.cur_layer));
    // La fenêtre « Nouveau calque ».
    b.app.open_new_layer_dialog();
    let f = b.app.new_layer_dialog.take().unwrap();
    b.app.create_layer_from(&f);
    assert!(devant_la_camera(&b, b.app.cur_layer));
    // Dupliquer garde la place de sa source (ici, celle d'un calque d'avant : sur la feuille).
    b.app.select_layer(0);
    b.app.duplicate_layer();
    let dup = b.app.cur_layer;
    let world = Monde::du_plan(&b.app.doc, 0);
    assert_eq!(world.carte(&b.app.layers()[dup], 0), world.carte(&b.app.layers()[0], 0), "le double est à la place de l'original");
    // Ctrl+Z : la création, avec sa place, en une fois.
    let avant = b.app.layers().len();
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.layers().len(), avant - 1);
    // Un dossier, un calque d'annotation : pas de place de carte.
    let mut dossier = Layer::new("Dossier", LayerKind::Vector);
    dossier.folder = true;
    assert!(!crate::calquedevant::a_poser(&b.app.doc, 0, 0, &dossier));
    assert!(!crate::calquedevant::a_poser(&b.app.doc, 0, 0, &Layer::new_annotation("Notes")));
    // « Nouveau plan depuis cette vue » (partie 200) : son premier calque, devant sa caméra.
    b.app.espace3d.cam = Some(m::Orbite { target: [0.5, -1.0, -6.0], azimuth: 30.0, elevation: 35.0, distance: 9.0, focal: 35.0 });
    b.app.o3_nouveau_plan_depuis_la_vue();
    let pi = b.app.cur_panel;
    if b.app.doc.repere_du_plan(pi, 0).is_some() {
        assert!(devant_la_camera(&b, 0), "le calque du plan fait depuis la vue");
    }
}

#[test]
fn a_classic_camera_or_the_preference_off_makes_exactly_todays_layer() {
    // La caméra de toujours : le calque d'avant, champ pour champ.
    let mut b = banc_3d(film());
    b.app.run_action("calque.nouveau_vectoriel");
    let l = b.app.layers()[b.app.cur_layer].clone();
    let mut attendu = Layer::new(l.name.clone(), LayerKind::Vector);
    attendu.id = l.id;
    attendu.parent = l.parent;
    assert_eq!(l, attendu, "rien de changé");
    // La caméra libre, le réglage décoché : comme avant aussi.
    let mut b = banc_3d(plan_camera_libre());
    b.app.select_panel(0);
    b.app.settings.calque_devant_la_camera = false;
    b.chauffer(1);
    b.app.run_action("calque.nouveau_vectoriel");
    let l = b.app.layers()[b.app.cur_layer].clone();
    let mut attendu = Layer::new(l.name.clone(), LayerKind::Vector);
    attendu.id = l.id;
    attendu.parent = l.parent;
    assert_eq!(l, attendu, "décoché : le calque d'avant");
    // Le réglage : coché d'origine, écrit seulement décoché, dans Préférences > Caméra.
    assert!(crate::settings::Settings::default().calque_devant_la_camera);
    let texte = toml::to_string(&crate::settings::Settings::default()).unwrap();
    assert!(!texte.contains("calque_devant_la_camera"));
    let texte = toml::to_string(&b.app.settings).unwrap();
    assert!(texte.contains("calque_devant_la_camera = false"));
}

// ------------------------------------------------------------------------------------------------ 201.6 : les glyphes

/// Les fichiers d'interface des parties 194 à 201 (ceux qu'elles ont créés, et ceux de la vue 3D qu'elles ont le plus retouchés).
const FICHIERS_194_201: [&str; 18] = [
    "colonne.rs",
    "colonneui.rs",
    "compteur.rs",
    "scene.rs",
    "sceneui.rs",
    "onglet3d.rs",
    "onglet3dui.rs",
    "couleurs3d.rs",
    "couleurs3dui.rs",
    "miseenscene3d.rs",
    "miseenscene3dui.rs",
    "timelineseq.rs",
    "seqdecor3d.rs",
    "quialamain.rs",
    "calquedevant.rs",
    "decor3dui.rs",
    "espace3dview.rs",
    "espace3dvues.rs",
];

/// Les chaînes littérales d'un source Rust, hors commentaires : (ligne, texte).
fn litteraux(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (n, ligne) in source.lines().enumerate() {
        let code = ligne.trim_start();
        if code.starts_with("//") {
            continue;
        }
        let mut dans = false;
        let mut echappe = false;
        let mut s = String::new();
        let mut prec = ' ';
        for c in ligne.chars() {
            if !dans {
                // (Un commentaire en fin de ligne : la suite n'est pas du code.)
                if c == '/' && prec == '/' {
                    break;
                }
                if c == '"' && prec != '\'' {
                    dans = true;
                    s.clear();
                }
            } else if echappe {
                echappe = false;
                s.push(c);
            } else if c == '\\' {
                echappe = true;
            } else if c == '"' {
                dans = false;
                out.push((n + 1, s.clone()));
            } else {
                s.push(c);
            }
            prec = c;
        }
    }
    out
}

#[test]
fn every_ui_string_of_parts_194_to_201_has_its_glyphs_in_feanor_fonts() {
    let b = banc_taille(film(), false, 800, 600);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut manquants = Vec::new();
    for nom in FICHIERS_194_201 {
        let texte = std::fs::read_to_string(dir.join(nom)).unwrap_or_else(|_| panic!("{nom}"));
        for (n, s) in litteraux(&texte) {
            for c in s.chars().filter(|c| !c.is_ascii()) {
                let ok = b.ctx.fonts(|f| f.has_glyph(&eframe::egui::FontId::proportional(12.0), c));
                if !ok {
                    manquants.push(format!("{nom}:{n} « {c} » (U+{:04X}) dans « {s} »", c as u32));
                }
            }
        }
    }
    assert!(manquants.is_empty(), "des caractères que les polices de FEANOR n'ont pas (une case vide à l'écran) :\n{}", manquants.join("\n"));
    // Les textes changés en 201.6, tels qu'ils s'écrivent : le losange de phosphor est dans les polices.
    use egui_phosphor::regular as ph;
    for s in [format!("{} Clé à l'image 1", ph::DIAMOND), format!("{} Clé sur Tree default", ph::DIAMOND), format!("{} Clé (choisis un objet)", ph::DIAMOND), format!("Pas de clé : « {} Clé » en haut", ph::DIAMOND), crate::colonneui::TOUS_SORTIS.to_string()] {
        assert!(b.ctx.fonts(|f| f.has_glyphs(&eframe::egui::FontId::proportional(12.0), &s)), "{s}");
    }
    // (Et l'essai voit bien un caractère qui manque.)
    assert!(!b.ctx.fonts(|f| f.has_glyph(&eframe::egui::FontId::proportional(12.0), '◆')));
}

// ------------------------------------------------------------------------------------------------ 2. les lampes sur les cartes

/// Le projet aux deux carrés (`decor3d_tests::projet_a_deux_carres`), une carte peinte d'un bloc vert au milieu de la feuille (qui
/// reçoit les ombres, ou non), le décor avec ou sans ses ombres, et ces lampes.
fn carte_et_lampes(lampes: &[crate::miseenscene3d::Lampe3d], recoit: bool, ombres: bool) -> Document {
    let mut doc = crate::decor3d_tests::projet_a_deux_carres();
    let seq = doc.sequence_of(0);
    {
        let d = doc.decors3d.par_sequence.get_mut(&seq).unwrap();
        d.aspect.ombres = ombres;
        d.lampes = lampes.to_vec();
    }
    let cel = doc.new_cel(LayerKind::Bitmap);
    let (sw, sh) = (doc.width, doc.height);
    let mut rgba = vec![0u8; sw * sh * 4];
    for y in sh / 2 - 8..sh / 2 + 8 {
        for x in sw / 2 - 8..sw / 2 + 8 {
            rgba[(y * sw + x) * 4..(y * sw + x) * 4 + 4].copy_from_slice(&[40, 150, 60, 255]);
        }
    }
    doc.cels.get_mut(&cel).unwrap().raster = crate::raster::Raster::from_rgba(sw, sh, &rgba);
    let n = doc.panels[0].frame_count();
    let mut l = Layer::new("Carte", LayerKind::Bitmap);
    l.frames = vec![Some(cel); n];
    l.recoit_ombres = recoit;
    doc.panels[0].layers = vec![l];
    doc
}

fn lampe(position: [f32; 3], couleur: [u8; 3]) -> crate::miseenscene3d::Lampe3d {
    let mut l = crate::miseenscene3d::Lampe3d::neuve(9_000, "Lampe".into(), position);
    l.couleur = couleur;
    l.intensite = 1.5;
    l.portee = 3.0;
    l
}

#[test]
fn a_lamp_lights_the_cards_that_receive_shadows_and_leaves_the_others_exactly_as_they_were_on_the_cpu() {
    let [w, h] = crate::decor3d_tests::projet_a_deux_carres().frame;
    let carte = (h / 2) * w + w / 2;
    let sans = crate::decor3d_tests::image_exportee(&carte_et_lampes(&[], true, false));
    assert!(sans[carte].g() > 100 && sans[carte].a() == 255, "la carte au milieu : {:?}", sans[carte]);
    // Une lampe qui n'atteint rien : l'image d'aujourd'hui, au bit près (le décor et la carte).
    let loin = crate::decor3d_tests::image_exportee(&carte_et_lampes(&[lampe([40.0, 40.0, 40.0], [255, 255, 255])], true, false));
    assert!(loin == sans, "une lampe hors de portée ne change aucun pixel");
    // Une lampe blanche devant la carte : son encre s'éclaire.
    let blanche = crate::decor3d_tests::image_exportee(&carte_et_lampes(&[lampe([0.0, 0.0, 0.6], [255, 255, 255])], true, false));
    let (a, b) = (sans[carte], blanche[carte]);
    assert!(b.r() > a.r() && b.g() > a.g() + 20 && b.b() > a.b(), "éclairée : {a:?} → {b:?}");
    // Une lampe rouge : sa couleur.
    let rouge = crate::decor3d_tests::image_exportee(&carte_et_lampes(&[lampe([0.0, 0.0, 0.6], [255, 40, 20])], true, false));
    let c = rouge[carte];
    assert!(c.r() as i32 - a.r() as i32 > c.b() as i32 - a.b() as i32 + 10, "teintée : {a:?} → {c:?}");
    // « Reçoit les ombres » décoché : ni ombres ni lampes, la carte telle qu'elle est dessinée.
    let decoche = crate::decor3d_tests::image_exportee(&carte_et_lampes(&[lampe([0.0, 0.0, 0.6], [255, 255, 255])], false, false));
    let sans_decoche = crate::decor3d_tests::image_exportee(&carte_et_lampes(&[], false, false));
    assert_eq!(decoche[carte], sans_decoche[carte], "décochée : pas de lumière");
    // Les ombres du décor allumées (sans carte graphique : son rendu par le processeur n'a pas de soleil) : les lampes éclairent
    // toujours la carte.
    let avec_ombres = crate::decor3d_tests::image_exportee(&carte_et_lampes(&[lampe([0.0, 0.0, 0.6], [255, 255, 255])], true, true));
    assert_eq!(avec_ombres[carte], blanche[carte]);
    // La lumière d'un pixel de carte : la même que sur le décor, au point et à la normale de la carte.
    let doc = carte_et_lampes(&[lampe([0.0, 0.0, 0.6], [255, 255, 255])], true, false);
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    let Some(crate::render::FondPart::Decor(k)) = plan.items.iter().find_map(|i| i.fond.clone()) else { panic!() };
    let lum = crate::ombres::Lumiere::des_lampes(&k.aspect, &k.prise().unwrap());
    let px = eframe::egui::Color32::from_rgb(40, 150, 60);
    assert_eq!(lum.eclairer_carte(px, [40.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.0), px, "hors de portée : tel quel");
    let l = k.aspect.lampes.lumiere([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert!((l[0] - 1.5 * 0.64).abs() < 1e-3, "{l:?}");
}

#[test]
fn on_the_gpu_a_lamp_lights_the_card_and_a_lamp_out_of_reach_changes_no_pixel() {
    if crate::gpu_tests::headless().is_none() {
        return;
    }
    let capture = |doc: Document| {
        let mut b = banc_taille(doc, true, 1280, 800);
        b.app.set_espace(Espace::Storyboard);
        b.app.renderer.invalidate_all();
        b.chauffer(12);
        crate::decor3d::wait_idle();
        b.chauffer(4);
        let p = b.app.layer_to_screen(pos2(b.app.doc.width as f32 / 2.0, b.app.doc.height as f32 / 2.0));
        let rgba = b.capturer().unwrap();
        let w = b.taille.x as usize;
        let at = (p.y as usize * w + p.x as usize) * 4;
        ([rgba[at], rgba[at + 1], rgba[at + 2]], rgba)
    };
    let (sans, img_sans) = capture(carte_et_lampes(&[], true, false));
    let (loin, img_loin) = capture(carte_et_lampes(&[lampe([40.0, 40.0, 40.0], [255, 255, 255])], true, false));
    assert_eq!(sans, loin);
    assert!(img_sans == img_loin, "une lampe hors de portée : l'écran d'aujourd'hui, au pixel près");
    let (blanche, _) = capture(carte_et_lampes(&[lampe([0.0, 0.0, 0.6], [255, 255, 255])], true, false));
    assert!(blanche[1] > sans[1] + 20, "éclairée sur la carte graphique : {sans:?} → {blanche:?}");
    let (decoche, _) = capture(carte_et_lampes(&[lampe([0.0, 0.0, 0.6], [255, 255, 255])], false, false));
    let (decoche_sans, _) = capture(carte_et_lampes(&[], false, false));
    assert_eq!(decoche, decoche_sans, "décochée : pas de lumière");
}

/// La mesure de la partie 201 : huit lampes et dix cartes sur une feuille de 1920 × 1080, l'image composée (le processeur, comme à
/// l'export ; et la carte graphique, quand le banc en a une), avec les lampes et sans.
#[test]
fn mesure_des_lampes_sur_les_cartes() {
    let faire = |lampes: bool| {
        let mut doc = Document::new(1920, 1080, 24);
        doc.frame = [1920, 1080];
        let seq = doc.sequence_of(0);
        let modele = modele_du_carre();
        let mut d = Decor3d::new(Aspect { ciel: false, brume: false, ombres: false, ..Aspect::default() });
        let id = doc.alloc();
        d.objets.push(Objet3d { id, nom: "Fond".into(), source: String::new(), allege: Vec::new(), place: Place3d { position: [0.0, 0.0, -3.0], rotation_y: 0.0, echelle: 3.0 }, origine: [0.0; 3], aimante: false, modele, couleur: None, cache: false, verrou: false, cles: Vec::new(), caches_dans: Vec::new() });
        if lampes {
            for k in 0..8 {
                let mut l = crate::miseenscene3d::Lampe3d::neuve(9_000 + k, format!("Lampe {k}"), [k as f32 * 0.8 - 2.8, 0.3, 0.8]);
                l.portee = 4.0;
                d.lampes.push(l);
            }
        }
        doc.decors3d.par_sequence.insert(seq, d);
        let n = doc.panels[0].frame_count();
        let mut layers = Vec::new();
        for c in 0..10 {
            let cel = doc.new_cel(LayerKind::Bitmap);
            let mut rgba = vec![0u8; 1920 * 1080 * 4];
            let (x0, y0) = (100 + (c % 5) * 340, 150 + (c / 5) * 450);
            for y in y0..y0 + 300 {
                for x in x0..x0 + 300 {
                    rgba[(y * 1920 + x) * 4..(y * 1920 + x) * 4 + 4].copy_from_slice(&[60 + c as u8 * 15, 120, 200, 255]);
                }
            }
            doc.cels.get_mut(&cel).unwrap().raster = crate::raster::Raster::from_rgba(1920, 1080, &rgba);
            let mut l = Layer::new(&format!("Carte {c}"), LayerKind::Bitmap);
            l.frames = vec![Some(cel); n];
            layers.push(l);
        }
        doc.panels[0].layers = layers;
        doc
    };
    let mediane = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        (v[v.len() / 2], v[v.len() - 1])
    };
    // Le processeur : l'image composée de l'export.
    for lampes in [false, true] {
        let doc = faire(lampes);
        let _ = crate::decor3d_tests::image_exportee(&doc);
        let mut t = Vec::new();
        for _ in 0..5 {
            let t0 = std::time::Instant::now();
            let _ = crate::decor3d_tests::image_exportee(&doc);
            t.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        let (m, p) = mediane(t);
        eprintln!("MESURE 201 : processeur (export), {} : médiane {m:.1} ms, pire {p:.1} ms", if lampes { "huit lampes, dix cartes" } else { "sans lampe" });
    }
    // La carte graphique : l'image composée à l'écran, recomposée à chaque image (une carte bouge d'un pixel).
    if crate::gpu_tests::headless().is_some() {
        for lampes in [false, true] {
            let mut b = banc_taille(faire(lampes), true, 1600, 1000);
            b.app.set_espace(Espace::Storyboard);
            b.chauffer(10);
            crate::decor3d::wait_idle();
            b.chauffer(4);
            let mut t = Vec::new();
            for k in 0..30 {
                b.app.doc.panels[0].layers[0].transform.x = (k % 2) as f32;
                b.app.renderer.invalidate_params();
                let m = b.image(Vec::new(), 1.0 / 60.0);
                t.push(m.carte);
            }
            let (m, p) = mediane(t);
            eprintln!("MESURE 201 : carte graphique (écran, image recomposée), {} : médiane {m:.2} ms, pire {p:.2} ms", if lampes { "huit lampes, dix cartes" } else { "sans lampe" });
        }
    }
}
