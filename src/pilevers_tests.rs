//! La pile des versions du montage (`pilevers.rs`, `pileversui.rs`) : nomenclature « V2 », étiquette, pile au
//! survol, nouvelle version vide, suppression des versions non affichées, noms de clips de l'export.

use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Color32, Event, Modifiers, PointerButton, Rect};

use crate::document::{Cel, Document, Layer, LayerKind};
use crate::editorial::{self, Options};
use crate::perf_tests::{banc, se_reposer, Banc};
use crate::pilevers::{self, Etiquette, Pile, Pointeur};
use crate::planvers::etiquette;
use crate::planversui::{phrase_purge, titre_avec_version, titre_long_avec_version};

fn rouge(r: u8) -> Color32 {
    Color32::from_rgba_premultiplied(r, 10, 10, 255)
}

/// Un film de `n` plans de 48 images (assez larges, dans la bande, pour porter une étiquette) : un calque bitmap, un dessin par plan.
fn film(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    for k in 1..n {
        doc.add_scene_after(k - 1);
    }
    for i in 0..n {
        let mut layer = Layer::new("Calque", LayerKind::Bitmap);
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(0)[0] = rouge((20 + i * 9) as u8);
        let id = doc.add_cel(cel);
        layer.frames = vec![Some(id); 48];
        doc.panels[i].layers = vec![layer];
        doc.panels[i].length = 48;
    }
    doc
}

fn retoucher(doc: &mut Document, i: usize, r: u8) {
    let id = doc.panels[i].layers[0].frames[0].unwrap();
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.raster.tile_mut(7)[3] = rouge(r);
    cel.version += 1;
}

fn banc_projet(n: usize) -> Banc {
    let mut b = banc(film(n), false);
    b.app.modified = false;
    b
}

// ------------------------------------------------------------------------------------ nomenclature

#[test]
fn versions_are_named_in_capitals_everywhere() {
    assert_eq!((etiquette(1).as_str(), etiquette(12).as_str()), ("V1", "V12"));
    let mut doc = film(2);
    assert_eq!(titre_avec_version(&doc, 0), "Plan 1", "un plan à une version reste propre");
    doc.nouvelle_version(0, 1);
    assert_eq!(titre_avec_version(&doc, 0), "Plan 1 · V2");
    assert!(titre_long_avec_version(&doc, 0).ends_with(" · V2"), "{}", titre_long_avec_version(&doc, 0));
    assert_eq!(titre_avec_version(&doc, 1), "Plan 2");
}

#[test]
fn the_label_only_shows_when_the_plan_has_several_versions() {
    assert!(!pilevers::etiquette_visible(1) && pilevers::etiquette_visible(2) && pilevers::etiquette_visible(9));
    let mut doc = film(2);
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    assert!(pilevers::cartes(&doc, a).is_empty(), "pas de versions : pas de cartes");
    doc.nouvelle_version(0, 5);
    doc.nouvelle_version(0, 6);
    assert_eq!(doc.nombre_de_versions(a), 3);
    assert!(pilevers::cartes(&doc, b).is_empty());
    let c = pilevers::cartes(&doc, a);
    assert_eq!(c.iter().map(|c| c.numero).collect::<Vec<_>>(), vec![1, 2, 3], "de haut en bas, la plus récente près du bloc");
    assert_eq!(c.iter().filter(|c| c.actuelle).map(|c| c.numero).collect::<Vec<_>>(), vec![3]);
}

// ------------------------------------------------------------------------------------ l'étiquette sur le bloc

fn bloc(l: f32) -> Rect {
    Rect::from_min_size(pos2(100.0, 50.0), vec2(l, 90.0))
}

#[test]
fn the_label_is_centred_on_a_wide_block_and_steps_aside_for_the_amber_badge() {
    let b = bloc(300.0);
    let Etiquette::Texte(r) = pilevers::placer_etiquette(b, 14.0, false) else { panic!("une étiquette") };
    assert!((r.center().x - b.center().x).abs() < 0.5, "centrée");
    assert!(r.top() >= b.top() && r.bottom() < b.bottom());
    // Sur un bloc large, la pastille (en haut à droite) est loin : toujours centrée.
    let Etiquette::Texte(r2) = pilevers::placer_etiquette(b, 14.0, true) else { panic!("une étiquette") };
    assert_eq!(r, r2);
}

