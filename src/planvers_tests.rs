//! Versions d'un plan (`planvers.rs`, `planversui.rs`) : le modèle (nouvelle version, passer, restaurer,
//! supprimer), le fichier et ses protections, l'annulation, le storyboard verrouillé.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui::Color32;

use crate::document::{Cel, CelId, Document, Layer, LayerKind, Panel};
use crate::empreinte::{self, Donnees};
use crate::fiabilite_fixture::{arbre, dossier};
use crate::perf_tests::{banc, Banc};
use crate::planvers::{etiquette, MAX_VERSIONS};
use crate::project;

fn rouge(r: u8) -> Color32 {
    Color32::from_rgba_premultiplied(r, 10, 10, 255)
}

/// Un film de `n` plans de 4 images : un calque bitmap, deux dessins par plan.
fn film(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    for k in 1..n {
        doc.add_scene_after(k - 1);
    }
    for i in 0..n {
        let mut layer = Layer::new("Calque", LayerKind::Bitmap);
        let mut ids = Vec::new();
        for d in 0..2 {
            let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
            cel.raster.tile_mut(d)[0] = rouge((20 + i * 7 + d * 40) as u8);
            ids.push(doc.add_cel(cel));
        }
        layer.frames = vec![Some(ids[0]), Some(ids[0]), Some(ids[1]), Some(ids[1])];
        doc.panels[i].layers = vec![layer];
        doc.panels[i].length = 4;
    }
    doc
}

/// Ce qu'est un plan, pixels compris, sans les numéros de dessins : deux plans qui se ressemblent ont la même.
fn contenu(doc: &Document, p: &Panel) -> u64 {
    let cle = |id: CelId| doc.cels.get(&id).map_or(id, |c| Donnees::de(c).empreinte());
    empreinte::de_octets(&project::digest_bytes(p, &cle))
}

fn contenu_actif(doc: &Document, plan: u64) -> u64 {
    contenu(doc, doc.panels.iter().find(|p| p.id == plan).expect("plan"))
}

fn contenu_range(doc: &Document, plan: u64, numero: u32) -> u64 {
    contenu(doc, &doc.versions_du_plan(plan).and_then(|v| v.rangee(numero)).expect("version rangée").panel)
}

/// Retouche le premier dessin du plan `i` : un pixel de plus.
fn retoucher(doc: &mut Document, i: usize, r: u8) -> CelId {
    let id = doc.panels[i].layers[0].frames[0].unwrap();
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.raster.tile_mut(7)[3] = rouge(r);
    cel.version += 1;
    id
}

fn manifeste(dir: &Path) -> PathBuf {
    dir.join("Film").join("Film.pelure")
}

fn enregistrer(doc: &Document, manifeste: &Path) {
    project::prepare_save(doc, manifeste, &mut HashMap::new(), None).expect("préparation").run().expect("écriture");
}

// ------------------------------------------------------------------------------------ le modèle

