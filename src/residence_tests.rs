//! Essais des dessins en mémoire (`residence.rs`, `residenceapp.rs`) : un dessin n'est laissé hors de la mémoire que s'il
//! est écrit à sa version ; il revient avec les mêmes pixels ; l'enregistrement, l'export, l'annulation, la copie et la
//! lecture le trouvent ; le budget est tenu.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::perf_tests::{banc, Banc};
use crate::raster::{empty_tile, TILE_PIXELS};

/// `plans` plans, deux calques bitmap de quatre images (un dessin neuf toutes les deux images sur le premier) ; un dessin
/// sur trois a aussi une tuile hors de la feuille (toile infinie).
fn film(plans: usize) -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    let mut at = 0;
    for _ in 1..plans {
        at = doc.add_scene_after(at);
    }
    let mut n = 0usize;
    for p in 0..doc.panels.len() {
        let mut ls = Vec::new();
        for l in 0..2 {
            let mut layer = Layer::new(&format!("Calque {}", l + 1), LayerKind::Bitmap);
            let mut dernier = None;
            for f in 0..4 {
                if f == 0 || (l == 0 && f % 2 == 0) {
                    n += 1;
                    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
                    let mut t = empty_tile();
                    for (k, px) in t.iter_mut().enumerate() {
                        if (k + p * 7 + l * 3 + f) % 9 < 3 {
                            *px = Color32::from_rgba_premultiplied(30, (20 + p * 13) as u8, (40 + n) as u8, 200);
                        }
                    }
                    let tiles = cel.raster.tiles.len();
                    cel.raster.tiles[(p + f + l) % tiles] = Some(t.clone());
                    if n % 3 == 0 {
                        cel.raster.outer.insert((-2, -1 - (n % 2) as i32), t);
                    }
                    dernier = Some(doc.add_cel(cel));
                }
                layer.frames.push(dernier);
            }
            ls.push(layer);
        }
        doc.panels[p].layers = ls;
        doc.panels[p].length = 4;
    }
    doc
}

fn empreinte(c: &Cel) -> u64 {
    crate::empreinte::Donnees::de(c).empreinte()
}

/// L'empreinte des pixels de chaque dessin présent.
fn empreintes(doc: &Document) -> HashMap<CelId, u64> {
    doc.cels.iter().map(|(id, c)| (*id, empreinte(c))).collect()
}

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shot-residence-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier d'essai");
    d
}

/// Un banc dont le projet est enregistré dans `dir`, sous un budget de `budget` octets (sur ce fil).
fn banc_enregistre(doc: Document, dir: &std::path::Path, budget: u64) -> Banc {
    let mut b = banc(doc, false);
    assert!(b.app.save_to_blocking(dir.join("Film.pelure")), "enregistrement");
    crate::residence::budget_essai(Some(budget));
    b.chauffer(2);
    b
}

