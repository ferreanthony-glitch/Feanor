//! Essais de l'environnement d'un plan (partie 204, `planenv.rs`, `Document::env_cle`, `docs/INTERFACE.md` partie 204) : sortir de
//! l'environnement de sa séquence (vide, copie, lié), le lien en direct, « Détacher », « Revenir » (la question, Ctrl+Z), la marque
//! sur le bloc du plan, l'image du plan et l'export, l'onglet 3D sur un plan sorti, les clés, dupliquer et supprimer, l'enregistrement
//! et un lecteur d'avant, les caches, et rien de changé pour les plans restés.

use std::sync::Arc;

use eframe::egui::{Color32, Modifiers, Rect, Shape};

use crate::decor3d::{self, cle_de_plan, EnvPlan, Objet3d, Place3d};
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::clic;
use crate::planenv::{ChoixEnv, ICONE_LIE, ICONE_PROPRE};
use crate::workspace::Espace;

// ------------------------------------------------------------------------------------------------ les aides

/// Trois plans : 0 et 1 dans la séquence A (son décor : deux carrés, à gauche et à droite du cadre), 2 dans la séquence B (son décor :
/// un carré au milieu). Rend le projet et les deux séquences.
fn film() -> (Document, u64, u64) {
    let mut doc = crate::decor3d_tests::projet_a_deux_carres();
    doc.add_scene_after(0);
    doc.add_sequence_after(1);
    let (a, b) = (doc.sequence_of(0), doc.sequence_of(2));
    assert_eq!(doc.sequence_of(1), a);
    assert_ne!(a, b);
    let modele = doc.decors3d.par_sequence[&a].objets[0].modele.clone();
    let place = Place3d { position: [0.0, 0.0, -0.001], ..doc.decors3d.par_sequence[&a].objets[0].place };
    let mut d = decor3d::Decor3d::new(doc.decors3d.par_sequence[&a].aspect);
    let id = doc.alloc();
    d.objets.push(Objet3d { id, nom: "Milieu".into(), source: "carre.glb".into(), allege: Vec::new(), place, origine: [0.0; 3], aimante: false, modele, couleur: None, cache: false, verrou: false, cles: Vec::new(), caches_dans: Vec::new() });
    doc.decors3d.par_sequence.insert(b, d);
    (doc, a, b)
}