#[test]
fn a_new_version_keeps_the_plan_as_it_is_and_continues_on_a_copy() {
    let mut doc = film(2);
    let plan = doc.panels[0].id;
    let avant = contenu_actif(&doc, plan);
    let cels_avant = doc.cels.len();
    assert_eq!((doc.numero_de_version(plan), doc.nombre_de_versions(plan)), (1, 1));
    assert_eq!(doc.nouvelle_version(0, 1_760_000_000), Some(2));
    assert_eq!((doc.numero_de_version(plan), doc.nombre_de_versions(plan)), (2, 2));
    let v = doc.versions_du_plan(plan).unwrap();
    assert_eq!((v.actuelle, v.prochaine, v.autres.len()), (2, 3, 1));
    assert_eq!(v.autres[0].numero, 1);
    // La version rangée a le contenu d'avant ; l'active aussi, pour l'instant.
    assert_eq!(contenu_range(&doc, plan, 1), avant);
    assert_eq!(contenu_actif(&doc, plan), avant);
    // Elles ont chacune leurs dessins : retoucher l'active ne touche pas l'autre.
    let rangee_ids: Vec<CelId> = doc.versions_du_plan(plan).unwrap().autres[0].panel.layers[0].frames.iter().flatten().copied().collect();
    let active_ids: Vec<CelId> = doc.panels[0].layers[0].frames.iter().flatten().copied().collect();
    assert!(rangee_ids.iter().all(|id| !active_ids.contains(id)), "des dessins à chacune");
    assert_eq!(doc.cels.len(), cels_avant + 2);
    retoucher(&mut doc, 0, 200);
    assert_ne!(contenu_actif(&doc, plan), avant);
    assert_eq!(contenu_range(&doc, plan, 1), avant, "la version rangée n'a pas bougé");
    // Le numéro du plan, sa scène et sa place dans le film sont ceux du plan ; un autre plan n'a rien.
    assert_eq!(doc.panels[0].id, plan);
    assert_eq!(doc.nombre_de_versions(doc.panels[1].id), 1);
    // Et la version rangée a ses propres numéros (jamais deux plans du même numéro).
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).chain(doc.plans_ranges().map(|p| p.id)).collect();
    let mut triees = ids.clone();
    triees.sort_unstable();
    triees.dedup();
    assert_eq!(triees.len(), ids.len());
}

#[test]
fn switching_exchanges_the_contents_and_switching_back_restores_them() {
    let mut doc = film(1);
    let plan = doc.panels[0].id;
    let v1 = contenu_actif(&doc, plan);
    doc.nouvelle_version(0, 10);
    retoucher(&mut doc, 0, 222);
    doc.panels[0].action = "Elle ouvre la porte.".into();
    let v2 = contenu_actif(&doc, plan);
    assert_ne!(v1, v2);
    assert!(doc.changer_de_version(0, 1));
    assert_eq!(doc.numero_de_version(plan), 1);
    assert_eq!(contenu_actif(&doc, plan), v1);
    assert!(doc.panels[0].action.is_empty());
    assert_eq!(contenu_range(&doc, plan, 2), v2);
    assert!(doc.changer_de_version(0, 2));
    assert_eq!((doc.numero_de_version(plan), contenu_actif(&doc, plan)), (2, v2));
    assert_eq!(contenu_range(&doc, plan, 1), v1);
    // Un numéro qu'on n'a pas, ou l'actif : rien ne se passe.
    assert!(!doc.changer_de_version(0, 9) && !doc.changer_de_version(0, 2));
    assert_eq!((doc.numero_de_version(plan), contenu_actif(&doc, plan)), (2, v2));
}

#[test]
fn restoring_makes_a_copy_of_an_old_version_the_active_one_and_loses_nothing() {
    let mut doc = film(1);
    let plan = doc.panels[0].id;
    let v1 = contenu_actif(&doc, plan);
    doc.nouvelle_version(0, 10);
    retoucher(&mut doc, 0, 99);
    let v2 = contenu_actif(&doc, plan);
    assert_eq!(doc.restaurer_version(0, 1, 20), Some(3));
    assert_eq!((doc.numero_de_version(plan), doc.nombre_de_versions(plan)), (3, 3));
    assert_eq!(contenu_actif(&doc, plan), v1, "l'active est la copie de v1");
    assert_eq!(contenu_range(&doc, plan, 1), v1, "v1 est toujours là");
    assert_eq!(contenu_range(&doc, plan, 2), v2, "et v2, l'ancienne active");
    // La copie est indépendante de v1.
    retoucher(&mut doc, 0, 140);
    assert_eq!(contenu_range(&doc, plan, 1), v1);
    assert_eq!(doc.restaurer_version(0, 9, 20), None);
}

