//! Essais de la partie 191 : **dupliquer la séquence** (`dupliquerseq.rs`). La copie a les mêmes plans (nombre, durées, noms,
//! caméra, calques, dessins au pixel près), juste après l'originale ; une étape d'annulation ; les deux sont indépendantes après
//! des retouches (un trait, un réglage de calque, une clé de caméra, un objet du décor) ; le décor copié avec des numéros neufs et
//! les modèles partagés ; les versions des plans, les transitions, les commentaires, le contenu rogné, les références, les notes
//! de la planche et les versions de séquence copiés et renumérotés ; la commande et son bloc dans keymap.toml ; l'enregistrement
//! et la relecture ; une séquence sans décor ; deux copies, deux noms.

use std::collections::HashSet;
use std::sync::Arc;

use eframe::egui::{self, Color32, Pos2};

use crate::app::App;
use crate::camera::{set_key, Camera};
use crate::decor3d::Decor3d;
use crate::decor3d_tests::{boite, objet};
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::transitions::Kind;
use crate::variantes::Nouvelle;

/// Peint un rectangle plein de la couleur `c` sur le dessin `cel`.
fn peindre(doc: &mut Document, cel: CelId, [x0, x1, y0, y1]: [usize; 4], c: Color32) {
    let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = r.tile_index_of(x, y);
            let (ox, oy) = r.tile_origin(idx);
            r.tile_mut(idx)[(y as i64 - oy) as usize * crate::raster::TILE + (x as i64 - ox) as usize] = c;
        }
    }
    doc.cels.get_mut(&cel).unwrap().version += 1;
}

/// Les pixels d'un dessin.
fn pixels(doc: &Document, cel: CelId) -> Vec<Color32> {
    let r = &doc.cels[&cel].raster;
    (0..doc.height).flat_map(|y| (0..doc.width).map(move |x| (x, y))).map(|(x, y)| r.get(x, y)).collect()
}

/// Un plan de 24 images : un calque bitmap « Perso » (un dessin peint, tenu) et un calque vectoriel « Trait ».
fn remplir(doc: &mut Document, pi: usize, couleur: Color32) {
    let c = doc.new_cel(LayerKind::Bitmap);
    peindre(doc, c, [100, 160, 60, 120], couleur);
    let mut perso = Layer::new("Perso", LayerKind::Bitmap);
    perso.frames = vec![Some(c); 24];
    let v = doc.new_cel(LayerKind::Vector);
    let mut trait_ = Layer::new("Trait", LayerKind::Vector);
    trait_.frames = vec![Some(v); 24];
    doc.panels[pi].layers = vec![perso, trait_];
    doc.panels[pi].length = 24;
}

const BANC: u64 = 900;

/// Trois séquences : A (un plan), « Poursuite » (deux plans, l'un nommé « Bus » avec deux versions, une caméra animée, une couleur de
/// repère, un commentaire, une transition entre eux, une à la sortie, un banc dans le décor 3D), C (un plan).
fn film() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    remplir(&mut doc, 0, Color32::RED);
    let b = doc.add_sequence_after(0);
    remplir(&mut doc, b, Color32::BLUE);
    let b2 = doc.add_scene_after(b);
    remplir(&mut doc, b2, Color32::GREEN);
    let c = doc.add_sequence_after(b2);
    remplir(&mut doc, c, Color32::YELLOW);
    let seq = doc.sequence_of(1);
    doc.sequence_mut(seq).unwrap().name = "Poursuite".into();
    doc.scene_mut(doc.panels[1].scene).unwrap().name = "Bus".into();
    let cam = doc.default_camera();
    set_key(&mut doc.panels[1].camera, 0, cam);
    set_key(&mut doc.panels[1].camera, 20, Camera { zoom: 2.0, x: cam.x + 30.0, ..cam });
    doc.panels[2].color = Some(3);
    doc.panels[2].layers[0].opacity = 0.6;
    doc.nouvelle_version(1, 1_000).expect("une V2");
    doc.add_comment(doc.panels[2].id, 5, "Anthony", 0);
    doc.set_transition(2, Kind::Dissolve).expect("entre les deux plans de la séquence");
    doc.set_transition(3, Kind::Black).expect("à la sortie de la séquence");
    let mut d = Decor3d::new(Default::default());
    d.objets.push(objet(BANC, boite([-0.3, 0.0, -0.2], [0.3, 0.5, 0.2]), 0.0, -3.0));
    doc.decors3d.par_sequence.insert(seq, d);
    doc
}

