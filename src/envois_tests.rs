//! Envois (`envois.rs`, `envoisui.rs`) : le fichier et ses protections, l'empreinte des plans (ce qui
//! compte comme un changement, et ce qui n'en est pas un), les instantanés (écrits une fois, relus tels
//! quels), le suivi des plans modifiés dans l'application.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui::Color32;

use crate::camera::{Camera, CameraKey};
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::envois::{self, Copie, Demande, Envoi, Etat, NouvelEnvoi, PlanCopie, PlanEnvoye};
use crate::fiabilite_fixture::{arbre, dossier, projet_complet};
use crate::fx::{Effect, EffectKind};
use crate::perf_tests::{banc, se_reposer, Banc};
use crate::project::{self, Tete};

fn rouge(r: u8) -> Color32 {
    Color32::from_rgba_premultiplied(r, 10, 10, 255)
}

/// Un film de `n` plans de 4 images : un calque bitmap, deux dessins par plan, chacun d'une couleur
/// qui dépend du plan.
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

/// Ce que `construire` reçoit pour tout `doc` (et pour un envoi, si `envoi`).
fn demande(doc: &Document, envoi: Option<NouvelEnvoi>) -> Demande {
    let plans: Vec<PlanCopie> = doc.panels.iter().enumerate().map(|(i, p)| PlanCopie { panel: p.clone(), titre: format!("Plan {}", i + 1), version: 1, nom: String::new(), sequence: String::new() }).collect();
    let mut cels = HashMap::new();
    for p in &doc.panels {
        for id in crate::empreinte::dessins_du_plan(p) {
            if let Some(c) = doc.cels.get(&id) {
                cels.insert(id, Copie { version: c.version, donnees: crate::empreinte::Donnees::de(c) });
            }
        }
    }
    Demande { plans, cels, connus: HashMap::new(), tete: Tete::de(doc), envoi }
}

/// Plan → empreinte de son contenu.
fn empreintes(doc: &Document) -> HashMap<u64, u64> {
    envois::construire(demande(doc, None)).expect("calcul").plans.into_iter().collect()
}

fn nouvel(id: u64, nom: &str) -> NouvelEnvoi {
    NouvelEnvoi { id, nom: nom.into(), date: 1_760_000_000, note_le: 1_760_000_100, unique: 0xABCD_0000 + id as u32 }
}

/// Marque un envoi sur `doc` (comme l'application, sans l'interface) et rend son numéro.
fn marquer(doc: &mut Document, nom: &str) -> u64 {
    let id = doc.envois.prochain_numero();
    let r = envois::construire(demande(doc, Some(nouvel(id, nom)))).expect("envoi");
    let (envoi, instantane) = r.envoi.expect("l'envoi");
    doc.envois.ajouter(envoi, instantane);
    id
}

fn manifeste(dir: &Path) -> PathBuf {
    dir.join("Film").join("Film.pelure")
}

fn enregistrer(doc: &Document, manifeste: &Path, written: &mut HashMap<CelId, u64>) {
    let job = project::prepare_save(doc, manifeste, written, None).expect("préparation");
    job.run().expect("écriture");
}

fn fichiers_de(dir: &Path) -> Vec<String> {
    arbre(dir).into_keys().collect()
}

// ------------------------------------------------------------------------------------ dates

#[test]
fn dates_go_to_days_and_back() {
    for (y, m, d) in [(1970, 1, 1), (2000, 2, 29), (2024, 2, 29), (2026, 10, 20), (2099, 12, 31)] {
        let days = envois::jours_depuis_1970(y, m, d);
        assert_eq!(crate::projetui::civil_from_days(days), (y, m, d), "{y}-{m}-{d}");
    }
    let t = envois::lire_date("20/10/2026").expect("date");
    assert_eq!(envois::date_courte(t), "20/10/2026");
    assert_eq!(envois::lire_date("2026-10-20"), Some(t));
    assert_eq!(envois::lire_date(" 20.10.2026 "), Some(t));
    for mauvaise in ["", "demain", "31/02/2026", "29/02/2025", "0/1/2026", "12/13/2026", "12/10", "12/10/1969", "1/2/3/4"] {
        assert_eq!(envois::lire_date(mauvaise), None, "« {mauvaise} »");
    }
    assert!(envois::lire_date("29/02/2024").is_some());
}

// ------------------------------------------------------------------------------------ le fichier

fn exemple() -> Vec<Envoi> {
    vec![
        Envoi {
            id: 1,
            nom: "v1 au réal".into(),
            date: 1_759_000_000,
            note_le: 1_759_000_050,
            fichier: "envoi-1-0000abcd.json".into(),
            plans: vec![PlanEnvoye { plan: 4, titre: "Plan 1".into(), empreinte: u64::MAX - 5, version: 1, images: 24, nom: String::new(), sequence: String::new() }, PlanEnvoye { plan: 9, titre: "Plan 2".into(), empreinte: 0x1234_5678_9abc_def0, version: 3, images: 12, nom: String::new(), sequence: String::new() }],
            variantes: vec![],
        },
        Envoi { id: 2, nom: "v2 au réal".into(), date: 1_760_000_000, note_le: 0, fichier: String::new(), plans: vec![], variantes: vec![crate::envois::VarianteEnvoyee { sequence: 3, numero: 2, nom: "serré".into() }] },
    ]
}

