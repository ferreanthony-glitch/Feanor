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
