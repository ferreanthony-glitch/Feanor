//! La couleur d'aplat d'un calque de personnage vient du panneau Personnages (`aplatperso.rs`).

use eframe::egui::{self, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::app::Tool;
use crate::aplatperso::recolorer_strate;
use crate::document::{CelId, Document, LayerKind, Strate};
use crate::fond::FondReglages;
use crate::perf_tests::{banc_taille, Banc};
use crate::personnagesui::Choix;
use crate::raster::{premultiply, Raster};
use crate::workspace::Espace;

const ROUGE: [u8; 3] = [200, 30, 30];
const ROSE: [u8; 3] = [150, 97, 97];
const VERT: [u8; 3] = [40, 170, 60];
const BLANC: Color32 = Color32::WHITE;
/// Au moins ce nombre de pixels d'aplat peints par un anneau (la taille à l'écran change avec le zoom du banc).
const MIN: usize = 150;

// ------------------------------------------------------------------------------------------ outils d'essai

fn banc() -> Banc {
    crate::fxcache::monde::ecrire();
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    assert_eq!(b.app.espace, Espace::Storyboard);
    b
}

fn appui(p: Pos2, enfonce: bool) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: enfonce, modifiers: Modifiers::NONE }
}

/// Un trait au milieu du dessin, de gauche à droite, plus haut ou plus bas de `dy` ; la zone fermée par un anneau n'est pas nécessaire :
/// l'aplat suit tout ce que le trait enferme, et le fil de fond peint ce que le trait gagne.
fn anneau(b: &mut Banc, dy: f32) {
    let c = b.app.canvas_rect.center() + vec2(0.0, dy);
    let (x0, x1, y0, y1) = (c.x - 50.0, c.x + 50.0, c.y - 40.0, c.y + 40.0);
    let mut pts = Vec::new();
    for k in 0..=20 {
        pts.push(pos2(x0 + (x1 - x0) * k as f32 / 20.0, y0));
    }
    for k in 1..=16 {
        pts.push(pos2(x1, y0 + (y1 - y0) * k as f32 / 16.0));
    }
    for k in 1..=20 {
        pts.push(pos2(x1 - (x1 - x0) * k as f32 / 20.0, y1));
    }
    for k in 1..=17 {
        pts.push(pos2(x0, y1 - (y1 - y0) * k as f32 / 16.0));
    }
    b.image(vec![Event::PointerMoved(pts[0])], 1.0 / 60.0);
    b.image(vec![appui(pts[0], true)], 1.0 / 60.0);
    for p in &pts[1..] {
        b.image(vec![Event::PointerMoved(*p)], 1.0 / 60.0);
    }
    b.image(vec![appui(*pts.last().unwrap(), false)], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
    b.app.perso_attendre();
}

fn personnage(b: &mut Banc, nom: &str, ligne: [u8; 3], fond: Option<[u8; 3]>) -> u64 {
    let id = b.app.perso_ajouter().expect("un personnage de plus");
    b.app.perso_ecrire(Choix::Personnage(id), nom.into(), ligne, fond);
    b.app.perso_choisir(Choix::Personnage(id));
    id
}

/// Les dessins (de strate) des calques du personnage `id`, dans tous les plans, versions comprises.
fn dessins_de(doc: &Document, id: u64) -> Vec<CelId> {
    let mut out: Vec<CelId> = Vec::new();
    for p in doc.tous_les_plans() {
        for l in p.layers.iter().filter(|l| l.personnage == Some(id)) {
            for c in l.frames.iter().flatten() {
                if !out.contains(c) {
                    out.push(*c);
                }
            }
        }
    }
    out
}

/// Ceux de ces dessins qui ont de la peinture dans leur strate d'aplat.
fn peints(doc: &Document, id: u64) -> Vec<CelId> {
    dessins_de(doc, id).into_iter().filter(|c| doc.cels.get(c).and_then(|c| c.fond.as_deref()).is_some_and(|s| !s.peinture.raster.is_blank())).collect()
}

fn calques_de(doc: &Document, id: u64) -> Vec<Option<[u8; 3]>> {
    doc.tous_les_plans().iter().flat_map(|p| p.layers.iter()).filter(|l| l.personnage == Some(id)).map(|l| l.fond.map(|f| f.couleur)).collect()
}

fn proche(c: Color32, couleur: [u8; 3]) -> bool {
    let a = c.a();
    let e = premultiply(couleur[0], couleur[1], couleur[2], a);
    a > 0 && c.r().abs_diff(e.r()) <= 2 && c.g().abs_diff(e.g()) <= 2 && c.b().abs_diff(e.b()) <= 2
}

/// Combien de pixels de la strate d'aplat du dessin `cel` ont cette couleur (à l'opacité près).
fn compte(doc: &Document, cel: CelId, couleur: [u8; 3]) -> usize {
    let Some(s) = doc.cels.get(&cel).and_then(|c| c.fond.as_deref()) else { return 0 };
    let r = &s.peinture.raster;
    let mut n = 0;
    for y in 0..r.height {
        for x in 0..r.width {
            if proche(r.get(x, y), couleur) {
                n += 1;
            }
        }
    }
    n
}

fn compte_blanc(doc: &Document, cel: CelId) -> usize {
    let Some(s) = doc.cels.get(&cel).and_then(|c| c.fond.as_deref()) else { return 0 };
    let r = &s.peinture.raster;
    (0..r.height).flat_map(|y| (0..r.width).map(move |x| (x, y))).filter(|&(x, y)| r.get(x, y) == BLANC).count()
}

/// Un carré blanc peint à la main dans la strate d'aplat de `cel` (un blanc d'œil), au milieu de l'aplat.
fn peindre_blanc(b: &mut Banc, cel: CelId, x0: usize, y0: usize) {
    let c = b.app.doc.cels.get_mut(&cel).expect("dessin");
    let s = c.fond.as_mut().expect("strate");
    for y in y0..y0 + 6 {
        for x in x0..x0 + 6 {
            let id = s.peinture.raster.tile_index_of(x, y);
            s.peinture.raster.tile_mut(id)[(y % 64) * 64 + x % 64] = BLANC;
        }
    }
    s.peinture.version += 1;
    c.version += 1;
}

fn etapes(b: &Banc) -> usize {
    b.app.history.names().0.len()
}

/// Un changement de couleur d'aplat comme le fait le champ du panneau : l'étape du geste, puis l'écriture.
fn changer_couleur(b: &mut Banc, id: u64, fond: [u8; 3]) {
    let (nom, ligne) = b.app.doc.personnages.iter().find(|p| p.id == id).map(|p| (p.name.clone(), p.line)).expect("personnage");
    b.app.note_change(egui::Id::new("champ-aplat-essai"));
    b.app.perso_ecrire(Choix::Personnage(id), nom, ligne, Some(fond));
}

/// Deux plans qui ont chacun le calque de Lou, peint (la couleur de Lou : `ROSE`), plus une version rangée du premier plan.
/// Rend (Lou, le dessin de Lou du plan 0).
fn deux_plans_et_une_version(b: &mut Banc) -> (u64, CelId) {
    let lou = personnage(b, "Lou", ROUGE, Some(ROSE));
    anneau(b, 0.0);
    let cel = dessins_de(&b.app.doc, lou)[0];
    assert!(compte(&b.app.doc, cel, ROSE) > MIN, "l'aplat est peint de la couleur de Lou dans la strate");
    // Un deuxième plan, son propre calque de Lou.
    assert!(b.app.structure_op(|d, i| Some(d.add_scene_after(i))));
    b.chauffer(3);
    anneau(b, 0.0);
    // Une version rangée du premier plan : le plan tel qu'il est, avec ses calques (copiés).
    assert!(b.app.doc.nouvelle_version(0, 7).is_some());
    (lou, cel)
}

// ------------------------------------------------------------------------------------------ la règle

#[test]
fn a_stroke_with_a_neutral_on_a_character_layer_does_not_change_its_aplat_colour() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    anneau(&mut b, 0.0);
    let rang = b.app.doc.calque_de_personnage(0, lou).expect("le calque de Lou");
    assert_eq!(b.app.layers()[rang].fond.map(|f| f.couleur), Some(ROSE));
    // Une couleur neutre à aplat dessine dans le calque de Lou (c'est le calque courant) : sa couleur d'aplat ne change pas.
    let gris = b.app.settings.brosses_classiques[0].fill.expect("aplat du gris");
    assert_ne!(gris, ROSE);
    b.app.perso_choisir(Choix::Classique(0));
    assert_eq!(b.app.cur_layer, rang, "le neutre dessine dans le calque courant");
    anneau(&mut b, 100.0);
    assert_eq!(b.app.layers()[rang].fond.map(|f| f.couleur), Some(ROSE), "la couleur d'aplat du calque de Lou n'a pas bougé");
    let cel = dessins_de(&b.app.doc, lou)[0];
    assert_eq!(compte(&b.app.doc, cel, gris), 0, "et le trait du neutre y a peint l'aplat de Lou, pas le sien");
    assert!(compte(&b.app.doc, cel, ROSE) > 2 * MIN, "les deux anneaux ont l'aplat de Lou");
    // La couleur libre non plus (adapté 2026-10-29 : plus de pastille « classique », on lâche la brosse).
    b.app.perso_lacher();
    assert_eq!(b.app.layers()[rang].fond.map(|f| f.couleur), Some(ROSE));
}

