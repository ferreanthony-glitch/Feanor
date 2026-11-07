//! Comparer un plan avec lui-même (`comparer.rs`, `comparerui.rs`) : le calcul des images, de la
//! superposition et de la différence ; le fil qui relit un envoi ; la fenêtre, qui ne change rien au plan.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{Color32, ColorImage};

use crate::comparer::{self, difference, pelure, petit_projet, rendre, Entree, Mode, Moteur, Reponse, Requete, Source};
use crate::document::{Document, Layer, LayerKind};
use crate::envois::{self, Copie, Demande, NouvelEnvoi, PlanCopie};
use crate::fiabilite_fixture::dossier;
use crate::perf_tests::{banc, se_reposer, Banc};
use crate::project::{self, Tete};

fn plein(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Un film de `n` plans de 4 images ; le premier dessin de chaque plan est un rectangle de `couleur` dans le
/// cadre de la caméra (la feuille fait 640 × 360, le cadre 320 × 180 au centre).
fn film(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    for k in 1..n {
        doc.add_scene_after(k - 1);
    }
    for i in 0..n {
        let mut layer = Layer::new("Calque", LayerKind::Bitmap);
        let mut ids = Vec::new();
        for _ in 0..2 {
            ids.push(doc.new_cel(LayerKind::Bitmap));
        }
        layer.frames = vec![Some(ids[0]), Some(ids[0]), Some(ids[1]), Some(ids[1])];
        doc.panels[i].layers = vec![layer];
        doc.panels[i].length = 4;
        rectangle(&mut doc, i, 0, 60 + i as u8 * 30);
    }
    doc
}

/// Un rectangle plein, dans le cadre, sur le dessin `d` du plan `i`.
fn rectangle(doc: &mut Document, i: usize, d: usize, rouge: u8) {
    let id = doc.panels[i].layers[0].frames[d * 2].unwrap();
    let cel = doc.cels.get_mut(&id).unwrap();
    for y in 120..240 {
        for x in 200..320 {
            let t = cel.raster.tile_index_of(x, y);
            cel.raster.tile_mut(t)[(y % 64) * 64 + x % 64] = Color32::from_rgba_premultiplied(rouge, 20, 20, 255);
        }
    }
    cel.version += 1;
}

fn image_de(doc: &Document, i: usize, largeur: usize) -> ColorImage {
    rendre(&petit_projet(doc, &doc.panels[i]), 0, largeur)
}

fn identiques(a: &ColorImage, b: &ColorImage) -> bool {
    a.size == b.size && a.pixels == b.pixels
}

// ------------------------------------------------------------------------------------ le calcul

#[test]
fn a_plan_is_rendered_as_the_camera_films_it() {
    let mut doc = Document::with_frame(320, 180, 24);
    let id = doc.new_cel(LayerKind::Bitmap);
    {
        let cel = doc.cels.get_mut(&id).unwrap();
        for y in 90..270 {
            for x in 160..480 {
                let t = cel.raster.tile_index_of(x, y);
                cel.raster.tile_mut(t)[(y % 64) * 64 + x % 64] = Color32::from_rgba_premultiplied(255, 0, 0, 255);
            }
        }
    }
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    layer.frames = vec![Some(id)];
    doc.panels[0].layers = vec![layer];
    let img = image_de(&doc, 0, 64);
    assert_eq!(img.size, [64, 36]);
    // Tout le cadre est rouge ; la copie légère donne la même image que le projet entier.
    assert!(img.pixels.iter().all(|p| p.r() > 250 && p.g() < 5 && p.b() < 5), "{:?}", &img.pixels[..4]);
    let entier = {
        let plan = crate::render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
        let cam = doc.panel_camera(0, 0);
        crate::render::thumbnail(&doc, &plan, &cam, 64, 36)
    };
    assert!(identiques(&img, &entier));
}

#[test]
fn the_overlay_colours_what_is_on_one_side_only() {
    let noir = plein(0, 0, 0);
    let blanc = plein(255, 255, 255);
    let a = ColorImage { size: [4, 1], pixels: vec![blanc, noir, blanc, noir] };
    let b = ColorImage { size: [4, 1], pixels: vec![blanc, blanc, noir, noir] };
    let p = pelure(&a, &b);
    assert_eq!(p.pixels, vec![blanc, plein(255, 0, 0), plein(0, 0, 255), noir]);
    // Deux tailles : la seconde est ramenée à la première.
    let petit = ColorImage { size: [2, 1], pixels: vec![blanc, noir] };
    assert_eq!(pelure(&a, &petit).size, [4, 1]);
}

#[test]
fn the_difference_is_the_share_of_pixels_that_changed() {
    let noir = plein(0, 0, 0);
    let a = ColorImage { size: [4, 1], pixels: vec![noir; 4] };
    let mut b = a.clone();
    assert_eq!(difference(&a, &b, 8), 0.0);
    b.pixels[1] = plein(200, 0, 0);
    b.pixels[2] = plein(5, 5, 5);
    assert_eq!(difference(&a, &b, 8), 0.25, "un pixel sur quatre, l'écart de 5 est sous le seuil");
}

// ------------------------------------------------------------------------------------ le fil

/// Les réponses du fil jusqu'à l'image `numero`.
fn attendre(m: &Moteur, numero: u64) -> (Option<Reponse>, comparer::Rendu) {
    let debut = Instant::now();
    let mut prete = None;
    loop {
        while let Some(r) = m.recevoir() {
            match r {
                Reponse::Rendu(r) if r.numero == numero => return (prete, r),
                Reponse::Rendu(_) => {}
                p @ Reponse::Prete { .. } => prete = Some(p),
            }
        }
        assert!(debut.elapsed() < Duration::from_secs(60), "le fil ne répond pas");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn the_engine_compares_a_stored_version_with_the_present_plan() {
    let mut doc = film(2);
    let plan = doc.panels[0].id;
    doc.nouvelle_version(0, 1);
    let v1 = doc.versions_du_plan(plan).unwrap().rangee(1).unwrap().panel.clone();
    let ctx = eframe::egui::Context::default();
    // Les mêmes : aucune différence.
    let m = Moteur::demarrer(Entree::Doc(Box::new(petit_projet(&doc, &v1))), Entree::Doc(Box::new(petit_projet(&doc, &doc.panels[0]))), ctx.clone());
    m.demander(Requete { numero: 1, frame: 0, largeur: 128, avec_pelure: true });
    let (prete, r) = attendre(&m, 1);
    let Some(Reponse::Prete { images, erreurs, .. }) = prete else { panic!("le fil dit d'abord qu'il est prêt") };
    assert_eq!((images, erreurs), ([Some(4), Some(4)], [None, None]));
    assert_eq!(r.difference, Some(0.0));
    assert!(r.a.is_some() && r.b.is_some() && r.pelure.is_some());
    // Une retouche de l'active (sur ses dessins à elle) : les deux côtés diffèrent, la version rangée a gardé l'ancien.
    rectangle(&mut doc, 0, 0, 240);
    let m = Moteur::demarrer(Entree::Doc(Box::new(petit_projet(&doc, &v1))), Entree::Doc(Box::new(petit_projet(&doc, &doc.panels[0]))), ctx);
    m.demander(Requete { numero: 7, frame: 0, largeur: 128, avec_pelure: false });
    let (_, r) = attendre(&m, 7);
    assert!(r.difference.unwrap() > 0.01, "{:?}", r.difference);
    assert!(r.pelure.is_none(), "la superposition n'est calculée que si on la demande");
    // Le côté gauche est l'ancien, tel que la composition directe le donne.
    assert!(identiques(&r.a.unwrap(), &rendre(&petit_projet(&doc, &v1), 0, 128)));
}

#[test]
fn the_engine_reads_an_envoi_back_as_it_was() {
    let travail = dossier("comparer-envoi");
    let m = travail.join("Film").join("Film.pelure");
    let dir = m.parent().unwrap().to_path_buf();
    let mut doc = film(2);
    let avant = image_de(&doc, 1, 160);
    // L'envoi (comme l'application le marque), puis le plan change.
    let plans: Vec<PlanCopie> = doc.panels.iter().enumerate().map(|(i, p)| PlanCopie { panel: p.clone(), titre: format!("Plan {}", i + 1), version: 1, nom: String::new(), sequence: String::new() }).collect();
    let mut cels = std::collections::HashMap::new();
    for p in &doc.panels {
        for id in crate::empreinte::dessins_du_plan(p) {
            cels.insert(id, Copie { version: doc.cels[&id].version, donnees: crate::empreinte::Donnees::de(&doc.cels[&id]) });
        }
    }
    let r = envois::construire(Demande { plans, cels, connus: Default::default(), tete: Tete::de(&doc), envoi: Some(NouvelEnvoi { id: 1, nom: "v1 au réal".into(), date: 1_760_000_000, note_le: 0, unique: 7 }) }).unwrap();
    let (envoi, instantane) = r.envoi.unwrap();
    doc.envois.ajouter(envoi.clone(), instantane);
    rectangle(&mut doc, 1, 0, 230);
    project::prepare_save(&doc, &m, &mut Default::default(), None).unwrap().run().unwrap();
    let plan = doc.panels[1].id;
    let ctx = eframe::egui::Context::default();
    let moteur = Moteur::demarrer(Entree::Envoi { dossier: dir.clone(), envoi: Box::new(envoi.clone()), plan }, Entree::Doc(Box::new(petit_projet(&doc, &doc.panels[1]))), ctx.clone());
    moteur.demander(Requete { numero: 1, frame: 0, largeur: 160, avec_pelure: true });
    let (_, rendu) = attendre(&moteur, 1);
    assert!(identiques(rendu.a.as_ref().unwrap(), &avant), "le côté de l'envoi est le plan d'alors");
    assert!(!identiques(rendu.b.as_ref().unwrap(), &avant), "le côté d'aujourd'hui a changé");
    assert!(rendu.difference.unwrap() > 0.01);
    // Un envoi dont l'instantané manque : une phrase pour ce côté, l'autre se montre.
    std::fs::remove_file(dir.join("versions").join(&envoi.fichier)).unwrap();
    let moteur = Moteur::demarrer(Entree::Envoi { dossier: dir, envoi: Box::new(envoi), plan }, Entree::Doc(Box::new(petit_projet(&doc, &doc.panels[1]))), ctx);
    moteur.demander(Requete { numero: 2, frame: 0, largeur: 64, avec_pelure: false });
    let (prete, rendu) = attendre(&moteur, 2);
    let Some(Reponse::Prete { images, erreurs, .. }) = prete else { panic!("prête") };
    assert_eq!(images, [None, Some(4)]);
    assert!(erreurs[0].as_deref().is_some_and(|e| e.contains("introuvable")), "{erreurs:?}");
    assert!(rendu.a.is_none() && rendu.b.is_some() && rendu.difference.is_none());
}

#[test]
fn the_frame_asked_for_is_the_frame_shown_and_a_late_request_replaces_an_early_one() {
    let mut doc = film(1);
    // Le deuxième dessin du plan est d'une autre couleur : l'image 3 (le 2e dessin) diffère de l'image 1.
    rectangle(&mut doc, 0, 1, 200);
    let ctx = eframe::egui::Context::default();
    let p = || Entree::Doc(Box::new(petit_projet(&doc, &doc.panels[0])));
    let m = Moteur::demarrer(p(), p(), ctx);
    m.demander(Requete { numero: 1, frame: 0, largeur: 96, avec_pelure: false });
    m.demander(Requete { numero: 2, frame: 3, largeur: 96, avec_pelure: false });
    let (_, r) = attendre(&m, 2);
    assert_eq!(r.frame, 3);
    let attendu = rendre(&petit_projet(&doc, &doc.panels[0]), 3, 96);
    assert!(identiques(r.a.as_ref().unwrap(), &attendu));
    assert!(!identiques(r.a.as_ref().unwrap(), &rendre(&petit_projet(&doc, &doc.panels[0]), 0, 96)));
    // Une image hors du plan montre la dernière.
    m.demander(Requete { numero: 3, frame: 99, largeur: 96, avec_pelure: false });
    let (_, r) = attendre(&m, 3);
    assert!(identiques(r.a.as_ref().unwrap(), &attendu));
}

// ------------------------------------------------------------------------------------ la fenêtre

fn banc_projet(nom: &str, n: usize) -> (Banc, PathBuf) {
    let travail = dossier(nom);
    // (Un projet `.shot` : marquer un envoi enregistre ; un `.pelure` attend « Enregistrer », `stabilite_audit_tests.rs`.)
    let m = project::manifeste_shot(&travail.join("Film").join("Film.pelure"));
    let mut b = banc(film(n), false);
    b.app.path = Some(m.clone());
    b.app.modified = true;
    (b, m)
}

fn marquer(b: &mut Banc, nom: &str) {
    let ctx = b.ctx.clone();
    b.app.envois_marquer(&ctx, nom, envois::lire_date("20/10/2026").unwrap());
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 0.25);
        if b.app.envois_ui.au_repos() && !b.app.save_en_cours() {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("l'envoi ne se termine pas");
}

/// Des images jusqu'à ce que la fenêtre montre ses images.
fn attendre_fenetre(b: &mut Banc) {
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 0.1);
        if b.app.comparaison.as_ref().is_some_and(|c| c.vue_frame().is_some()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    panic!("la fenêtre n'a pas reçu ses images");
}

/// Tout ce qui fait le plan et le projet, pour s'assurer qu'une vue en lecture seule n'y a rien changé.
fn etat(b: &Banc) -> (Vec<u64>, u64, u64, bool, Vec<(u64, u64)>) {
    let cle = |id: u64| b.app.doc.cels.get(&id).map_or(id, |c| crate::empreinte::Donnees::de(c).empreinte());
    let plans = b.app.doc.panels.iter().map(|p| crate::empreinte::de_octets(&project::digest_bytes(p, &cle))).collect();
    let mut versions: Vec<(u64, u64)> = b.app.doc.cels.iter().map(|(id, c)| (*id, c.version)).collect();
    versions.sort_unstable();
    (plans, b.app.history.serial(), b.app.doc.cels.len() as u64, b.app.modified, versions)
}

#[test]
fn the_sides_offered_and_their_names() {
    let (mut b, _m) = banc_projet("comparer-cotes", 2);
    marquer(&mut b, "v1 au réal");
    let plan = b.app.doc.panels[0].id;
    assert_eq!(b.app.comparer_sources(0), vec![Source::Actuelle, Source::Envoi(1)]);
    b.app.run_action("version.nouvelle");
    assert_eq!(b.app.comparer_sources(0), vec![Source::Actuelle, Source::Version(1), Source::Envoi(1)]);
    assert_eq!(b.app.comparer_libelle(plan, &Source::Actuelle), "Aujourd'hui (V2)");
    assert_eq!(b.app.comparer_libelle(plan, &Source::Version(1)), "Version V1");
    let envoi = b.app.comparer_libelle(plan, &Source::Envoi(1));
    assert!(envoi.contains("« v1 au réal »") && envoi.contains("20/10/2026"), "{envoi}");
    // Un plan sans versions, comparé au même envoi.
    assert_eq!(b.app.comparer_sources(1), vec![Source::Actuelle, Source::Envoi(1)]);
}

#[test]
fn opening_proposes_the_previous_version_then_the_last_envoi_and_says_when_there_is_nothing() {
    let (mut b, _m) = banc_projet("comparer-defaut", 2);
    b.app.comparer_ouvrir(0, None, None);
    assert!(b.app.comparaison.is_none());
    assert!(b.app.status.contains("Rien à comparer"), "{}", b.app.status);
    marquer(&mut b, "v1");
    b.app.comparer_ouvrir(0, None, None);
    let c = b.app.comparaison.take().expect("la fenêtre");
    assert_eq!((c.a.clone(), c.b.clone()), (Source::Envoi(1), Source::Actuelle));
    drop(c);
    b.app.run_action("version.nouvelle");
    b.app.comparer_ouvrir(0, None, None);
    let c = b.app.comparaison.take().expect("la fenêtre");
    assert_eq!((c.a.clone(), c.b.clone()), (Source::Version(1), Source::Actuelle), "la version rangée passe avant l'envoi");
}

#[test]
fn the_window_shows_the_images_and_changes_nothing_in_the_project() {
    let (mut b, _m) = banc_projet("comparer-lecture-seule", 3);
    marquer(&mut b, "v1 au réal");
    b.app.run_action("version.nouvelle");
    rectangle(&mut b.app.doc, 1, 0, 33);
    b.app.run_action("version.nouvelle");
    b.app.doc.panels[0].action = "Elle entre.".into();
    for _ in 0..4 {
        b.image(Vec::new(), 0.3);
    }
    let avant = etat(&b);
    let (modifie, serie) = (b.app.modified, b.app.history.serial());
    // L'envoi contre aujourd'hui, puis la version rangée, dans chaque mode, une autre image, un autre côté.
    b.app.comparer_ouvrir(0, Some(Source::Envoi(1)), Some(Source::Actuelle));
    attendre_fenetre(&mut b);
    assert!(b.app.comparaison.as_ref().unwrap().difference().is_some());
    for mode in [Mode::Volet, Mode::Fondu, Mode::Pelure, Mode::Cote] {
        b.app.comparaison.as_mut().unwrap().mode = mode;
        for _ in 0..30 {
            b.image(Vec::new(), 0.1);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    b.app.comparer_ouvrir(0, Some(Source::Version(1)), Some(Source::Actuelle));
    attendre_fenetre(&mut b);
    assert!(b.app.comparaison.as_ref().unwrap().difference().unwrap() >= 0.0);
    // Fermer la fenêtre : rien n'a changé non plus.
    b.app.comparaison = None;
    for _ in 0..3 {
        b.image(Vec::new(), 0.3);
    }
    let apres = etat(&b);
    assert_eq!(avant.0, apres.0, "les plans sont les mêmes");
    assert_eq!(avant.4, apres.4, "aucun dessin n'a changé de version");
    assert_eq!(avant.2, apres.2, "aucun dessin de plus");
    assert_eq!(serie, b.app.history.serial(), "l'historique n'a pas bougé");
    assert_eq!(modifie, b.app.modified, "le projet n'est pas marqué modifié");
}

#[test]
fn the_comparison_window_does_not_keep_the_application_awake() {
    crate::fxcache::monde::ecrire();
    let (mut b, _m) = banc_projet("comparer-repos", 2);
    marquer(&mut b, "v1");
    b.app.comparer_ouvrir(0, None, None);
    attendre_fenetre(&mut b);
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT finit par se reposer avec la fenêtre ouverte");
    let repos: Vec<_> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    let pire = repos.iter().min().copied().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "la fenêtre demande à être repeinte dans {pire:?}");
}

#[test]
fn an_envoi_that_cannot_be_read_is_said_in_the_window() {
    let (mut b, _m) = banc_projet("comparer-illisible", 1);
    marquer(&mut b, "v1");
    let e = b.app.doc.envois.list[0].clone();
    let dir = b.app.path.as_ref().unwrap().parent().unwrap().to_path_buf();
    std::fs::remove_file(dir.join("versions").join(&e.fichier)).unwrap();
    b.app.comparer_ouvrir(0, Some(Source::Envoi(1)), Some(Source::Actuelle));
    attendre_fenetre(&mut b);
    let c = b.app.comparaison.as_ref().unwrap();
    assert!(c.erreurs[0].as_deref().is_some_and(|e| e.contains("introuvable")), "{:?}", c.erreurs);
    assert!(c.erreurs[1].is_none());
}

#[test]
fn the_compare_command_is_in_the_palette_the_menus_and_the_shortcut_file() {
    assert!(crate::keymap::action_info("comparer.ouvrir").is_some_and(|a| a.implemented));
    assert!(crate::barres::Barre::Montage.cherche("comparer.ouvrir").is_some());
    let (mut b, _m) = banc_projet("comparer-commande", 1);
    b.app.run_action("comparer.ouvrir");
    assert!(b.app.comparaison.is_none() && b.app.status.contains("Rien à comparer"));
    b.app.run_action("version.nouvelle");
    b.app.run_action("comparer.ouvrir");
    assert!(b.app.comparaison.is_some());
    b.app.run_action("comparer.ouvrir");
    assert!(b.app.comparaison.is_none(), "la commande ferme la fenêtre ouverte");
}