#[test]
fn the_envois_file_round_trips_with_big_numbers_intact() {
    let mut doc = film(2);
    doc.envois.list = exemple();
    let octets = envois::to_bytes(&doc).expect("écriture").expect("un fichier");
    let texte = String::from_utf8(octets).expect("texte");
    // Les empreintes s'écrivent en hexadécimal : JSON ne garde pas sûrement les entiers au-delà de 2^53.
    assert!(texte.contains("fffffffffffffffa") && texte.contains("123456789abcdef0"), "{texte}");
    assert_eq!(envois::parse(&texte).expect("relecture"), exemple());
    // Pas d'envoi : pas de fichier.
    doc.envois.list.clear();
    assert_eq!(envois::to_bytes(&doc).expect("écriture"), None);
}

#[test]
fn a_hand_damaged_file_is_cleaned_not_trusted() {
    let mut e = exemple();
    e[0].fichier = "..\\..\\evil.json".into();
    e[1].id = 1;
    e[1].nom = "x".repeat(5000);
    let mut doc = film(1);
    doc.envois.list = e;
    let texte = String::from_utf8(envois::to_bytes(&doc).unwrap().unwrap()).unwrap();
    let lu = envois::parse(&texte).expect("lisible");
    assert_eq!(lu.len(), 2);
    assert!(lu[0].fichier.is_empty(), "un nom qui sort du dossier est oublié : {:?}", lu[0].fichier);
    assert_ne!(lu[0].id, lu[1].id, "un numéro en double en reçoit un neuf");
    assert!(lu.iter().all(|e| e.id != 0));
    assert_eq!(lu[1].nom.chars().count(), 120, "un nom démesuré est coupé");
    // Pas un fichier d'envois, ou d'une version plus récente : refusé, avec des mots clairs.
    assert!(envois::parse("{\"format\":\"autre\",\"version\":1}").unwrap_err().contains("pas un fichier d'envois"));
    assert!(envois::parse("{\"format\":\"shot-envois\",\"version\":99}").unwrap_err().contains("version plus récente"));
    assert!(envois::parse("[1,2").unwrap_err().contains("illisible"));
    assert!(envois::parse("\u{feff}{\"format\":\"shot-envois\",\"version\":1}").unwrap().is_empty(), "un BOM ne gêne pas");
}

#[test]
fn a_project_without_envois_is_exactly_as_before() {
    let travail = dossier("envois-sans");
    let doc = film(3);
    let m = manifeste(&travail);
    enregistrer(&doc, &m, &mut Default::default());
    let fichiers = fichiers_de(m.parent().unwrap());
    assert!(!fichiers.iter().any(|f| f.contains("envois") || f.contains("versions")), "{fichiers:?}");
    let ouvert = project::load(&m).expect("ouverture");
    assert!(ouvert.warnings.is_empty(), "{:?}", ouvert.warnings);
    assert!(ouvert.doc.envois.is_empty());
    // Réenregistrer sur place ne change rien.
    let avant = arbre(m.parent().unwrap());
    enregistrer(&ouvert.doc, &m, &mut ouvert.written.clone());
    assert_eq!(arbre(m.parent().unwrap()), avant);
}

/// Un projet dont `envois.json` contient `contenu` : il s'ouvre sans envois, l'enregistrer n'y touche pas,
/// et marquer un envoi met l'ancien fichier de côté au lieu de le perdre.
fn fichier_d_envois_illisible(nom: &str, contenu: &str) {
    let travail = dossier(&format!("envois-illisible-{nom}"));
    let m = manifeste(&travail);
    let dir = m.parent().unwrap().to_path_buf();
    enregistrer(&film(2), &m, &mut Default::default());
    std::fs::write(dir.join(envois::FILE), contenu).unwrap();
    let ouvert = project::load(&m).expect("le projet s'ouvre quand même");
    assert!(ouvert.warnings.iter().any(|w| w.contains("envois.json") && w.contains("ne sont pas chargés")), "{nom} : {:?}", ouvert.warnings);
    assert!(ouvert.doc.envois.is_empty());
    let written = ouvert.written.clone();
    // Enregistrer sans envoi : le fichier reste tel quel.
    enregistrer(&ouvert.doc, &m, &mut written.clone());
    assert_eq!(std::fs::read_to_string(dir.join(envois::FILE)).unwrap(), contenu, "{nom} : le fichier est intact");
    // Marquer un envoi puis enregistrer : l'ancien est mis de côté, jamais perdu.
    let mut neuf = ouvert.doc;
    marquer(&mut neuf, "v1");
    enregistrer(&neuf, &m, &mut written.clone());
    let aside: Vec<String> = fichiers_de(&dir).into_iter().filter(|f| f.starts_with("envois.json.illisible")).collect();
    assert!(!aside.is_empty(), "{nom} : mis de côté ({:?})", fichiers_de(&dir));
    assert!(aside.iter().any(|f| std::fs::read_to_string(dir.join(f)).unwrap() == contenu), "{nom} : le contenu d'avant est dans le fichier mis de côté");
    assert_eq!(project::load(&m).unwrap().doc.envois.list.len(), 1);
}