/// Des images jusqu'à ce que le chargeur n'ait plus rien en route et qu'aucune annulation ni commande n'attende (au plus
/// `max`), puis assez d'images pour un contrôle du budget.
fn au_calme(b: &mut Banc, max: Duration) {
    let t = Instant::now();
    // (Deux fenêtres calmes de suite : une vérification arrivée dans la première laisse partir son dessin au contrôle de la
    // seconde.)
    let mut calmes = 0;
    while calmes < 2 && t.elapsed() < max {
        for _ in 0..20 {
            b.image(Vec::new(), 1.0 / 60.0);
        }
        let occupe = crate::residence::en_cours() > 0 || b.app.residence.attente_historique.is_some() || b.app.residence.action_en_attente.is_some() || b.app.rejeu.has_pending();
        calmes = if occupe { 0 } else { calmes + 1 };
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn absents(b: &Banc) -> usize {
    b.app.doc.cels.absents()
}

#[test]
fn a_drawing_that_is_not_saved_never_leaves_memory() {
    crate::residence::budget_essai(Some(1));
    let mut b = banc(film(6), false);
    b.chauffer(40);
    // Projet neuf, jamais enregistré : tout reste.
    assert_eq!(absents(&b), 0, "un projet jamais enregistré garde tous ses dessins");
    // Enregistré : ce qui n'est pas voulu part ; un dessin modifié depuis ne part pas.
    let dir = dossier("non-enregistre");
    assert!(b.app.save_to_blocking(dir.join("Film.pelure")));
    let cel = b.app.doc.panels[4].layers[0].frames[0].expect("un dessin");
    {
        let c = b.app.doc.cels.get_mut(&cel).expect("présent");
        c.raster.tile_mut(0)[0] = Color32::RED;
        c.version += 1;
    }
    b.app.modified = true;
    b.app.select_panel(0);
    au_calme(&mut b, Duration::from_secs(30));
    assert!(absents(&b) > 0, "des dessins enregistrés quittent la mémoire");
    assert!(b.app.doc.cels.get(&cel).is_some(), "le dessin modifié depuis l'enregistrement reste en mémoire");
    for (id, c) in b.app.doc.cels.tous() {
        if c.est_absent() {
            assert_eq!(b.app.written.get(id), Some(&c.version), "un dessin absent est écrit à sa version");
        }
    }
    crate::residence::budget_essai(None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_drawing_that_left_memory_comes_back_with_the_same_pixels() {
    let dir = dossier("revient");
    let mut b = banc_enregistre(film(8), &dir, 1);
    let avant = empreintes(&b.app.doc);
    let lectures = crate::residence::lectures();
    b.app.select_panel(0);
    au_calme(&mut b, Duration::from_secs(30));
    let n = absents(&b);
    assert!(n > 10, "les plans lointains quittent la mémoire ({n})");
    // On revient sur chaque plan : ses dessins reviennent, mêmes pixels (hors de la feuille compris).
    for p in 0..b.app.doc.panels.len() {
        b.app.select_panel(p);
        au_calme(&mut b, Duration::from_secs(30));
        for id in crate::empreinte::dessins_du_plan(&b.app.doc.panels[p]) {
            let c = b.app.doc.cels.get(&id).unwrap_or_else(|| panic!("le dessin {id} du plan courant est revenu"));
            assert_eq!(empreinte(c), avant[&id], "le dessin {id} a les mêmes pixels");
        }
    }
    assert!(crate::residence::lectures() > lectures, "les dessins sont revenus par le chargeur");
    crate::residence::budget_essai(None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_budget_is_kept() {
    let dir = dossier("budget");
    // Un dessin de ce film : deux tuiles au plus (fenêtre et hors-feuille) ; un plan : quatre dessins. Le budget tient le
    // plan courant et ses deux voisins.
    let budget = 3 * 4 * 2 * crate::residence::OCTETS_TUILE + crate::residence::OCTETS_TUILE;
    let mut b = banc_enregistre(film(12), &dir, budget);
    for p in 0..b.app.doc.panels.len() {
        b.app.select_panel(p);
        au_calme(&mut b, Duration::from_secs(30));
        let s = b.app.residence.stats;
        assert!(s.octets_presents <= budget, "plan {p} : {} octets présents pour un budget de {budget}", s.octets_presents);
        assert!(s.absents > 0, "plan {p} : des dessins sont hors de la mémoire");
    }
    crate::residence::budget_essai(None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn saving_never_removes_the_file_of_an_absent_drawing_and_save_as_copies_it() {
    let dir = dossier("enregistrer");
    let mut b = banc_enregistre(film(8), &dir, 1);
    let avant = empreintes(&b.app.doc);
    b.app.select_panel(0);
    au_calme(&mut b, Duration::from_secs(30));
    // Le plan 6 n'est plus dans le film (son dessin reste dans le projet pour l'annulation), puis on enregistre.
    let cel = b.app.doc.panels[6].layers[0].frames[0].expect("un dessin");
    assert!(b.app.doc.cels.est_absent(&cel), "le dessin du plan 6 est hors de la mémoire");
    let id = b.app.doc.panels[6].id;
    b.app.structure_op(|d, _| d.remove_panels(&[id]));
    assert!(b.app.save_to_blocking(dir.join("Film.pelure")));
    assert!(dir.join("dessins").join(format!("{cel}.png")).is_file(), "le fichier du dessin absent est gardé");
    // L'annulation le rend au film ; il revient avec ses pixels.
    b.app.undo();
    au_calme(&mut b, Duration::from_secs(30));
    let i = b.app.doc.panels.iter().position(|p| p.id == id).expect("le plan revient");
    b.app.select_panel(i);
    au_calme(&mut b, Duration::from_secs(30));
    assert_eq!(empreinte(b.app.doc.cels.get(&cel).expect("revenu")), avant[&cel]);
    // « Enregistrer sous » ailleurs avec des dessins absents : ils y sont recopiés ; l'ancien dossier peut partir.
    b.app.select_panel(0);
    au_calme(&mut b, Duration::from_secs(30));
    assert!(absents(&b) > 0);
    let autre = dossier("enregistrer-sous");
    assert!(b.app.save_to_blocking(autre.join("Copie.pelure")));
    let relu = crate::project::load(&autre.join("Copie.pelure")).expect("relecture");
    for (id, e) in &avant {
        if let Some(c) = relu.doc.cels.get(id) {
            assert_eq!(empreinte(c), *e, "dessin {id} recopié tel quel");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    for p in 0..b.app.doc.panels.len() {
        b.app.select_panel(p);
        au_calme(&mut b, Duration::from_secs(30));
        for id in crate::empreinte::dessins_du_plan(&b.app.doc.panels[p]) {
            let c = b.app.doc.cels.get(&id).unwrap_or_else(|| panic!("le dessin {id} revient du nouveau dossier"));
            assert_eq!(empreinte(c), avant[&id]);
        }
    }
    crate::residence::budget_essai(None);
    let _ = std::fs::remove_dir_all(autre);
}

#[test]
fn undoing_an_old_step_waits_for_its_drawing_then_restores_it_exactly() {
    let dir = dossier("annuler");
    let mut b = banc_enregistre(film(8), &dir, 1);
    // Une étape sur un dessin du plan 7 : sa tuile 0 change.
    b.app.select_panel(7);
    au_calme(&mut b, Duration::from_secs(30));
    let cel = b.app.doc.panels[7].layers[1].frames[0].expect("un dessin");
    let avant = empreinte(b.app.doc.cels.get(&cel).expect("présent"));
    let ancienne = b.app.doc.cels.get_mut(&cel).expect("présent").raster.replace_slot(0, Some(crate::raster::Tile::new([Color32::BLUE; TILE_PIXELS])));
    b.app.push_pixels(cel, None, vec![(0, ancienne)]);
    let apres = empreinte(b.app.doc.cels.get(&cel).expect("présent"));
    assert_ne!(avant, apres);
    // Cinq étapes de structure par-dessus (l'étape du dessin n'est plus parmi les prochaines), enregistrées.
    for k in 0..5 {
        b.app.structure_op(|d, _| {
            d.panels[0].action = format!("action {k}");
            Some(0)
        });
    }
    assert!(b.app.save_to_blocking(dir.join("Film.pelure")));
    b.app.select_panel(0);
    au_calme(&mut b, Duration::from_secs(30));
    assert!(b.app.doc.cels.est_absent(&cel), "le dessin modifié, enregistré depuis, a quitté la mémoire");
    for _ in 0..5 {
        b.app.undo();
    }
    // L'étape du dessin attend son retour, puis s'applique.
    b.app.undo();
    au_calme(&mut b, Duration::from_secs(30));
    let c = b.app.doc.cels.get(&cel).expect("le dessin est revenu pour l'annulation");
    assert_eq!(empreinte(c), avant, "l'annulation rend exactement le dessin d'avant");
    b.app.redo();
    au_calme(&mut b, Duration::from_secs(30));
    assert_eq!(empreinte(b.app.doc.cels.get(&cel).expect("présent")), apres, "le rétablissement aussi");
    crate::residence::budget_essai(None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_export_finds_the_drawings_that_left_memory() {
    let dir = dossier("export");
    let mut b = banc_enregistre(film(6), &dir, 1);
    let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let images = |doc: &Document| -> Vec<Vec<Color32>> {
        let snap = crate::exportrender::snapshot_of(doc, &plans);
        plans.iter().map(|&p| crate::render::camera_image(&snap, &crate::render::panel_plan_at(&snap, p, 0, Some(snap.paper)), &snap.panel_camera(p, 0))).collect()
    };
    let attendu = images(&b.app.doc);
    b.app.select_panel(0);
    au_calme(&mut b, Duration::from_secs(30));
    assert!(absents(&b) > 0);
    assert!(images(&b.app.doc) == attendu, "l'export relit les dessins absents : mêmes images");
    crate::residence::budget_essai(None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_copy_of_absent_plans_pastes_with_their_pixels() {
    let dir = dossier("copier");
    let mut b = banc_enregistre(film(8), &dir, 1);
    let avant = empreintes(&b.app.doc);
    b.app.select_panel(0);
    au_calme(&mut b, Duration::from_secs(30));
    let ids: Vec<u64> = b.app.doc.panels[5..7].iter().map(|p| p.id).collect();
    let originaux: Vec<Vec<CelId>> = b.app.doc.panels[5..7].iter().map(crate::empreinte::dessins_du_plan).collect();
    assert!(originaux.iter().flatten().any(|id| b.app.doc.cels.est_absent(id)), "des dessins copiés sont absents");
    let clip = b.app.doc.copy_panels(&ids).expect("copie");
    b.app.structure_op(|d, cur| d.paste_plans(&clip, cur));
    for p in 0..b.app.doc.panels.len() {
        b.app.select_panel(p);
        au_calme(&mut b, Duration::from_secs(30));
        b.chauffer(2);
    }
    // Les plans collés (juste après le plan 0) ont les pixels de leurs originaux.
    for (k, orig) in originaux.iter().enumerate() {
        b.app.select_panel(1 + k);
        au_calme(&mut b, Duration::from_secs(30));
        let colle = crate::empreinte::dessins_du_plan(&b.app.doc.panels[1 + k]);
        let mut e: Vec<u64> = colle.iter().map(|id| empreinte(b.app.doc.cels.get(id).expect("le dessin collé est revenu"))).collect();
        let mut attendu: Vec<u64> = orig.iter().map(|id| avant[id]).collect();
        e.sort_unstable();
        attendu.sort_unstable();
        assert_eq!(e, attendu, "le plan collé {k} a les pixels de son original");
    }
    crate::residence::budget_essai(None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn opening_without_reading_the_pixels_gives_the_same_project() {
    let (f, manifeste) = crate::fiabilite_fixture::projet_enregistre("residence-paresseux");
    let complet = crate::project::load(&manifeste).expect("lecture complète");
    let paresseux = crate::project::load_paresseux(&manifeste).expect("lecture sans pixels");
    assert!(paresseux.doc.cels.absents() > 0, "des dessins s'ouvrent absents");
    // Les images vectorielles se refont (rendu exact) : seuls leurs traits se comparent.
    let sans = |doc: &Document| -> std::collections::BTreeSet<CelId> { f.sans_pixels.iter().copied().chain(doc.cels.tous().filter(|(_, c)| c.vector.is_some()).map(|(id, _)| *id)).collect() };
    let attendu = crate::fiabilite_fixture::instantane(&complet.doc, &sans(&complet.doc));
    let (revenu, perdus) = crate::residence::copie_resolue(&paresseux.doc);
    assert!(perdus.is_empty(), "{perdus:?}");
    crate::fiabilite_fixture::meme_projet(&attendu, &crate::fiabilite_fixture::instantane(&revenu, &sans(&revenu))).expect("même projet");
    // L'enregistrer ailleurs, tel quel (dessins absents recopiés de leur fichier), donne le même projet.
    let copie = crate::fiabilite_fixture::racine(&manifeste).parent().expect("dossier").join("copie").join("Film.pelure");
    crate::project::save(&paresseux.doc, &copie, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&copie).expect("relecture");
    crate::fiabilite_fixture::meme_projet(&attendu, &crate::fiabilite_fixture::instantane(&relu.doc, &sans(&relu.doc))).expect("même projet relu");
    // Et au même endroit (rien n'est réécrit, rien n'est effacé).
    let mut written = paresseux.written.clone();
    crate::project::save(&paresseux.doc, &manifeste, &mut written).expect("enregistrement sur place");
    let relu = crate::project::load(&manifeste).expect("relecture");
    crate::fiabilite_fixture::meme_projet(&attendu, &crate::fiabilite_fixture::instantane(&relu.doc, &sans(&relu.doc))).expect("même projet relu sur place");
}

/// Pendant la lecture, une image dont un dessin n'est ni en mémoire ni sur la carte montre la vignette du plan : jamais
/// une image vide là où le plan a une vignette.
#[test]
fn playback_never_shows_an_empty_frame_where_a_thumbnail_exists() {
    let dir = dossier("lecture");
    let mut b = banc_enregistre(film(20), &dir, 3 * 4 * 2 * crate::residence::OCTETS_TUILE);
    // Les vignettes du montage d'abord (elles ont leur propre cache), puis le début du film.
    b.app.set_espace(crate::workspace::Espace::Montage);
    for _ in 0..200 {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(2));
    }
    b.app.goto_global(0);
    au_calme(&mut b, Duration::from_secs(30));
    crate::residenceapp::IMAGES_VIDES.with(|n| n.set(0));
    crate::residenceapp::IMAGES_EN_ATTENTE.with(|n| n.set(0));
    b.app.toggle_play_scope(true);
    for _ in 0..(20 * 4 + 10) {
        b.image(Vec::new(), 1.0 / 24.0);
        std::thread::sleep(Duration::from_millis(4));
    }
    b.app.playing = false;
    let vides = crate::residenceapp::IMAGES_VIDES.with(|n| n.get());
    let attentes = crate::residenceapp::IMAGES_EN_ATTENTE.with(|n| n.get());
    println!("lecture : {attentes} image(s) montrée(s) par la vignette du plan, {vides} sans rien");
    assert_eq!(vides, 0, "aucune image vide là où le plan a une vignette");
    assert!(absents(&b) > 0, "la lecture laisse partir ce qui est passé");
    crate::residence::budget_essai(None);
    let _ = std::fs::remove_dir_all(dir);
}

fn cliche_de_secours(b: &Banc, projet: Option<PathBuf>) -> crate::recovery::Snapshot<'_> {
    crate::recovery::Snapshot { doc: &b.app.doc, serial: 1, modified: true, title: "Film".into(), project: projet, project_written: &b.app.written }
}

/// La copie de secours (avec ou sans le projet enregistré comme source de dessins déjà écrits) recopie les dessins qui sont
/// hors de la mémoire : relue, elle donne les mêmes pixels.
#[test]
fn a_recovery_copy_keeps_the_pixels_of_drawings_that_left_memory() {
    use crate::recovery::Recovery;
    for avec_le_projet in [false, true] {
        let dir = dossier(if avec_le_projet { "secours-projet" } else { "secours" });
        let mut b = banc_enregistre(film(8), &dir, 1);
        let avant = empreintes(&b.app.doc);
        b.app.select_panel(0);
        au_calme(&mut b, Duration::from_secs(30));
        assert!(absents(&b) > 0, "des dessins sont hors de la mémoire");
        let racine = dir.with_extension("secours");
        let _ = std::fs::remove_dir_all(&racine);
        let mut rec = Recovery::start(&racine).expect("séance de secours");
        let projet = b.app.path.clone();
        let projet = if avec_le_projet { projet } else { None };
        rec.start_copy(cliche_de_secours(&b, projet.clone()));
        let t = Instant::now();
        while !rec.tick(cliche_de_secours(&b, projet.clone()), None, false) {
            assert!(rec.error.is_none(), "{:?}", rec.error);
            std::thread::sleep(Duration::from_millis(5));
            assert!(t.elapsed() < Duration::from_secs(60), "la copie de secours ne finit pas");
        }
        let manifeste = rec.dir.join("projet").join("secours.pelure");
        drop(rec);
        let relu = crate::project::load(&manifeste).expect("relecture de la copie de secours");
        for (id, e) in &avant {
            let c = relu.doc.cels.get(id).unwrap_or_else(|| panic!("le dessin {id} est dans la copie de secours"));
            assert_eq!(empreinte(c), *e, "dessin {id} copié tel quel (avec le projet : {avec_le_projet})");
        }
        crate::residence::budget_essai(None);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&racine);
    }
}
/// Un projet qui a d'autres versions de plans s'ouvre lui aussi sans lire ses pixels (leurs panneaux passent par la
/// lecture dans une séquence à part, que l'ouverture ne met pas bout à bout).
#[test]
fn a_project_with_plan_versions_also_opens_without_reading_its_pixels() {
    let dir = dossier("versions");
    let mut doc = film(6);
    assert!(doc.nouvelle_version(2, 1).is_some(), "une version de plan");
    let manifeste = dir.join("Film.pelure");
    crate::project::save(&doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let complet = crate::project::load(&manifeste).expect("lecture complète");
    let paresseux = crate::project::load_paresseux(&manifeste).expect("lecture sans pixels");
    assert!(!paresseux.doc.versions.is_empty(), "les versions de plans sont relues");
    assert_eq!(paresseux.doc.cels.absents(), paresseux.doc.cels.len(), "tous les dessins s'ouvrent absents");
    let (revenu, perdus) = crate::residence::copie_resolue(&paresseux.doc);
    assert!(perdus.is_empty(), "{perdus:?}");
    assert_eq!(complet.doc.cels.len(), revenu.cels.len());
    for (id, c) in complet.doc.cels.iter() {
        assert_eq!(empreinte(revenu.cels.get(id).expect("dessin revenu")), empreinte(c), "dessin {id}");
    }
    let _ = std::fs::remove_dir_all(dir);
}