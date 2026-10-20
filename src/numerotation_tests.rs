//! Numérotation des plans (`numerotation.rs`) : suivie ou verrouillée, insertion, suppression,
//! déplacement, copie, renuméroter et son annulation, enregistrement, anciens projets.

use eframe::egui;

use crate::app::App;
use crate::document::Document;
use crate::numerotation::Num;

fn num(t: &str) -> Num {
    Num::parse(t).unwrap_or_else(|| panic!("numéro illisible : {t}"))
}

/// Un film de `n` plans dans une séquence.
fn film(n: usize) -> Document {
    let mut doc = Document::new(64, 36, 24);
    for _ in 1..n {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    doc
}

fn codes(doc: &Document) -> Vec<String> {
    doc.scene_order().iter().map(|&s| doc.plan_code(s)).collect()
}

fn v(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn id_of(doc: &Document, code: &str) -> u64 {
    doc.panels.iter().find(|p| doc.plan_code(p.scene) == code).unwrap_or_else(|| panic!("pas de plan {code}")).id
}

fn index_of(doc: &Document, code: &str) -> usize {
    doc.panels.iter().position(|p| doc.plan_code(p.scene) == code).unwrap_or_else(|| panic!("pas de plan {code}"))
}

// ------------------------------------------------------------------------------------ les numéros

#[test]
fn a_number_reads_and_writes_the_same_text() {
    for t in ["1", "12", "12A", "12AA", "12A1", "12.5", "0", "-1", "7Z", "7ZA"] {
        assert_eq!(num(t).text(), t, "{t}");
    }
    assert_eq!(num("12a").text(), "12A", "les minuscules sont lues comme des majuscules");
    assert_eq!(num(" 12B ").text(), "12B");
    assert_eq!(num("12").padded(3), "012");
    assert_eq!(num("12A").padded(3), "012A");
    assert_eq!(num("12.5").padded(3), "012.5");
    for bad in ["", "A", "A12", "12-", "12A0", "12.", "12.A", "1 2", "12é"] {
        assert!(Num::parse(bad).is_none(), "« {bad} » n'est pas un numéro");
    }
}

#[test]
fn numbers_are_ordered_by_base_then_tail() {
    let order = ["0", "1", "2", "12", "12.5", "12A", "12A1", "12AA", "12B", "12Z", "12ZA", "13", "100"];
    for w in order.windows(2) {
        assert!(num(w[0]) < num(w[1]), "{} < {}", w[0], w[1]);
    }
    assert!(num("-1") < num("0"));
}

#[test]
fn the_insertion_rule_gives_the_documented_numbers() {
    let between = |p: Option<&str>, n: Option<&str>| Num::between(p.map(num).as_ref(), n.map(num).as_ref()).text();
    assert_eq!(between(Some("12"), Some("13")), "12A", "entre le 12 et le 13 : 12A");
    assert_eq!(between(Some("12A"), Some("13")), "12B");
    assert_eq!(between(Some("12Z"), Some("13")), "12ZA", "après Z : on prolonge");
    assert_eq!(between(Some("12"), Some("14")), "12A", "le 13 supprimé n'est pas redonné");
    assert_eq!(between(Some("12A"), Some("12B")), "12A1", "entre deux lettres qui se touchent : un chiffre");
    assert_eq!(between(Some("12A1"), Some("12B")), "12A2");
    assert_eq!(between(Some("12A"), Some("12A1")), "12A01");
    assert_eq!(between(Some("12"), Some("12A")), "12.5", "entre 12 et 12A : un point");
    assert_eq!(between(Some("12"), Some("12.5")), "12.2", "(la subdivision continue)");
    assert_eq!(between(Some("12"), None), "13", "à la fin du film : l'entier suivant");
    assert_eq!(between(None, Some("1")), "0", "avant le premier");
    assert_eq!(between(None, Some("0")), "-1");
    assert_eq!(between(None, Some("5A")), "5", "avant un 5A sans 5 : le 5");
    assert_eq!(between(Some("4"), Some("1")), "4A", "numéros dans le désordre (plan déplacé) : juste après le précédent");
    assert_eq!(between(None, None), "1");
}

#[test]
fn many_random_insertions_stay_ordered_and_unique() {
    // Un petit générateur déterministe.
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut rnd = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let mut list: Vec<Num> = (1..=6).map(Num::new).collect();
    for step in 0..1500 {
        let at = rnd(list.len() + 1);
        let mut cand = Num::between(at.checked_sub(1).map(|i| &list[i]), list.get(at));
        let mut guard = 0;
        while list.contains(&cand) {
            cand = cand.next_in_base();
            guard += 1;
            assert!(guard < 100);
        }
        list.insert(at, cand);
        for w in list.windows(2) {
            assert!(w[0] < w[1], "pas dans l'ordre après {step} insertions : {} puis {}", w[0].text(), w[1].text());
        }
    }
    for n in &list {
        assert_eq!(Num::parse(&n.text()).as_ref(), Some(n), "{} se relit", n.text());
        assert!(!n.tail.ends_with('0'));
    }
}

// ------------------------------------------------------------------------------------ numérotation suivie

#[test]
fn followed_numbering_renumbers_by_position() {
    let mut doc = film(3);
    assert!(!doc.numbering_locked);
    assert_eq!(codes(&doc), v(&["1", "2", "3"]));
    doc.add_scene_after(0);
    assert_eq!(codes(&doc), v(&["1", "2", "3", "4"]), "un plan inséré fait glisser les suivants");
    let i = index_of(&doc, "2");
    doc.remove_panels(&[doc.panels[i].id]);
    assert_eq!(codes(&doc), v(&["1", "2", "3"]), "un plan supprimé : les suivants remontent");
    assert!(doc.scenes.iter().all(|s| s.number.is_empty()), "rien n'est retenu en numérotation suivie");
    assert_eq!(doc.renumber(), 0, "rien à renuméroter en numérotation suivie");
}

// ------------------------------------------------------------------------------------ numérotation verrouillée

#[test]
fn locking_keeps_the_current_numbers() {
    let mut doc = film(4);
    let before = codes(&doc);
    doc.lock_numbering();
    assert!(doc.numbering_locked);
    assert_eq!(codes(&doc), before, "rien ne change à l'écran en verrouillant");
    assert_eq!(doc.sequence_code(doc.sequence_of(0)), "1");
}

#[test]
fn a_plan_inserted_between_12_and_13_becomes_12a() {
    let mut doc = film(14);
    doc.lock_numbering();
    let at = index_of(&doc, "12");
    let i = doc.add_scene_after(at);
    assert_eq!(doc.plan_code(doc.panels[i].scene), "12A");
    assert_eq!(doc.plan_ref(doc.panels[i].scene), "PL012A");
    assert_eq!(doc.plan_full_ref(i), "SQ01_PL012A");
    // Les autres n'ont pas bougé.
    assert_eq!(&codes(&doc)[10..15], &["11", "12", "12A", "13", "14"]);
    // Un second juste après le 12A : 12B ; et un autre juste après le 12 : 12.5 (rien d'autre ne tient).
    let b = doc.add_scene_after(index_of(&doc, "12A"));
    assert_eq!(doc.plan_code(doc.panels[b].scene), "12B");
    let c = doc.add_scene_after(index_of(&doc, "12"));
    assert_eq!(doc.plan_code(doc.panels[c].scene), "12.5");
    let d = doc.add_scene_after(index_of(&doc, "12A"));
    assert_eq!(doc.plan_code(doc.panels[d].scene), "12A1");
    assert_eq!(&codes(&doc)[10..18], &["11", "12", "12.5", "12A", "12A1", "12B", "13", "14"]);
}

#[test]
fn a_deleted_plan_leaves_a_gap_that_is_never_reused() {
    let mut doc = film(5);
    doc.lock_numbering();
    let id = id_of(&doc, "3");
    doc.remove_panels(&[id]);
    assert_eq!(codes(&doc), v(&["1", "2", "4", "5"]), "les autres ne bougent pas");
    let i = doc.add_scene_after(index_of(&doc, "2"));
    assert_eq!(doc.plan_code(doc.panels[i].scene), "2A", "le 3 n'est pas redonné");
    // Un plan ajouté à la fin prend l'entier suivant le dernier.
    let end = doc.add_scene_after(doc.panels.len() - 1);
    assert_eq!(doc.plan_code(doc.panels[end].scene), "6");
}

#[test]
fn a_moved_plan_keeps_its_number() {
    let mut doc = film(4);
    doc.lock_numbering();
    let four = id_of(&doc, "4");
    let first = doc.panels[0].id;
    doc.move_plans(&[four], Some(first));
    assert_eq!(codes(&doc), v(&["4", "1", "2", "3"]), "le 4 est devant, il garde son numéro");
    // Un plan inséré après ce 4 (qui précède le 1) passe juste après lui.
    let i = doc.add_scene_after(0);
    assert_eq!(doc.plan_code(doc.panels[i].scene), "4A");
    assert_eq!(codes(&doc), v(&["4", "4A", "1", "2", "3"]));
}

#[test]
fn duplicating_splitting_and_pasting_follow_the_insertion_rule() {
    let mut doc = film(3);
    doc.lock_numbering();
    // Dupliquer le 2.
    let dup = doc.duplicate_scene_after(index_of(&doc, "2"));
    assert_eq!(doc.plan_code(doc.panels[dup].scene), "2A");
    assert_eq!(codes(&doc), v(&["1", "2", "2A", "3"]));
    // Couper le 1 (un plan de 48 images).
    doc.panels[0].set_length(48);
    let cut = doc.split_plan(0, 24).expect("coupé");
    assert_eq!(doc.plan_code(doc.panels[cut].scene), "1A");
    assert_eq!(codes(&doc), v(&["1", "1A", "2", "2A", "3"]));
    // Coller deux plans après le 3 : 4 puis 5.
    let clip = doc.copy_panels(&[doc.panels[0].id, doc.panels[1].id]).expect("copié");
    let last = doc.panels.len() - 1;
    doc.paste_plans(&clip, last).expect("collé");
    assert_eq!(codes(&doc), v(&["1", "1A", "2", "2A", "3", "4", "5"]));
    // Coller après le 2 (avant le 2A) : deux plans entre 2 et 2A -> 2.5 puis 2.75.
    let at = index_of(&doc, "2");
    doc.paste_plans(&clip, at).expect("collé");
    let list = codes(&doc);
    assert_eq!(&list[..3], &["1", "1A", "2"]);
    assert_eq!(list[3], "2.5");
    assert_eq!(list.len(), 9);
    let nums: Vec<Num> = list.iter().map(|c| num(c)).collect();
    assert!(nums.windows(2).all(|w| w[0] < w[1]), "toujours dans l'ordre : {list:?}");
    let unique: std::collections::HashSet<&String> = list.iter().collect();
    assert_eq!(unique.len(), list.len(), "aucun numéro en double : {list:?}");
}

#[test]
fn joining_two_plans_drops_one_number_and_keeps_the_others() {
    let mut doc = film(4);
    doc.lock_numbering();
    doc.join_next_plan(index_of(&doc, "2")).expect("réuni");
    assert_eq!(codes(&doc), v(&["1", "2", "4"]), "le 3 est réuni au 2 : trou au 3");
}

#[test]
fn sequences_are_numbered_the_same_way() {
    let mut doc = film(3);
    doc.add_sequence_after(2);
    doc.lock_numbering();
    assert_eq!(doc.sequence_order().iter().map(|&s| doc.sequence_code(s)).collect::<Vec<_>>(), v(&["1", "2"]));
    let s = doc.add_sequence_after(0);
    let seq = doc.sequence_of(s);
    assert_eq!(doc.sequence_code(seq), "1A", "une séquence insérée entre la 1 et la 2");
    assert_eq!(doc.sequence_ref(seq), "SQ01A");
    assert_eq!(doc.plan_full_ref(s), format!("SQ01A_PL{}", doc.plan_num(doc.panels[s].scene).padded(3)));
    assert_eq!(doc.sequence_ref(doc.sequence_of(doc.panels.len() - 1)), "SQ02");
}

#[test]
fn a_chosen_name_still_wins_over_the_number_in_labels() {
    let mut doc = film(2);
    doc.lock_numbering();
    let scene = doc.panels[1].scene;
    doc.scene_mut(scene).unwrap().name = "Contrechamp".into();
    assert_eq!(doc.scene_label(scene), "Contrechamp");
    assert_eq!(doc.plan_code(scene), "2", "le numéro reste disponible pour les noms de fichiers");
    assert_eq!(doc.plan_ref(scene), "PL002");
}

#[test]
fn unlocking_goes_back_to_positions() {
    let mut doc = film(4);
    doc.lock_numbering();
    doc.add_scene_after(1);
    assert_eq!(codes(&doc), v(&["1", "2", "2A", "3", "4"]));
    doc.unlock_numbering();
    assert!(!doc.numbering_locked);
    assert_eq!(codes(&doc), v(&["1", "2", "3", "4", "5"]));
    assert!(doc.scenes.iter().all(|s| s.number.is_empty()) && doc.sequences.iter().all(|s| s.number.is_empty()));
}

#[test]
fn renumbering_gives_clean_numbers_in_film_order() {
    let mut doc = film(4);
    doc.lock_numbering();
    doc.add_scene_after(1);
    doc.add_scene_after(1);
    let id = id_of(&doc, "4");
    doc.remove_panels(&[id]);
    // (Le second plan inséré après le 2 est entre le 2 et le 2A : 2.5.)
    assert_eq!(codes(&doc), v(&["1", "2", "2.5", "2A", "3"]));
    let changed = doc.renumber();
    assert_eq!(codes(&doc), v(&["1", "2", "3", "4", "5"]));
    assert_eq!(changed, 3, "2.5, 2A et 3 changent");
    assert!(doc.numbering_locked, "le verrou reste");
    // Ensuite, insérer redonne une lettre.
    let i = doc.add_scene_after(1);
    assert_eq!(doc.plan_code(doc.panels[i].scene), "2A");
}

// ------------------------------------------------------------------------------------ annulation (l'application)

fn app_with(doc: Document, name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-numerotation-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_frame = 0;
    app
}

#[test]
fn locking_and_renumbering_are_one_named_undo_step_each() {
    let mut app = app_with(film(4), "annuler");
    app.run_action("numerotation.verrouiller");
    assert!(app.doc.numbering_locked);
    assert_eq!(app.history.names().0.last().copied(), Some("Verrouiller / déverrouiller la numérotation des plans"));
    // Un plan inséré entre le 2 et le 3 (par la commande de l'interface), puis la suppression du 1.
    app.cur_panel = index_of(&app.doc, "2");
    app.new_panel();
    assert_eq!(codes(&app.doc), v(&["1", "2", "2A", "3", "4"]));
    app.cur_panel = 0;
    app.delete_chosen_panels();
    assert_eq!(codes(&app.doc), v(&["2", "2A", "3", "4"]), "un trou au 1");
    // Renuméroter : la commande demande confirmation, rien ne change avant.
    app.run_action("numerotation.renumeroter");
    assert!(app.renumber_confirm);
    assert_eq!(codes(&app.doc), v(&["2", "2A", "3", "4"]));
    app.renumber_plans();
    assert_eq!(codes(&app.doc), v(&["1", "2", "3", "4"]));
    assert_eq!(app.history.names().0.last().copied(), Some("Renuméroter les plans"));
    // Une seule annulation remet tout.
    app.undo();
    assert_eq!(codes(&app.doc), v(&["2", "2A", "3", "4"]));
    app.redo();
    assert_eq!(codes(&app.doc), v(&["1", "2", "3", "4"]));
    app.undo();
    app.undo();
    app.undo();
    app.undo();
    assert!(!app.doc.numbering_locked, "annuler tout rend la numérotation suivie");
    assert_eq!(codes(&app.doc), v(&["1", "2", "3", "4"]));
}

#[test]
fn undoing_the_lock_forgets_the_numbers_and_redoing_gives_them_back() {
    let mut app = app_with(film(3), "verrou");
    app.run_action("numerotation.verrouiller");
    app.cur_panel = 0;
    app.new_panel();
    assert_eq!(codes(&app.doc), v(&["1", "1A", "2", "3"]));
    app.undo();
    app.undo();
    assert!(!app.doc.numbering_locked);
    app.redo();
    app.redo();
    assert!(app.doc.numbering_locked);
    assert_eq!(codes(&app.doc), v(&["1", "1A", "2", "3"]));
}

#[test]
fn the_new_commands_are_in_the_bars_and_the_menus() {
    use crate::barres::Barre;
    let ids: Vec<&str> = Barre::Montage.commandes().iter().map(|c| c.id).collect();
    for id in ["numerotation.verrouiller", "numerotation.renumeroter"] {
        assert!(ids.contains(&id), "{id} est dans la barre du montage (sous « Plus… »)");
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id} est une action");
    }
}

// ------------------------------------------------------------------------------------ enregistrement

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-numerotation-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Enregistre puis rouvre ; renvoie le projet relu et le chemin de son manifeste.
fn roundtrip(doc: &Document, name: &str) -> (Document, std::path::PathBuf) {
    let dir = temp(name);
    let (_, manifest) = crate::project::target_for(&dir.join("Essai.pelure"));
    let mut written = std::collections::HashMap::new();
    crate::project::save(doc, &manifest, &mut written).expect("enregistré");
    let loaded = crate::project::load(&manifest).expect("ouvert");
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    (loaded.doc, manifest)
}

#[test]
fn locked_numbers_survive_save_and_load() {
    let mut doc = film(5);
    doc.lock_numbering();
    doc.add_scene_after(1);
    doc.add_scene_after(1);
    let id = id_of(&doc, "4");
    doc.remove_panels(&[id]);
    doc.add_sequence_after(0);
    let want = codes(&doc);
    assert!(want.contains(&"2.5".to_string()) && want.contains(&"2A".to_string()) && !want.contains(&"4".to_string()), "{want:?}");
    let seqs: Vec<String> = doc.sequence_order().iter().map(|&s| doc.sequence_code(s)).collect();
    let (loaded, _) = roundtrip(&doc, "aller-retour");
    assert!(loaded.numbering_locked);
    assert_eq!(codes(&loaded), want);
    assert_eq!(loaded.sequence_order().iter().map(|&s| loaded.sequence_code(s)).collect::<Vec<_>>(), seqs);
    // Et le projet rouvert continue d'insérer entre ses voisins.
    let mut loaded = loaded;
    let i = loaded.add_scene_after(index_of(&loaded, "2A"));
    assert_eq!(loaded.plan_code(loaded.panels[i].scene), "2B");
}

#[test]
fn followed_numbering_writes_nothing_new_in_the_manifest() {
    // Un projet suivi s'enregistre exactement comme avant : aucun champ de numérotation. Une
    // version plus ancienne le lit donc sans rien perdre, et un ancien projet s'ouvre tel quel.
    let doc = film(3);
    let (loaded, manifest) = roundtrip(&doc, "suivie");
    assert!(!loaded.numbering_locked);
    assert_eq!(codes(&loaded), v(&["1", "2", "3"]));
    let text = std::fs::read_to_string(manifest).unwrap();
    assert!(!text.contains("numero") && !text.contains("numerotation"), "{text}");
}

#[test]
fn an_old_manifest_without_numbering_fields_opens_followed() {
    // On fabrique un « ancien » manifeste en retirant les champs de numérotation d'un projet verrouillé.
    let mut doc = film(3);
    doc.lock_numbering();
    doc.add_scene_after(0);
    let dir = temp("ancien");
    let (proj_dir, manifest) = crate::project::target_for(&dir.join("Ancien.pelure"));
    crate::project::save(&doc, &manifest, &mut std::collections::HashMap::new()).expect("enregistré");
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("numerotation_verrouillee") && text.contains("\"numero\""));
    let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
    json.as_object_mut().unwrap().remove("numerotation_verrouillee");
    for seq in json["sequences"].as_array_mut().unwrap() {
        seq.as_object_mut().unwrap().remove("numero");
        for scene in seq["scenes"].as_array_mut().unwrap() {
            scene.as_object_mut().unwrap().remove("numero");
        }
    }
    std::fs::write(&manifest, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    let loaded = crate::project::load(&manifest).expect("ouvert");
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert!(!loaded.doc.numbering_locked);
    assert_eq!(codes(&loaded.doc), v(&["1", "2", "3", "4"]), "numérotation suivie, par rang");
    let _ = proj_dir;
}

#[test]
fn a_locked_project_missing_a_number_gets_one_on_opening() {
    // Fichier édité à la main : un plan sans numéro reçoit le sien entre ses voisins.
    let mut doc = film(3);
    doc.lock_numbering();
    let id = doc.panels[1].scene;
    doc.scene_mut(id).unwrap().number.clear();
    doc.normalize();
    assert_eq!(codes(&doc), v(&["1", "1A", "3"]));
}