#[test]
fn a_damaged_envois_file_is_never_deleted() {
    fichier_d_envois_illisible("abime", "{\"format\":\"shot-envois\",\"version\":1,\"envois\":[{\"id\":");
    fichier_d_envois_illisible("octets", "\u{1}\u{2} pas du texte {{{");
}

#[test]
fn a_newer_envois_file_is_never_deleted() {
    fichier_d_envois_illisible("recent", "{\"format\":\"shot-envois\",\"version\":42,\"envois\":[]}");
}

// ------------------------------------------------------------------------------------ l'empreinte

#[test]
fn a_blank_tile_is_the_same_as_no_tile_and_one_pixel_is_a_difference() {
    let doc = film(1);
    let mut autre = film(1);
    let id = autre.panels[0].layers[0].frames[0].unwrap();
    // Une tuile allouée mais vide (une gomme en laisse) ne change rien à ce qu'on voit.
    autre.cels.get_mut(&id).unwrap().raster.tile_mut(30);
    assert_eq!(empreintes(&doc), empreintes(&autre));
    // Un pixel de plus : un autre plan.
    autre.cels.get_mut(&id).unwrap().raster.tile_mut(31)[5] = rouge(99);
    assert_ne!(empreintes(&doc), empreintes(&autre));
}

#[test]
fn fingerprints_survive_the_png_round_trip() {
    // Le PNG garde la couleur non prémultipliée : l'empreinte d'un dessin enregistré puis rouvert doit
    // être celle du dessin en mémoire, même avec des pixels translucides de toutes sortes.
    let mut c = Cel::new(130, 70, LayerKind::Bitmap);
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    for y in 0..70 {
        for x in 0..130 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let a = (s >> 8) as u8;
            let m = |v: u64| ((v as u32 * a as u32 + 127) / 255) as u8;
            let t = c.raster.tile_index_of(x, y);
            c.raster.tile_mut(t)[(y % 64) * 64 + x % 64] = Color32::from_rgba_premultiplied(m(s >> 16 & 255), m(s >> 24 & 255), m(s >> 32 & 255), a);
        }
    }
    let avant = crate::empreinte::Donnees::de(&c).empreinte();
    let png = crate::io::encode_png(&c.raster.to_rgba(), 130, 70).unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    let relu = Cel::bitmap(crate::raster::Raster::from_rgba(130, 70, image.as_raw()));
    assert_eq!(crate::empreinte::Donnees::de(&relu).empreinte(), avant);
}

#[test]
fn the_digests_of_a_complete_project_survive_save_and_reopen() {
    let travail = dossier("envois-complet");
    let f = projet_complet(&travail);
    let avant = empreintes(&f.doc);
    assert!(avant.len() >= 5);
    let m = manifeste(&travail);
    project::save(&f.doc, &m, &mut Default::default()).expect("enregistrement");
    for (nom, ouvert) in [("lecture complète", project::load(&m).expect("lecture")), ("lecture rapide", project::load_quick(&m).expect("lecture rapide"))] {
        assert_eq!(empreintes(&ouvert.doc), avant, "{nom} : un projet rouvert ne doit pas paraître modifié");
    }
}

/// L'empreinte du plan 0 de `doc` après `changer`.
fn apres(changer: impl FnOnce(&mut Document)) -> u64 {
    let mut doc = film(2);
    changer(&mut doc);
    empreintes(&doc)[&doc.panels[0].id]
}