#[test]
fn the_label_never_covers_the_badge_or_the_edge_handles_and_degrades_on_narrow_blocks() {
    let mut vus = [0usize; 3];
    for largeur in (6..400).step_by(2) {
        for pastille in [false, true] {
            let b = bloc(largeur as f32);
            match pilevers::placer_etiquette(b, 14.0, pastille) {
                Etiquette::Texte(r) => {
                    vus[0] += 1;
                    assert!(r.left() >= b.left() + 10.0 && r.right() <= b.right() - 10.0, "{largeur}: loin des poignées");
                    if pastille {
                        // La pastille couvre [droite - 26, droite - 10] de x.
                        assert!(r.right() <= b.right() - 26.0 + 0.01, "{largeur}: ne couvre pas la pastille");
                    }
                }
                Etiquette::Point(r) => {
                    vus[1] += 1;
                    assert!(b.contains_rect(r), "{largeur}: le point est dans le bloc");
                    if pastille {
                        assert!(r.right() <= b.right() - 26.0 + 0.01, "{largeur}: le point ne couvre pas la pastille");
                    }
                }
                Etiquette::Aucune => vus[2] += 1,
            }
        }
    }
    assert!(vus.iter().all(|&n| n > 0), "chaque degré existe : {vus:?}");
    // Un bloc trop bas n'a rien.
    assert_eq!(pilevers::placer_etiquette(Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 20.0)), 14.0, false), Etiquette::Aucune);
    // Un bloc étroit garde un point qui s'ouvre comme l'étiquette.
    assert!(matches!(pilevers::placer_etiquette(bloc(40.0), 14.0, false), Etiquette::Point(_)));
}

// ------------------------------------------------------------------------------------ la pile : état et place

#[test]
fn the_pile_opens_on_the_label_stays_on_the_cards_and_closes_after_a_short_grace() {
    let mut p = Pile::default();
    assert!(p.avancer(Pointeur::Etiquette(7), false, false, 0.0), "elle vient de s'ouvrir");
    assert_eq!(p.ouverte(), Some(7));
    assert!(!p.avancer(Pointeur::Etiquette(7), false, false, 0.1), "déjà ouverte");
    // Du bord de l'étiquette aux cartes : jamais fermée.
    p.avancer(Pointeur::Pile, false, false, 0.2);
    assert_eq!(p.ouverte(), Some(7));
    // Le pointeur part : elle tient encore pendant le délai de grâce.
    p.avancer(Pointeur::Ailleurs, false, false, 1.0);
    p.avancer(Pointeur::Ailleurs, false, false, 1.0 + pilevers::GRACE * 0.5);
    assert_eq!(p.ouverte(), Some(7));
    assert!(p.reveil(1.0 + pilevers::GRACE * 0.5).is_some_and(|s| s > 0.0), "une image est demandée pour la fermer");
    // Il revient avant la fin : le compte repart.
    p.avancer(Pointeur::Pile, false, false, 1.0 + pilevers::GRACE * 0.9);
    assert!(p.quitte_a.is_none() && p.reveil(2.0).is_none());
    p.avancer(Pointeur::Ailleurs, false, false, 3.0);
    p.avancer(Pointeur::Ailleurs, false, false, 3.0 + pilevers::GRACE + 0.01);
    assert_eq!(p.ouverte(), None);
    // Fermée : rien à réveiller.
    assert!(p.reveil(10.0).is_none());
}

#[test]
fn another_plan_label_replaces_the_pile_and_a_busy_strip_closes_it() {
    let mut p = Pile::default();
    p.avancer(Pointeur::Etiquette(1), false, false, 0.0);
    p.confirme = Some(3);
    assert!(p.avancer(Pointeur::Etiquette(2), false, false, 0.1));
    assert_eq!((p.ouverte(), p.confirme), (Some(2), None), "une autre pile, sans confirmation en cours");
    // Un geste de la bande (glisser un plan, rogner…) ferme tout de suite.
    p.avancer(Pointeur::Pile, false, true, 0.2);
    assert_eq!(p.ouverte(), None);
    p.avancer(Pointeur::Etiquette(2), false, true, 0.3);
    assert_eq!(p.ouverte(), None, "pas d'ouverture pendant un geste");
}