#[test]
fn an_ordinary_layer_keeps_the_rule_of_the_pastilles() {
    let mut b = banc();
    let gris = b.app.settings.brosses_classiques[0].fill.expect("aplat du gris");
    b.app.perso_choisir(Choix::Classique(0));
    anneau(&mut b, 0.0);
    let li = b.app.cur_layer;
    assert!(b.app.layers()[li].personnage.is_none());
    assert_eq!(b.app.layers()[li].fond.map(|f| f.couleur), Some(gris), "un calque ordinaire prend la couleur d'aplat de la pastille");
    // Un autre neutre : le calque ordinaire change encore de couleur.
    let autre = b.app.settings.brosses_classiques[1].fill.expect("aplat du second gris");
    b.app.perso_choisir(Choix::Classique(1));
    anneau(&mut b, 100.0);
    assert_eq!(b.app.layers()[li].fond.map(|f| f.couleur), Some(autre));
    // Changer la couleur d'aplat d'un personnage ne touche pas un calque ordinaire de la même couleur.
    let lou = personnage(&mut b, "Lou", ROUGE, Some(autre));
    anneau(&mut b, -100.0);
    let calque_de_lou = b.app.doc.calque_de_personnage(0, lou).expect("le calque de Lou");
    assert_ne!(calque_de_lou, li);
    changer_couleur(&mut b, lou, VERT);
    b.app.attendre_aplat_perso();
    assert_eq!(b.app.layers()[calque_de_lou].fond.map(|f| f.couleur), Some(VERT));
    assert_eq!(b.app.layers()[li].fond.map(|f| f.couleur), Some(autre), "le calque ordinaire garde sa couleur");
}