#[test]
fn what_counts_as_a_change_and_what_does_not() {
    let base = apres(|_| {});
    assert_eq!(base, apres(|_| {}), "deux calculs du même plan");
    let cel0 = |d: &Document| d.panels[0].layers[0].frames[0].unwrap();
    // Ce qui se voit ou dure : un trait, l'opacité, la visibilité, un effet, une clé de caméra, la durée,
    // le texte du storyboard, une marque d'image, le décalage d'un calque, l'ordre des images.
    let changements: Vec<(&str, Box<dyn FnOnce(&mut Document)>)> = vec![
        ("un trait", Box::new(move |d| {
            let id = cel0(d);
            d.cels.get_mut(&id).unwrap().raster.tile_mut(9)[3] = rouge(200);
        })),
        ("l'opacité d'un calque", Box::new(|d| d.panels[0].layers[0].opacity = 0.5)),
        ("la visibilité d'un calque", Box::new(|d| d.panels[0].layers[0].visible = false)),
        ("un effet", Box::new(|d| d.panels[0].layers[0].fx.push(Effect::new(EffectKind::GaussianBlur)))),
        ("une clé de caméra", Box::new(|d| d.panels[0].camera.push(CameraKey::new(0, Camera { x: 100.0, y: 50.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 })))),
        ("la durée", Box::new(|d| d.panels[0].set_length(7))),
        ("le texte d'action", Box::new(|d| d.panels[0].action = "Elle entre.".into())),
        ("le dialogue", Box::new(|d| d.panels[0].dialogue = "— Bonjour.".into())),
        ("une marque d'image", Box::new(|d| {
            d.toggle_mark(0, 2);
        })),
        ("le décalage d'un calque", Box::new(|d| d.panels[0].layers[0].transform.x = 12.0)),
        ("l'exposition", Box::new(|d| {
            let l = &mut d.panels[0].layers[0];
            l.frames.swap(1, 2);
        })),
        ("la profondeur", Box::new(|d| d.panels[0].layers[0].depth = 0.5)),
        ("un calque de plus", Box::new(|d| d.panels[0].layers.push(Layer::new("Autre", LayerKind::Bitmap)))),
    ];
    for (quoi, f) in changements {
        assert_ne!(apres(f), base, "{quoi} doit compter comme un changement");
    }
    // Ce qui aide à travailler sans changer le film : la couleur du plan, un signet, le nom ou le verrou
    // d'un calque, un calque replié, le groupe de couleur, la pelure d'oignon, un nom d'instance.
    let neutres: Vec<(&str, Box<dyn FnOnce(&mut Document)>)> = vec![
        ("la couleur du plan", Box::new(|d| d.panels[0].color = Some(2))),
        ("un signet", Box::new(|d| d.panels[0].cues.bookmarks.push(2))),
        ("le point d'entrée", Box::new(|d| d.panels[0].cues.in_point = Some(1))),
        ("le nom d'un calque", Box::new(|d| d.panels[0].layers[0].name = "Encrage".into())),
        ("le verrou d'un calque", Box::new(|d| d.panels[0].layers[0].locked = true)),
        ("un calque replié", Box::new(|d| d.panels[0].layers[0].collapsed = true)),
        ("le groupe de couleur", Box::new(|d| d.panels[0].layers[0].group = 2)),
        ("la pelure d'oignon", Box::new(|d| d.panels[0].layers[0].onion = true)),
        ("un nom d'instance", Box::new(move |d| {
            let id = cel0(d);
            d.panels[0].layers[0].names.insert(id, "clé".into());
        })),
        ("la version d'un dessin", Box::new(move |d| {
            let id = cel0(d);
            d.cels.get_mut(&id).unwrap().version += 5;
        })),
    ];
    for (quoi, f) in neutres {
        assert_eq!(apres(f), base, "{quoi} ne doit pas compter comme un changement");
    }
    // Un autre plan ne change pas l'empreinte de celui-ci.
    let mut doc = film(2);
    let avant = empreintes(&doc);
    doc.panels[1].layers[0].opacity = 0.3;
    let apres = empreintes(&doc);
    assert_eq!(avant[&doc.panels[0].id], apres[&doc.panels[0].id]);
    assert_ne!(avant[&doc.panels[1].id], apres[&doc.panels[1].id]);
}

#[test]
fn two_drawings_with_the_same_content_are_the_same_plan() {
    // Un dessin dupliqué puis posé à la place de l'original : même image, même plan.
    let mut doc = film(1);
    let avant = empreintes(&doc);
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    let copie = doc.duplicate_cel(id);
    for f in doc.panels[0].layers[0].frames.iter_mut().filter(|f| **f == Some(id)) {
        *f = Some(copie);
    }
    assert_eq!(empreintes(&doc), avant);
}

// ------------------------------------------------------------------------------------ les instantanés

fn dessins_ecrits(dir: &Path) -> Vec<String> {
    fichiers_de(dir).into_iter().filter(|f| f.starts_with("versions/dessins/")).collect()
}

#[test]
fn an_envoi_is_written_once_and_unchanged_drawings_are_not_written_again() {
    let travail = dossier("envois-ecriture");
    let m = manifeste(&travail);
    let dir = m.parent().unwrap().to_path_buf();
    let mut doc = film(3);
    let mut written = HashMap::new();
    marquer(&mut doc, "v1 au réal");
    enregistrer(&doc, &m, &mut written);
    let dessins1 = dessins_ecrits(&dir);
    assert_eq!(dessins1.len(), 6, "trois plans, deux dessins : {dessins1:?}");
    let fichiers = fichiers_de(&dir);
    assert!(fichiers.contains(&"envois.json".to_string()));
    assert_eq!(fichiers.iter().filter(|f| f.starts_with("versions/envoi-1-")).count(), 1);
    // Le même contenu, un deuxième envoi : aucun dessin de plus.
    doc.envois.liberer(&[1]);
    marquer(&mut doc, "v2 au réal");
    enregistrer(&doc, &m, &mut written);
    assert_eq!(dessins_ecrits(&dir), dessins1, "rien n'a changé : les mêmes dessins");
    assert_eq!(fichiers_de(&dir).iter().filter(|f| f.starts_with("versions/envoi-")).count(), 2);
    // Un dessin retouché : un seul de plus.
    doc.envois.liberer(&[2]);
    let id = doc.panels[1].layers[0].frames[0].unwrap();
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.raster.tile_mut(12)[7] = rouge(222);
    cel.version += 1;
    marquer(&mut doc, "v3 au réal");
    enregistrer(&doc, &m, &mut written);
    assert_eq!(dessins_ecrits(&dir).len(), 7);
    // Les anciens fichiers sont intacts (jamais réécrits ni effacés).
    let apres = arbre(&dir);
    for f in &dessins1 {
        assert!(apres.contains_key(f), "{f}");
    }
    let lu = project::load(&m).expect("ouverture");
    assert!(lu.warnings.is_empty(), "{:?}", lu.warnings);
    assert_eq!(lu.doc.envois.list.len(), 3);
    assert_eq!(lu.doc.envois.list[2].nom, "v3 au réal");
}

