//! Passe de débogage (Anthony, 2026-10-22 : « tu peux me faire une passe de débogage ? »). Les essais des cas
//! limites qui plantaient ou pouvaient planter : un projet abîmé qui s'ouvre quand même et que toute l'interface
//! doit montrer sans planter, et les cas un à un (plan vide, plan sans calque, un seul plan, clé de caméra après
//! la fin…).

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::app::{BoardView, Mode};
use crate::document::Document;
use crate::fiabilite_entrees_tests::{adresses, muter};
use crate::fiabilite_fixture::{ecrire_json, lire_json, projet_enregistre, Lcg};
use crate::perf_tests::{banc_taille, Banc};
use crate::workspace::Espace;

/// Le message d'un panic rattrapé.
fn message(e: &Box<dyn std::any::Any + Send>) -> String {
    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "plantage".into())
}

/// Un banc sans son ni carte graphique, qui ouvre les projets à la place du projet affiché.
fn banc_leger() -> Banc {
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1280, 720);
    b.app.sound_on = false;
    b.app.sound_scrub = false;
    b.app.settings.open_in_new_tab = false;
    b
}

/// Ce qu'un lancement montre d'un projet : les quatre espaces, la vue Vignettes, le dernier plan, ses dernières
/// images (et l'image juste après la fin), une courte lecture, une annulation.
fn parcourir(b: &mut Banc) {
    for espace in Espace::TOUS {
        b.app.set_espace(espace);
        b.chauffer(2);
    }
    b.app.set_mode(Mode::Storyboard);
    b.app.board_view = BoardView::Panels;
    b.chauffer(2);
    let n = b.app.doc.panels.len();
    b.app.select_panel(n - 1);
    b.chauffer(2);
    b.app.enter_animation(n - 1);
    let fc = b.app.frame_count();
    b.app.goto_frame(fc - 1);
    b.chauffer(2);
    b.app.goto_frame(fc);
    b.chauffer(1);
    b.app.goto_frame(0);
    b.app.toggle_play_scope(false);
    for _ in 0..4 {
        b.image(Vec::new(), 1.0 / 24.0);
    }
    b.app.toggle_play_scope(false);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(2);
    b.app.undo();
    b.chauffer(1);
}