#[test]
fn deleting_a_version_never_deletes_the_active_one() {
    let mut doc = film(1);
    let plan = doc.panels[0].id;
    doc.nouvelle_version(0, 10);
    doc.nouvelle_version(0, 11);
    assert_eq!(doc.nombre_de_versions(plan), 3);
    assert!(!doc.supprimer_version(plan, 3), "l'active ne se supprime pas");
    assert!(doc.supprimer_version(plan, 1));
    assert_eq!(doc.versions_du_plan(plan).unwrap().numeros(), vec![2, 3]);
    assert!(doc.supprimer_version(plan, 2));
    assert!(doc.versions_du_plan(plan).is_none(), "plus qu'une version : le plan n'a plus de versions");
    assert_eq!(doc.numero_de_version(plan), 1);
    assert!(!doc.supprimer_version(plan, 2));
}

#[test]
fn the_number_of_versions_is_bounded_and_the_neighbours_turn() {
    let mut doc = film(1);
    let plan = doc.panels[0].id;
    for _ in 1..MAX_VERSIONS {
        assert!(doc.nouvelle_version(0, 1).is_some());
    }
    assert_eq!(doc.nombre_de_versions(plan), MAX_VERSIONS);
    assert_eq!(doc.nouvelle_version(0, 1), None, "pas plus de {MAX_VERSIONS} versions");
    assert_eq!(doc.version_voisine(plan, 1), Some(1), "après la dernière, la première");
    assert_eq!(doc.version_voisine(plan, -1), Some(MAX_VERSIONS as u32 - 1));
    assert_eq!(etiquette(12), "V12");
}

#[test]
fn the_cels_of_stored_versions_are_used_cels_and_a_deleted_plan_loses_its_versions() {
    let mut doc = film(2);
    let plan = doc.panels[1].id;
    doc.nouvelle_version(1, 1);
    let rangees: Vec<CelId> = doc.versions_du_plan(plan).unwrap().autres[0].panel.layers[0].frames.iter().flatten().copied().collect();
    assert!(!rangees.is_empty());
    let utilises = doc.used_cels();
    assert!(rangees.iter().all(|id| utilises.contains(id)), "les dessins d'une version rangée s'enregistrent");
    // Le plan disparaît (l'annulation le ramène avec ses versions, `Structure`) : ses versions n'ont plus de dessins utilisés.
    doc.remove_panels(&[plan]);
    assert_eq!(doc.versions_vivantes().count(), 0);
    assert!(rangees.iter().all(|id| !doc.used_cels().contains(id)));
}

// ------------------------------------------------------------------------------------ le fichier

#[test]
fn versions_survive_save_and_reopen_with_their_drawings() {
    let travail = dossier("versions-fichier");
    let m = manifeste(&travail);
    let mut doc = film(3);
    let plan = doc.panels[1].id;
    doc.nouvelle_version(1, 1_760_000_000);
    retoucher(&mut doc, 1, 150);
    doc.panels[1].dialogue = "— Tu viens ?".into();
    doc.nouvelle_version(1, 1_760_000_500);
    doc.panels[1].action = "Il hésite.".into();
    let (actif, v1, v2) = (contenu_actif(&doc, plan), contenu_range(&doc, plan, 1), contenu_range(&doc, plan, 2));
    enregistrer(&doc, &m);
    assert!(m.parent().unwrap().join("plans_versions.json").exists());
    for (nom, ouvert) in [("lecture complète", project::load(&m).expect("lecture")), ("lecture rapide", project::load_quick(&m).expect("lecture rapide"))] {
        assert!(ouvert.warnings.is_empty(), "{nom} : {:?}", ouvert.warnings);
        let d = &ouvert.doc;
        assert_eq!(d.panels.len(), 3, "{nom} : les versions rangées ne sont pas des plans du film");
        assert_eq!(d.numero_de_version(plan), 3, "{nom}");
        assert_eq!(d.nombre_de_versions(plan), 3, "{nom}");
        assert_eq!((contenu_actif(d, plan), contenu_range(d, plan, 1), contenu_range(d, plan, 2)), (actif, v1, v2), "{nom}");
        let v = d.versions_du_plan(plan).unwrap();
        assert_eq!((v.prochaine, v.autres.iter().map(|r| r.date).collect::<Vec<_>>()), (4, vec![0, 1_760_000_000]), "{nom}");
        assert_eq!(d.nombre_de_versions(d.panels[0].id), 1, "{nom}");
        // Les plans rangés ne se confondent avec aucun autre.
        let ids: Vec<u64> = d.panels.iter().map(|p| p.id).chain(d.plans_ranges().map(|p| p.id)).collect();
        let mut tries = ids.clone();
        tries.sort_unstable();
        tries.dedup();
        assert_eq!(tries.len(), ids.len(), "{nom}");
        // Le film est toujours le même (mêmes numéros de plans, même durée).
        assert_eq!(d.total_frames(), doc.total_frames(), "{nom}");
    }
}