#[test]
fn a_snapshot_is_read_back_exactly_as_it_was() {
    let travail = dossier("envois-relecture");
    let m = manifeste(&travail);
    let dir = m.parent().unwrap().to_path_buf();
    let mut doc = film(3);
    let avant = empreintes(&doc);
    let envoi = marquer(&mut doc, "v1 au réal");
    // Le film change ensuite : un trait, un effet, la durée.
    let id = doc.panels[0].layers[0].frames[0].unwrap();
    doc.cels.get_mut(&id).unwrap().raster.tile_mut(20)[1] = rouge(77);
    doc.cels.get_mut(&id).unwrap().version += 1;
    doc.panels[1].layers[0].fx.push(Effect::new(EffectKind::GaussianBlur));
    doc.panels[2].set_length(9);
    assert_ne!(empreintes(&doc), avant);
    enregistrer(&doc, &m, &mut Default::default());
    let ouvert = project::load(&m).expect("ouverture");
    let e = ouvert.doc.envois.trouve(envoi).expect("l'envoi").clone();
    for p in &ouvert.doc.panels {
        let relu = envois::charger_plan(&dir, &e, p.id);
        let relu = relu.unwrap_or_else(|er| panic!("plan {} : {er}", p.id));
        assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
        assert_eq!(relu.doc.panels.len(), 1);
        // Le plan relu est celui d'alors : même empreinte que celle de l'envoi, et que celle de l'époque.
        let d = empreintes(&relu.doc);
        assert_eq!(d[&p.id], e.plan(p.id).unwrap().empreinte, "plan {}", p.id);
        assert_eq!(d[&p.id], avant[&p.id]);
    }
    // Le plan d'aujourd'hui, lui, a bien changé.
    let aujourd_hui = empreintes(&ouvert.doc);
    assert_ne!(aujourd_hui[&ouvert.doc.panels[0].id], avant[&ouvert.doc.panels[0].id]);
    assert_eq!(aujourd_hui[&ouvert.doc.panels[1].id] == avant[&ouvert.doc.panels[1].id], false);
}

#[test]
fn the_complete_project_snapshot_reads_back_with_every_plan_intact() {
    let travail = dossier("envois-complet-instantane");
    let mut f = projet_complet(&travail);
    let avant = empreintes(&f.doc);
    let envoi = marquer(&mut f.doc, "v1 au réal");
    let m = manifeste(&travail);
    project::save(&f.doc, &m, &mut Default::default()).expect("enregistrement");
    let dir = m.parent().unwrap().to_path_buf();
    let ouvert = project::load(&m).expect("ouverture");
    assert!(ouvert.warnings.is_empty(), "{:?}", ouvert.warnings);
    let e = ouvert.doc.envois.trouve(envoi).expect("l'envoi").clone();
    for p in &ouvert.doc.panels {
        let relu = envois::charger_plan(&dir, &e, p.id).unwrap_or_else(|er| panic!("plan {} : {er}", p.id));
        assert!(relu.warnings.is_empty(), "plan {} : {:?}", p.id, relu.warnings);
        assert_eq!(empreintes(&relu.doc)[&p.id], avant[&p.id], "plan {}", p.id);
    }
}

#[test]
fn save_as_takes_the_snapshots_along() {
    let travail = dossier("envois-enregistrer-sous");
    let m = manifeste(&travail);
    let mut doc = film(2);
    marquer(&mut doc, "v1");
    let mut written = HashMap::new();
    enregistrer(&doc, &m, &mut written);
    doc.envois.liberer(&[1]);
    doc.envois.dossier = m.parent().map(Path::to_path_buf);
    // Ailleurs : un autre dossier (tout est réécrit, `written` est vidé comme le fait `start_save`).
    let autre = travail.join("Copie").join("Copie.pelure");
    written.clear();
    enregistrer(&doc, &autre, &mut written);
    let dir = autre.parent().unwrap();
    assert_eq!(dessins_ecrits(dir).len(), 4, "{:?}", fichiers_de(dir));
    let ouvert = project::load(&autre).expect("ouverture de la copie");
    let e = ouvert.doc.envois.list[0].clone();
    for p in &ouvert.doc.panels {
        envois::charger_plan(dir, &e, p.id).expect("l'instantané est dans la copie");
    }
}

#[test]
fn removing_an_envoi_from_the_list_never_deletes_its_files() {
    let travail = dossier("envois-retrait");
    let m = manifeste(&travail);
    let dir = m.parent().unwrap().to_path_buf();
    let mut doc = film(2);
    let id = marquer(&mut doc, "v1");
    enregistrer(&doc, &m, &mut Default::default());
    let avant = arbre(&dir);
    assert!(doc.envois.retirer(id));
    doc.envois.liberer(&[id]);
    enregistrer(&doc, &m, &mut Default::default());
    let apres = arbre(&dir);
    for (f, h) in &avant {
        if f == "envois.json" {
            continue;
        }
        assert_eq!(apres.get(f), Some(h), "{f} est resté tel quel");
    }
    assert!(project::load(&m).unwrap().doc.envois.is_empty());
}

