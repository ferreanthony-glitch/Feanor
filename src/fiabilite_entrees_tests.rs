//! Entrées abîmées, première partie : le manifeste. Un manifeste tronqué, d'une version plus
//! récente ou fait de valeurs absurdes donne un message clair, jamais un plantage ni un blocage, et
//! **aucun fichier n'est effacé ni modifié** par l'ouverture. Plus des essais « à la louche » (fuzz) :
//! des troncatures et des valeurs changées au hasard, avec un garde-fou contre les blocages.
//! (Les dessins abîmés sont dans `fiabilite_dessins_tests.rs`, les fichiers annexes dans
//! `fiabilite_annexes_tests.rs`.)

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use crate::document::{Document, Layer, LayerKind};
use crate::fiabilite_fixture::{arbre, charger, dossier, ecrire_json, lire_json, projet_complet, projet_enregistre, racine, Lcg};
use crate::project;
use crate::raster::{premultiply, TILE};

// ------------------------------------------------------------------ le projet d'essai est complet

/// Le projet d'essai contient bien tout ce qu'il dit contenir : s'il maigrit, les essais
/// d'aller-retour ne protègent plus rien.
#[test]
fn the_fixture_really_holds_everything_the_project_can_store() {
    use crate::fx::EffectKind;
    let travail = dossier("complet");
    let f = projet_complet(&travail);
    let d = &f.doc;
    let calques: Vec<&Layer> = d.panels.iter().flat_map(|p| p.layers.iter()).collect();
    assert!(d.sequences.len() >= 2 && d.panels.len() >= 5);
    assert!(calques.iter().any(|l| l.kind == LayerKind::Bitmap) && calques.iter().any(|l| l.kind == LayerKind::Vector));
    assert!(calques.iter().any(|l| l.video.is_some()), "calque vidéo");
    assert!(calques.iter().any(|l| l.ctg.is_some()), "calque CTG");
    assert!(calques.iter().any(|l| l.folder) && calques.iter().any(|l| l.parent.is_some()), "dossier et calque rangé");
    assert!(calques.iter().any(|l| l.fond.is_some()), "fond sous le trait (calque)");
    assert!(d.cels.values().any(|c| c.fond.as_ref().is_some_and(|s| !s.is_empty())), "strate de fond peinte (dessin)");
    for kind in [EffectKind::Boil, EffectKind::GaussianBlur, EffectKind::HueSaturation] {
        assert!(calques.iter().flat_map(|l| &l.fx).any(|e| e.kind == kind && !e.keys.is_empty()), "effet {kind:?} avec clés");
    }
    assert!(calques.iter().flat_map(|l| &l.fx).any(|e| !e.enabled), "un effet éteint");
    assert!(calques.iter().any(|l| l.blend != crate::blend::BlendMode::Normal), "mode de fusion");
    assert!(
        d.cels.values().filter_map(|c| c.vector.as_ref()).flat_map(|v| &v.elements).any(|e| matches!(e, crate::vector::Element::Stroke { brush: Some(_), trace, .. } if !trace.is_empty())),
        "trait texturé avec sa trace"
    );
    assert!(d.panels.iter().any(|p| p.camera.len() > 5), "clés de caméra, avec un effet de caméra posé");
    assert!(d.panels.iter().any(|p| p.ref3d.is_some()) && !d.models.is_empty(), "repère 3D");
    assert!(d.scenes3d.plans.values().any(|p| p.objects.len() >= 8 && p.objects.iter().any(|o| o.group.is_some())) && !d.scenes3d.decors.is_empty(), "scène 3D de blocage avec son décor");
    assert!(d.numbering_locked && d.scenes.iter().any(|s| s.number == "2A") && d.sequences.iter().any(|s| s.number == "1A"), "numérotation verrouillée, avec un plan 2A et une séquence 1A");
    assert!(d.panels.iter().any(|p| !p.marks.is_empty()), "marques d'image");
    assert!(d.panels.iter().any(|p| p.color.is_some()), "couleur de plan");
    assert!(d.panels.iter().any(|p| !p.cues.bookmarks.is_empty() && !p.cues.notes.is_empty()), "signets et notes dessinées");
    let rangés: usize = d.reserves.values().map(|r| r.head.comments.len() + r.tail.comments.len()).sum();
    assert!(d.comments.len() >= 3 && rangés >= 2, "commentaires, dont deux rangés dans la réserve d'un plan rogné");
    assert!(d.reserves.values().any(|r| r.head.len > 0 && r.tail.len > 0), "plan rogné des deux côtés, avec sa réserve");
    assert!(!d.sounds.is_empty() && d.audio_tracks.len() >= 2, "son sur deux pistes");
    assert!(d.audio_tracks.iter().flat_map(|t| &t.clips).any(|c| !c.fx.is_default() && !c.fx.keys.is_empty() && !c.fx.is_dry()) && d.audio_tracks.iter().any(|t| t.solo), "effets des sons et solo");
    assert!(d.transitions.len() >= 4 && d.transitions.iter().any(|t| t.out.is_none()) && d.transitions.iter().any(|t| t.inn.is_none()), "transitions : au bord du film et entre deux plans");
    assert!(!d.sons_proposes.indications.is_empty() && d.sons_proposes.envoi_en_ligne_confirme, "sons proposés (suggestions sonores)");
    assert!(
        d.references.len() >= 2 && d.references.iter().any(|r| r.is_video()) && d.references.iter().any(|r| !r.plans.is_empty() && !r.tags.is_empty()),
        "références visuelles : une image étiquetée et liée, une vidéo"
    );
    assert!(
        d.crayonnes.cases.len() >= 24 && d.crayonnes.cases.iter().any(|c| c.has_ink()) && d.crayonnes.cases.iter().any(|c| c.has_paint() && c.used) && d.crayonnes.cases.iter().any(|c| !c.caption.is_empty() && !c.has_drawing()),
        "planche de crayonnés : traits, case peinte et cochée, légende seule"
    );
    assert!(!d.guides.is_empty() && !d.board.strokes.is_empty() && !d.board.texts.is_empty(), "guides et notes de la planche");
    assert!(d.storyboard_locked && d.start_frame != 1 && d.master_volume != 1.0, "réglages du projet");
    let _ = std::fs::remove_dir_all(&travail);
}