fn textes_de(shape: &egui::Shape, out: &mut Vec<(String, Pos2)>) {
    match shape {
        egui::Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0)),
        egui::Shape::Vec(v) => v.iter().for_each(|s| textes_de(s, out)),
        _ => {}
    }
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![appui(p, true)], 1.0 / 60.0);
    b.image(vec![appui(p, false)], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
}

/// Les textes peints par la dernière image, avec leur centre.
fn a_l_ecran(b: &Banc) -> Vec<(String, Pos2)> {
    let mut out = Vec::new();
    for cs in &b.formes {
        textes_de(&cs.shape, &mut out);
    }
    out
}

#[test]
fn the_aplat_colour_line_of_a_character_layer_cannot_be_edited_and_points_to_the_panel() {
    let mut b = banc();
    b.garder_formes = true;
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    anneau(&mut b, 0.0);
    let li = b.app.doc.calque_de_personnage(0, lou).expect("le calque de Lou");
    assert!(b.app.aplat_vient_du_personnage(li));
    // La sous-ligne de l'aplat est choisie : Réglages montre « Aplat de Lou » et ses réglages.
    b.app.aplat_cible = Some(b.app.layers()[li].id);
    b.app.show_pane(crate::dock::Pane::ToolOptions);
    if b.app.pane_shown(crate::dock::Pane::Personnages) {
        b.app.toggle_pane(crate::dock::Pane::Personnages);
    }
    b.chauffer(4);
    assert!(!b.app.pane_shown(crate::dock::Pane::Personnages));
    let textes = a_l_ecran(&b);
    let renvoi = textes.iter().find(|(s, _)| s.contains("se change dans le panneau Personnages")).unwrap_or_else(|| panic!("le renvoi n'est pas à l'écran : {:?}", textes.iter().map(|t| &t.0).collect::<Vec<_>>()));
    assert_eq!(renvoi.0, "Couleur de « Lou » : se change dans le panneau Personnages");
    assert!(b.app.couleur_attente().is_none(), "rien à modifier ici");
    assert!(textes.iter().any(|(s, _)| s.contains("Changer la couleur de l'aplat sur")), "la portée est montrée, grisée");
    // Un clic sur le renvoi ouvre (ou montre) le panneau Personnages.
    clic(&mut b, renvoi.1);
    assert!(b.app.pane_shown(crate::dock::Pane::Personnages), "le clic sur le renvoi montre le panneau Personnages");
    // Un calque ordinaire, lui, a sa case de couleur.
    let ordinaire = b.app.layers().iter().position(|l| l.personnage.is_none()).expect("un calque ordinaire");
    b.app.layers_mut()[ordinaire].fond = Some(FondReglages::default());
    assert!(!b.app.aplat_vient_du_personnage(ordinaire));
    b.app.select_layer(ordinaire);
    b.app.aplat_cible = Some(b.app.layers()[ordinaire].id);
    b.chauffer(4);
    let textes = a_l_ecran(&b);
    assert!(!textes.iter().any(|(s, _)| s.contains("se change dans le panneau Personnages")), "un calque ordinaire n'a pas le renvoi");
    assert!(textes.iter().any(|(s, _)| s == "Couleur de l'aplat"), "il a sa ligne « Couleur de l'aplat » comme avant");
}