#[test]
fn a_project_without_versions_is_exactly_as_before() {
    let travail = dossier("versions-sans");
    let m = manifeste(&travail);
    enregistrer(&film(3), &m);
    let fichiers: Vec<String> = arbre(m.parent().unwrap()).into_keys().collect();
    assert!(!fichiers.iter().any(|f| f.contains("versions")), "{fichiers:?}");
    let ouvert = project::load(&m).expect("ouverture");
    assert!(ouvert.warnings.is_empty());
    assert_eq!(ouvert.doc.versions_vivantes().count(), 0);
    assert!(project::VERSION >= 27, "les versions de plans ne changent pas le format du projet (la 30 est celle du calque de couleur et de la toile infinie)");
}

#[test]
fn deleting_the_last_other_version_removes_the_file_and_its_drawings() {
    let travail = dossier("versions-retrait");
    let m = manifeste(&travail);
    let dir = m.parent().unwrap().to_path_buf();
    let mut doc = film(1);
    let plan = doc.panels[0].id;
    doc.nouvelle_version(0, 1);
    enregistrer(&doc, &m);
    assert!(dir.join("plans_versions.json").exists());
    let dessins = |d: &Path| std::fs::read_dir(d.join("dessins")).unwrap().count();
    assert_eq!(dessins(&dir), 4, "deux dessins pour l'active, deux pour la version rangée");
    assert!(doc.supprimer_version(plan, 1));
    enregistrer(&doc, &m);
    assert!(!dir.join("plans_versions.json").exists());
    assert_eq!(dessins(&dir), 2, "les dessins de la version supprimée partent à l'enregistrement");
}

/// Un projet dont `plans_versions.json` contient `contenu` : il s'ouvre sans versions, l'enregistrer n'y touche
/// pas et n'efface aucun dessin, et créer une version met l'ancien fichier de côté.
fn fichier_illisible(nom: &str, contenu: &str) {
    let travail = dossier(&format!("versions-illisible-{nom}"));
    let m = manifeste(&travail);
    let dir = m.parent().unwrap().to_path_buf();
    let mut base = film(2);
    base.nouvelle_version(0, 1);
    enregistrer(&base, &m);
    let dessins_avant: Vec<String> = arbre(&dir).into_keys().filter(|f| f.starts_with("dessins/")).collect();
    assert_eq!(dessins_avant.len(), 6);
    std::fs::write(dir.join("plans_versions.json"), contenu).unwrap();
    let ouvert = project::load(&m).expect("le projet s'ouvre quand même");
    assert!(ouvert.warnings.iter().any(|w| w.contains("plans_versions.json") && w.contains("ne sont pas chargées")), "{nom} : {:?}", ouvert.warnings);
    assert_eq!(ouvert.doc.versions_vivantes().count(), 0);
    // Enregistrer sans version : le fichier reste, et les dessins qu'il décrit aussi.
    project::prepare_save(&ouvert.doc, &m, &mut ouvert.written.clone(), None).unwrap().run().unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("plans_versions.json")).unwrap(), contenu, "{nom} : le fichier est intact");
    let apres: Vec<String> = arbre(&dir).into_keys().filter(|f| f.starts_with("dessins/")).collect();
    assert_eq!(apres, dessins_avant, "{nom} : aucun dessin effacé");
    // Une version neuve : l'ancien fichier est mis de côté, jamais perdu.
    let mut neuf = ouvert.doc;
    neuf.nouvelle_version(0, 2);
    project::prepare_save(&neuf, &m, &mut ouvert.written.clone(), None).unwrap().run().unwrap();
    let aside: Vec<String> = arbre(&dir).into_keys().filter(|f| f.starts_with("plans_versions.json.illisible")).collect();
    assert!(!aside.is_empty(), "{nom} : mis de côté");
    assert!(aside.iter().any(|f| std::fs::read_to_string(dir.join(f)).unwrap() == contenu));
}