#[test]
fn a_clicked_pile_stays_until_a_press_elsewhere() {
    let mut p = Pile::default();
    p.ouvrir(4);
    p.epinglee = true;
    p.avancer(Pointeur::Ailleurs, false, false, 0.0);
    p.avancer(Pointeur::Ailleurs, false, false, 50.0);
    assert_eq!(p.ouverte(), Some(4), "pas de délai de grâce pour une pile épinglée");
    assert!(p.reveil(50.0).is_none(), "et aucune image à demander");
    p.avancer(Pointeur::Pile, true, false, 51.0);
    assert_eq!(p.ouverte(), Some(4), "un appui sur les cartes la laisse");
    p.avancer(Pointeur::Ailleurs, true, false, 52.0);
    assert_eq!(p.ouverte(), None);
}

#[test]
fn the_pile_sits_above_the_label_inside_the_screen_and_scrolls_when_there_are_many_cards() {
    let ecran = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0));
    let etiq = Rect::from_min_size(pos2(700.0, 600.0), vec2(26.0, 15.0));
    let n = pilevers::cartes_visibles(3, etiq, ecran);
    assert_eq!(n, 3);
    let d = pilevers::disposer(etiq, n, ecran);
    assert!(d.cadre.bottom() <= etiq.top(), "au-dessus de l'étiquette");
    assert_eq!(d.cartes.len(), 3);
    assert!(d.cartes.windows(2).all(|w| w[0].bottom() < w[1].top()), "les cartes ne se recouvrent pas");
    assert!(d.cartes.iter().all(|c| d.cadre.contains_rect(*c)));
    assert!(d.zone.contains_rect(d.cadre) && d.zone.contains(etiq.center()), "la zone relie la pile à l'étiquette");
    // Beaucoup de versions : au plus six cartes, la molette fait le reste.
    assert_eq!(pilevers::cartes_visibles(40, etiq, ecran), pilevers::CARTES_MAX);
    // Peu de place au-dessus : moins de cartes, au moins une.
    let haute = Rect::from_min_size(pos2(700.0, 80.0), vec2(26.0, 15.0));
    let m = pilevers::cartes_visibles(10, haute, ecran);
    assert!((1..pilevers::CARTES_MAX).contains(&m), "{m}");
    let d = pilevers::disposer(haute, m, ecran);
    assert!(d.cadre.top() >= ecran.top());
    // Au bord de l'écran, la pile reste dedans.
    let coin = Rect::from_min_size(pos2(1590.0, 600.0), vec2(26.0, 15.0));
    assert!(pilevers::disposer(coin, 3, ecran).cadre.right() <= ecran.right());
    // Défiler reste entre 0 et le reste.
    let mut p = Pile::default();
    p.ouvrir(1);
    p.defiler(1, 10, 6);
    assert_eq!(p.premiere, 4, "elle part du bas : les plus récentes");
    p.defiler(-9, 10, 6);
    assert_eq!(p.premiere, 0);
    p.defiler(9, 10, 6);
    assert_eq!(p.premiere, 4);
}

// ------------------------------------------------------------------------------------ nouvelle version vide

#[test]
fn an_empty_new_version_keeps_duration_frame_and_camera_but_starts_a_blank_drawing() {
    let mut doc = film(2);
    let plan = doc.panels[0].id;
    let mut cam = doc.default_camera();
    cam.zoom = 1.5;
    doc.panels[0].camera = vec![crate::camera::CameraKey { frame: 1, cam, smooth: false, ease_in: 0.0, ease_out: 0.0 }];
    doc.panels[0].action = "Elle ouvre la porte.".into();
    let cle_camera = doc.panels[0].camera.clone();
    let duree = doc.panels[0].frame_count();
    let cadre = doc.frame;
    let avant = doc.panels[0].layers[0].frames.clone();
    assert_eq!(doc.nouvelle_version_vide(0, 42), Some(2));
    let p = &doc.panels[0];
    assert_eq!(p.id, plan);
    assert_eq!(doc.numero_de_version(plan), 2);
    assert_eq!(p.frame_count(), duree, "même durée");
    assert_eq!(doc.frame, cadre, "même cadre (celui du film)");
    assert_eq!(p.camera, cle_camera, "la caméra est gardée : le cadrage ne bouge pas");
    assert_eq!(p.action, "Elle ouvre la porte.", "le texte du plan reste");
    assert_eq!(p.layers.len(), 1, "un seul calque de dessin");
    assert_eq!((p.layers[0].kind, p.layers[0].name.as_str()), (LayerKind::Vector, "Dessin"));
    assert!(p.layers[0].frames.is_empty(), "vide");
    assert!(p.marks.is_empty());
    // L'ancienne version est rangée telle quelle, avec ses dessins.
    let v1 = doc.versions_du_plan(plan).unwrap().rangee(1).expect("V1 rangée");
    assert_eq!(v1.panel.layers[0].frames.len(), avant.len());
    assert!(v1.panel.layers[0].frames.iter().flatten().all(|id| doc.cels.contains_key(id)));
    // Un autre plan n'a rien.
    assert_eq!(doc.nombre_de_versions(doc.panels[1].id), 1);
}