/// La comparaison remarque un écart (un contrôle qui ne voit rien ne protège de rien).
#[test]
fn the_comparison_notices_a_difference() {
    use crate::fiabilite_fixture::{instantane, meme_projet};
    type Changement = Box<dyn Fn(&mut Document)>;
    let travail = dossier("comparaison");
    let changements: Vec<(&str, Changement)> = vec![
        ("nom d'un calque", Box::new(|d| d.panels[0].layers[0].name.push('!'))),
        ("opacité", Box::new(|d| d.panels[0].layers[2].opacity = 0.79)),
        ("clé de caméra", Box::new(|d| d.panels[0].camera[0].cam.x += 1.0)),
        ("commentaire", Box::new(|d| d.comments[0].text.push('?'))),
        (
            "pixel",
            Box::new(|d| {
                let id = d.panels[0].layers[0].frames[0].expect("dessin");
                let c = d.cels.get_mut(&id).expect("dessin");
                let i = c.raster.tile_index_of(5, 5);
                c.raster.tile_mut(i)[5 * TILE + 5] = premultiply(1, 2, 3, 255);
            }),
        ),
        ("réserve", Box::new(|d| d.reserves.values_mut().next().expect("réserve").head.len += 1)),
        ("propriété", Box::new(|d| d.properties.copyright.clear())),
        ("effet", Box::new(|d| d.panels[0].layers[2].fx[0].values[0] += 1.0)),
        ("référence", Box::new(|d| d.references[0].rotation += 1.0)),
        ("lecture d'une vidéo de référence", Box::new(|d| d.references[1].video.as_mut().expect("vidéo").speed += 0.25)),
        ("son proposé", Box::new(|d| d.sons_proposes.indications[0].description.push('!'))),
        ("légende d'une case de crayonné", Box::new(|d| d.crayonnes.cases[0].caption.push('!'))),
        ("coche d'une case de crayonné", Box::new(|d| d.crayonnes.cases[13].used = true)),
    ];
    let mut f = projet_complet(&travail);
    let sans = f.sans_pixels.clone();
    let base = instantane(&f.doc, &sans);
    assert!(meme_projet(&base, &instantane(&f.doc, &sans)).is_ok());
    for (nom, changer) in changements {
        changer(&mut f.doc);
        assert!(meme_projet(&base, &instantane(&f.doc, &sans)).is_err(), "le changement « {nom} » n'est pas vu par la comparaison");
        // (Le changement reste : chacun s'ajoute au précédent, la comparaison doit voir chaque fois un écart.)
    }
    let _ = std::fs::remove_dir_all(&travail);
}

// ------------------------------------------------------------------ versions plus récentes