#[test]
fn a_missing_snapshot_says_so_in_plain_words() {
    let travail = dossier("envois-manquant");
    let m = manifeste(&travail);
    let dir = m.parent().unwrap().to_path_buf();
    let mut doc = film(1);
    marquer(&mut doc, "v1");
    enregistrer(&doc, &m, &mut Default::default());
    let e = doc.envois.list[0].clone();
    let plan = doc.panels[0].id;
    // Un dessin de moins : le plan se relit avec un avertissement.
    let un = dessins_ecrits(&dir).remove(0);
    std::fs::remove_file(dir.join(&un)).unwrap();
    let relu = envois::charger_plan(&dir, &e, plan).expect("le plan se relit");
    assert!(!relu.warnings.is_empty());
    // Le fichier de l'instantané de moins : une phrase qui dit quoi faire.
    std::fs::remove_file(dir.join("versions").join(&e.fichier)).unwrap();
    let err = envois::charger_plan(&dir, &e, plan).err().expect("échec");
    assert!(err.contains("introuvable") && err.contains("v1"), "{err}");
    // Un envoi sans instantané.
    let mut sans = e.clone();
    sans.fichier.clear();
    assert!(envois::charger_plan(&dir, &sans, plan).err().unwrap().contains("rien à comparer"));
    // Un instantané abîmé.
    std::fs::write(dir.join("versions").join(&e.fichier), "{ pas du json").unwrap();
    assert!(envois::charger_plan(&dir, &e, plan).err().unwrap().contains("illisible"));
}

#[test]
fn the_old_projects_of_the_fixture_open_with_no_envois() {
    let travail = dossier("envois-fixture-sans");
    let f = projet_complet(&travail);
    let m = manifeste(&travail);
    project::save(&f.doc, &m, &mut Default::default()).expect("enregistrement");
    let dir = m.parent().unwrap();
    assert!(!dir.join("versions").exists() && !dir.join(envois::FILE).exists());
    assert!(project::load(&m).expect("ouverture").doc.envois.is_empty());
    assert!(project::VERSION >= 27, "les envois ne changent pas le format du projet (la 30 est celle du calque de couleur et de la toile infinie)");
}

// ------------------------------------------------------------------------------------ dans l'application

fn banc_projet(nom: &str, n: usize) -> (Banc, PathBuf) {
    let travail = dossier(nom);
    let m = project::manifeste_shot(&manifeste(&travail));
    let mut b = banc(film(n), false);
    b.app.path = Some(m.clone());
    b.app.modified = true;
    (b, m)
}