#[test]
fn each_new_version_gesture_is_one_named_undo_step() {
    let mut b = banc_projet(2);
    let plan = b.app.doc.panels[0].id;
    let n0 = b.app.history.names().0.len();
    let n0 = n0;
    b.app.run_action("version.nouvelle");
    b.app.run_action("version.vide");
    let (undo, _) = b.app.history.names();
    assert_eq!(&undo[n0..], ["Nouvelle version du plan", "Nouvelle version du plan, vide"], "{undo:?}");
    assert_eq!((b.app.doc.numero_de_version(plan), b.app.doc.nombre_de_versions(plan)), (3, 3));
    assert!(b.app.status.contains("V3") && b.app.status.contains("V2"), "{}", b.app.status);
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.doc.numero_de_version(plan), b.app.doc.nombre_de_versions(plan)), (2, 2), "un geste défait");
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.nombre_de_versions(plan), 1);
    b.app.run_action("edition.retablir");
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.numero_de_version(plan), 3);
    assert_eq!(b.app.doc.panels[0].layers.len(), 1);
    // Le storyboard verrouillé refuse aussi la nouvelle version vide.
    b.app.doc.storyboard_locked = true;
    b.app.run_action("version.vide");
    assert_eq!(b.app.doc.nombre_de_versions(plan), 3);
}

#[test]
fn the_empty_version_commands_are_in_the_palette_the_menus_and_the_shortcut_file() {
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    for id in ["version.vide", "version.purger", "version.purger_plan"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
        assert!(map.bindings.iter().any(|b| b.action == id && b.keys.is_empty()), "{id} : un bloc sans touche dans keymap.toml");
        assert!(map.warnings.iter().all(|w| !w.contains(id)), "{id}");
    }
}

// ------------------------------------------------------------------------------------ passer, supprimer

#[test]
fn switching_deleting_and_the_last_version() {
    let mut b = banc_projet(1);
    let plan = b.app.doc.panels[0].id;
    // Une seule version : rien à supprimer, ni elle ni une autre.
    b.app.supprimer_la_version(0, 1);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 1);
    b.app.run_action("version.nouvelle");
    b.app.run_action("version.nouvelle");
    assert_eq!(b.app.doc.versions_du_plan(plan).unwrap().numeros(), vec![1, 2, 3]);
    // La version affichée ne se supprime pas (la croix n'existe pas sur sa carte, et le modèle refuse aussi).
    b.app.supprimer_la_version(0, 3);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 3);
    let n0 = b.app.history.names().0.len();
    b.app.passer_a_la_version(0, 1);
    b.app.supprimer_la_version(0, 2);
    let (undo, _) = b.app.history.names();
    assert_eq!(&undo[n0..], ["Passer à la version V1", "Supprimer la version V2"], "{undo:?}");
    assert_eq!(b.app.doc.versions_du_plan(plan).unwrap().numeros(), vec![1, 3]);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.versions_du_plan(plan).unwrap().numeros(), vec![1, 2, 3], "annuler ramène la version supprimée");
    // Supprimer jusqu'à ce qu'il n'en reste qu'une : le plan n'a plus de versions et la dernière est celle du film.
    b.app.supprimer_la_version(0, 2);
    b.app.supprimer_la_version(0, 3);
    assert_eq!((b.app.doc.nombre_de_versions(plan), b.app.doc.numero_de_version(plan)), (1, 1));
    b.app.supprimer_la_version(0, 1);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 1, "jamais la dernière");
}