/// L'image exportée du plan `p` (son image 0).
fn exporter(doc: &Document, p: usize) -> Vec<Color32> {
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    let rendu = Rendu::new(snapshot_of(doc, &[p]), Papier::Aucun, false, Vec::new(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
    (*rendu.image(p, 0).expect("image")).clone()
}

/// Ce que l'image du plan `p` montre à gauche (w/4), au milieu, à droite (3w/4) : un carré (opaque) ou rien.
fn carres(doc: &Document, p: usize) -> [bool; 3] {
    let img = exporter(doc, p);
    let [w, h] = doc.frame;
    [w / 4, w / 2, 3 * w / 4].map(|x| img[(h / 2) * w + x].a() == 255)
}

fn banc_sb(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1600, 1000);
    b.app.set_espace(Espace::Storyboard);
    b.garder_formes = true;
    b.chauffer(6);
    b
}

fn etapes(b: &Banc) -> Vec<String> {
    b.app.history.names().0.into_iter().map(String::from).collect()
}

fn annuler(b: &mut Banc) {
    b.app.run_action("edition.annuler");
    b.chauffer(2);
}

fn textes(b: &Banc) -> Vec<(String, Rect)> {
    fn chercher(s: &Shape, out: &mut Vec<(String, Rect)>) {
        match s {
            Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
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

/// Déplace (en retouche du décor, comme le panneau) le premier objet de l'environnement de clé `cle` de `dx` le long de x.
fn pousser(b: &mut Banc, cle: u64, dx: f32) {
    let mut d = b.app.doc.decors3d.par_sequence[&cle].clone();
    d.objets[0].place.position[0] += dx;
    b.app.decor3d_retoucher(cle, d);
    b.app.decor3d_retouche_finir();
}

// ------------------------------------------------------------------------------------------------ 1. sortir

#[test]
fn a_plan_leaves_its_sequence_environment_empty_as_a_copy_or_linked_and_each_is_one_undo_step() {
    let (doc, a, b) = film();
    let mut b_ = banc_sb(doc);
    let bc = &mut b_;
    assert_eq!((carres(&bc.app.doc, 0), carres(&bc.app.doc, 1), carres(&bc.app.doc, 2)), ([true, false, true], [true, false, true], [false, true, false]));
    // Vide : plus rien dans ce plan ; l'autre plan de la séquence garde ses carrés.
    assert!(bc.app.changer_env(1, ChoixEnv::Vide));
    assert_eq!(bc.app.doc.decors3d.envs.get(&bc.app.doc.panels[1].id).copied().map(|e| matches!(e, EnvPlan::Propre { .. })), Some(true));
    assert!(bc.app.doc.decor_du_plan(1).is_none());
    assert_eq!(carres(&bc.app.doc, 1), [false, false, false]);
    assert_eq!(carres(&bc.app.doc, 0), [true, false, true]);
    assert_eq!(etapes(bc).last().map(String::as_str), Some("Environnement propre vide"));
    annuler(bc);
    assert!(bc.app.doc.decors3d.envs.is_empty() && carres(&bc.app.doc, 1) == [true, false, true]);
    // Copie : les mêmes carrés, d'autres objets (numéros, décor) ; la retoucher ne touche qu'à ce plan.
    assert!(bc.app.changer_env(1, ChoixEnv::Copie));
    let p1 = bc.app.doc.panels[1].id;
    let (da, dp) = (&bc.app.doc.decors3d.par_sequence[&a], &bc.app.doc.decors3d.par_sequence[&cle_de_plan(p1)]);
    assert_eq!(dp.objets.len(), 2);
    assert!(dp.uid != da.uid && dp.objets.iter().all(|o| da.objet(o.id).is_none()));
    assert_eq!(carres(&bc.app.doc, 1), [true, false, true]);
    bc.app.cur_panel = 1;
    pousser(bc, cle_de_plan(p1), 0.0);
    // Lié à B : son carré du milieu ; le même décor (le même numéro).
    assert!(bc.app.changer_env(1, ChoixEnv::Lie(b)));
    assert_eq!(bc.app.doc.env_cle(1), b);
    assert_eq!(bc.app.doc.decor_du_plan(1).unwrap().uid, bc.app.doc.decors3d.par_sequence[&b].uid);
    assert_eq!(carres(&bc.app.doc, 1), [false, true, false]);
    assert!(!bc.app.doc.decors3d.par_sequence.contains_key(&cle_de_plan(p1)), "l'environnement propre d'avant est parti");
    assert_eq!(etapes(bc).last().map(String::as_str), Some("Environnement lié"));
    // Lié à sa propre séquence, c'est la séquence : rien.
    assert!(bc.app.changer_env(1, ChoixEnv::Sequence));
    assert!(!bc.app.changer_env(1, ChoixEnv::Lie(a)));
    assert!(bc.app.doc.decors3d.envs.is_empty());
}

#[test]
fn a_linked_plan_sees_the_other_sequence_live_and_detacher_makes_it_its_own() {
    let (doc, _a, b) = film();
    let mut bc = banc_sb(doc);
    assert!(bc.app.changer_env(1, ChoixEnv::Lie(b)));
    let avant = carres(&bc.app.doc, 1);
    // On déplace le carré de B vers la gauche : le plan 1 le voit, tout de suite.
    pousser(&mut bc, b, -0.8);
    let x = bc.app.doc.decors3d.par_sequence[&b].objets[0].place.position[0];
    assert_eq!(bc.app.doc.decor_du_plan(1).unwrap().objets[0].place.position[0], x);
    assert_ne!(carres(&bc.app.doc, 1), avant, "en direct");
    assert_eq!(carres(&bc.app.doc, 1), carres(&bc.app.doc, 2), "le plan 1 montre ce que montre B");
    // Détacher : une copie à lui ; B bouge encore, le plan 1 non.
    assert!(bc.app.changer_env(1, ChoixEnv::Detacher));
    assert_eq!(etapes(&bc).last().map(String::as_str), Some("Détacher l'environnement"));
    let p1 = bc.app.doc.panels[1].id;
    assert!(matches!(bc.app.doc.decors3d.envs[&p1], EnvPlan::Propre { .. }));
    let fige = carres(&bc.app.doc, 1);
    pousser(&mut bc, b, 0.8);
    assert_eq!(carres(&bc.app.doc, 1), fige, "détaché : il ne suit plus");
    assert_ne!(carres(&bc.app.doc, 2), fige);
    // Détacher n'a de sens que pour un lien.
    assert!(!bc.app.changer_env(1, ChoixEnv::Detacher));
}

#[test]
fn revenir_asks_only_for_an_edited_own_environment_and_undo_brings_it_back() {
    let (doc, _a, _b) = film();
    let mut bc = banc_sb(doc);
    let p1 = bc.app.doc.panels[1].id;
    // Une copie pas retouchée : revenir sans question.
    assert!(bc.app.changer_env(1, ChoixEnv::Copie));
    assert!(!bc.app.env_propre_retouche(1));
    bc.app.demander_env(1, ChoixEnv::Sequence);
    assert!(bc.app.o3.env_a_jeter.is_none() && bc.app.doc.decors3d.envs.is_empty());
    // Retouchée : la question.
    assert!(bc.app.changer_env(1, ChoixEnv::Copie));
    bc.app.cur_panel = 1;
    pousser(&mut bc, cle_de_plan(p1), 0.5);
    assert!(bc.app.env_propre_retouche(1));
    let retouche = bc.app.doc.decors3d.par_sequence[&cle_de_plan(p1)].objets[0].place;
    bc.app.demander_env(1, ChoixEnv::Sequence);
    bc.chauffer(3);
    assert_eq!(bc.app.o3.env_a_jeter, Some(p1));
    assert!(bc.app.doc.decors3d.envs.contains_key(&p1), "rien avant la réponse");
    let annuler_ = textes(&bc).into_iter().rev().find(|(t, _)| t == "Annuler").expect("« Annuler »").1;
    clic(&mut bc, annuler_.center(), Modifiers::NONE);
    bc.chauffer(2);
    assert!(bc.app.o3.env_a_jeter.is_none() && bc.app.doc.decors3d.envs.contains_key(&p1));
    bc.app.demander_env(1, ChoixEnv::Sequence);
    bc.chauffer(3);
    let oui = textes(&bc).into_iter().find(|(t, _)| t == "Revenir à la séquence").expect("« Revenir à la séquence »").1;
    clic(&mut bc, oui.center(), Modifiers::NONE);
    bc.chauffer(2);
    assert!(bc.app.doc.decors3d.envs.is_empty() && !bc.app.doc.decors3d.par_sequence.contains_key(&cle_de_plan(p1)));
    assert_eq!(etapes(&bc).last().map(String::as_str), Some("Revenir à l'environnement de la séquence"));
    annuler(&mut bc);
    assert_eq!(bc.app.doc.decors3d.par_sequence[&cle_de_plan(p1)].objets[0].place, retouche, "Ctrl+Z rend l'environnement retouché");
}

// ------------------------------------------------------------------------------------------------ 2. la marque

#[test]
fn the_plan_block_shows_a_mark_with_its_tooltip_in_the_storyboard_and_the_3d_timeline() {
    let (doc, _a, b) = film();
    let mut bc = banc_sb(doc);
    let (p0, p1, p2) = (bc.app.doc.panels[0].id, bc.app.doc.panels[1].id, bc.app.doc.panels[2].id);
    let marques = || crate::planenv::MARQUES.with(|m| m.borrow().clone());
    assert!(marques().is_empty(), "aucun plan sorti : aucune marque");
    bc.app.changer_env(1, ChoixEnv::Lie(b));
    bc.app.changer_env(2, ChoixEnv::Copie);
    bc.chauffer(3);
    let m = marques();
    assert!(m.iter().all(|(p, _, _)| *p != p0), "le plan resté n'a rien");
    let lie = m.iter().find(|(p, _, _)| *p == p1).expect("la marque du plan lié");
    assert!(lie.2.contains("lié à la séquence"), "{}", lie.2);
    let propre = m.iter().find(|(p, _, _)| *p == p2).expect("la marque du plan propre");
    assert!(propre.2.contains("propre"), "{}", propre.2);
    let t: Vec<String> = textes(&bc).into_iter().map(|(t, _)| t).collect();
    assert!(t.iter().any(|x| x == ICONE_LIE) && t.iter().any(|x| x == ICONE_PROPRE) && ICONE_LIE != ICONE_PROPRE);
    // Dans la timeline de la séquence de l'onglet 3D (la séquence A : le plan 1, lié).
    bc.app.select_panel(1);
    bc.app.set_espace(Espace::TroisD);
    bc.chauffer(6);
    assert!(marques().iter().any(|(p, _, phrase)| *p == p1 && phrase.contains("lié")), "{:?}", marques());
    // Les glyphes sont dans les polices de FEANOR.
    assert!(bc.ctx.fonts(|f| f.has_glyphs(&eframe::egui::FontId::proportional(11.0), &format!("{ICONE_LIE}{ICONE_PROPRE}"))));
}

// ------------------------------------------------------------------------------------------------ 3. l'onglet 3D sur un plan sorti

#[test]
fn on_a_plan_with_its_own_environment_the_3d_tab_edits_that_environment() {
    let (doc, a, b) = film();
    let mut bc = banc_sb(doc);
    bc.app.select_panel(1);
    assert!(bc.app.changer_env(1, ChoixEnv::Copie));
    bc.app.set_espace(Espace::TroisD);
    bc.chauffer(6);
    let p1 = bc.app.doc.panels[1].id;
    let id = bc.app.doc.decors3d.par_sequence[&cle_de_plan(p1)].objets[0].id;
    // Dupliquer un objet (le panneau, la vue) : dans l'environnement du plan, pas dans celui de la séquence.
    bc.app.decor3d_dupliquer_objet(id);
    assert_eq!(bc.app.doc.decors3d.par_sequence[&cle_de_plan(p1)].objets.len(), 3);
    assert_eq!(bc.app.doc.decors3d.par_sequence[&a].objets.len(), 2);
    assert_eq!(bc.app.doc.decor_du_plan(1).unwrap().objets.len(), 3);
    assert_eq!(bc.app.doc.decor_du_plan(0).unwrap().objets.len(), 2);
    // Le cyclo aussi : celui du plan.
    bc.app.changer_cyclo("Cyclo", Some(crate::cyclos::Preset::Nuit.cyclo()));
    assert!(bc.app.doc.decors3d.par_sequence[&cle_de_plan(p1)].cyclo.is_some() && bc.app.doc.decors3d.par_sequence[&a].cyclo.is_none());
    // La tête de Scène 3D dit lequel on retouche.
    bc.chauffer(4);
    let t: Vec<String> = textes(&bc).into_iter().map(|(t, _)| t).collect();
    assert!(t.iter().any(|x| x.contains("environnement propre du plan")), "{t:?}");
    // Lié à B : on retouche celui de B (la tête le dit).
    assert!(bc.app.changer_env(1, ChoixEnv::Lie(b)));
    bc.chauffer(4);
    let idb = bc.app.doc.decors3d.par_sequence[&b].objets[0].id;
    bc.app.decor3d_dupliquer_objet(idb);
    assert_eq!(bc.app.doc.decors3d.par_sequence[&b].objets.len(), 2);
    let t: Vec<String> = textes(&bc).into_iter().map(|(t, _)| t).collect();
    assert!(t.iter().any(|x| x.contains(": environnement de la séquence")), "{t:?}");
}

// ------------------------------------------------------------------------------------------------ 4. les clés

#[test]
fn object_keys_are_read_at_the_plans_image_in_its_own_sequence_whatever_its_environment() {
    let (mut doc, _a, b) = film();
    // Un objet animé dans B : de x = −1 à l'image 0 de la séquence à x = +1 à l'image 40.
    {
        let d = doc.decors3d.par_sequence.get_mut(&b).unwrap();
        let o = &mut d.objets[0];
        let p0 = o.place;
        o.cles = vec![crate::miseenscene3d::CleObjet { image: 0, place: Place3d { position: [-1.0, 0.0, -0.001], ..p0 } }, crate::miseenscene3d::CleObjet { image: 40, place: Place3d { position: [1.0, 0.0, -0.001], ..p0 } }];
    }
    let mut bc = banc_sb(doc);
    assert!(bc.app.changer_env(1, ChoixEnv::Lie(b)));
    let doc = &bc.app.doc;
    for f in [0usize, 5, 12] {
        let cam = doc.camera_seen(1, f);
        let item = decor3d::item(doc, 1, f, &cam, None).expect("le décor du plan 1");
        let Some(crate::render::FondPart::Decor(k)) = item.fond else { panic!() };
        let image = doc.image_de_sequence(1, f);
        assert_eq!(k.image, image, "l'image de la séquence du plan 1, pas de celle de B");
        let poses = decor3d::objets_de(doc, &k).unwrap();
        assert_eq!(poses[0].1, doc.decors3d.par_sequence[&b].objets[0].matrice_a(image));
    }
    // Une copie garde les clés (elle s'anime comme l'original).
    assert!(bc.app.changer_env(1, ChoixEnv::Detacher));
    let p1 = bc.app.doc.panels[1].id;
    assert_eq!(bc.app.doc.decors3d.par_sequence[&cle_de_plan(p1)].objets[0].cles.len(), 2);
}

// ------------------------------------------------------------------------------------------------ 5. dupliquer, supprimer

#[test]
fn duplicating_a_sequence_keeps_own_environments_and_links_and_deleting_a_linked_sequence_asks_then_copies() {
    let (doc, a, b) = film();
    let mut bc = banc_sb(doc);
    assert!(bc.app.changer_env(0, ChoixEnv::Copie));
    assert!(bc.app.changer_env(1, ChoixEnv::Lie(b)));
    let (p0, p1) = (bc.app.doc.panels[0].id, bc.app.doc.panels[1].id);
    bc.app.doc.dupliquer_sequence(a).expect("la copie");
    let neuve = *bc.app.doc.sequence_order().iter().find(|s| **s != a && **s != b).unwrap();
    let copies: Vec<usize> = (0..bc.app.doc.panels.len()).filter(|&i| bc.app.doc.sequence_of(i) == neuve).collect();
    assert_eq!(copies.len(), 2);
    let (c0, c1) = (bc.app.doc.panels[copies[0]].id, bc.app.doc.panels[copies[1]].id);
    assert!(matches!(bc.app.doc.decors3d.envs[&c0], EnvPlan::Propre { .. }));
    let (d0, dc0) = (&bc.app.doc.decors3d.par_sequence[&cle_de_plan(p0)], &bc.app.doc.decors3d.par_sequence[&cle_de_plan(c0)]);
    assert!(d0.uid != dc0.uid && dc0.objets.len() == d0.objets.len() && dc0.objets.iter().all(|o| d0.objet(o.id).is_none()), "une copie à part");
    assert_eq!(bc.app.doc.decors3d.envs[&c1], EnvPlan::Lie(b), "le lien reste un lien vers B");
    assert_eq!(bc.app.doc.decors3d.envs[&p1], EnvPlan::Lie(b));
    // Supprimer B (son seul plan) : deux plans y sont liés ; la question d'abord.
    let pb = bc.app.doc.panels[(0..bc.app.doc.panels.len()).find(|&i| bc.app.doc.sequence_of(i) == b).unwrap()].id;
    bc.app.delete_panels(&[pb]);
    bc.chauffer(3);
    assert_eq!(bc.app.o3.suppression_liee, Some(vec![pb]));
    assert!(bc.app.doc.panels.iter().any(|p| p.id == pb), "rien avant la réponse");
    let oui = textes(&bc).into_iter().find(|(t, _)| t == "Supprimer").expect("« Supprimer »").1;
    clic(&mut bc, oui.center(), Modifiers::NONE);
    bc.chauffer(2);
    assert!(!bc.app.doc.panels.iter().any(|p| p.id == pb));
    for p in [p1, c1] {
        assert!(matches!(bc.app.doc.decors3d.envs[&p], EnvPlan::Propre { .. }), "le lien est devenu une copie");
        let d = &bc.app.doc.decors3d.par_sequence[&cle_de_plan(p)];
        assert_eq!(d.objets.len(), 1);
        assert_eq!(d.objets[0].nom, "Milieu");
    }
    let i1 = bc.app.doc.panels.iter().position(|p| p.id == p1).unwrap();
    assert_eq!(carres(&bc.app.doc, i1), [false, true, false], "il montre toujours le carré du milieu");
    // Ctrl+Z : B revient, et les liens.
    annuler(&mut bc);
    assert!(bc.app.doc.panels.iter().any(|p| p.id == pb));
    assert_eq!(bc.app.doc.decors3d.envs[&p1], EnvPlan::Lie(b));
}

// ------------------------------------------------------------------------------------------------ 6. l'enregistrement

#[test]
fn own_linked_and_empty_environments_are_saved_and_read_back_and_an_old_reader_still_opens_the_project() {
    let dir = crate::decor3d_tests::dossier("planenv204");
    let manifeste = dir.join("Film.shot");
    let (doc, a, b) = film();
    let mut bc = banc_sb(doc);
    assert!(bc.app.changer_env(0, ChoixEnv::Vide));
    assert!(bc.app.changer_env(1, ChoixEnv::Lie(b)));
    assert!(bc.app.changer_env(2, ChoixEnv::Copie));
    bc.app.cur_panel = 2;
    let p2 = bc.app.doc.panels[2].id;
    pousser(&mut bc, cle_de_plan(p2), 0.3);
    let doc = &bc.app.doc;
    crate::project::save(doc, &manifeste, &mut Default::default()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    assert_eq!(relu.doc.decors3d.envs.len(), 3);
    for i in 0..3 {
        let pid = doc.panels[i].id;
        assert_eq!(std::mem::discriminant(&relu.doc.decors3d.envs[&pid]), std::mem::discriminant(&doc.decors3d.envs[&pid]));
        assert_eq!(relu.doc.env_cle(i), doc.env_cle(i));
        assert_eq!(carres(&relu.doc, i), carres(doc, i), "le plan {i} relu montre la même chose");
    }
    assert_eq!(relu.doc.decors3d.par_sequence[&cle_de_plan(p2)].objets[0].place, doc.decors3d.par_sequence[&cle_de_plan(p2)].objets[0].place);
    // Le format des projets ne change pas ; un lecteur d'avant (la séquence exigée) lit les séquences et laisse de côté les
    // environnements des plans (il les garde tels quels) : il montre l'environnement des séquences.
    assert!(std::fs::read_to_string(&manifeste).unwrap().contains("\"version\": 61"));
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct SequenceAvant {
        sequence: u64,
        objets: Vec<serde_json::Value>,
    }
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap()).unwrap();
    let (mut lues, mut laissees) = (Vec::new(), 0);
    for e in v["decors"].as_array().unwrap() {
        match serde_json::from_value::<SequenceAvant>(e.clone()) {
            Ok(s) => lues.push(s.sequence),
            Err(_) => {
                assert!(e.get("plan").is_some() && e.get("sequence").is_none(), "seuls les environnements des plans : {e}");
                laissees += 1;
            }
        }
    }
    lues.sort();
    let mut attendues = vec![a, b];
    attendues.sort();
    assert_eq!((lues, laissees), (attendues, 3));
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------------------ 7. les caches, les plans restés

#[test]
fn the_decor_cache_key_follows_the_resolved_environment_and_plans_that_never_left_keep_their_pixels() {
    let (doc, a, b) = film();
    let avant = [exporter(&doc, 0), exporter(&doc, 2)];
    let cle = |doc: &Document, i: usize| {
        let item = decor3d::item(doc, i, 0, &doc.camera_seen(i, 0), None);
        item.and_then(|it| if let Some(crate::render::FondPart::Decor(k)) = it.fond { Some(k) } else { None })
    };
    let k0 = cle(&doc, 0).unwrap();
    let k1 = cle(&doc, 1).unwrap();
    assert_eq!(k0.decor, k1.decor, "les deux plans de A : le même décor");
    let mut bc = banc_sb(doc);
    assert!(bc.app.changer_env(1, ChoixEnv::Copie));
    let kc = cle(&bc.app.doc, 1).unwrap();
    assert_ne!(kc.decor, k0.decor, "une copie : un autre décor, une autre image gardée");
    assert_ne!(decor3d::id_of(&kc), decor3d::id_of(&k1));
    assert!(bc.app.changer_env(1, ChoixEnv::Lie(b)));
    let kl = cle(&bc.app.doc, 1).unwrap();
    assert_eq!(kl.decor, bc.app.doc.decors3d.par_sequence[&b].uid, "lié : le décor de B, partagé (un seul `Arc` des modèles)");
    assert!(Arc::ptr_eq(&bc.app.doc.decor_du_plan(1).unwrap().objets[0].modele, &bc.app.doc.decors3d.par_sequence[&b].objets[0].modele));
    // Les plans restés : les mêmes pixels qu'avant, au bit près.
    assert!(exporter(&bc.app.doc, 0) == avant[0] && exporter(&bc.app.doc, 2) == avant[1]);
    assert_eq!(cle(&bc.app.doc, 0).unwrap().decor, bc.app.doc.decors3d.par_sequence[&a].uid);
    // Rien de sorti : `env_cle` est la séquence, partout.
    let (doc, _, _) = film();
    assert!((0..3).all(|i| doc.env_cle(i) == doc.sequence_of(i)));
}

#[test]
fn a_right_click_on_a_plan_of_the_3d_timeline_opens_its_environment_choices() {
    let (doc, _a, _b) = film();
    let mut bc = banc_sb(doc);
    bc.app.select_panel(0);
    bc.app.set_espace(Espace::TroisD);
    bc.chauffer(8);
    let blocs = crate::onglet3dui::TIMELINE_SEQ.with(|t| t.borrow().clone()).expect("la timeline de la séquence").1;
    let r = blocs.iter().find(|(pi, _)| *pi == 1).expect("le plan 2").1;
    clic_droit(&mut bc, r.center());
    let t: Vec<String> = textes(&bc).into_iter().map(|(t, _)| t).collect();
    assert!(t.iter().any(|x| x == "Propre : vide") && t.iter().any(|x| x == "Environnement du plan 2"), "{t:?}");
    // Un choix s'y fait : le plan 2 a son environnement à lui, vide.
    let q = ou_est(&bc, "Propre : vide").unwrap();
    clic(&mut bc, q, Modifiers::NONE);
    assert!(matches!(bc.app.env_du_plan(1), Some(EnvPlan::Propre { .. })), "{}", bc.app.status);
}

// ------------------------------------------------------------------------------------------------ 9. la corbeille (partie 205)

/// Quatre plans dans A (0 à 3 ; le décor aux deux carrés), un dans B (4 ; le carré du milieu).
fn film_a_quatre() -> (Document, u64, u64) {
    let (mut doc, a, b) = film();
    doc.add_scene_after(1);
    doc.add_scene_after(2);
    assert!((0..4).all(|i| doc.sequence_of(i) == a) && doc.sequence_of(4) == b);
    (doc, a, b)
}

/// Enregistre `doc` dans `dir` et le rouvre dans un banc (le chemin du projet posé, comme une ouverture).
fn rouvrir(doc: &Document, dir: &std::path::Path) -> Banc {
    let manifeste = dir.join("Film.shot");
    crate::project::save(doc, &manifeste, &mut Default::default()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let mut b = banc_sb(relu.doc);
    b.app.path = Some(manifeste);
    b
}

/// Restaure l'entrée de la corbeille du plan `plan` et attend ses dessins ; rend l'index du plan revenu.
fn restaurer_plan(b: &mut Banc, plan: u64) -> usize {
    let e = b.app.doc.corbeille.entrees.iter().find(|e| e.plan == plan).expect("le plan dans la corbeille").id;
    b.app.corbeille_restaurer(e);
    let debut = std::time::Instant::now();
    while b.app.corbeille_en_cours() {
        assert!(debut.elapsed().as_secs() < 30, "la restauration ne se termine pas");
        std::thread::sleep(std::time::Duration::from_millis(5));
        b.app.corbeille_sonder();
    }
    b.chauffer(2);
    b.app.doc.panels.iter().position(|p| p.id == plan).unwrap_or_else(|| panic!("le plan {plan} est revenu : {}", b.app.status))
}

/// Les entrées « plan » de `decors3d.json` : (plan, lié à).
fn entrees_de_plans(dir: &std::path::Path) -> Vec<(u64, Option<u64>)> {
    let Ok(t) = std::fs::read_to_string(dir.join(decor3d::FILE)) else { return Vec::new() };
    let v: serde_json::Value = serde_json::from_str(&t).unwrap();
    v["decors"].as_array().unwrap().iter().filter_map(|e| e.get("plan").and_then(|p| p.as_u64()).map(|p| (p, e.get("lie").and_then(|l| l.as_u64())))).collect()
}

#[test]
fn a_plan_restored_from_the_trash_after_reopening_gets_back_its_own_empty_or_linked_environment() {
    let dir = crate::decor3d_tests::dossier("planenv205-retour");
    let (doc, _a, b) = film_a_quatre();
    let mut bc = banc_sb(doc);
    // Le plan 0 : une copie retouchée (un carré poussé, des clés, une lampe, le cyclo Nuit) ; le plan 1 : vide ; le plan 2 : lié à B.
    assert!(bc.app.changer_env(0, ChoixEnv::Copie));
    let (p0, p1, p2) = (bc.app.doc.panels[0].id, bc.app.doc.panels[1].id, bc.app.doc.panels[2].id);
    bc.app.select_panel(0);
    pousser(&mut bc, cle_de_plan(p0), 0.3);
    let mut d = bc.app.doc.decors3d.par_sequence[&cle_de_plan(p0)].clone();
    let pl = d.objets[1].place;
    d.objets[1].cles = vec![crate::miseenscene3d::CleObjet { image: 0, place: pl }, crate::miseenscene3d::CleObjet { image: 10, place: Place3d { position: [pl.position[0] + 0.2, pl.position[1], pl.position[2]], ..pl } }];
    let mut l = crate::miseenscene3d::Lampe3d::neuve(bc.app.doc.alloc(), "Lampe".into(), [0.0, 1.0, -2.0]);
    l.intensite = 1.5;
    d.lampes.push(l);
    bc.app.decor3d_retoucher(cle_de_plan(p0), d);
    bc.app.decor3d_retouche_finir();
    bc.app.changer_cyclo("Cyclo", Some(crate::cyclos::Preset::Nuit.cyclo()));
    assert!(bc.app.changer_env(1, ChoixEnv::Vide));
    assert!(bc.app.changer_env(2, ChoixEnv::Lie(b)));
    let avant = bc.app.doc.decors3d.par_sequence[&cle_de_plan(p0)].clone();
    let images: Vec<[bool; 3]> = (0..3).map(|i| carres(&bc.app.doc, i)).collect();
    // (Le plan 0 a un cyclo : son image est pleine.)
    assert_eq!(images[1..], [[false, false, false], [false, true, false]]);
    assert_eq!(images[0], [true, true, true]);
    // Supprimés (dans la corbeille), enregistrés : leurs environnements restent dans `decors3d.json`, au format 61.
    bc.app.delete_panels(&[p0, p1, p2]);
    bc.chauffer(2);
    assert_eq!(bc.app.doc.panels.len(), 2);
    assert_eq!(bc.app.doc.corbeille.len(), 3);
    let mut bc = rouvrir(&bc.app.doc, &dir);
    let mut e = entrees_de_plans(&dir);
    e.sort();
    assert_eq!(e, vec![(p0, None), (p1, None), (p2, Some(b))]);
    assert!(std::fs::read_to_string(dir.join("Film.shot")).unwrap().contains("\"version\": 61"));
    // Rouverts, rendus de la corbeille : chacun retrouve le sien.
    for (k, p) in [p0, p1, p2].into_iter().enumerate() {
        let i = restaurer_plan(&mut bc, p);
        assert_eq!(carres(&bc.app.doc, i), images[k], "le plan {k} revenu montre la même chose");
    }
    let d = |bc: &Banc, p: u64| bc.app.doc.decors3d.envs.get(&p).copied();
    assert!(matches!(d(&bc, p0), Some(EnvPlan::Propre { .. })) && matches!(d(&bc, p1), Some(EnvPlan::Propre { .. })) && d(&bc, p2) == Some(EnvPlan::Lie(b)));
    let apres = &bc.app.doc.decors3d.par_sequence[&cle_de_plan(p0)];
    assert_eq!(apres.objets.iter().map(|o| (o.id, o.place, o.cles.clone())).collect::<Vec<_>>(), avant.objets.iter().map(|o| (o.id, o.place, o.cles.clone())).collect::<Vec<_>>());
    assert_eq!(apres.lampes, avant.lampes);
    assert_eq!(apres.cyclo.map(|c| c.cle()), avant.cyclo.map(|c| c.cle()));
    assert!(bc.app.doc.decors3d.par_sequence.get(&cle_de_plan(p1)).is_none_or(|x| x.est_vide()), "le vide reste vide");
    let i2 = bc.app.doc.panels.iter().position(|p| p.id == p2).unwrap();
    assert_eq!(bc.app.doc.env_cle(i2), b, "le lien, en direct");
    // Les marques reviennent sur leurs blocs.
    bc.chauffer(3);
    let m = crate::planenv::MARQUES.with(|m| m.borrow().clone());
    assert!([p0, p1, p2].iter().all(|p| m.iter().any(|(q, _, _)| q == p)), "{m:?}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_trashed_plan_linked_to_a_sequence_deleted_meanwhile_comes_back_with_a_copy() {
    let dir = crate::decor3d_tests::dossier("planenv205-orphelin");
    let (doc, _a, b) = film_a_quatre();
    let mut bc = banc_sb(doc);
    assert!(bc.app.changer_env(1, ChoixEnv::Lie(b)));
    let p1 = bc.app.doc.panels[1].id;
    bc.app.delete_panels(&[p1]);
    bc.chauffer(2);
    let mut bc = rouvrir(&bc.app.doc, &dir);
    assert_eq!(bc.app.doc.decors3d.envs.get(&p1), Some(&EnvPlan::Lie(b)), "rouvert : le lien du plan de la corbeille");
    // On supprime B (son seul plan) : le plan lié est dans la corbeille, pas dans le film : pas de question ; son lien devient une
    // copie (la règle de la partie 204, `liens_orphelins_en_copies`).
    let pb = bc.app.doc.panels[(0..bc.app.doc.panels.len()).find(|&i| bc.app.doc.sequence_of(i) == b).unwrap()].id;
    bc.app.delete_panels(&[pb]);
    bc.chauffer(2);
    assert_eq!(bc.app.o3.suppression_liee, None, "pas de question pour un plan de la corbeille");
    assert!(!bc.app.doc.sequences.iter().any(|s| s.id == b));
    assert!(matches!(bc.app.doc.decors3d.envs.get(&p1), Some(EnvPlan::Propre { .. })));
    // Enregistré, rouvert, rendu : sa copie du carré du milieu.
    let dir2 = crate::decor3d_tests::dossier("planenv205-orphelin2");
    let mut bc = rouvrir(&bc.app.doc, &dir2);
    let i = restaurer_plan(&mut bc, p1);
    assert!(matches!(bc.app.doc.decors3d.envs.get(&p1), Some(EnvPlan::Propre { .. })));
    assert_eq!(bc.app.doc.decors3d.par_sequence[&cle_de_plan(p1)].objets.iter().map(|o| o.nom.as_str()).collect::<Vec<_>>(), vec!["Milieu"]);
    assert_eq!(carres(&bc.app.doc, i), [false, true, false]);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&dir2).ok();
}

#[test]
fn emptying_the_trash_leaves_no_plan_environment_behind() {
    let dir = crate::decor3d_tests::dossier("planenv205-vider");
    let (doc, _a, b) = film_a_quatre();
    let mut bc = banc_sb(doc);
    assert!(bc.app.changer_env(0, ChoixEnv::Copie));
    assert!(bc.app.changer_env(1, ChoixEnv::Lie(b)));
    bc.app.select_panel(0);
    bc.app.changer_cyclo("Cyclo", Some(crate::cyclos::Preset::Nuit.cyclo()));
    let (p0, p1) = (bc.app.doc.panels[0].id, bc.app.doc.panels[1].id);
    bc.app.delete_panels(&[p0, p1]);
    bc.chauffer(2);
    let mut bc = rouvrir(&bc.app.doc, &dir);
    assert_eq!(entrees_de_plans(&dir).len(), 2);
    // Vider la corbeille : leurs environnements partent (de la mémoire, puis du fichier) ; les décors des séquences restent.
    bc.app.corbeille_supprimer(None);
    assert!(bc.app.doc.decors3d.envs.is_empty());
    assert!(bc.app.doc.decors3d.par_sequence.keys().all(|&k| decor3d::plan_de_cle(k).is_none()));
    crate::project::save(&bc.app.doc, &dir.join("Film.shot"), &mut Default::default()).unwrap();
    assert!(entrees_de_plans(&dir).is_empty(), "rien laissé dans decors3d.json");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap()).unwrap();
    assert_eq!(v["decors"].as_array().unwrap().len(), 2, "les décors de A et de B");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------------------ 8. pour regarder

/// Recadre `rgba` (`w` de large) sur `r`, agrandi `k` fois (pixels nets).
fn recadrer(rgba: &[u8], w: u32, r: Rect, k: u32) -> (Vec<u8>, u32, u32) {
    let (x0, y0) = (r.left().max(0.0) as u32, r.top().max(0.0) as u32);
    let (cw, ch) = (r.width() as u32, r.height() as u32);
    let mut out = Vec::with_capacity((cw * ch * k * k * 4) as usize);
    for y in 0..ch * k {
        for x in 0..cw * k {
            let o = (((y0 + y / k) * w + x0 + x / k) * 4) as usize;
            out.extend_from_slice(&rgba[o..o + 4]);
        }
    }
    (out, cw * k, ch * k)
}

fn ecrire_png(dir: &str, nom: &str, rgba: &[u8], w: u32, h: u32) {
    image::save_buffer(format!("{dir}/{nom}"), rgba, w, h, image::ColorType::Rgba8).unwrap();
}

/// Le projet des captures : la séquence 1 (quatre plans) a un décor (un immeuble, un chêne, un banc) sous le cyclo Montagne ; la
/// séquence 2 (un plan), un feu de camp, une souche et un sapin, la nuit. Rend le banc (carte graphique, Storyboard).
fn banc_des_captures() -> Banc {
    let mut doc = Document::with_frame(1280, 720, 24);
    for i in 0..3 {
        doc.add_scene_after(i);
    }
    doc.add_sequence_after(3);
    let mut b = banc_taille(doc, true, 1600, 1000);
    b.app.settings.decor3d = Some(crate::decor3d_tests::budget_large());
    b.app.set_espace(Espace::Storyboard);
    b.garder_formes = true;
    b.chauffer(4);
    let rang = |nom: &str| crate::banque3d::modeles().iter().position(|m| m.nom == nom).unwrap_or_else(|| panic!("{nom} dans la banque"));
    let mut poser = |b: &mut Banc, plan: usize, nom: &str, xz: [f32; 2]| {
        b.app.select_panel(plan);
        b.app.decor3d_ajouter_banque(rang(nom), Some(xz));
        crate::decor3d_tests::attendre(b);
    };
    poser(&mut b, 0, "building-type-a", [-2.2, -7.0]);
    poser(&mut b, 0, "tree_oak", [1.6, -5.5]);
    poser(&mut b, 0, "detail-bench", [0.2, -4.0]);
    b.app.select_panel(0);
    b.app.changer_cyclo("Cyclo", Some(crate::cyclos::Preset::Montagne.cyclo()));
    poser(&mut b, 4, "campfire_logs", [0.0, -4.0]);
    poser(&mut b, 4, "stump_round", [-1.4, -4.6]);
    poser(&mut b, 4, "tree_default", [1.8, -6.5]);
    b.app.select_panel(4);
    b.app.changer_cyclo("Cyclo", Some(crate::cyclos::Preset::Nuit.cyclo()));
    // Les trois façons de sortir : le plan 2 vide, le plan 3 une copie retouchée (un camion de plus, le chêne retiré, le ciel de la
    // ville), le plan 4 lié à la séquence 2.
    let s2 = b.app.doc.sequence_of(4);
    assert!(b.app.changer_env(1, ChoixEnv::Vide));
    assert!(b.app.changer_env(2, ChoixEnv::Copie));
    assert!(b.app.changer_env(3, ChoixEnv::Lie(s2)));
    poser(&mut b, 2, "truck-green", [0.8, -5.0]);
    let cle = cle_de_plan(b.app.doc.panels[2].id);
    let mut d = b.app.doc.decors3d.par_sequence[&cle].clone();
    d.objets.retain(|o| o.nom != crate::banque3d::modeles()[rang("tree_oak")].nom_lisible());
    b.app.decor3d_retoucher(cle, d);
    b.app.decor3d_retouche_finir();
    b.app.select_panel(2);
    b.app.changer_cyclo("Cyclo", Some(crate::cyclos::Preset::Ville.cyclo()));
    crate::cyclos::attendre();
    decor3d::wait_idle();
    b.app.montage_fit = true;
    b.app.renderer.invalidate_all();
    b.chauffer(10);
    b.attendre_vignettes();
    decor3d::wait_idle();
    b.chauffer(6);
    b
}

/// Le point de `t` à l'écran (le premier texte qui le contient).
fn ou_est(b: &Banc, t: &str) -> Option<eframe::egui::Pos2> {
    textes(b).into_iter().find(|(x, _)| x.contains(t)).map(|(_, r)| r.center())
}

fn survoler(b: &mut Banc, p: eframe::egui::Pos2) {
    for _ in 0..6 {
        b.image(vec![eframe::egui::Event::PointerMoved(p)], 1.0 / 60.0);
    }
    b.chauffer(4);
}

fn clic_droit(b: &mut Banc, p: eframe::egui::Pos2) {
    b.image(vec![eframe::egui::Event::PointerMoved(p)], 1.0 / 60.0);
    for pressed in [true, false] {
        b.image(vec![eframe::egui::Event::PointerButton { pos: p, button: eframe::egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    }
    b.chauffer(4);
}

/// (Pour regarder : `FEANOR_PLANENV_PNG=<dossier>` écrit les écrans de la partie 204 : la bande du Storyboard et ses marques, le
/// menu du clic droit d'un plan, l'onglet 3D sur un plan à l'environnement propre (Scène 3D, Propriétés > Décor), la timeline de la
/// séquence, et les images des plans.)
#[test]
fn ecrire_les_ecrans_de_l_environnement_du_plan_en_png() {
    let Ok(dir) = std::env::var("FEANOR_PLANENV_PNG") else { return };
    if crate::gpu_tests::headless().is_none() {
        return;
    }
    std::fs::create_dir_all(&dir).unwrap();
    let mut b = banc_des_captures();
    let (w, h) = (b.taille.x as u32, b.taille.y as u32);
    // a) La bande du Storyboard : les quatre états ; et les blocs, agrandis.
    b.app.select_panel(0);
    b.chauffer(6);
    let ecran = b.capturer().unwrap();
    ecrire_png(&dir, "a-storyboard.png", &ecran, w, h);
    let blocs = crate::storyboard::BLOCS_DU_MONTAGE.with(|x| x.borrow().clone());
    assert_eq!(blocs.len(), 5);
    let tout = blocs.iter().fold(blocs[0], |r, x| r.union(*x)).expand(6.0).intersect(Rect::from_min_size(eframe::egui::Pos2::ZERO, b.taille));
    let (z, zw, zh) = recadrer(&ecran, w, tout, 2);
    ecrire_png(&dir, "a-blocs-x2.png", &z, zw, zh);
    let marques = crate::planenv::MARQUES.with(|m| m.borrow().clone());
    assert_eq!(marques.len(), 3, "{marques:?}");
    for (k, (_, zone, _)) in marques.iter().enumerate() {
        let r = Rect::from_center_size(zone.center() - eframe::egui::vec2(30.0, 0.0), eframe::egui::vec2(96.0, 32.0));
        let (z, zw, zh) = recadrer(&ecran, w, r, 6);
        ecrire_png(&dir, &format!("a-marque-{}-x6.png", k + 1), &z, zw, zh);
    }
    // La bulle de la marque du plan lié.
    survoler(&mut b, marques[2].1.center());
    b.chauffer(30);
    ecrire_png(&dir, "a-bulle-du-lien.png", &b.capturer().unwrap(), w, h);
    // b) Le clic droit sur le bloc du plan 3 (la copie), « Environnement du plan » ouvert.
    let p = blocs[2].center();
    clic_droit(&mut b, p);
    ecrire_png(&dir, "b-menu-du-plan.png", &b.capturer().unwrap(), w, h);
    if let Some(q) = ou_est(&b, "Environnement du plan") {
        survoler(&mut b, q);
    }
    ecrire_png(&dir, "b-menu-environnement.png", &b.capturer().unwrap(), w, h);
    if let Some(q) = ou_est(&b, "Transitions du plan") {
        survoler(&mut b, q);
        ecrire_png(&dir, "b-menu-transitions.png", &b.capturer().unwrap(), w, h);
    }
    b.image(vec![eframe::egui::Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(4);
    // c) L'onglet 3D sur le plan 3 (environnement propre) : la tête de Scène 3D, Propriétés > Décor.
    b.app.select_panel(2);
    b.app.set_espace(Espace::TroisD);
    b.app.colonne.montrer(crate::dock::Pane::Decor3d);
    b.chauffer(10);
    decor3d::wait_idle();
    crate::cyclos::attendre();
    b.chauffer(8);
    let ecran = b.capturer().unwrap();
    ecrire_png(&dir, "c-onglet-3d-plan-propre.png", &ecran, w, h);
    // d) La timeline de la séquence et ses marques, agrandie.
    let (piste, _) = crate::onglet3dui::TIMELINE_SEQ.with(|t| t.borrow().clone()).expect("la timeline de la séquence");
    let (z, zw, zh) = recadrer(&ecran, w, piste.expand(4.0).intersect(Rect::from_min_size(eframe::egui::Pos2::ZERO, b.taille)), 2);
    ecrire_png(&dir, "d-timeline-sequence-x2.png", &z, zw, zh);
    // Le clic droit d'un plan dans la timeline de la séquence.
    let blocs3d = crate::onglet3dui::TIMELINE_SEQ.with(|t| t.borrow().clone()).unwrap().1;
    clic_droit(&mut b, blocs3d.iter().find(|(pi, _)| *pi == 3).map(|(_, r)| r.center()).expect("le plan 4"));
    if let Some(q) = ou_est(&b, "Environnement du plan").filter(|_| ou_est(&b, "Propre : vide").is_none()) {
        survoler(&mut b, q);
    }
    ecrire_png(&dir, "d-menu-timeline-sequence.png", &b.capturer().unwrap(), w, h);
    b.image(vec![eframe::egui::Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(4);
    // e) Les images des cinq plans, côte à côte (à moitié), sur du blanc : le plan resté, le vide, la copie retouchée, le lié, et la
    // séquence 2.
    let doc = &b.app.doc;
    let [fw, fh] = doc.frame;
    let (pw, ph_) = (fw / 2, fh / 2);
    let mut planche = vec![255u8; 5 * pw * ph_ * 4];
    for p in 0..5 {
        let img = exporter(doc, p);
        for y in 0..ph_ {
            for x in 0..pw {
                let c = img[(y * 2) * fw + x * 2];
                let a = c.a() as u32;
                let o = (y * 5 * pw + p * pw + x) * 4;
                for (k, v) in [c.r(), c.g(), c.b()].into_iter().enumerate() {
                    planche[o + k] = (v as u32 + (255 * (255 - a)) / 255).min(255) as u8;
                }
            }
        }
    }
    // (Un filet gris entre les images : le plan vide est blanc.)
    for p in 1..5 {
        for y in 0..ph_ {
            for x in p * pw - 1..=p * pw {
                let o = (y * 5 * pw + x) * 4;
                planche[o..o + 3].copy_from_slice(&[90, 90, 90]);
            }
        }
    }
    ecrire_png(&dir, "e-images-des-plans.png", &planche, (5 * pw) as u32, ph_ as u32);
}