#[test]
fn a_layer_whose_character_was_deleted_is_an_ordinary_layer_again() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    anneau(&mut b, 0.0);
    let li = b.app.doc.calque_de_personnage(0, lou).expect("le calque de Lou");
    b.app.perso_supprimer(Choix::Personnage(lou));
    assert!(!b.app.aplat_vient_du_personnage(li), "sans personnage, la couleur se change comme celle de n'importe quel aplat");
}

// ------------------------------------------------------------------------------- changer la couleur du personnage

#[test]
fn changing_the_aplat_colour_of_a_character_recolours_all_its_layers_and_only_the_painted_aplat() {
    let mut b = banc();
    let (lou, cel0) = deux_plans_et_une_version(&mut b);
    // Un calque ordinaire qui a la même couleur que Lou : il ne bouge pas.
    let ordinaire = {
        let cel = b.app.doc.new_cel(LayerKind::Bitmap);
        let (w, h) = (b.app.doc.width, b.app.doc.height);
        b.app.doc.cels.get_mut(&cel).unwrap().fond = Some(Box::new(Strate::new(w, h)));
        let layer = crate::document::Layer::new("Autre".to_string(), LayerKind::Bitmap);
        b.app.doc.panels[0].layers.push(layer);
        let k = b.app.doc.panels[0].layers.len() - 1;
        b.app.doc.panels[0].layers[k].set(0, Some(cel));
        b.app.doc.panels[0].layers[k].fond = Some(FondReglages { couleur: ROSE, ..FondReglages::default() });
        peindre_blanc(&mut b, cel, 10, 10);
        {
            let s = b.app.doc.cels.get_mut(&cel).unwrap().fond.as_mut().unwrap();
            let id = s.peinture.raster.tile_index_of(30, 30);
            s.peinture.raster.tile_mut(id)[30 * 64 + 30] = premultiply(ROSE[0], ROSE[1], ROSE[2], 255);
        }
        cel
    };
    // Un blanc d'œil peint à la main dans l'aplat de Lou, plan 0.
    peindre_blanc(&mut b, cel0, 190, 140);
    assert_eq!(compte_blanc(&b.app.doc, cel0), 36);
    let tous = peints(&b.app.doc, lou);
    assert_eq!(tous.len(), 3, "le plan 0, le plan 1, et la version rangée du plan 0 : {tous:?}");
    assert_eq!(calques_de(&b.app.doc, lou), vec![Some(ROSE); 3], "un calque de Lou par plan, et celui de la version rangée");
    let avant_etapes = etapes(&b);
    changer_couleur(&mut b, lou, VERT);
    b.app.attendre_aplat_perso();
    // Tous les calques de Lou ont la nouvelle couleur d'aplat, dans les deux plans et dans la version rangée.
    assert_eq!(calques_de(&b.app.doc, lou), vec![Some(VERT); 3]);
    for &cel in &tous {
        assert_eq!(compte(&b.app.doc, cel, ROSE), 0, "plus rien de l'ancienne couleur dans la strate de {cel:?}");
        assert!(compte(&b.app.doc, cel, VERT) > MIN, "la peinture brune est devenue verte");
    }
    assert_eq!(compte_blanc(&b.app.doc, cel0), 36, "le blanc de l'œil n'est pas touché");
    // Le calque ordinaire n'a bougé ni de couleur ni de peinture.
    assert_eq!(b.app.doc.panels[0].layers.iter().find(|l| l.frames.first().copied().flatten() == Some(ordinaire)).and_then(|l| l.fond).map(|f| f.couleur), Some(ROSE));
    assert_eq!(compte(&b.app.doc, ordinaire, ROSE), 1);
    assert_eq!(compte_blanc(&b.app.doc, ordinaire), 36);
    // Une seule étape d'annulation pour tout.
    assert_eq!(etapes(&b), avant_etapes + 1, "{:?}", b.app.history.names().0);
    b.app.undo();
    assert_eq!(etapes(&b), avant_etapes, "une seule annulation remet tout");
    assert_eq!(b.app.doc.personnages.iter().find(|p| p.id == lou).and_then(|p| p.fill), Some(ROSE));
    assert_eq!(calques_de(&b.app.doc, lou), vec![Some(ROSE); 3]);
    for &cel in &tous {
        assert_eq!(compte(&b.app.doc, cel, VERT), 0, "la peinture a retrouvé sa couleur");
        assert!(compte(&b.app.doc, cel, ROSE) > MIN);
    }
    assert_eq!(compte_blanc(&b.app.doc, cel0), 36);
    // Et on le rétablit d'un coup.
    b.app.redo();
    assert_eq!(calques_de(&b.app.doc, lou), vec![Some(VERT); 3]);
    for &cel in &tous {
        assert_eq!(compte(&b.app.doc, cel, ROSE), 0);
        assert!(compte(&b.app.doc, cel, VERT) > MIN);
    }
}

