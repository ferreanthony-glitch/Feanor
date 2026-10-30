//! Fichiers annexes du projet (`commentaires.json`, `rognages.json`) que cette version ne sait pas
//! lire — écrits par une version plus récente, ou abîmés. Le projet s'ouvre sans eux, avec un
//! avertissement ; mais l'enregistrement qui suit ne doit **jamais** les effacer : ce sont les
//! commentaires et le contenu rogné d'Anthony, qu'une autre version relira, ou qu'on réparera à la
//! main. (Avant cette passe, « pas de commentaire lu » voulait dire « fichier retiré à
//! l'enregistrement suivant ».)

use serde_json::Value;

use crate::fiabilite_fixture::{arbre, ecrire_json, empreinte, lire_json, projet_enregistre, racine};
use crate::project;

const ANNEXES: [&str; 2] = [crate::commentaires::FILE, crate::rognage::FILE];

#[test]
fn sidecars_from_a_newer_version_are_reported_and_survive_a_save() {
    let (_f, manifeste) = projet_enregistre("annexes-recentes");
    let dir = racine(&manifeste).to_path_buf();
    for fichier in ANNEXES {
        let mut v = lire_json(&dir.join(fichier));
        v["version"] = Value::from(3);
        v["champ_inconnu_d_une_version_future"] = Value::from("à garder");
        ecrire_json(&dir.join(fichier), &v);
    }
    let avant = arbre(&dir);
    let mut l = project::load(&manifeste).expect("le projet s'ouvre sans ses fichiers annexes");
    assert_eq!(l.warnings.len(), 2, "{:#?}", l.warnings);
    assert!(l.warnings.iter().any(|w| w.contains(crate::commentaires::FILE) && w.contains("plus récente")), "{:?}", l.warnings);
    assert!(l.warnings.iter().any(|w| w.contains(crate::rognage::FILE) && w.contains("plus récente")), "{:?}", l.warnings);
    assert!(l.doc.comments.is_empty() && l.doc.reserves.is_empty(), "rien n'est lu des fichiers d'une version plus récente");
    assert_eq!(arbre(&dir), avant, "ouvrir ne touche à aucun fichier");

    // Enregistrer par-dessus : les fichiers que cette version ne sait pas lire restent là, intacts.
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    let apres = arbre(&dir);
    for fichier in ANNEXES {
        assert_eq!(apres.get(fichier), avant.get(fichier), "{fichier} d'une version plus récente a été effacé ou réécrit par l'enregistrement");
    }

    // Si l'on ajoute un commentaire entre-temps, le fichier neuf est écrit, et l'ancien est gardé à côté.
    let plan = l.doc.panels[0].id;
    let id = l.doc.add_comment(plan, 1, "Anthony", 1_760_000_000);
    l.doc.comment_mut(id).expect("commentaire").text = "Nouveau.".into();
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    let relu = project::load(&manifeste).expect("relecture");
    assert!(relu.warnings.iter().all(|w| !w.contains(crate::commentaires::FILE)), "le fichier neuf se relit : {:?}", relu.warnings);
    assert_eq!(relu.doc.comments.len(), 1);
    let garde = dir.join(format!("{}.illisible", crate::commentaires::FILE));
    assert!(garde.exists(), "l'ancien {} est gardé sous le nom {}", crate::commentaires::FILE, garde.display());
    assert_eq!(empreinte(&std::fs::read(&garde).expect("lecture")), *avant.get(crate::commentaires::FILE).expect("fichier d'origine"));
    // Et le fichier des rognages, lui, est toujours celui d'origine.
    assert_eq!(arbre(&dir).get(crate::rognage::FILE), avant.get(crate::rognage::FILE));
}

#[test]
fn damaged_sidecars_are_reported_and_survive_a_save() {
    let (_f, manifeste) = projet_enregistre("annexes-abimees");
    let dir = racine(&manifeste).to_path_buf();
    let c = std::fs::read(dir.join(crate::commentaires::FILE)).expect("commentaires");
    let r = std::fs::read(dir.join(crate::rognage::FILE)).expect("rognages");
    std::fs::write(dir.join(crate::commentaires::FILE), &c[..c.len() / 2]).expect("écriture");
    std::fs::write(dir.join(crate::rognage::FILE), &r[..r.len() / 3]).expect("écriture");
    let avant = arbre(&dir);
    let mut l = project::load(&manifeste).expect("le projet s'ouvre");
    assert_eq!(l.warnings.len(), 2, "{:#?}", l.warnings);
    assert_eq!(arbre(&dir), avant);
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    let apres = arbre(&dir);
    for fichier in ANNEXES {
        assert_eq!(apres.get(fichier), avant.get(fichier), "{fichier} abîmé a été effacé par l'enregistrement : on ne peut plus le réparer à la main");
    }
}