/// Les cas limites d'un projet, un à un, se montrent partout sans planter : un seul plan, un plan de durée nulle,
/// un plan sans calque, une clé de caméra bien après la fin du plan, un calque qui a plus d'images que la durée
/// du plan, une marque d'image après la fin, un projet à 0 image par seconde.
#[test]
fn edge_case_projects_show_everywhere_without_crash() {
    use crate::camera::{set_key, Camera};
    type Cas = (&'static str, Box<dyn Fn() -> Document>);
    let cas: Vec<Cas> = vec![
        ("un seul plan", Box::new(|| Document::with_frame(160, 90, 24))),
        (
            "un plan de durée nulle",
            Box::new(|| {
                let mut d = Document::with_frame(160, 90, 24);
                d.add_panel_after(0);
                d.panels[1].length = 0;
                for l in &mut d.panels[1].layers {
                    l.frames.clear();
                }
                d
            }),
        ),
        (
            "un plan sans calque",
            Box::new(|| {
                let mut d = Document::with_frame(160, 90, 24);
                d.add_panel_after(0);
                d.panels[1].layers.clear();
                d
            }),
        ),
        (
            "une clé de caméra après la fin",
            Box::new(|| {
                let mut d = Document::with_frame(160, 90, 24);
                let a = d.default_camera();
                let cam = Camera { zoom: 2.0, ..a };
                set_key(&mut d.panels[0].camera, 0, a);
                set_key(&mut d.panels[0].camera, 10_000, cam);
                d
            }),
        ),
        (
            "un calque plus long que le plan",
            Box::new(|| {
                let mut d = Document::with_frame(160, 90, 24);
                let n = d.panels[0].frame_count();
                d.panels[0].layers[0].frames.resize(n + 40, None);
                d
            }),
        ),
        (
            "une marque d'image après la fin",
            Box::new(|| {
                let mut d = Document::with_frame(160, 90, 24);
                d.panels[0].marks.push(crate::document::ImageMark { id: 9, frame: 5_000, color: 200, action: "x".into(), dialogue: String::new(), notes: String::new() });
                d
            }),
        ),
        (
            "0 image par seconde",
            Box::new(|| {
                let mut d = Document::with_frame(160, 90, 24);
                d.add_panel_after(0);
                d.fps = 0;
                d
            }),
        ),
    ];
    let mut b = banc_leger();
    let mut plantages = Vec::new();
    for (nom, faire) in &cas {
        let r = catch_unwind(AssertUnwindSafe(|| {
            b.app.modified = false;
            assert!(b.app.charger_projet(faire()));
            parcourir(&mut b);
        }));
        if let Err(e) = r {
            plantages.push(format!("{nom} : {}", message(&e)));
            b = banc_leger();
        }
    }
    assert!(plantages.is_empty(), "{}", plantages.join("\n"));
}

/// Un projet fait de cas limites : un plan ordinaire avec un dessin, un plan de durée nulle, un plan dont la caméra a
/// une clé et une marque d'image bien après sa fin, un plan dont un calque a plus d'images que sa durée.
fn projet_limite() -> Document {
    use crate::camera::{set_key, Camera};
    use crate::document::LayerKind;
    let mut d = Document::with_frame(160, 90, 24);
    let cel = d.new_cel(LayerKind::Bitmap);
    d.panels[0].layers[0] = crate::document::Layer::new("Dessin", LayerKind::Bitmap);
    d.panels[0].layers[0].set(0, Some(cel));
    d.add_panel_after(0);
    d.panels[1].length = 0;
    for l in &mut d.panels[1].layers {
        l.frames.clear();
    }
    d.add_panel_after(1);
    let a = d.default_camera();
    set_key(&mut d.panels[2].camera, 0, a);
    set_key(&mut d.panels[2].camera, 10_000, Camera { zoom: 2.0, x: a.x + 30.0, ..a });
    d.panels[2].marks.push(crate::document::ImageMark { id: 9_001, frame: 5_000, color: 3, action: "après la fin".into(), ..Default::default() });
    d.add_panel_after(2);
    let n = d.panels[3].frame_count();
    d.panels[3].layers[0].frames.resize(n + 40, None);
    d
}

/// Les commandes qu'un essai ne lance pas : celles qui ouvrent une fenêtre de fichiers de Windows (l'essai attendrait),
/// touchent le presse-papiers de Windows, écrivent ou lisent ailleurs que dans le bac à sable, ou parlent au réseau.
fn commande_hors_essai(id: &str) -> bool {
    id.starts_with("fichier.")
        || id.starts_with("equipe.")
        || id.starts_with("suggestion.")
        || ["importer", "exporter", "copier", "couper", "coller", "enregistrer", "propager"].iter().any(|m| id.contains(m))
        || matches!(id, "onglet.nouveau" | "onglet.fermer_autres" | "aide.diagnostic" | "projet.nouveau_depuis_plans" | "envoi.marquer" | "bruitage.ecouter" | "son.lecture")
}

/// Chaque commande de SHOT, lancée sur un projet fait de cas limites (en storyboard puis en animation), puis annulée :
/// rien ne plante. (Les commandes vont à la suite, chacune sur ce que les précédentes ont laissé : supprimer, réunir,
/// couper des plans au fil des commandes ramène le projet jusqu'à un seul plan.)
#[test]
fn every_command_on_an_edge_case_project_never_crashes() {
    let ids: Vec<&str> = crate::keymap::ACTIONS.iter().filter(|a| a.implemented && !commande_hors_essai(a.id)).map(|a| a.id).collect();
    assert!(ids.len() > 300, "{} commandes", ids.len());
    let mut b = banc_leger();
    let mut plantages: Vec<String> = Vec::new();
    for animation in [false, true] {
        b.app.modified = false;
        assert!(b.app.charger_projet(projet_limite()));
        if animation {
            b.app.enter_animation(2);
            b.app.goto_frame(b.app.frame_count());
        } else {
            b.app.set_espace(Espace::Vignettes);
            b.app.select_panel(1);
        }
        b.chauffer(2);
        for id in &ids {
            let r = catch_unwind(AssertUnwindSafe(|| {
                b.app.run_action(id);
                b.image(Vec::new(), 1.0 / 60.0);
                b.app.run_action("edition.annuler");
                b.image(Vec::new(), 1.0 / 60.0);
            }));
            if let Err(e) = r {
                plantages.push(format!("{} « {id} » : {}", if animation { "animation" } else { "storyboard" }, message(&e)));
                b = banc_leger();
                b.app.modified = false;
                assert!(b.app.charger_projet(projet_limite()));
                if animation {
                    b.app.enter_animation(2);
                }
                b.chauffer(2);
            }
        }
    }
    assert!(plantages.is_empty(), "{} plantage(s) :\n{}", plantages.len(), plantages.join("\n"));
}

/// Des commandes au hasard à la suite (sans annuler chaque fois), avec des annulations et des rétablissements au milieu,
/// sur le projet de cas limites puis sur un projet d'un seul plan : rien ne plante, et le plan et le calque courants
/// restent dans le film.
#[test]
fn random_commands_in_a_row_never_crash() {
    let ids: Vec<&str> = crate::keymap::ACTIONS.iter().filter(|a| a.implemented && !commande_hors_essai(a.id)).map(|a| a.id).collect();
    let n: usize = std::env::var("SHOT_DEBOGAGE_ESSAIS").ok().and_then(|v| v.parse().ok()).map_or(500, |k: usize| k * 10);
    let mut plantages: Vec<String> = Vec::new();
    let projets: [fn() -> Document; 2] = [projet_limite, || Document::with_frame(160, 90, 24)];
    for (p, faire) in projets.iter().enumerate() {
        let mut b = banc_leger();
        b.app.modified = false;
        assert!(b.app.charger_projet(faire()));
        b.chauffer(2);
        let mut rng = Lcg(1_792_689_702 + p as u64);
        let mut faites: Vec<&str> = Vec::new();
        // Les commandes qui ont vraiment changé le projet (une étape d'annulation de plus) : l'essai ne tourne pas à vide.
        let mut efficaces = 0;
        for _ in 0..n {
            let id = match rng.sous(10) {
                0 => "edition.annuler",
                1 => "edition.retablir",
                _ => ids[rng.sous(ids.len())],
            };
            faites.push(id);
            let avant = b.app.history.serial();
            let r = catch_unwind(AssertUnwindSafe(|| {
                b.app.run_action(id);
                efficaces += usize::from(b.app.history.serial() != avant);
                b.image(Vec::new(), 1.0 / 60.0);
                assert!(b.app.cur_panel < b.app.doc.panels.len(), "plan courant hors du film");
                assert!(b.app.cur_layer < b.app.doc.panels[b.app.cur_panel].layers.len(), "calque courant hors du plan");
            }));
            if let Err(e) = r {
                let fin = faites.len().saturating_sub(12);
                plantages.push(format!("projet {p}, « {id} » : {} ; dernières commandes : {:?}", message(&e), &faites[fin..]));
                b = banc_leger();
                b.app.modified = false;
                assert!(b.app.charger_projet(faire()));
                b.chauffer(2);
                faites.clear();
            }
        }
        println!("DEBOGAGE commandes au hasard, projet {p} : {n} commandes, {efficaces} ont changé le projet ; {} plan(s) à la fin", b.app.doc.panels.len());
        assert!(efficaces * 10 > n, "les commandes changent le projet ({efficaces} sur {n})");
    }
    assert!(plantages.is_empty(), "{} plantage(s) :\n{}", plantages.len(), plantages.join("\n"));
}

/// Un ancien `.pelure` (un seul fichier) abîmé est refusé sans planter : une feuille de taille nulle (elle s'ouvrait,
/// et une feuille vide fait diviser par zéro), puis des octets changés au hasard.
#[test]
fn a_damaged_legacy_file_is_refused_without_panic() {
    use crate::document::LayerKind;
    let dir = std::env::temp_dir().join(format!("shot-debogage-ancien-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dossier");
    let path = dir.join("ancien.pelure");
    // La largeur à zéro (un projet sans dessin : rien d'autre ne la contrôle).
    let vide = Document::new(64, 64, 24);
    crate::io::save_legacy_v2(&vide, &path).expect("écriture");
    assert!(crate::io::load_legacy(&path).is_ok(), "le fichier intact s'ouvre");
    let octets = std::fs::read(&path).expect("lecture");
    let json = std::str::from_utf8(&octets[20..]).expect("JSON");
    let abime = json.replacen("\"width\":64", "\"width\":0", 1);
    assert_ne!(abime, json, "la largeur est dans le JSON");
    let mut f = octets[..12].to_vec();
    f.extend_from_slice(&(abime.len() as u64).to_le_bytes());
    f.extend_from_slice(abime.as_bytes());
    std::fs::write(&path, &f).expect("écriture");
    assert!(crate::io::load_legacy(&path).is_err(), "une feuille de largeur nulle est refusée");
    // Un projet avec un dessin bitmap (son image PNG après le JSON).
    let mut doc = Document::new(64, 64, 24);
    let cel = doc.new_cel(LayerKind::Bitmap);
    doc.panels[0].layers[0] = crate::document::Layer::new("Dessin", LayerKind::Bitmap);
    doc.panels[0].layers[0].set(0, Some(cel));
    crate::io::save_legacy_v2(&doc, &path).expect("écriture");
    assert!(crate::io::load_legacy(&path).is_ok(), "le fichier intact s'ouvre");
    let bon = std::fs::read(&path).expect("lecture");
    // Des octets changés au hasard (en-tête, longueur du JSON, positions des dessins…).
    let mut rng = Lcg(1792663194);
    for _ in 0..400 {
        let mut b = bon.clone();
        for _ in 0..1 + rng.sous(3) {
            let i = if rng.sous(2) == 0 { rng.sous(24) } else { rng.sous(b.len()) };
            b[i] = rng.next() as u8;
        }
        std::fs::write(&path, &b).expect("écriture");
        let r = catch_unwind(|| crate::io::load_legacy(&path).map(|d| d.panels.len()));
        assert!(r.is_ok(), "un ancien fichier abîmé a planté la lecture");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Le rapport d'une erreur dit ce qu'on venait de faire : les dernières commandes et étapes d'annulation.
#[test]
fn a_crash_report_lists_the_last_actions() {
    // (La liste est commune à tous les essais qui tournent en même temps : on la relit juste après la note, et on
    // réessaie si d'autres essais l'ont remplie entre-temps.)
    let vu = (0..50).any(|k| {
        let marque = format!("essai du rapport {k}");
        crate::plantage::noter_action(&marque);
        crate::plantage::report_text("index out of bounds", "src/x.rs:1:1", "main", "").contains(&marque)
    });
    assert!(vu, "la dernière action est dans le rapport");
    let t = crate::plantage::report_text("x", "y", "z", "");
    assert!(t.contains("Dernières actions : "), "{t}");
}

/// La carte graphique perdue : une copie de secours de l'onglet affiché part tout de suite (sans attendre le délai),
/// et la barre d'état dit quoi faire (le 2026-10-22, SHOT a tourné deux minutes sur une carte perdue avant de se
/// fermer, `plantage-1792663086.txt` puis `plantage-1792663194.txt`).
#[test]
fn a_lost_graphics_card_starts_a_recovery_copy_at_once() {
    let racine = std::env::temp_dir().join(format!("shot-debogage-carte-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&racine);
    let mut b = banc_leger();
    b.app.recovery = Some(crate::recovery::Recovery::start(&racine).expect("dossier de secours"));
    b.app.settings.autosave_minutes = 30;
    b.app.modified = true;
    b.app.secours_carte_perdue();
    assert!(b.app.recovery.as_ref().is_some_and(crate::recovery::Recovery::busy), "la copie part tout de suite");
    assert!(b.app.status.contains("carte graphique"), "{}", b.app.status);
    let t = std::time::Instant::now();
    while b.app.recovery.as_ref().is_some_and(crate::recovery::Recovery::busy) && t.elapsed() < std::time::Duration::from_secs(30) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let dir = b.app.recovery.as_ref().expect("secours").dir.clone();
    assert!(dir.join("projet").join("secours.pelure").exists(), "la copie est écrite");
    if let Some(r) = b.app.recovery.take() {
        r.close();
    }
    let _ = std::fs::remove_dir_all(&racine);
}

/// Change au hasard une valeur de `v` à l'adresse `adresse` en gardant son genre (un entier reste un entier : la lecture
/// refuse un nombre à virgule là où elle attend un entier, et l'essai ne passerait jamais la lecture). Les tableaux
/// perdent, doublent ou vident leurs éléments.
fn muter_valeur(v: &mut serde_json::Value, adresse: &str, rng: &mut Lcg) -> String {
    use serde_json::Value;
    let entiers: [i128; 12] = [0, 1, 2, 3, 7, 24, 255, 65_535, 4_294_967_295, 4_294_967_296, 1_000_000_000_000, u64::MAX as i128];
    let reels = [0.0, -1.0, 1e-9, 0.5, 2.0, 1e6, -1e6, 1e30, -1e30];
    let Some(x) = v.pointer_mut(adresse) else { return format!("{adresse} : absente") };
    let fait = match x {
        Value::Number(n) if n.is_u64() || n.is_i64() => {
            let k = entiers[rng.sous(entiers.len())];
            let neg = n.is_i64() && rng.sous(4) == 0;
            *x = if neg { Value::from(-(k.min(i64::MAX as i128) as i64)) } else { Value::from(k as u64) };
            format!("entier {x}")
        }
        Value::Number(_) => {
            let r = reels[rng.sous(reels.len())];
            *x = Value::from(r);
            format!("réel {r}")
        }
        Value::Bool(b) => {
            *b = !*b;
            "booléen inversé".into()
        }
        Value::Array(a) if !a.is_empty() => match rng.sous(3) {
            0 => {
                a.remove(rng.sous(a.len()));
                "un élément retiré".into()
            }
            1 => {
                let i = rng.sous(a.len());
                let e = a[i].clone();
                a.insert(i, e);
                "un élément doublé".into()
            }
            _ => {
                a.clear();
                "vidé".into()
            }
        },
        Value::String(s) => {
            *s = ["", "0", "x", "../x"][rng.sous(4)].into();
            format!("texte « {s} »")
        }
        _ => "rien".into(),
    };
    format!("{adresse} : {fait}")
}

/// Un projet dont le manifeste a été abîmé valeur par valeur (en gardant le genre de chaque valeur : c'est ce qu'un
/// fichier écrit par une autre version, ou retouché à la main, peut avoir), puis ouvert et montré partout.
#[test]
fn a_project_with_odd_values_never_crashes_the_interface() {
    let (_f, manifeste) = projet_enregistre("debogage-valeurs");
    let original = lire_json(&manifeste);
    let mut toutes = Vec::new();
    adresses(&original, "", &mut toutes);
    // (Les valeurs des plans d'abord : séquences, scènes, plans, calques, clés, marques.)
    let dedans: Vec<String> = toutes.into_iter().filter(|a| a.starts_with("/sequences/") || a.starts_with("/largeur") || a.starts_with("/hauteur") || a.starts_with("/cadre") || a.starts_with("/cadence")).collect();
    let mut b = banc_leger();
    let mut rng = Lcg(5_2026_10_22);
    let mut ouverts = 0;
    let mut plantages: Vec<String> = Vec::new();
    let essais: usize = std::env::var("SHOT_DEBOGAGE_ESSAIS").ok().and_then(|v| v.parse().ok()).unwrap_or(40);
    for k in 0..essais {
        let mut v = original.clone();
        let mut faits = Vec::new();
        for _ in 0..1 + rng.sous(3) {
            let adresse = dedans[rng.sous(dedans.len())].clone();
            faits.push(muter_valeur(&mut v, &adresse, &mut rng));
        }
        ecrire_json(&manifeste, &v);
        let lu = catch_unwind(|| crate::project::load_quick(&manifeste));
        let loaded = match lu {
            Ok(Ok(l)) => l,
            Ok(Err(_)) => continue,
            Err(e) => {
                plantages.push(format!("essai {k} (lecture) : {} ; changements : {faits:?}", message(&e)));
                continue;
            }
        };
        ouverts += 1;
        let r = catch_unwind(AssertUnwindSafe(|| {
            b.app.modified = false;
            b.app.finish_open(manifeste.clone(), Ok(loaded));
            parcourir(&mut b);
        }));
        if let Err(e) = r {
            plantages.push(format!("essai {k} : {} ; changements : {faits:?}", message(&e)));
            b = banc_leger();
        }
    }
    ecrire_json(&manifeste, &original);
    println!("DEBOGAGE valeurs changées : {ouverts} projets ouverts sur {essais}, {} plantage(s)", plantages.len());
    assert!(ouverts * 2 > essais, "la plupart des projets s'ouvrent ({ouverts} sur {essais})");
    assert!(plantages.is_empty(), "{} plantage(s) :\n{}", plantages.len(), plantages.join("\n"));
}

/// Un projet dont le manifeste a été abîmé au hasard, mais que la lecture accepte (avec ou sans avertissements),
/// s'ouvre et se montre partout sans planter : la lecture ne plante pas (`fiabilite_entrees_tests.rs`), l'interface
/// non plus avec ce qu'elle a lu.
#[test]
fn a_damaged_project_that_opens_never_crashes_the_interface() {
    let (_f, manifeste) = projet_enregistre("debogage-ouvert-abime");
    let original = lire_json(&manifeste);
    let mut toutes = Vec::new();
    adresses(&original, "", &mut toutes);
    let mut b = banc_leger();
    let mut rng = Lcg(2026_10_22);
    let mut ouverts = 0;
    let mut plantages: Vec<String> = Vec::new();
    let essais: usize = std::env::var("SHOT_DEBOGAGE_ESSAIS").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
    for k in 0..essais {
        let mut v = original.clone();
        let mut faits = Vec::new();
        for _ in 0..1 + rng.sous(3) {
            let adresse = toutes[rng.sous(toutes.len())].clone();
            faits.push(muter(&mut v, &adresse, &mut rng));
        }
        ecrire_json(&manifeste, &v);
        let Ok(loaded) = crate::project::load_quick(&manifeste) else { continue };
        ouverts += 1;
        let r = catch_unwind(AssertUnwindSafe(|| {
            b.app.modified = false;
            b.app.finish_open(manifeste.clone(), Ok(loaded));
            parcourir(&mut b);
        }));
        if let Err(e) = r {
            plantages.push(format!("essai {k} : {} ; changements : {faits:?}", message(&e)));
            // (L'interface est dans un état quelconque après un plantage : un banc neuf.)
            b = banc_leger();
        }
    }
    ecrire_json(&manifeste, &original);
    println!("DEBOGAGE projets abîmés ouverts : {ouverts} sur {essais}, {} plantage(s)", plantages.len());
    assert!(ouverts > 0, "au moins un projet abîmé s'ouvre");
    assert!(plantages.is_empty(), "{} plantage(s) :\n{}", plantages.len(), plantages.join("\n"));
}