#[test]
fn a_newer_project_is_refused_with_a_clear_message_and_nothing_is_touched() {
    let (_f, manifeste) = projet_enregistre("plus-recent");
    let mut m = lire_json(&manifeste);
    m["version"] = Value::from(project::VERSION + 1);
    ecrire_json(&manifeste, &m);
    let avant = arbre(racine(&manifeste));
    let err = match project::load(&manifeste) {
        Err(e) => e,
        Ok(_) => panic!("un projet d'une version plus récente ne doit pas s'ouvrir"),
    };
    assert!(err.contains("plus récente") && err.contains(&(project::VERSION + 1).to_string()), "{err}");
    assert_eq!(arbre(racine(&manifeste)), avant, "ouvrir un projet refusé ne touche à aucun fichier");

    // Par l'application : un message dans la barre d'état, le projet en cours reste en place.
    let mut b = crate::perf_tests::banc(Document::with_frame(160, 90, 24), false);
    b.app.open_path(manifeste.clone());
    let t = std::time::Instant::now();
    while b.app.open_job.is_some() && t.elapsed() < Duration::from_secs(30) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(b.app.status.starts_with("Impossible d'ouvrir le projet") && b.app.status.contains("plus récente"), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), 1, "le projet en cours reste en place");
    assert!(b.app.path.is_none());
    assert_eq!(arbre(racine(&manifeste)), avant);
}

// ------------------------------------------------------------------ essais à la louche

/// Tous les points de `v` (adresses JSON, `Pointer`) : feuilles et conteneurs.
pub(crate) fn adresses(v: &Value, ici: &str, out: &mut Vec<String>) {
    out.push(ici.to_string());
    match v {
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                adresses(x, &format!("{ici}/{i}"), out);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                adresses(x, &format!("{ici}/{}", k.replace('~', "~0").replace('/', "~1")), out);
            }
        }
        _ => {}
    }
}

/// Change un point de `v` au hasard. Renvoie ce qui a été fait (pour le message d'un échec).
pub(crate) fn muter(v: &mut Value, adresse: &str, rng: &mut Lcg) -> String {
    let rares = [-1.0, 0.0, 0.5, 1.0, 7.0, 255.0, 65_536.0, 4_294_967_295.0, 1e12, -1e12, 3.4e38];
    let (parent, cle) = adresse.rsplit_once('/').unwrap_or(("", ""));
    let choix = rng.sous(7);
    let action = match choix {
        0 => {
            let n = rares[rng.sous(rares.len())];
            if let Some(x) = v.pointer_mut(adresse) {
                *x = serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number);
            }
            format!("valeur numérique {n}")
        }
        1 => {
            if let Some(x) = v.pointer_mut(adresse) {
                *x = Value::Null;
            }
            "null".to_string()
        }
        2 => {
            let t = ["", "x", "../../x", "\u{0}", &"é".repeat(5_000)][rng.sous(5)].to_string();
            if let Some(x) = v.pointer_mut(adresse) {
                *x = Value::String(t.clone());
            }
            format!("texte de {} caractères", t.chars().count())
        }
        3 => {
            match v.pointer_mut(parent) {
                Some(Value::Object(o)) => {
                    o.remove(&cle.replace("~1", "/").replace("~0", "~"));
                }
                Some(Value::Array(a)) => {
                    if let Ok(i) = cle.parse::<usize>() {
                        if i < a.len() {
                            a.remove(i);
                        }
                    }
                }
                _ => {}
            }
            "retiré".to_string()
        }
        4 => {
            if let Some(Value::Array(a)) = v.pointer_mut(parent) {
                if let Ok(i) = cle.parse::<usize>() {
                    if i < a.len() {
                        let x = a[i].clone();
                        a.insert(i, x);
                    }
                }
            }
            "dupliqué".to_string()
        }
        5 => {
            if let Some(x) = v.pointer_mut(adresse) {
                *x = match x {
                    Value::Array(_) => Value::Array(Vec::new()),
                    Value::Object(_) => Value::Object(Default::default()),
                    Value::String(_) => Value::String(String::new()),
                    Value::Bool(b) => Value::Bool(!*b),
                    _ => Value::from(0),
                };
            }
            "vidé".to_string()
        }
        _ => {
            if let Some(x) = v.pointer_mut(adresse) {
                *x = Value::Bool(true);
            }
            "vrai".to_string()
        }
    };
    format!("{adresse} : {action}")
}