#[test]
fn a_drag_in_the_colour_field_is_one_step_and_lands_at_the_end_of_the_gesture() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    anneau(&mut b, 0.0);
    let cel = dessins_de(&b.app.doc, lou)[0];
    let avant = etapes(&b);
    // Le glissé : dix couleurs de suite, sans rien poser avant la fin.
    let teintes: Vec<[u8; 3]> = (0..10).map(|k| [40 + 10 * k as u8, 170, 60]).collect();
    for t in &teintes {
        changer_couleur(&mut b, lou, *t);
    }
    assert!(compte(&b.app.doc, cel, ROSE) > MIN, "pendant le glissé, la peinture ne bouge pas");
    assert_eq!(b.app.layers()[b.app.doc.calque_de_personnage(0, lou).unwrap()].fond.map(|f| f.couleur), Some(ROSE));
    let fin = *teintes.last().unwrap();
    b.app.attendre_aplat_perso();
    assert_eq!(compte(&b.app.doc, cel, ROSE), 0);
    assert!(compte(&b.app.doc, cel, fin) > MIN, "la dernière couleur du glissé");
    assert_eq!(calques_de(&b.app.doc, lou), vec![Some(fin)]);
    assert_eq!(etapes(&b), avant + 1, "une seule étape pour tout le glissé : {:?}", b.app.history.names().0);
    // La fenêtre de choix est restée ouverte : un autre changement est encore le même geste, et la même étape.
    changer_couleur(&mut b, lou, VERT);
    b.app.attendre_aplat_perso();
    assert!(compte(&b.app.doc, cel, VERT) > MIN);
    assert_eq!(compte(&b.app.doc, cel, fin), 0, "l'ancienne couleur de la peinture est celle d'après le premier changement");
    assert_eq!(etapes(&b), avant + 1, "toujours une seule étape : {:?}", b.app.history.names().0);
    b.app.undo();
    assert_eq!(etapes(&b), avant);
    assert!(compte(&b.app.doc, cel, ROSE) > MIN, "une annulation remet la couleur d'avant tout le geste");
    assert_eq!(compte(&b.app.doc, cel, VERT) + compte(&b.app.doc, cel, fin), 0);
    // Un geste d'après (le champ s'est fermé) est une étape à lui.
    b.app.gesture.reset();
    changer_couleur(&mut b, lou, fin);
    b.app.attendre_aplat_perso();
    assert_eq!(etapes(&b), avant + 1);
    b.app.gesture.reset();
    changer_couleur(&mut b, lou, VERT);
    b.app.attendre_aplat_perso();
    assert_eq!(etapes(&b), avant + 2);
    b.app.undo();
    assert!(compte(&b.app.doc, cel, fin) > MIN, "le dernier geste s'annule seul");
}