#[test]
fn a_damaged_versions_file_is_never_deleted() {
    fichier_illisible("abime", "{\"format\":\"shot-plans-versions\",\"version\":1,\"plans\":[{\"plan\":");
    fichier_illisible("octets", "\u{1}\u{2} pas du texte {{{");
}

#[test]
fn a_newer_versions_file_is_never_deleted() {
    fichier_illisible("recent", "{\"format\":\"shot-plans-versions\",\"version\":42,\"plans\":[]}");
}

#[test]
fn a_hand_damaged_versions_file_is_cleaned_not_trusted() {
    use crate::planvers::parse;
    let texte = r#"{"format":"shot-plans-versions","version":1,"plans":[
        {"plan":5,"actuelle":0,"prochaine":0,"autres":[{"numero":1,"entree":{}},{"numero":1,"entree":{}},{"numero":0,"entree":{}},{"numero":2,"entree":{}}]},
        {"plan":5,"actuelle":3,"prochaine":4,"autres":[]}]}"#;
    let f = parse(texte).expect("lisible");
    assert_eq!(f.plans.len(), 1, "un plan en double n'est gardé qu'une fois");
    let p = &f.plans[0];
    assert_eq!(p.actuelle, 1, "au moins 1");
    assert_eq!(p.autres.iter().map(|v| v.numero).collect::<Vec<_>>(), vec![2], "numéro nul, en double ou égal à l'actif : oubliés");
    assert!(p.prochaine > 2);
    assert!(parse("{\"format\":\"autre\",\"version\":1}").err().unwrap().contains("pas un fichier de versions"));
    assert!(parse("[").err().unwrap().contains("illisible"));
}

// ------------------------------------------------------------------------------------ dans l'application

fn banc_projet(n: usize) -> Banc {
    let mut b = banc(film(n), false);
    b.app.modified = false;
    b
}