/// `n` lectures du projet enregistré dans `manifeste` dont `fichier` (relatif au dossier du projet) a
/// été changé au hasard : ni plantage ni blocage ; le dossier n'est touché que par ce changement.
pub(crate) fn muter_fichier(manifeste: &std::path::Path, fichier: &str, n: usize, graine: u64) {
    let chemin: PathBuf = racine(manifeste).join(fichier);
    let original: Value = lire_json(&chemin);
    let mut toutes = Vec::new();
    adresses(&original, "", &mut toutes);
    let avant = arbre(racine(manifeste));
    let mut rng = Lcg(graine);
    let mut resultats: BTreeMap<&str, usize> = BTreeMap::new();
    for k in 0..n {
        let mut v = original.clone();
        // Un à trois changements à la fois.
        let mut faits = Vec::new();
        for _ in 0..1 + rng.sous(3) {
            let adresse = toutes[rng.sous(toutes.len())].clone();
            faits.push(muter(&mut v, &adresse, &mut rng));
        }
        ecrire_json(&chemin, &v);
        match charger(manifeste, Duration::from_secs(60)) {
            Ok(Ok(_)) => *resultats.entry("ouvert").or_default() += 1,
            Ok(Err(_)) => *resultats.entry("refusé avec un message").or_default() += 1,
            Err(probleme) => panic!("{fichier}, essai {k} : la lecture {probleme}. Changements : {faits:#?}"),
        }
    }
    // Le dossier n'a changé que par le fichier muté.
    ecrire_json(&chemin, &original);
    let mut apres = arbre(racine(manifeste));
    let mut voulu = avant;
    apres.remove(fichier);
    voulu.remove(fichier);
    assert_eq!(apres, voulu, "{fichier} : lire des fichiers abîmés a changé le dossier du projet");
    println!("FIABILITE {fichier} : {n} lectures de fichiers changés au hasard : {resultats:?}");
}

#[test]
fn a_manifest_with_random_values_changed_never_crashes_the_reader() {
    let (_f, manifeste) = projet_enregistre("fuzz-manifeste");
    muter_fichier(&manifeste, "Film.pelure", 90, 11);
}

#[test]
fn sidecars_with_random_values_changed_never_crash_the_reader() {
    let (_f, manifeste) = projet_enregistre("fuzz-annexes");
    muter_fichier(&manifeste, crate::commentaires::FILE, 40, 12);
    muter_fichier(&manifeste, crate::rognage::FILE, 40, 13);
    muter_fichier(&manifeste, crate::scene3dstore::FILE, 40, 14);
}

/// Coupe le manifeste en 120 endroits : toujours un refus clair (jamais un plantage).
#[test]
fn a_truncated_manifest_is_always_a_clear_refusal() {
    let (_f, manifeste) = projet_enregistre("troncatures");
    let original = std::fs::read(&manifeste).expect("manifeste");
    let mut rng = Lcg(5);
    let mut coupes: Vec<usize> = vec![0, 1, 2, original.len() / 2, original.len() - 1, original.len() - 2];
    coupes.extend((0..114).map(|_| rng.sous(original.len())));
    for coupe in coupes {
        std::fs::write(&manifeste, &original[..coupe]).expect("écriture");
        match charger(&manifeste, Duration::from_secs(30)) {
            Ok(Err(m)) => assert!(m.contains("manifeste illisible"), "coupé à {coupe} octets : « {m} »"),
            Ok(Ok(_)) => panic!("un manifeste coupé à {coupe} octets s'est ouvert"),
            Err(p) => panic!("manifeste coupé à {coupe} octets : la lecture {p}"),
        }
    }
    // Les fichiers annexes coupés n'empêchent pas d'ouvrir le projet.
    std::fs::write(&manifeste, &original).expect("écriture");
    for fichier in [crate::commentaires::FILE, crate::rognage::FILE, crate::scene3dstore::FILE] {
        let chemin = racine(&manifeste).join(fichier);
        let octets = std::fs::read(&chemin).expect("annexe");
        for _ in 0..25 {
            let coupe = rng.sous(octets.len());
            std::fs::write(&chemin, &octets[..coupe]).expect("écriture");
            match charger(&manifeste, Duration::from_secs(30)) {
                Ok(Ok(l)) => assert!(l.warnings.iter().any(|w| w.contains(fichier)), "{fichier} coupé à {coupe} octets : aucun avertissement"),
                Ok(Err(m)) => panic!("{fichier} coupé à {coupe} octets : le projet est refusé (« {m} »)"),
                Err(p) => panic!("{fichier} coupé à {coupe} octets : la lecture {p}"),
            }
        }
        std::fs::write(&chemin, &octets).expect("écriture");
    }
}