/// Un son que cette version ne lit pas (fichier abîmé, codec absent) sort du projet, et un ancien modèle 3D n'est plus lu :
/// mais son fichier reste dans le dossier après l'enregistrement.
#[test]
fn unreadable_sounds_and_models_keep_their_files_through_a_save() {
    let (_f, manifeste) = projet_enregistre("sons-illisibles");
    let dir = racine(&manifeste).to_path_buf();
    let avant = arbre(&dir);
    let son = avant.keys().find(|k| k.starts_with("sons/")).expect("son").clone();
    let modele = avant.keys().find(|k| k.starts_with("modeles/")).expect("modèle").clone();
    std::fs::write(dir.join(&son), b"RIFF mais pas un son").expect("écriture");
    std::fs::write(dir.join(&modele), b"pas un modele").expect("écriture");
    let abimes = arbre(&dir);
    let mut l = project::load(&manifeste).expect("le projet s'ouvre");
    // (Le modèle 3D du repère n'est plus lu du tout : seul le son illisible est signalé.)
    assert_eq!(l.warnings.len(), 1, "{:#?}", l.warnings);
    assert!(l.doc.sounds.is_empty());
    assert_eq!(arbre(&dir), abimes, "ouvrir ne touche à aucun fichier");
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    let apres = arbre(&dir);
    for f in [&son, &modele] {
        assert_eq!(apres.get(f), abimes.get(f), "{f} illisible a été effacé par l'enregistrement");
    }
}

/// Abîme ce que cette version ne sait pas lire : `commentaires.json` tronqué, `rognages.json` d'une
/// version plus récente, un son et un modèle 3D qui ne sont pas ce qu'ils disent. Rend l'arbre du
/// dossier ainsi abîmé.
pub(crate) fn abimer_les_annexes(dir: &std::path::Path) -> (std::collections::BTreeMap<String, u64>, String, String) {
    let avant = arbre(dir);
    let son = avant.keys().find(|k| k.starts_with("sons/")).expect("son").clone();
    let modele = avant.keys().find(|k| k.starts_with("modeles/")).expect("modèle").clone();
    let c = std::fs::read(dir.join(crate::commentaires::FILE)).expect("commentaires");
    std::fs::write(dir.join(crate::commentaires::FILE), &c[..c.len() / 2]).expect("écriture");
    let mut v = lire_json(&dir.join(crate::rognage::FILE));
    v["version"] = Value::from(3);
    v["champ_inconnu_d_une_version_future"] = Value::from("à garder");
    ecrire_json(&dir.join(crate::rognage::FILE), &v);
    std::fs::write(dir.join(&son), b"RIFF mais pas un son").expect("écriture");
    std::fs::write(dir.join(&modele), b"pas un modele").expect("écriture");
    (arbre(dir), son, modele)
}