/// Des images jusqu'à ce que les fils de fond de l'envoi aient fini (et l'écriture).
fn pomper(b: &mut Banc) {
    for _ in 0..800 {
        b.image(Vec::new(), 0.25);
        if b.app.envois_ui.au_repos() && !b.app.save_en_cours() {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("les envois ne se calment pas");
}

fn marquer_dans_l_appli(b: &mut Banc, nom: &str) {
    let ctx = b.ctx.clone();
    let date = envois::lire_date("20/10/2026").unwrap();
    b.app.envois_marquer(&ctx, nom, date);
    pomper(b);
}

#[test]
fn marking_an_envoi_in_the_application_notes_every_plan_and_saves() {
    let (mut b, m) = banc_projet("envois-appli", 3);
    marquer_dans_l_appli(&mut b, "v1 au réal");
    assert_eq!(b.app.doc.envois.list.len(), 1);
    let e = &b.app.doc.envois.list[0];
    assert_eq!((e.nom.as_str(), e.plans.len()), ("v1 au réal", 3));
    assert!(e.plans.iter().all(|p| p.images == 4 && p.titre.starts_with("Plan")));
    assert!(b.app.doc.envois.pending.is_empty(), "l'enregistrement a libéré les instantanés");
    assert!(!b.app.modified, "le projet est enregistré");
    assert!(m.parent().unwrap().join("envois.json").exists());
    // Rien n'a changé depuis : aucun plan n'est repéré.
    assert!(b.app.plans_modifies().is_empty());
    for p in &b.app.doc.panels {
        assert!(b.app.envoi_repere(p.id).is_none());
    }
    // Il se rouvre avec ses envois, et le suivi repart sans rien repérer.
    let ouvert = project::load(&m).expect("ouverture");
    assert_eq!(ouvert.doc.envois.list.len(), 1);
}

#[test]
fn a_marking_copies_by_slices_and_starts_again_when_the_project_changes() {
    let (mut b, m) = banc_projet("envois-tranches", 3);
    crate::envoisui::MARQUAGE_UN_DESSIN_PAR_IMAGE.with(|c| c.set(true));
    let ctx = b.ctx.clone();
    b.app.envois_marquer(&ctx, "v1 au réal", envois::lire_date("20/10/2026").unwrap());
    // Un seul dessin est copié : le marquage attend les images suivantes, aucun envoi n'est encore dans le projet.
    assert!(b.app.envois_ui.occupe(), "la copie continue d'image en image");
    assert!(b.app.doc.envois.list.is_empty());
    // Le projet change pendant la copie (un plan de plus) : elle recommence, l'instantané montre le film d'un seul instant.
    b.app.structure_op(|d, cur| Some(d.add_scene_after(cur)));
    pomper(&mut b);
    crate::envoisui::MARQUAGE_UN_DESSIN_PAR_IMAGE.with(|c| c.set(false));
    assert_eq!(b.app.doc.envois.list.len(), 1);
    assert_eq!(b.app.doc.envois.list[0].plans.len(), 4, "l'envoi note le film d'après le changement : quatre plans");
    assert!(b.app.doc.envois.pending.is_empty(), "l'enregistrement a libéré l'instantané");
    assert!(m.parent().unwrap().join("envois.json").exists());
}

#[test]
fn changed_plans_are_flagged_after_a_calm_moment_and_unflagged_when_undone() {
    let (mut b, _m) = banc_projet("envois-suivi", 3);
    marquer_dans_l_appli(&mut b, "v1 au réal");
    let (p0, p1) = (b.app.doc.panels[0].id, b.app.doc.panels[1].id);
    // Un trait dans le plan 2 (le plan 1 ne bouge pas), comme le ferait un geste : l'historique garde le retour.
    let id = b.app.doc.panels[1].layers[0].frames[0].unwrap();
    let cel = b.app.doc.cels.get_mut(&id).unwrap();
    cel.raster.tile_mut(5)[2] = rouge(180);
    cel.version += 1;
    // Pendant la première moitié de seconde, rien n'est encore repéré (on attend le calme).
    b.image(Vec::new(), 0.1);
    assert!(b.app.envoi_repere(p1).is_none());
    pomper(&mut b);
    let (etat, phrase) = b.app.envoi_repere(p1).expect("le plan retouché est repéré");
    assert_eq!(etat, Etat::Modifie);
    assert!(phrase.contains("v1 au réal") && phrase.contains("20/10/2026"), "{phrase}");
    assert!(b.app.envoi_repere(p0).is_none());
    assert_eq!(b.app.plans_modifies(), vec![1]);
    // Revenir à l'état de l'envoi : le repère s'en va.
    let cel = b.app.doc.cels.get_mut(&id).unwrap();
    cel.raster.tile_mut(5)[2] = Color32::TRANSPARENT;
    cel.version += 1;
    pomper(&mut b);
    assert!(b.app.envoi_repere(p1).is_none(), "revenu à l'état de l'envoi");
    // Un plan neuf est « ajouté ».
    b.app.doc.add_scene_after(2);
    pomper(&mut b);
    let neuf = b.app.doc.panels[3].id;
    assert_eq!(b.app.envoi_repere(neuf).map(|r| r.0), Some(Etat::Nouveau));
}

#[test]
fn a_reopened_project_shows_no_modified_plan_until_something_changes() {
    let (mut b, m) = banc_projet("envois-rouvert", 3);
    marquer_dans_l_appli(&mut b, "v1 au réal");
    // Ouvrir le projet enregistré, comme le fait l'application : un autre document, un autre projet.
    let ouvert = project::load(&m).expect("ouverture");
    b.app.doc = ouvert.doc;
    b.app.doc_epoch += 1;
    b.app.written = ouvert.written;
    b.app.history.clear();
    pomper(&mut b);
    assert!(b.app.envois_ui.suivi_fait(), "le suivi a regardé le projet rouvert");
    assert!(b.app.plans_modifies().is_empty(), "des dessins relus du disque ne sont pas des dessins modifiés");
    // Le premier trait repère son plan.
    let id = b.app.doc.panels[2].layers[0].frames[0].unwrap();
    let cel = b.app.doc.cels.get_mut(&id).unwrap();
    cel.raster.tile_mut(4)[4] = rouge(111);
    cel.version += 1;
    pomper(&mut b);
    assert_eq!(b.app.plans_modifies(), vec![2]);
}

#[test]
fn the_commands_jump_from_one_modified_plan_to_the_next() {
    let (mut b, _m) = banc_projet("envois-navigation", 5);
    b.app.run_action("envoi.modifie_suivant");
    assert!(b.app.status.contains("Aucun envoi"), "{}", b.app.status);
    marquer_dans_l_appli(&mut b, "v1");
    b.app.run_action("envoi.modifie_suivant");
    assert!(b.app.status.contains("Aucun plan modifié"), "{}", b.app.status);
    for i in [1, 3] {
        b.app.doc.panels[i].layers[0].opacity = 0.4;
    }
    b.app.doc.panels[1].action = "Elle sourit.".into();
    b.app.doc.panels[3].action = "Il part.".into();
    // (Le suivi regarde l'historique et les versions des dessins : une retouche de calque passe par l'historique.)
    b.app.history.push(crate::history::Edit::Group(Vec::new()));
    pomper(&mut b);
    assert_eq!(b.app.plans_modifies(), vec![1, 3]);
    b.app.select_panel(0);
    b.app.run_action("envoi.modifie_suivant");
    assert_eq!(b.app.cur_panel, 1);
    b.app.run_action("envoi.modifie_suivant");
    assert_eq!(b.app.cur_panel, 3);
    b.app.run_action("envoi.modifie_suivant");
    assert_eq!(b.app.cur_panel, 1, "il tourne");
    b.app.run_action("envoi.modifie_precedent");
    assert_eq!(b.app.cur_panel, 3);
    b.app.run_action("envoi.modifies_choisir");
    let ids: Vec<u64> = [1, 3].iter().map(|&i| b.app.doc.panels[i].id).collect();
    assert_eq!(b.app.board_sel, ids);
}

#[test]
fn a_project_without_envois_does_no_work_and_asks_for_no_repaint() {
    // (Le moindre calcul d'effet d'un autre essai réveille cette application : état commun pour elle seule.)
    crate::fxcache::monde::ecrire();
    let (mut b, _m) = banc_projet("envois-aucun", 4);
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT finit par se reposer");
    assert!(!b.app.envois_ui.occupe());
    let repos: Vec<_> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    let pire = repos.iter().min().copied().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "SHOT demande à être repeint dans {pire:?}");
}

#[test]
fn once_the_plans_are_followed_the_application_goes_back_to_rest() {
    crate::fxcache::monde::ecrire();
    let (mut b, _m) = banc_projet("envois-repos", 4);
    marquer_dans_l_appli(&mut b, "v1");
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT finit par se reposer après le suivi");
    let repos: Vec<_> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    let pire = repos.iter().min().copied().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "après le suivi, SHOT demande à être repeint dans {pire:?}");
    assert!(b.app.envois_ui.au_repos());
}