// ------------------------------------------------------------------------------------ supprimer les non affichées

#[test]
fn the_unshown_versions_are_counted_per_film_or_per_plan() {
    let mut doc = film(4);
    assert_eq!(doc.versions_non_affichees(None), (0, 0));
    doc.nouvelle_version(0, 1);
    doc.nouvelle_version(0, 2);
    doc.nouvelle_version(0, 3);
    doc.nouvelle_version(2, 4);
    let (p0, p1, p2) = (doc.panels[0].id, doc.panels[1].id, doc.panels[2].id);
    assert_eq!(doc.versions_non_affichees(None), (4, 2), "3 + 1 versions rangées de 2 plans");
    assert_eq!(doc.versions_non_affichees(Some(p0)), (3, 1));
    assert_eq!(doc.versions_non_affichees(Some(p1)), (0, 0));
    assert_eq!(doc.supprimer_versions_non_affichees(Some(p0)), (3, 1));
    assert_eq!(doc.nombre_de_versions(p0), 1);
    assert_eq!(doc.nombre_de_versions(p2), 2, "l'autre plan n'a pas bougé");
    assert_eq!(doc.numero_de_version(p0), 1, "plus de versions : le plan n'a plus que sa version affichée");
    assert_eq!(doc.supprimer_versions_non_affichees(None), (1, 1));
    assert_eq!(doc.versions_non_affichees(None), (0, 0));
    assert_eq!(doc.supprimer_versions_non_affichees(None), (0, 0));
}

#[test]
fn the_confirmation_sentence_counts_and_agrees() {
    assert_eq!(phrase_purge((7, 4), false), "7 versions de 4 plans seront supprimées");
    assert_eq!(phrase_purge((1, 1), false), "1 version de 1 plan sera supprimée");
    assert_eq!(phrase_purge((3, 1), true), "3 versions de 1 plan supprimées");
}

#[test]
fn deleting_the_unshown_versions_asks_first_is_one_undo_step_and_has_nothing_to_do_when_empty() {
    let mut b = banc_projet(3);
    let (p0, p2) = (b.app.doc.panels[0].id, b.app.doc.panels[2].id);
    // Rien à supprimer : on le dit, aucune confirmation.
    b.app.run_action("version.purger");
    assert!(b.app.purge_versions.is_none());
    assert!(b.app.status.contains("Aucune version à supprimer"), "{}", b.app.status);
    b.app.cur_panel = 0;
    b.app.run_action("version.nouvelle");
    b.app.run_action("version.nouvelle");
    b.app.cur_panel = 2;
    b.app.run_action("version.nouvelle");
    let n0 = b.app.history.names().0.len();
    // Le plan courant seulement.
    b.app.run_action("version.purger_plan");
    assert_eq!(b.app.purge_versions, Some(Some(p2)));
    b.chauffer(2);
    assert!(b.app.purge_versions.is_some(), "la confirmation reste ouverte");
    b.app.purge_versions = None;
    // Tout le film : confirmation, puis une seule étape nommée.
    b.app.run_action("version.purger");
    assert_eq!(b.app.purge_versions, Some(None));
    assert_eq!(b.app.doc.nombre_de_versions(p0), 3);
    let (v, p) = b.app.doc.versions_non_affichees(None);
    assert_eq!(phrase_purge((v, p), false), "3 versions de 2 plans seront supprimées");
    b.app.purge_versions = None;
    b.app.purge_versions_faire(None);
    assert_eq!((b.app.doc.nombre_de_versions(p0), b.app.doc.nombre_de_versions(p2)), (1, 1));
    assert!(b.app.status.contains("3 versions de 2 plans supprimées"), "{}", b.app.status);
    let (undo, _) = b.app.history.names();
    assert_eq!(&undo[n0..], ["Supprimer les versions non affichées"], "{undo:?}");
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.doc.nombre_de_versions(p0), b.app.doc.nombre_de_versions(p2)), (3, 2), "une étape les ramène toutes");
    // Le storyboard verrouillé refuse.
    b.app.doc.storyboard_locked = true;
    b.app.run_action("version.purger");
    assert!(b.app.purge_versions.is_none());
}