fn app_avec(nom: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("feanor-dupseq-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (1, 0, 0);
    app
}

/// Le dessin de l'image 0 du calque `li` du plan `pi`.
fn dessin(doc: &Document, pi: usize, li: usize) -> CelId {
    doc.panels[pi].layers[li].frames[0].unwrap()
}

/// Les plans [a, b) et [c, c + b − a) sont-ils les mêmes, au contenu près (numéros mis à part) ?
fn memes_plans(doc: &Document, a: usize, b: usize, c: usize) {
    for k in 0..b - a {
        let (o, p) = (&doc.panels[a + k], &doc.panels[c + k]);
        assert_ne!(o.id, p.id, "un numéro neuf");
        assert_ne!(o.scene, p.scene, "une scène neuve");
        assert_eq!((o.length, o.frame_count()), (p.length, p.frame_count()), "la même durée");
        assert_eq!(doc.scene(o.scene).unwrap().name, doc.scene(p.scene).unwrap().name, "le même nom de plan");
        assert_eq!(o.camera, p.camera, "la même caméra");
        assert_eq!(o.color, p.color, "la même couleur de repère");
        assert_eq!(o.layers.len(), p.layers.len());
        for (x, y) in o.layers.iter().zip(&p.layers) {
            assert_ne!(x.id, y.id);
            assert_eq!((&x.name, x.kind, x.opacity, x.visible, x.frames.len()), (&y.name, y.kind, y.opacity, y.visible, y.frames.len()));
            for (f, g) in x.frames.iter().zip(&y.frames) {
                let (f, g) = (f.unwrap(), g.unwrap());
                assert_ne!(f, g, "un dessin à elle");
                assert_eq!(pixels(doc, f), pixels(doc, g), "au pixel près");
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------- la copie

#[test]
fn la_copie_a_les_memes_plans_juste_apres_l_originale_et_devient_courante() {
    let mut app = app_avec("memes", film());
    let (seq, c) = (app.doc.sequence_of(1), app.doc.sequence_of(3));
    app.sequence_dupliquer(seq);
    assert_eq!(app.doc.panels.len(), 6, "{}", app.status);
    let copie = app.doc.sequence_of(3);
    assert!(copie != seq && copie != c, "une séquence neuve");
    assert_eq!(app.doc.sequence_order(), vec![app.doc.sequence_of(0), seq, copie, c], "juste après l'originale, avant la suivante");
    assert_eq!((app.cur_panel, app.doc.sequence_run(3)), (3, (3, 5)), "la copie devient courante, sur son premier plan");
    memes_plans(&app.doc, 1, 3, 3);
    assert_eq!(app.doc.sequence_label(copie), "Poursuite (copie)");
    assert!(app.status.contains("Poursuite") && app.status.contains("Poursuite (copie)") && app.status.contains("2 plans"), "{}", app.status);
}

#[test]
fn une_seule_etape_d_annulation_l_annuler_retire_tout_le_retablir_rend_tout() {
    let mut app = app_avec("annuler", film());
    let seq = app.doc.sequence_of(1);
    let avant = app.history.undo_len();
    let (n_seq, n_scenes, n_trans, n_vers, n_comm) = (app.doc.sequences.len(), app.doc.scenes.len(), app.doc.transitions.len(), app.doc.versions.len(), app.doc.comments.len());
    app.run_action("sequence.dupliquer");
    assert_eq!(app.history.undo_len(), avant + 1, "une étape : {}", app.status);
    assert_eq!(app.doc.panels.len(), 6);
    let apres: Vec<u64> = app.doc.panels.iter().map(|p| p.id).collect();
    app.undo();
    assert_eq!(app.doc.panels.len(), 4);
    assert_eq!((app.doc.sequences.len(), app.doc.scenes.len(), app.doc.transitions.len(), app.doc.versions.len(), app.doc.comments.len()), (n_seq, n_scenes, n_trans, n_vers, n_comm), "tout ce que la copie avait ajouté est parti");
    assert_eq!(app.doc.decors3d.par_sequence.len(), 1, "un seul décor");
    assert_eq!(app.doc.sequence_of(1), seq);
    app.redo();
    assert_eq!(app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), apres, "Ctrl+Y rend la copie telle quelle");
    assert_eq!(app.doc.decors3d.par_sequence.len(), 2);
}

#[test]
fn apres_la_copie_retoucher_l_une_laisse_l_autre() {
    let mut app = app_avec("independance", film());
    app.sequence_dupliquer(app.doc.sequence_of(1));
    let d = &mut app.doc;
    let (orig, copie) = (dessin(d, 1, 0), dessin(d, 3, 0));
    let avant = pixels(d, orig);
    // Un trait dans un dessin de la copie.
    peindre(d, copie, [0, 20, 0, 20], Color32::BLACK);
    assert_eq!(pixels(d, orig), avant, "le dessin de l'originale n'a pas bougé");
    assert_ne!(pixels(d, copie), avant);
    // Et l'inverse.
    let (orig2, copie2) = (dessin(d, 2, 0), dessin(d, 4, 0));
    let avant2 = pixels(d, copie2);
    peindre(d, orig2, [200, 220, 0, 20], Color32::BLACK);
    assert_eq!(pixels(d, copie2), avant2);
    // Un réglage de calque.
    d.panels[3].layers[0].opacity = 0.2;
    d.panels[3].layers[1].name = "Autre".into();
    assert_eq!((d.panels[1].layers[0].opacity, d.panels[1].layers[1].name.as_str()), (1.0, "Trait"));
    // Une clé de caméra.
    let cam = d.panels[1].camera.clone();
    let defaut = d.default_camera();
    set_key(&mut d.panels[3].camera, 10, Camera { zoom: 3.0, ..defaut });
    assert_eq!(d.panels[1].camera, cam);
    // Un objet du décor.
    let (s, sc) = (d.sequence_of(1), d.sequence_of(3));
    let place = d.decors3d.par_sequence[&s].objets[0].place;
    d.decors3d.par_sequence.get_mut(&sc).unwrap().objets[0].place.position[0] += 2.0;
    assert_eq!(d.decors3d.par_sequence[&s].objets[0].place, place, "l'objet de l'originale n'a pas bougé");
    // Un nom de plan.
    let scene = d.panels[3].scene;
    d.scene_mut(scene).unwrap().name = "Car".into();
    assert_eq!(d.scene_label(d.panels[1].scene), "Bus");
}

#[test]
fn le_decor_est_copie_avec_des_numeros_neufs_et_le_meme_modele() {
    let mut app = app_avec("decor", film());
    app.sequence_dupliquer(app.doc.sequence_of(1));
    let (a, b) = (app.doc.decor_du_plan(1).unwrap(), app.doc.decor_du_plan(3).expect("la copie a son décor"));
    assert_eq!(a.objets.len(), b.objets.len());
    assert_ne!(a.uid, b.uid);
    for (x, y) in a.objets.iter().zip(&b.objets) {
        assert_ne!(x.id, y.id, "des numéros neufs");
        assert!(Arc::ptr_eq(&x.modele, &y.modele), "le même modèle, partagé");
        assert_eq!((&x.nom, x.place, x.origine, x.aimante), (&y.nom, y.place, y.origine, y.aimante));
    }
    assert!(app.doc.decor_du_plan(0).is_none() && app.doc.decor_du_plan(5).is_none(), "les autres séquences n'en ont pas");
}

#[test]
fn une_sequence_sans_decor_se_duplique_sans_decor() {
    let mut app = app_avec("sans-decor", film());
    app.select_panel(0);
    app.run_action("sequence.dupliquer");
    assert_eq!(app.doc.panels.len(), 5, "{}", app.status);
    assert_eq!(app.cur_panel, 1);
    assert_ne!(app.doc.sequence_of(1), app.doc.sequence_of(0));
    memes_plans(&app.doc, 0, 1, 1);
    assert!(app.doc.decor_du_plan(1).is_none());
    assert_eq!(app.doc.decors3d.par_sequence.len(), 1);
    // Sans nom, la copie n'en a pas : son numéro automatique la distingue.
    let (a, b) = (app.doc.sequence_of(0), app.doc.sequence_of(1));
    assert!(app.doc.sequences.iter().find(|s| s.id == b).unwrap().name.is_empty());
    assert_ne!(app.doc.sequence_label(a), app.doc.sequence_label(b));
}

#[test]
fn deux_copies_deux_noms() {
    let mut app = app_avec("noms", film());
    let seq = app.doc.sequence_of(1);
    app.sequence_dupliquer(seq);
    app.sequence_dupliquer(seq);
    // La seconde copie suit l'originale : la première est derrière elle.
    let noms: Vec<String> = app.doc.sequence_order().iter().map(|s| app.doc.sequence_label(*s)).collect();
    assert_eq!(&noms[1..4], &["Poursuite", "Poursuite (copie 2)", "Poursuite (copie)"]);
    // La copie d'une copie.
    let copie = app.doc.sequence_of(app.cur_panel);
    app.sequence_dupliquer(copie);
    assert_eq!(app.doc.sequence_label(app.doc.sequence_of(app.cur_panel)), "Poursuite (copie 3)");
    assert_eq!(crate::dupliquerseq::nom_de_copie(["A (copie)"].into_iter(), "A (copie)"), "A (copie 2)");
    assert_eq!(crate::dupliquerseq::nom_de_copie(std::iter::empty(), "  "), "");
    let long = "x".repeat(crate::planchesequencesui::NOM_MAX);
    assert_eq!(crate::dupliquerseq::nom_de_copie(std::iter::empty(), &long).chars().count(), crate::planchesequencesui::NOM_MAX);
}

// ---------------------------------------------------------------------------------- ce qui tient aux plans

#[test]
fn les_versions_des_plans_sont_copiees_et_independantes() {
    let mut doc = film();
    let seq = doc.sequence_of(1);
    let first = doc.dupliquer_sequence(seq).unwrap();
    assert_eq!(first, 3);
    let (o, c) = (doc.panels[1].id, doc.panels[3].id);
    let (vo, vc) = (doc.versions[&o].clone(), doc.versions[&c].clone());
    assert_eq!((vo.actuelle, vo.prochaine, vo.autres.len()), (vc.actuelle, vc.prochaine, vc.autres.len()), "les mêmes versions, sous les mêmes numéros");
    assert_eq!(vo.autres[0].numero, vc.autres[0].numero);
    let (po, pc) = (&vo.autres[0].panel, &vc.autres[0].panel);
    assert_ne!(po.id, pc.id);
    assert_eq!(pc.scene, doc.panels[3].scene, "la version rangée est dans la scène de la copie");
    let (a, b) = (po.layers[0].frames[0].unwrap(), pc.layers[0].frames[0].unwrap());
    assert_ne!(a, b, "ses dessins à elle");
    assert_eq!(pixels(&doc, a), pixels(&doc, b));
    // Passer la copie en V1 laisse l'originale en V2.
    assert!(doc.changer_de_version(3, 1));
    assert_eq!(doc.numero_de_version(c), 1);
    assert_eq!(doc.numero_de_version(o), 2);
}

#[test]
fn transitions_commentaires_rognage_references_et_notes_suivent_la_copie() {
    let mut doc = film();
    let seq = doc.sequence_of(1);
    // Du contenu rogné sur le premier plan, une référence liée aux deux, une note et une annotation sur la planche.
    let rogne = doc.new_cel(LayerKind::Bitmap);
    let calque = doc.panels[1].layers[0].id;
    let piece = crate::rognage::Piece { len: 1, layers: vec![crate::rognage::LayerPiece { layer: calque, frames: vec![Some(rogne)], ..Default::default() }], ..Default::default() };
    doc.reserves.insert(doc.panels[1].id, crate::rognage::Reserve { head: piece, tail: Default::default() });
    let mut r = crate::references::Reference::image(77, "Repère", "repere.png", [8, 8], Pos2::ZERO);
    r.plans = vec![doc.panels[1].id, doc.panels[2].id];
    doc.references.push(r);
    doc.board.texts.push(crate::board::NoteText { pos: Pos2::new(0.1, 0.2), text: "plus serré".into(), size: 0.1, color: [0, 0, 0], anchor: Some(doc.panels[2].id) });
    let calque_annot = 4_242;
    doc.annotations.calques.push(crate::annotations::Calque { id: calque_annot, nom: "Annotations".into(), visible: true, planche: false });
    let ancre = crate::annotations::Ancre { card: doc.panels[1].id, after: 0, rank: 1 };
    doc.annotations.traces.push(crate::annotations::Trace::new(calque_annot, ancre, [255, 0, 0], Vec::new()));
    let (o1, o2, suivant) = (doc.panels[1].id, doc.panels[2].id, doc.panels[3].id);
    doc.dupliquer_sequence(seq).unwrap();
    let (c1, c2) = (doc.panels[3].id, doc.panels[4].id);
    assert_eq!(doc.panels[5].id, suivant);
    // Les transitions : celle du milieu, copiée ; celle de la sortie, copiée à la fin de la copie (devant le plan suivant) ; celle de
    // l'originale attend.
    let t = |out, inn| doc.transition_on(Some(out), Some(inn)).map(|t| t.kind);
    assert_eq!(t(o1, o2), Some(Kind::Dissolve));
    assert_eq!(t(c1, c2), Some(Kind::Dissolve));
    assert_eq!(t(c2, suivant), Some(Kind::Black), "la coupe de sortie garde sa transition");
    assert_eq!(t(o2, c1), None, "la coupe entre l'originale et sa copie est franche");
    let ids: HashSet<u64> = doc.transitions.iter().map(|t| t.id).collect();
    assert_eq!(ids.len(), doc.transitions.len(), "des numéros neufs");
    let zones = crate::transitions::zones(&doc);
    assert_eq!(zones.len(), 3, "trois transitions jouent : {zones:?}");
    // Les commentaires.
    let de = |p: u64| doc.comments.iter().filter(|k| k.panel == p).count();
    assert_eq!((de(o2), de(c2)), (1, 1));
    // Le contenu rogné : ses dessins à lui, le calque de la copie.
    let rc = &doc.reserves[&c1];
    assert_eq!(rc.head.layers[0].layer, doc.panels[3].layers[0].id);
    assert_ne!(rc.head.layers[0].frames[0], Some(rogne));
    // La référence est liée aussi aux copies.
    assert_eq!(doc.references[0].plans, vec![o1, o2, c1, c2]);
    // La note et l'annotation de la planche.
    assert_eq!(doc.board.texts.iter().filter(|t| t.anchor == Some(c2) && t.text == "plus serré").count(), 1);
    assert_eq!(doc.annotations.traces.iter().filter(|t| t.anchor.card == c1).count(), 1);
    assert_eq!(doc.annotations.traces.iter().filter(|t| t.anchor.card == o1).count(), 1);
}

#[test]
fn la_planche_mere_dans_la_sequence_reste_liee_entre_les_copies_sans_lien_avec_l_original() {
    let mut doc = film();
    let seq = doc.sequence_of(1);
    // Le calque « Perso » du plan 1 est réutilisé dans le plan 2 (de la séquence) et dans le plan 0 (une autre séquence).
    let k = doc.reutiliser_calque(1, 0, &[2, 0]).expect("partageable");
    doc.dupliquer_sequence(seq).unwrap();
    let cle = |pi: usize| doc.panels[pi].layers.iter().find(|l| l.name == "Perso").and_then(|l| l.partage);
    assert_eq!((cle(0), cle(1), cle(2)), (Some(k), Some(k), Some(k)), "l'original garde son lien");
    let kc = cle(3).expect("les copies restent liées entre elles");
    assert_ne!(kc, k);
    assert_eq!(cle(4), Some(kc));
    let perso = |pi: usize| doc.panels[pi].layers.iter().find(|l| l.name == "Perso").unwrap().frames[0];
    assert_eq!(perso(3), perso(4), "les mêmes dessins entre les copies");
    assert_ne!(perso(3), perso(1), "pas ceux de l'original");
}

#[test]
fn les_versions_de_la_sequence_sont_copiees_avec_leur_garage() {
    let mut doc = film();
    let seq = doc.sequence_of(1);
    // A : les mêmes plans (partagés) ; B : une copie des plans (au garage) ; le film montre C.
    doc.nouvelle_variante(seq, Nouvelle::Partager, 0).unwrap();
    doc.nouvelle_variante(seq, Nouvelle::Copier, 0).unwrap();
    let vo = doc.variantes[&seq].clone();
    assert_eq!((vo.nombre(), vo.garage.len()), (3, 2));
    doc.dupliquer_sequence(seq).unwrap();
    let copie = doc.sequence_of(3);
    let vc = doc.variantes[&copie].clone();
    assert_eq!((vc.actuelle, vc.numeros()), (vo.actuelle, vo.numeros()), "les mêmes versions de séquence");
    assert_eq!(vc.garage.len(), 2);
    let garage_o: HashSet<u64> = vo.garage.keys().copied().collect();
    assert!(vc.garage.keys().all(|id| !garage_o.contains(id)), "le garage de la copie a ses propres plans");
    // Les emplacements de la version partagée montrent les plans de la copie.
    let film_copie: Vec<u64> = doc.panels[3..5].iter().map(|p| p.id).collect();
    let a = vc.autres.iter().find(|r| r.numero == 1).unwrap();
    assert_eq!(a.contenu.slots.iter().map(|e| e.plan).collect::<Vec<_>>(), film_copie);
    assert!(a.contenu.scenes.iter().all(|s| s.sequence == copie));
    // Passer la copie à une autre version : ses plans entrent, l'originale ne bouge pas.
    let film_orig: Vec<u64> = doc.panels[1..3].iter().map(|p| p.id).collect();
    assert!(doc.changer_de_variante(copie, 2));
    let (a, b) = doc.course_de_sequence(copie).unwrap();
    let entres: Vec<u64> = doc.panels[a..b].iter().map(|p| p.id).collect();
    assert!(entres.iter().all(|id| vc.garage.contains_key(id)), "les plans de son garage");
    assert_eq!(doc.panels[1..3].iter().map(|p| p.id).collect::<Vec<_>>(), film_orig);
    assert_eq!(doc.variantes[&seq].actuelle, vo.actuelle);
    // Et retour.
    assert!(doc.changer_de_variante(copie, 1));
    assert_eq!(doc.panels[a..a + 2].iter().map(|p| p.id).collect::<Vec<_>>(), film_copie);
}

#[test]
fn en_numerotation_verrouillee_la_copie_recoit_des_numeros_neufs() {
    let mut doc = film();
    doc.numbering_locked = true;
    doc.normalize();
    let seq = doc.sequence_of(1);
    doc.dupliquer_sequence(seq).unwrap();
    let nums: Vec<String> = doc.scene_order().iter().map(|s| doc.scene(*s).unwrap().number.clone()).collect();
    let uniques: HashSet<&String> = nums.iter().collect();
    assert_eq!(uniques.len(), nums.len(), "un numéro est unique dans le projet : {nums:?}");
    assert!(nums.iter().all(|n| !n.is_empty()));
    let sq: Vec<String> = doc.sequence_order().iter().map(|s| doc.sequences.iter().find(|q| q.id == *s).unwrap().number.clone()).collect();
    assert_eq!(sq.iter().collect::<HashSet<_>>().len(), sq.len(), "{sq:?}");
}

// --------------------------------------------------------------------------------------------- la commande

#[test]
fn la_commande_est_dans_la_palette_et_dans_keymap_toml_sans_touche() {
    let action = "sequence.dupliquer";
    let info = crate::keymap::action_info(action).expect("la commande existe");
    assert!(info.implemented && info.label.starts_with("Dupliquer la séquence"));
    assert!(crate::palette::palette_commands().iter().any(|(id, _)| *id == action), "elle est dans la palette");
    let map = crate::keymap_tests::default_map();
    assert!(map.bindings.iter().any(|b| b.action == action) && map.keys_of(action).is_empty(), "nommée dans keymap.toml, sans touche d'origine");
    // Sous le verrou du storyboard : refusé, rien ne change.
    let mut app = app_avec("verrou", film());
    app.doc.storyboard_locked = true;
    let avant = app.history.undo_len();
    app.run_action(action);
    assert_eq!((app.doc.panels.len(), app.history.undo_len()), (4, avant));
}

// ------------------------------------------------------------------------------------------ l'enregistrement

#[test]
fn un_projet_avec_une_sequence_dupliquee_s_enregistre_et_se_relit() {
    let mut doc = film();
    // Une autre version de la séquence, faite d'une copie de ses plans (au garage) : la copie de la séquence en a une à elle.
    let seq = doc.sequence_of(1);
    doc.nouvelle_variante(seq, Nouvelle::Copier, 0).unwrap();
    let mut app = app_avec("enregistrer", doc);
    app.sequence_dupliquer(seq);
    let dir = crate::decor3d_tests::dossier("dupseq-enregistrer");
    let manifeste = dir.join("Film.shot");
    crate::project::save(&app.doc, &manifeste, &mut Default::default()).unwrap();
    let modeles: Vec<_> = std::fs::read_dir(dir.join(crate::decor3d::DIR)).unwrap().flatten().collect();
    assert_eq!(modeles.len(), 1, "un modèle, deux bancs : un seul fichier");
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let d = &relu.doc;
    assert_eq!(d.panels.len(), 6);
    assert_eq!(d.sequence_order().len(), 4);
    assert_eq!(d.sequence_label(d.sequence_of(3)), "Poursuite (copie)");
    memes_plans(d, 1, 3, 3);
    for pi in 0..6 {
        for li in 0..2 {
            assert_eq!(pixels(d, dessin(d, pi, li)), pixels(&app.doc, dessin(&app.doc, pi, li)), "plan {pi}, calque {li} : relu tel quel");
        }
    }
    let (a, b) = (d.decor_du_plan(1).unwrap(), d.decor_du_plan(3).unwrap());
    assert!(Arc::ptr_eq(&a.objets[0].modele, &b.objets[0].modele), "relus : un seul modèle pour les deux bancs");
    assert_ne!(a.objets[0].id, b.objets[0].id);
    assert_eq!((d.versions.len(), d.transitions.len(), d.comments.len()), (app.doc.versions.len(), app.doc.transitions.len(), app.doc.comments.len()));
    // Les versions de séquence des deux, avec leur garage et ses dessins.
    let (vo, vc) = (&d.variantes[&d.sequence_of(1)], &d.variantes[&d.sequence_of(3)]);
    assert_eq!((vo.garage.len(), vc.garage.len()), (2, 2));
    assert!(vo.garage.keys().all(|k| !vc.garage.contains_key(k)));
    for (a, b) in vo.garage.values().zip(vc.garage.values()) {
        let (x, y) = (a.panel.layers[0].frames[0].unwrap(), b.panel.layers[0].frames[0].unwrap());
        assert_ne!(x, y);
        assert_eq!(pixels(d, x), pixels(d, y), "relus tels quels");
    }
    std::fs::remove_dir_all(&dir).ok();
}