#[test]
fn saving_right_after_the_change_waits_for_the_paint() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    anneau(&mut b, 0.0);
    changer_couleur(&mut b, lou, VERT);
    assert!(b.app.perso.aplat.occupe(), "le changement est retenu, pas encore posé");
    let dossier = std::env::temp_dir().join(format!("shot-aplatperso-enregistrer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    std::fs::create_dir_all(&dossier).unwrap();
    assert!(b.app.save_to_blocking(dossier.join("essai.shot")), "{}", b.app.status);
    assert!(!b.app.perso.aplat.occupe(), "plus rien en route après l'enregistrement");
    let relu = crate::project::load(&dossier.join("essai.shot")).expect("relecture");
    let cel = dessins_de(&relu.doc, lou)[0];
    assert_eq!(compte(&relu.doc, cel, ROSE), 0, "le fichier a la couleur nouvelle");
    assert!(compte(&relu.doc, cel, VERT) > MIN);
    assert_eq!(calques_de(&relu.doc, lou), vec![Some(VERT)]);
    let _ = std::fs::remove_dir_all(&dossier);
}

#[test]
fn other_commands_do_not_change_the_aplat_colour_of_a_character_layer() {
    let mut b = banc();
    let lou = personnage(&mut b, "Lou", ROUGE, Some(ROSE));
    anneau(&mut b, 0.0);
    let li = b.app.doc.calque_de_personnage(0, lou).expect("le calque de Lou");
    // « Couleur du projet à tous les aplats », « Tout le plan », un changement de couleur sur « Tout le storyboard » d'un autre calque.
    b.app.doc.properties.fond = Some([1, 2, 3]);
    b.app.run_action("fond.couleur_partout");
    assert_eq!(b.app.layers()[li].fond.map(|f| f.couleur), Some(ROSE));
    let autre = b.app.layers().iter().position(|l| l.personnage.is_none()).expect("calque ordinaire");
    b.app.select_layer(autre);
    b.app.layers_mut()[autre].fond = Some(FondReglages { couleur: [9, 9, 9], ..FondReglages::default() });
    b.app.run_action("fond.tout_le_plan");
    assert_eq!(b.app.layers()[li].fond.map(|f| f.couleur), Some(ROSE), "« tout le plan » garde la couleur de Lou");
    assert_eq!(b.app.layers()[autre].fond.map(|f| f.couleur), Some([9, 9, 9]));
    b.app.aplat_portee = crate::aplatcouleur::AplatPortee::Storyboard;
    b.app.choisir_couleur_aplat(autre, [200, 200, 0], 1);
    b.app.layers_mut()[autre].fond.as_mut().unwrap().couleur = ROSE;
    b.app.aplat_couleur_attente.as_mut().unwrap().set_old_for_test(ROSE);
    b.app.appliquer_couleur_aplat();
    assert_eq!(b.app.layers()[li].fond.map(|f| f.couleur), Some(ROSE), "« tout le storyboard » ne touche pas les calques de personnage");
    // Retirer puis remettre l'aplat d'un calque de personnage : la couleur de son personnage, pas celle du projet.
    b.app.toggle_fond(li);
    assert!(b.app.layers()[li].fond.is_none());
    b.app.toggle_fond(li);
    assert_eq!(b.app.layers()[li].fond.map(|f| f.couleur), Some(ROSE));
}

// ------------------------------------------------------------------------------------------ la tolérance

fn pixel(r: &Raster, x: usize) -> Color32 {
    r.get(x, 0)
}

#[test]
fn the_tolerance_takes_the_antialiased_edge_and_leaves_the_other_colours() {
    let (old, new) = (ROSE, VERT);
    let mut r = Raster::new(128, 64);
    let mut pose = |x: usize, c: Color32| {
        let id = r.tile_index_of(x, 0);
        r.tile_mut(id)[x % 64] = c;
    };
    pose(0, premultiply(old[0], old[1], old[2], 255));
    pose(1, premultiply(old[0], old[1], old[2], 128));
    pose(2, premultiply(old[0], old[1], old[2], 20));
    // Une couleur voisine d'un niveau, puis de dix : la première est de l'aplat à l'arrondi près, la seconde est une autre peinture.
    pose(3, Color32::from_rgb(old[0] + 1, old[1], old[2] - 1));
    pose(4, Color32::from_rgb(old[0] + 10, old[1], old[2]));
    pose(5, Color32::WHITE);
    pose(6, premultiply(255, 255, 255, 100));
    pose(7, Color32::TRANSPARENT);
    pose(8, Color32::from_rgb(200, 30, 30));
    let blanc_50 = pixel(&r, 6);
    let avant = recolorer_strate(&mut r, old, new);
    assert_eq!(avant.len(), 1, "une tuile touchée, une seule à rendre à l'annulation");
    for (x, a) in [(0, 255u8), (1, 128), (2, 20)] {
        assert_eq!(pixel(&r, x), premultiply(new[0], new[1], new[2], a), "pixel {x} : la nouvelle couleur, l'opacité gardée");
    }
    assert_eq!(pixel(&r, 3), premultiply(new[0], new[1], new[2], 255), "un niveau d'écart : même aplat");
    assert_eq!(pixel(&r, 4), Color32::from_rgb(old[0] + 10, old[1], old[2]), "dix niveaux d'écart : une autre peinture");
    assert_eq!(pixel(&r, 5), Color32::WHITE, "le blanc reste blanc");
    assert_eq!(pixel(&r, 6), blanc_50, "même un blanc transparent");
    assert_eq!(pixel(&r, 7), Color32::TRANSPARENT);
    assert_eq!(pixel(&r, 8), Color32::from_rgb(200, 30, 30));
    // Rien à recolorier : aucune tuile à rendre.
    assert!(recolorer_strate(&mut r, old, new).is_empty());
}