#[test]
fn the_envois_are_not_touched_by_deleting_the_unshown_versions() {
    let mut b = banc_projet(2);
    let plan = b.app.doc.panels[0].id;
    b.app.run_action("version.nouvelle");
    let ctx = b.ctx.clone();
    b.app.envois_marquer(&ctx, "v1 au réal", 1_760_000_000);
    for _ in 0..800 {
        b.image(Vec::new(), 0.25);
        if b.app.envois_ui.au_repos() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let avant = b.app.doc.envois.list.clone();
    assert_eq!(avant.len(), 1);
    b.app.purge_versions_faire(None);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 1);
    assert_eq!(b.app.doc.envois.list.len(), 1);
    assert_eq!(b.app.doc.envois.list[0].plan(plan).map(|p| p.version), avant[0].plan(plan).map(|p| p.version), "l'envoi se souvient de la version partie");
}

// ------------------------------------------------------------------------------------ envois et export

#[test]
fn switching_to_a_version_that_differs_marks_the_plan_modified_since_the_last_envoi() {
    let mut b = banc_projet(2);
    let plan = b.app.doc.panels[0].id;
    let ctx = b.ctx.clone();
    let attendre = |b: &mut Banc| {
        for _ in 0..1200 {
            b.image(Vec::new(), 0.25);
            if b.app.envois_ui.au_repos() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    b.app.envois_marquer(&ctx, "v1 au réal", 1_760_000_000);
    attendre(&mut b);
    assert!(b.app.envois_ui.etat(plan).is_none());
    // Une nouvelle version vide : le plan n'est plus comme à l'envoi.
    b.app.run_action("version.vide");
    attendre(&mut b);
    assert!(b.app.envois_ui.etat(plan).is_some(), "la version vide n'est pas celle de l'envoi");
    // Revenir à V1 (la version envoyée) : le repère disparaît.
    b.app.passer_a_la_version(0, 1);
    attendre(&mut b);
    assert!(b.app.envois_ui.etat(plan).is_none(), "revenu à la version envoyée");
    // La comparaison peut choisir n'importe quelle version.
    assert!(b.app.comparer_sources(0).contains(&crate::comparer::Source::Version(2)));
}

#[test]
fn clips_of_the_editorial_export_carry_the_version_only_when_the_plan_has_several() {
    let mut doc = film(3);
    let o = Options::default();
    let sans = editorial::build_plan(&doc, &o, 0, "Film").unwrap();
    assert!(sans.clips.iter().all(|c| !c.name.contains("_V")), "{:?}", sans.clips.iter().map(|c| &c.name).collect::<Vec<_>>());
    doc.nouvelle_version(1, 1);
    doc.nouvelle_version(1, 2);
    let avec = editorial::build_plan(&doc, &o, 0, "Film").unwrap();
    assert_eq!(avec.clips[0].name, sans.clips[0].name, "un plan à une version garde son nom");
    assert_eq!(avec.clips[1].name, format!("{}_V3", sans.clips[1].name));
    assert!(avec.clips[1].file.ends_with("_V3.mov"), "{}", avec.clips[1].file);
    assert!(avec.clips[1].title.ends_with(" · V3"), "{}", avec.clips[1].title);
    assert_eq!(avec.clips[1].reel, sans.clips[1].reel, "la bobine reste celle du plan (8 caractères au plus)");
    assert_eq!(avec.clips[2].name, sans.clips[2].name);
    // Revenir à V1 change le nom du clip : c'est la version qui part.
    doc.changer_de_version(1, 1);
    let v1 = editorial::build_plan(&doc, &o, 0, "Film").unwrap();
    assert_eq!(v1.clips[1].name, format!("{}_V1", sans.clips[1].name));
}

// ------------------------------------------------------------------------------------ dans l'application : le survol

/// Fait tourner le banc jusqu'à ce que `cond` soit vraie (au plus `max` images).
fn jusqu_a(b: &mut Banc, max: usize, cond: impl Fn(&Banc) -> bool) -> bool {
    for _ in 0..max {
        b.image(Vec::new(), 1.0 / 60.0);
        if cond(b) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

fn etiquette_du_plan(b: &Banc, plan: u64) -> Option<Rect> {
    b.app.pile_versions.etiquettes.iter().find(|(_, p)| *p == plan).map(|(r, _)| *r)
}

#[test]
fn hovering_the_label_opens_the_pile_clicking_a_card_switches_and_leaving_closes() {
    let mut doc = film(2);
    doc.nouvelle_version(0, 10);
    retoucher(&mut doc, 0, 200);
    doc.nouvelle_version(0, 11);
    let plan = doc.panels[0].id;
    let mut b = banc(doc, false);
    b.app.modified = false;
    assert!(jusqu_a(&mut b, 120, |b| etiquette_du_plan(b, plan).is_some()), "l'étiquette « V3 » est posée sur le bloc du plan");
    // Un plan à une seule version n'en a pas.
    let autre = b.app.doc.panels[1].id;
    assert!(etiquette_du_plan(&b, autre).is_none());
    let zone = etiquette_du_plan(&b, plan).unwrap();
    let ev = b.souris(zone.center().x, zone.center().y);
    b.image(ev, 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.pile_versions.ouverte(), Some(plan), "le survol ouvre la pile");
    let pile = b.app.pile_versions.zone.expect("la pile est posée");
    assert!(pile.bottom() >= zone.top() - 5.0 && pile.top() < zone.top(), "au-dessus de l'étiquette");
    // Les aperçus viennent de la file de fond et finissent par arriver ; jamais composés ici.
    assert!(jusqu_a(&mut b, 800, |b| !b.app.vignettes_en_cours()), "les aperçus finissent");
    // Le pointeur va sur une carte : elle reste ouverte. La carte de la V1 est la plus haute.
    let cadre_haut = pile.min + vec2(pilevers::CARTE_L / 2.0, pilevers::BORD + pilevers::CARTE_H / 2.0);
    for p in [pos2(zone.center().x, (zone.top() + pile.bottom()) / 2.0), cadre_haut] {
        let ev = b.souris(p.x, p.y);
        b.image(ev, 1.0 / 60.0);
        assert_eq!(b.app.pile_versions.ouverte(), Some(plan), "elle reste ouverte jusqu'aux cartes");
    }
    // Clic sur la carte V1 : le plan passe à V1 (une étape d'annulation nommée).
    let n0 = b.app.history.names().0.len();
    let clic = |pressed| Event::PointerButton { pos: cadre_haut, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    b.image(vec![clic(true)], 1.0 / 60.0);
    b.image(vec![clic(false)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.doc.numero_de_version(plan), 1, "le clic a passé le plan à V1");
    let (undo, _) = b.app.history.names();
    assert_eq!(&undo[n0..], ["Passer à la version V1"], "{undo:?}");
    assert_eq!(b.app.pile_versions.ouverte(), Some(plan), "elle reste ouverte pour comparer d'un coup d'œil");
    // Le pointeur s'en va : elle se ferme après le délai de grâce.
    let loin = pos2(5.0, 5.0);
    let ev = b.souris(loin.x, loin.y);
    b.image(ev, 1.0 / 60.0);
    assert_eq!(b.app.pile_versions.ouverte(), Some(plan), "pas tout de suite");
    b.image(Vec::new(), pilevers::GRACE as f64 + 0.05);
    b.image(Vec::new(), 0.05);
    assert_eq!(b.app.pile_versions.ouverte(), None);
}

#[test]
fn the_cross_asks_for_confirmation_then_deletes_and_the_current_card_has_no_cross() {
    let mut doc = film(1);
    doc.nouvelle_version(0, 10);
    let plan = doc.panels[0].id;
    let mut b = banc(doc, false);
    b.app.modified = false;
    assert!(jusqu_a(&mut b, 120, |b| etiquette_du_plan(b, plan).is_some()));
    let zone = etiquette_du_plan(&b, plan).unwrap();
    let ev = b.souris(zone.center().x, zone.center().y);
    b.image(ev, 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let pile = b.app.pile_versions.zone.expect("la pile est posée");
    // Deux cartes : V1 en haut, V2 (affichée) en bas. La croix de V1 est en haut à droite de sa carte.
    let croix = pos2(pile.min.x + pilevers::BORD + pilevers::CARTE_L - 12.0, pile.min.y + pilevers::BORD + 12.0);
    let sur = |b: &mut Banc, p: egui::Pos2| {
        let ev = b.souris(p.x, p.y);
        b.image(ev, 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
    };
    let clic = |b: &mut Banc, p: egui::Pos2| {
        for pressed in [true, false] {
            b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }], 1.0 / 60.0);
        }
        b.image(Vec::new(), 1.0 / 60.0);
    };
    // Cliquer la croix : une confirmation sur la carte, rien n'est supprimé.
    sur(&mut b, croix);
    clic(&mut b, croix);
    assert_eq!(b.app.pile_versions.confirme, Some(1));
    assert_eq!(b.app.doc.nombre_de_versions(plan), 2, "pas avant la confirmation");
    // « Garder » (annuler) : rien ne change.
    b.app.pile_versions.confirme = None;
    // Redemander puis confirmer : le bouton « Supprimer » est dans la carte.
    b.app.pile_versions.confirme = Some(1);
    b.image(Vec::new(), 1.0 / 60.0);
    let apercu = ((pilevers::CARTE_H - 8.0) * 320.0 / 180.0).clamp(30.0, 80.0);
    let oui = pos2(pile.min.x + pilevers::BORD + 4.0 + apercu + 7.0 + 31.0, pile.min.y + pilevers::BORD + pilevers::CARTE_H - 13.0);
    sur(&mut b, oui);
    clic(&mut b, oui);
    assert_eq!(b.app.doc.nombre_de_versions(plan), 1, "la version est supprimée après confirmation");
    assert!(b.app.status.contains("V1 supprimée"), "{}", b.app.status);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.nombre_de_versions(plan), 2, "Ctrl+Z la ramène");
}

#[test]
fn a_closed_pile_and_an_open_one_leave_the_application_idle() {
    let mut doc = film(2);
    doc.nouvelle_version(0, 10);
    let plan = doc.panels[0].id;
    let mut b = banc(doc, false);
    b.app.modified = false;
    assert!(jusqu_a(&mut b, 120, |b| etiquette_du_plan(b, plan).is_some()));
    assert!(se_reposer(&mut b, 80) < 80, "la pile fermée ne demande aucune image");
    let zone = etiquette_du_plan(&b, plan).unwrap();
    let ev = b.souris(zone.center().x, zone.center().y);
    b.image(ev, 1.0 / 60.0);
    assert_eq!(b.app.pile_versions.ouverte(), Some(plan));
    assert!(se_reposer(&mut b, 120) < 120, "la pile ouverte, pointeur immobile, ne demande aucune image non plus");
}

#[test]
fn the_pile_thumbnails_come_from_the_background_queue_and_show_each_version() {
    let mut doc = film(1);
    retoucher(&mut doc, 0, 90);
    doc.nouvelle_version(0, 10);
    let plan = doc.panels[0].id;
    let rangee_id = doc.versions_du_plan(plan).unwrap().rangee(1).unwrap().panel.id;
    let ctx = egui::Context::default();
    let mut thumbs = crate::thumbs::Thumbs::default();
    // Le premier appel ne compose rien et n'a rien à montrer.
    let far = std::time::Instant::now() + Duration::from_secs(3600);
    let largeur = 128;
    assert!(thumbs.texture_rangee(&ctx, &doc, &doc.versions_du_plan(plan).unwrap().rangee(1).unwrap().panel, largeur, far).is_none(), "rien sur le fil de l'interface");
    let debut = std::time::Instant::now();
    loop {
        crate::fx::begin_frame(&ctx);
        thumbs.begin_frame(&ctx);
        let r = &doc.versions_du_plan(plan).unwrap().rangee(1).unwrap().panel;
        if thumbs.texture_rangee(&ctx, &doc, r, largeur, far).is_some() && !thumbs.busy() {
            break;
        }
        assert!(debut.elapsed() < Duration::from_secs(60), "l'aperçu n'arrive pas");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(thumbs.image_of(rangee_id, largeur).is_some());
    // Les plans rangés ne sont pas élagués avec les plans du film.
    thumbs.prune(&doc);
    assert!(thumbs.image_of(rangee_id, largeur).is_some(), "l'aperçu d'une version rangée survit à l'élagage");
}