/// L'enregistrement en arrière-plan (`prepare_save` sur le fil de l'interface, `SaveJob::run_with` sur
/// des fils d'écriture) garde les protections de l'enregistrement d'avant : un `commentaires.json`
/// abîmé, un `rognages.json` d'une version plus récente, un son et un modèle 3D illisibles restent
/// tels quels ; le fichier neuf d'un commentaire ajouté met l'ancien de côté (`.illisible`) ; et
/// aucun `.tmp` ne traîne.
#[test]
fn a_background_save_job_keeps_unreadable_sidecars_sounds_and_models() {
    let (_f, manifeste) = projet_enregistre("annexes-arriere-plan");
    let dir = racine(&manifeste).to_path_buf();
    let (abimes, son, modele) = abimer_les_annexes(&dir);
    let mut l = project::load(&manifeste).expect("le projet s'ouvre");
    assert!(l.warnings.len() >= 3, "{:#?}", l.warnings);
    assert!(l.doc.comments.is_empty() && l.doc.reserves.is_empty() && l.doc.sounds.is_empty());

    let ecrire = |doc: &crate::document::Document, written: &mut std::collections::HashMap<crate::document::CelId, u64>| {
        let job = project::prepare_save(doc, &manifeste, written, None).expect("cliché");
        let progress = std::sync::Arc::new(project::Progress::default());
        let q = progress.clone();
        let rapport = std::thread::spawn(move || job.run_with(Some(&q), 3)).join().expect("fil d'écriture").expect("écriture");
        let (fait, total) = (progress.done.load(std::sync::atomic::Ordering::Relaxed), progress.total.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(fait, total, "l'avancement va jusqu'au bout");
        rapport
    };

    // Le fil d'écriture n'efface ni ne réécrit rien de ce qu'il ne sait pas lire.
    l.written.clear();
    ecrire(&l.doc, &mut l.written);
    let apres = arbre(&dir);
    for f in [crate::commentaires::FILE, crate::rognage::FILE, son.as_str(), modele.as_str()] {
        assert_eq!(apres.get(f), abimes.get(f), "{f} illisible a été effacé ou réécrit par l'enregistrement en arrière-plan");
    }
    assert!(!apres.keys().any(|k| k.ends_with(".illisible") || k.ends_with(".tmp")), "{:?}", apres.keys().collect::<Vec<_>>());

    // Un commentaire neuf : son fichier est écrit, l'ancien est mis de côté, le rognage d'une version
    // plus récente ne bouge toujours pas.
    let plan = l.doc.panels[0].id;
    let id = l.doc.add_comment(plan, 1, "Anthony", 1_760_000_000);
    l.doc.comment_mut(id).expect("commentaire").text = "Nouveau.".into();
    ecrire(&l.doc, &mut l.written);
    let garde = dir.join(format!("{}.illisible", crate::commentaires::FILE));
    assert!(garde.exists(), "l'ancien commentaires.json est gardé");
    assert_eq!(empreinte(&std::fs::read(&garde).expect("lecture")), *abimes.get(crate::commentaires::FILE).expect("fichier abîmé"));
    let apres = arbre(&dir);
    for f in [crate::rognage::FILE, son.as_str(), modele.as_str()] {
        assert_eq!(apres.get(f), abimes.get(f), "{f} illisible a été touché");
    }
    assert!(!apres.keys().any(|k| k.ends_with(".tmp")), "{:?}", apres.keys().collect::<Vec<_>>());
    let relu = project::load(&manifeste).expect("relecture");
    assert_eq!(relu.doc.comments.len(), 1, "le commentaire neuf se relit");
}

/// Les références visuelles (`references.json` et `references/`) suivent la même règle : un fichier
/// abîmé ou d'une version plus récente, ou une image que cette version ne cite pas, n'est jamais
/// effacé par l'enregistrement ; un fichier neuf met l'ancien de côté.
#[test]
fn references_sidecar_and_files_survive_a_save_when_unreadable() {
    let (_f, manifeste) = projet_enregistre("annexes-references");
    let dir = racine(&manifeste).to_path_buf();
    let mut l = project::load(&manifeste).expect("lecture");
    let work = dir.join("..").join(format!("attente-references-{}", std::process::id()));
    std::fs::create_dir_all(&work).expect("dossier");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([9, 9, 9, 255])).save(work.join("900201.png")).expect("image");
    let id = l.doc.new_reference_id();
    let mut r = crate::references::Reference::image(id, "Rue", "900201.png", [4, 4], eframe::egui::pos2(0.0, 0.0));
    r.home = Some(work.clone());
    l.doc.add_reference(r);
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    assert!(dir.join("references").join("900201.png").exists() && dir.join(crate::references::FILE).exists());

    // Une version plus récente écrit son fichier ; une image inconnue traîne dans references/.
    let mut v = lire_json(&dir.join(crate::references::FILE));
    v["version"] = Value::from(3);
    v["champ_inconnu_d_une_version_future"] = Value::from("à garder");
    ecrire_json(&dir.join(crate::references::FILE), &v);
    std::fs::write(dir.join("references").join("900202.png"), b"image que cette version ne cite pas").expect("écriture");
    let avant = arbre(&dir);
    let mut l = project::load(&manifeste).expect("le projet s'ouvre sans les références");
    assert!(l.warnings.iter().any(|w| w.contains(crate::references::FILE) && w.contains("plus récente")), "{:?}", l.warnings);
    assert!(l.doc.references.is_empty());
    assert_eq!(arbre(&dir), avant, "ouvrir ne touche à aucun fichier");
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    assert_eq!(arbre(&dir), avant, "le fichier d'une version plus récente et les images restent tels quels");

    // Une référence neuve : son fichier est écrit, l'ancien est gardé à côté, les images ne bougent pas.
    let id = l.doc.new_reference_id();
    l.doc.add_reference(crate::references::Reference::image(id, "Neuve", "900203.png", [4, 4], eframe::egui::pos2(0.0, 0.0)));
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    let apres = arbre(&dir);
    let garde = format!("{}.illisible", crate::references::FILE);
    assert_eq!(apres.get(&garde), avant.get(crate::references::FILE), "l'ancien références.json est gardé");
    assert_eq!(apres.get("references/900202.png"), avant.get("references/900202.png"));
    assert_eq!(apres.get("references/900201.png"), avant.get("references/900201.png"));
    assert!(!apres.keys().any(|k| k.ends_with(".tmp")), "{:?}", apres.keys().collect::<Vec<_>>());
    assert_eq!(project::load(&manifeste).expect("relecture").doc.references.len(), 1);
    std::fs::remove_dir_all(&work).ok();
}

/// Un projet sans annexes (sans commentaire ni rognage) n'en écrit pas, et retire celles qu'un
/// ancien enregistrement avait laissées : le comportement d'avant ne change pas pour un fichier lisible.
#[test]
fn readable_sidecars_still_follow_their_content() {
    let (_f, manifeste) = projet_enregistre("annexes-lisibles");
    let dir = racine(&manifeste).to_path_buf();
    let mut l = project::load(&manifeste).expect("lecture");
    assert!(!l.doc.comments.is_empty() && !l.doc.reserves.is_empty());
    l.doc.comments.clear();
    for p in 0..l.doc.panels.len() {
        l.doc.forget_reserve(p);
    }
    project::save(&l.doc, &manifeste, &mut l.written).expect("enregistrement");
    let apres = arbre(&dir);
    assert!(!apres.contains_key(crate::commentaires::FILE) && !apres.contains_key(crate::rognage::FILE), "sans contenu, ni commentaires.json ni rognages.json : {:?}", apres.keys().collect::<Vec<_>>());
    assert!(!apres.keys().any(|k| k.ends_with(".illisible")), "un fichier lisible n'est jamais mis de côté");
}