#[test]
fn the_per_frame_stamp_and_the_marking_copy_stay_cheap_on_a_big_film() {
    // 600 plans de 4 images, 1 200 dessins : le relevé de chaque image d'écran, et la copie d'un marquage.
    let doc = film(600);
    let mut b = banc(doc, false);
    let debut = std::time::Instant::now();
    let mut dernier = None;
    for _ in 0..200 {
        dernier = Some(b.app.envois_releve());
    }
    let par_image = debut.elapsed() / 200;
    assert!(dernier.is_some());
    assert!(par_image < Duration::from_millis(1), "le relevé coûte {par_image:?} par image d'écran");
    let ctx = b.ctx.clone();
    b.app.envois_marquer(&ctx, "v1", 1_760_000_000);
    // La copie des dessins se fait par morceaux, image d'écran après image d'écran.
    for _ in 0..400 {
        if b.app.envois_ui.derniere_copie.is_some() {
            break;
        }
        b.image(Vec::new(), 1.0 / 60.0);
    }
    let copie = b.app.envois_ui.derniere_copie.expect("mesure");
    eprintln!("envois : relevé par image d'écran {par_image:?} ; copie d'un marquage (600 plans, 1 200 dessins) {copie:?}");
    assert!(copie < Duration::from_millis(400), "la copie d'un marquage coûte {copie:?}");
    // On laisse les fils finir avant la fin de l'essai.
    for _ in 0..800 {
        b.image(Vec::new(), 0.25);
        if b.app.envois_ui.au_repos() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_envoi_commands_are_in_the_palette_the_menus_and_the_shortcut_file() {
    for id in ["envoi.marquer", "envoi.liste", "envoi.modifie_suivant", "envoi.modifie_precedent", "envoi.modifies_choisir"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
    assert!(crate::barres::Barre::Montage.cherche("envoi.marquer").is_some());
}

#[test]
fn the_last_envoi_is_the_last_marked_not_the_latest_dated() {
    // La date se tape : un envoi daté d'un jour qui vient ne doit pas rester « le dernier » quand on en marque un autre.
    let mut doc = film(1);
    for (id, nom, date) in [(1, "v1 au réal", 1_800_000_000), (2, "v2 au réal", 1_760_000_000)] {
        let n = NouvelEnvoi { id, nom: nom.into(), date, note_le: 0, unique: id as u32 };
        let (envoi, instantane) = envois::construire(demande(&doc, Some(n))).unwrap().envoi.unwrap();
        doc.envois.ajouter(envoi, instantane);
    }
    assert_eq!(doc.envois.dernier().map(|e| e.nom.as_str()), Some("v2 au réal"));
    let texte = String::from_utf8(envois::to_bytes(&doc).unwrap().unwrap()).unwrap();
    assert_eq!(envois::parse(&texte).unwrap().iter().map(|e| e.id).collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn the_marker_is_a_badge_on_a_wide_block_and_a_stripe_on_a_narrow_one() {
    use eframe::egui::{pos2, vec2, LayerId, Rect};
    let ctx = eframe::egui::Context::default();
    // (Le tracé du crayon demande les polices : il se fait dans une image d'écran.)
    let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
        let painter = ctx.layer_painter(LayerId::background());
        let large = Rect::from_min_size(pos2(10.0, 10.0), vec2(120.0, 90.0));
        let zone = crate::envoisui::peindre_repere_du_bloc(&painter, large, Etat::Modifie);
        assert!(large.contains_rect(zone) && zone.width() < 20.0 && zone.right() > large.right() - 30.0, "pastille en haut à droite : {zone:?}");
        // Un bloc de quelques points (la bande zoomée loin) : une bande le long du bord haut, aussi large que lui.
        let etroit = Rect::from_min_size(pos2(10.0, 10.0), vec2(6.0, 90.0));
        let zone = crate::envoisui::peindre_repere_du_bloc(&painter, etroit, Etat::Nouveau);
        assert!(zone.width() >= etroit.width() && zone.height() < 12.0 && zone.top() <= etroit.top() + 1.0, "bande au bord haut : {zone:?}");
        // Un bloc large mais bas : pas de pastille non plus (elle cacherait la poignée de bord).
        let bas = Rect::from_min_size(pos2(10.0, 10.0), vec2(120.0, 30.0));
        assert!(crate::envoisui::peindre_repere_du_bloc(&painter, bas, Etat::Modifie).height() < 12.0);
    });
}