#[test]
fn each_version_gesture_is_one_named_undo_step_and_undo_brings_everything_back() {
    let mut b = banc_projet(2);
    let plan = b.app.doc.panels[0].id;
    let v1 = contenu_actif(&b.app.doc, plan);
    let cels = b.app.doc.cels.len();
    let (undo0, _) = b.app.history.names();
    let n0 = undo0.len();
    b.app.run_action("version.nouvelle");
    assert_eq!(b.app.doc.numero_de_version(plan), 2);
    // (Une retouche du texte du plan : elle est dans la structure, donc dans les étapes d'annulation.)
    b.app.doc.panels[0].action = "Elle ouvre la porte.".into();
    let v2 = contenu_actif(&b.app.doc, plan);
    b.app.passer_a_la_version(0, 1);
    assert_eq!((b.app.doc.numero_de_version(plan), contenu_actif(&b.app.doc, plan)), (1, v1));
    b.app.restaurer_la_version(0, 2);
    assert_eq!((b.app.doc.numero_de_version(plan), contenu_actif(&b.app.doc, plan)), (3, v2));
    b.app.supprimer_la_version(0, 1);
    assert_eq!(b.app.doc.versions_du_plan(plan).unwrap().numeros(), vec![2, 3]);
    // Quatre gestes, quatre étapes, leurs noms dans l'historique.
    let (undo, _) = b.app.history.names();
    assert_eq!(undo.len(), n0 + 4, "{undo:?}");
    assert_eq!(&undo[n0..], ["Nouvelle version du plan", "Passer à la version V1", "Restaurer la version V2", "Supprimer la version V1"]);
    // Défaire une étape à la fois.
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.versions_du_plan(plan).unwrap().numeros(), vec![1, 2, 3]);
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.doc.numero_de_version(plan), contenu_actif(&b.app.doc, plan)), (1, v1));
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.doc.numero_de_version(plan), contenu_actif(&b.app.doc, plan), contenu_range(&b.app.doc, plan, 1)), (2, v2, v1));
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.numero_de_version(plan), 1);
    b.app.run_action("edition.annuler");
    // Jusqu'à la nouvelle version : défaite aussi, le plan est comme au début.
    b.app.run_action("edition.annuler");
    assert!(b.app.doc.versions_du_plan(plan).is_none());
    assert_eq!(b.app.doc.numero_de_version(plan), 1);
    assert_eq!(contenu_actif(&b.app.doc, plan), v1);
    assert!(b.app.doc.cels.len() >= cels, "les dessins ne se perdent pas en session");
}

#[test]
fn the_neighbour_commands_turn_through_the_versions_and_say_when_there_is_only_one() {
    let mut b = banc_projet(1);
    b.app.run_action("version.suivante");
    assert!(b.app.status.contains("qu'une version"), "{}", b.app.status);
    let plan = b.app.doc.panels[0].id;
    b.app.run_action("version.nouvelle");
    b.app.run_action("version.nouvelle");
    assert_eq!(b.app.doc.numero_de_version(plan), 3);
    b.app.run_action("version.suivante");
    assert_eq!(b.app.doc.numero_de_version(plan), 1, "après la dernière, la première");
    b.app.run_action("version.precedente");
    assert_eq!(b.app.doc.numero_de_version(plan), 3);
}

#[test]
fn a_locked_storyboard_refuses_every_version_gesture() {
    let mut b = banc_projet(1);
    b.app.doc.storyboard_locked = true;
    let plan = b.app.doc.panels[0].id;
    b.app.run_action("version.nouvelle");
    assert_eq!(b.app.doc.nombre_de_versions(plan), 1);
    b.app.doc.storyboard_locked = false;
    b.app.run_action("version.nouvelle");
    b.app.doc.storyboard_locked = true;
    b.app.passer_a_la_version(0, 1);
    b.app.supprimer_la_version(0, 1);
    b.app.restaurer_la_version(0, 1);
    assert_eq!((b.app.doc.numero_de_version(plan), b.app.doc.nombre_de_versions(plan)), (2, 2));
}

#[test]
fn the_envoi_remembers_which_version_of_the_plan_went_out() {
    let mut b = banc_projet(2);
    let plan = b.app.doc.panels[0].id;
    b.app.run_action("version.nouvelle");
    b.app.run_action("version.nouvelle");
    let ctx = b.ctx.clone();
    b.app.envois_marquer(&ctx, "v1 au réal", 1_760_000_000);
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 0.25);
        if b.app.envois_ui.au_repos() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let e = b.app.doc.envois.list.first().expect("l'envoi");
    assert_eq!(e.plan(plan).map(|p| p.version), Some(3));
    assert_eq!(e.plan(b.app.doc.panels[1].id).map(|p| p.version), Some(1));
}

#[test]
fn the_version_commands_are_in_the_palette_the_menus_and_the_shortcut_file() {
    for id in ["version.nouvelle", "version.precedente", "version.suivante"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
    assert!(crate::barres::Barre::Montage.cherche("version.nouvelle").is_some());
}
