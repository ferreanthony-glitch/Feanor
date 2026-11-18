//! Partie 167 : les versions d'un plan dans la timeline (`versiondepliee.rs`, `versiondeplieeui.rs`).

use crate::document::{Cel, Document, Layer, LayerKind};
use crate::versiondepliee::{Carte, Lieu};

/// Un film de `n` plans de `len` images, un dessin par plan.
fn film(n: usize, len: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    for k in 1..n {
        doc.add_scene_after(k - 1);
    }
    for i in 0..n {
        let mut layer = Layer::new("Calque", LayerKind::Bitmap);
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(0)[0] = eframe::egui::Color32::from_rgba_premultiplied((20 + i * 9) as u8, 10, 10, 255);
        let id = doc.add_cel(cel);
        layer.frames = vec![Some(id); len];
        doc.panels[i].layers = vec![layer];
        doc.panels[i].length = len;
    }
    doc
}

#[test]
fn the_map_inserts_an_unfolded_version_after_its_plan_and_shifts_only_what_follows() {
    let mut doc = film(3, 10);
    // Plan 1 : V1 (10 images) rangée, V2 en cours ; la V1 dure 6 images.
    doc.nouvelle_version(0, 1);
    let plan = doc.panels[0].id;
    doc.versions.get_mut(&plan).unwrap().autres[0].panel.length = 6;
    doc.versions.get_mut(&plan).unwrap().autres[0].panel.layers[0].frames.truncate(6);
    assert!(Carte::de(&doc, &[]).vide());
    let carte = Carte::de(&doc, &[(plan, 1)]);
    assert_eq!(carte.inserts.len(), 1);
    let i = carte.inserts[0];
    assert_eq!((i.host, i.at, i.len), (0, 10, 6));
    // Avant la fin du plan : rien ne bouge ; après : six images plus loin.
    assert_eq!(carte.d_of(0), 0);
    assert_eq!(carte.d_of(9), 9);
    assert_eq!(carte.d_of(10), 16);
    assert_eq!(carte.d_of(29), 35);
    assert_eq!(carte.total(30), 36);
    assert_eq!(carte.lieu(9), Lieu::Film(9));
    assert_eq!(carte.lieu(10), Lieu::Deplie { k: 0, frame: 0 });
    assert_eq!(carte.lieu(15), Lieu::Deplie { k: 0, frame: 5 });
    assert_eq!(carte.lieu(16), Lieu::Film(10));
    for d in 0..36 {
        assert_eq!(carte.d_de_lieu(carte.lieu(d)), d, "aller-retour de l'image {d}");
    }
    assert_eq!(carte.film_de(12), 9, "une image dépliée vise la dernière du plan pour les gestes du film");
    assert_eq!(carte.troncons(), vec![(0, 10, 0), (10, usize::MAX, 6)]);
    // Une version qui n'est pas rangée (la version en cours, ou un numéro inconnu) ne se déplie pas.
    assert!(Carte::de(&doc, &[(plan, 2), (plan, 7), (doc.panels[1].id, 1)]).vide());
}

#[test]
fn several_unfolded_versions_follow_each_other_in_order() {
    let mut doc = film(3, 8);
    doc.nouvelle_version(0, 1);
    doc.nouvelle_version(0, 2);
    doc.nouvelle_version(2, 3);
    let (a, c) = (doc.panels[0].id, doc.panels[2].id);
    // Dépliées dans le désordre : la bande les range par plan puis par numéro.
    let carte = Carte::de(&doc, &[(a, 2), (c, 1), (a, 1)]);
    assert_eq!(carte.inserts.iter().map(|i| (i.host, i.numero)).collect::<Vec<_>>(), vec![(0, 1), (0, 2), (2, 1)]);
    assert_eq!((carte.debut(0), carte.debut(1), carte.debut(2)), (8, 16, 40));
    assert_eq!(carte.d_of(8), 24, "le plan 2 vient après les deux versions dépliées du plan 1");
    assert_eq!(carte.d_of(23), 39);
    assert_eq!(carte.lieu(40), Lieu::Deplie { k: 2, frame: 0 });
    assert_eq!(carte.total(24), 48);
    assert_eq!(carte.troncons(), vec![(0, 8, 0), (8, 24, 16), (24, usize::MAX, 24)]);
    for d in 0..48 {
        assert_eq!(carte.d_de_lieu(carte.lieu(d)), d);
    }
}

#[test]
fn a_version_colour_is_kept_by_its_number_and_saved_in_the_versions_file() {
    let mut doc = film(2, 8);
    doc.nouvelle_version(0, 1);
    let plan = doc.panels[0].id;
    assert!(doc.colorer_version(plan, 1, Some(9)));
    assert!(!doc.colorer_version(plan, 1, Some(9)), "rien ne change");
    assert!(!doc.colorer_version(plan, 5, Some(2)), "pas une version du plan");
    assert!(doc.colorer_version(plan, 2, Some(6)));
    // Passer d'une version à l'autre : la couleur suit le numéro.
    doc.changer_de_version(0, 1);
    assert_eq!((doc.couleur_de_version(plan, 1), doc.couleur_de_version(plan, 2)), (Some(9), Some(6)));
    let bytes = crate::planvers::to_bytes(&doc).unwrap().unwrap();
    let texte = String::from_utf8(bytes).unwrap();
    assert!(texte.contains("\"couleurs\""), "{texte}");
    let lu = crate::planvers::parse(&texte).unwrap();
    assert_eq!(lu.plans[0].couleurs.get(&1), Some(&9));
    assert_eq!(lu.plans[0].couleurs.get(&2), Some(&6));
    // Sans couleur, le fichier reste celui d'avant (pas de champ « couleurs »).
    doc.colorer_version(plan, 1, None);
    doc.colorer_version(plan, 2, None);
    let texte = String::from_utf8(crate::planvers::to_bytes(&doc).unwrap().unwrap()).unwrap();
    assert!(!texte.contains("couleurs"), "{texte}");
}

#[test]
fn an_old_versions_file_without_colours_still_loads() {
    // Un fichier écrit avant la partie 167 : pas de champ « couleurs ».
    let ancien = r#"{"format":"shot-plans-versions","version":1,"plans":[{"plan":4,"actuelle":2,"date_actuelle":0,"prochaine":3,"autres":[{"numero":1,"date":0,"entree":{}}]}]}"#;
    let lu = crate::planvers::parse(ancien).expect("un fichier d'avant se lit");
    assert_eq!(lu.plans.len(), 1);
    assert!(lu.plans[0].couleurs.is_empty());
}

// ------------------------------------------------------------------------------------ dans l'application

use eframe::egui::{pos2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::perf_tests::{banc, Banc};
use crate::versiondeplieeui::{Sorte, ZoneVersion};

/// Plan 1 en V2 (en cours, 48 images) avec une V1 rangée de `v1` images, et `n` plans de 48 images en tout.
fn film_en_v2(n: usize, v1: usize) -> Document {
    let mut doc = film(n, 48);
    doc.nouvelle_version(0, 1);
    let plan = doc.panels[0].id;
    let r = &mut doc.versions.get_mut(&plan).unwrap().autres[0].panel;
    r.length = v1;
    r.layers[0].frames.truncate(v1);
    doc
}

fn banc_film(doc: Document) -> Banc {
    let mut b = banc(doc, false);
    b.app.modified = false;
    for _ in 0..6 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    b
}

fn zone(b: &Banc, numero: u32, sorte: Sorte) -> Option<ZoneVersion> {
    b.app.depli.zones.iter().copied().find(|z| z.numero == numero && z.sorte == sorte)
}

fn blocs() -> Vec<Rect> {
    crate::storyboard::BLOCS_DU_MONTAGE.with(|v| v.borrow().clone())
}

fn cliquer(b: &mut Banc, p: Pos2, button: PointerButton) {
    let ev = b.souris(p.x, p.y);
    b.image(ev, 1.0 / 60.0);
    for pressed in [true, false] {
        b.image(vec![Event::PointerButton { pos: p, button, pressed, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    }
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Ce que le projet enregistrerait et exporterait : rien de l'affichage n'y entre.
fn empreinte_projet(b: &Banc) -> (usize, Vec<usize>, Option<Vec<u8>>, Vec<String>) {
    let d = &b.app.doc;
    let export = crate::editorial::build_plan(d, &crate::editorial::Options::default(), 0, "Film").unwrap();
    (d.total_frames(), d.panels.iter().map(|p| p.frame_count()).collect(), crate::planvers::to_bytes(d).unwrap(), export.clips.iter().map(|c| format!("{} {}", c.name, c.file)).collect())
}

#[test]
fn the_other_versions_show_as_labels_next_to_the_plan_name_and_only_there() {
    let b = banc_film(film_en_v2(2, 30));
    let plan = b.app.doc.panels[0].id;
    let nom = zone(&b, 1, Sorte::Nom).expect("l'étiquette « V1 » à côté de « Plan 1 · V2 »");
    let fleche = zone(&b, 1, Sorte::Fleche).expect("sa flèche");
    assert_eq!((nom.plan, fleche.plan), (plan, plan));
    assert!(fleche.zone.left() >= nom.zone.right(), "la flèche suit l'étiquette");
    assert!(zone(&b, 2, Sorte::Nom).is_some(), "la version en cours, dans le nom du plan");
    let bloc = blocs()[0];
    for z in &b.app.depli.zones {
        assert!(bloc.contains_rect(z.zone), "dans le bloc du plan");
    }
    // Aucune étiquette ne chevauche une autre.
    let zs = &b.app.depli.zones;
    for (i, a) in zs.iter().enumerate() {
        for c in &zs[i + 1..] {
            assert!(!a.zone.shrink(0.5).intersects(c.zone.shrink(0.5)), "{a:?} / {c:?}");
        }
    }
    // Le plan 2, qui n'a qu'une version, n'en a pas.
    assert!(b.app.depli.zones.iter().all(|z| z.plan == plan));
}

#[test]
fn a_short_plan_folds_its_labels_into_a_plus_and_never_overlaps() {
    let mut doc = film(2, 48);
    for k in 0..4 {
        doc.nouvelle_version(0, k);
    }
    let mut b = banc_film(doc);
    // De large à étroit : toujours dans le bloc, jamais l'une sur l'autre ; à l'étroit, « +N » prend la place des autres.
    let mut vu_plus = false;
    for zoom in [8.0_f32, 4.0, 2.5, 1.8, 1.2] {
        b.app.montage_zoom = zoom;
        for _ in 0..3 {
            b.image(Vec::new(), 1.0 / 60.0);
        }
        let bloc = blocs()[0];
        let zs = b.app.depli.zones.clone();
        for (i, a) in zs.iter().enumerate() {
            assert!(bloc.contains_rect(a.zone), "zoom {zoom} : {a:?} dans {bloc:?}");
            for c in &zs[i + 1..] {
                assert!(!a.zone.shrink(0.5).intersects(c.zone.shrink(0.5)), "zoom {zoom} : {a:?} / {c:?}");
            }
        }
        let montrees = zs.iter().filter(|z| z.sorte == Sorte::Nom && z.numero != 5).count();
        let plus = zs.iter().any(|z| z.sorte == Sorte::Plus);
        vu_plus |= plus;
        if plus {
            assert!(montrees < 4, "zoom {zoom} : « +N » seulement quand toutes ne tiennent pas");
        }
    }
    assert!(vu_plus, "un plan court résume ses versions en « +N »");
}

#[test]
fn clicking_the_v1_label_switches_to_it_in_one_undo_step_with_the_usual_message() {
    let mut b = banc_film(film_en_v2(2, 30));
    let plan = b.app.doc.panels[0].id;
    let z = zone(&b, 1, Sorte::Nom).unwrap();
    let n0 = b.app.history.names().0.len();
    cliquer(&mut b, z.zone.center(), PointerButton::Primary);
    assert_eq!(b.app.doc.numero_de_version(plan), 1, "le clic sur « V1 » en fait la version en cours");
    assert!(b.app.status.starts_with("Le plan est maintenant en V1 ; la V2 est gardée."), "{}", b.app.status);
    let (undo, _) = b.app.history.names();
    assert_eq!(&undo[n0..], ["Passer à la version V1"], "{undo:?}");
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.numero_de_version(plan), 2, "une seule étape d'annulation");
}

#[test]
fn the_arrow_unfolds_the_version_next_to_the_plan_for_display_only_and_folds_it_back() {
    let mut b = banc_film(film_en_v2(3, 30));
    let plan = b.app.doc.panels[0].id;
    let avant = empreinte_projet(&b);
    let n0 = b.app.history.names().0.len();
    let blocs_avant = blocs();
    let ppf = b.app.montage_zoom;
    let fleche = zone(&b, 1, Sorte::Fleche).unwrap();
    cliquer(&mut b, fleche.zone.center(), PointerButton::Primary);
    assert_eq!(b.app.depli.liste, vec![(plan, 1)], "la V1 est dépliée");
    b.image(Vec::new(), 1.0 / 60.0);
    let blocs_apres = blocs();
    // Le plan 1 ne bouge pas ; les suivants glissent de la durée de la version dépliée.
    assert_eq!(blocs_apres[0], blocs_avant[0]);
    for i in 1..3 {
        assert!((blocs_apres[i].left() - blocs_avant[i].left() - 30.0 * ppf).abs() < 0.5, "plan {} : {:?} → {:?}", i + 1, blocs_avant[i], blocs_apres[i]);
        assert!((blocs_apres[i].width() - blocs_avant[i].width()).abs() < 0.5);
    }
    // Le bloc déplié est entre les deux, avec son étiquette ; sa flèche replie.
    let (colonne, k) = *b.app.depli.colonnes.first().expect("la colonne de la version dépliée");
    assert_eq!(k, 0);
    assert!(colonne.left() >= blocs_apres[0].right() - 1.0 && colonne.right() <= blocs_apres[1].left() + 1.0, "{colonne:?}");
    assert!(b.app.depli.zones.iter().any(|z| z.sorte == Sorte::Fleche && colonne.contains_rect(z.zone)));
    // Rien n'a changé dans le projet, ses exports et l'historique.
    assert_eq!(empreinte_projet(&b), avant, "le montage, les durées, les versions et l'export ne voient pas le dépliage");
    assert_eq!(b.app.history.names().0.len(), n0, "pas d'étape d'annulation");
    assert!(!b.app.modified, "rien à enregistrer");
    // Un clic dans le bloc déplié pose seulement la tête de lecture ; la vue montre la version en lecture seule.
    let p = pos2(colonne.left() + 10.5 * ppf, blocs_apres[0].center().y);
    cliquer(&mut b, p, PointerButton::Primary);
    let t = b.app.depli.tete.expect("la tête est dans la version dépliée");
    assert_eq!((t.plan, t.numero), (plan, 1));
    assert!((9..=11).contains(&t.frame), "{}", t.frame);
    assert_eq!(empreinte_projet(&b), avant, "un clic dans la version dépliée ne change rien au projet");
    // Replier : la flèche ◂ du nom du plan (ou celle du bloc déplié).
    let fleche = zone(&b, 1, Sorte::Fleche).unwrap();
    cliquer(&mut b, fleche.zone.center(), PointerButton::Primary);
    assert!(b.app.depli.liste.is_empty(), "repliée");
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(blocs(), blocs_avant, "les plans reprennent leur place");
    assert!(b.app.depli.tete.is_none());
    assert_eq!(b.app.history.names().0.len(), n0);
}

#[test]
fn playback_goes_through_the_plan_then_its_unfolded_versions_then_the_next_plan() {
    let mut doc = film_en_v2(2, 30);
    doc.nouvelle_version(0, 2);
    // Plan 1 : V3 en cours (48), V1 (30) et V2 (48) rangées.
    let mut b = banc_film(doc);
    let plan = b.app.doc.panels[0].id;
    b.app.depli_basculer(0, 2);
    b.app.depli_basculer(0, 1);
    b.app.sound_on = false;
    b.app.select_panel(0);
    b.app.cur_frame = 40;
    b.app.toggle_play_scope(true);
    assert!(b.app.playing);
    // Image par image : la fin du plan, la V1 dépliée, la V2 dépliée, puis le plan 2.
    let mut vus: Vec<(usize, Option<(u32, usize)>)> = Vec::new();
    for _ in 0..200 {
        b.image(Vec::new(), 1.0 / 24.0 + 1e-4);
        let e = (b.app.cur_panel, b.app.depli.tete.map(|t| (t.numero, t.frame)));
        if vus.last() != Some(&e) {
            vus.push(e);
        }
        if b.app.cur_panel == 1 && b.app.cur_frame >= 3 {
            break;
        }
    }
    let premier_v1 = vus.iter().position(|e| e.1 == Some((1, 0))).expect("la lecture entre dans la V1 dépliée");
    let premier_v2 = vus.iter().position(|e| e.1 == Some((2, 0))).expect("puis dans la V2 dépliée");
    let plan2 = vus.iter().position(|e| e.0 == 1).expect("puis dans le plan 2");
    assert!(premier_v1 < premier_v2 && premier_v2 < plan2, "{vus:?}");
    assert!(vus[..premier_v1].iter().all(|e| e.0 == 0 && e.1.is_none()), "d'abord la fin du plan 1 : {vus:?}");
    let v1: Vec<usize> = vus.iter().filter_map(|e| e.1.filter(|t| t.0 == 1).map(|t| t.1)).collect();
    assert!(v1.windows(2).all(|w| w[0] < w[1]) && *v1.last().unwrap() == 29, "les images de la V1 dans l'ordre jusqu'à la dernière : {v1:?}");
    let v2: Vec<usize> = vus.iter().filter_map(|e| e.1.filter(|t| t.0 == 2).map(|t| t.1)).collect();
    assert!(*v2.last().unwrap() == 47, "{v2:?}");
    assert!(b.app.playing, "la lecture continue après");
    assert_eq!(b.app.doc.numero_de_version(plan), 3, "la version en cours n'a pas changé");
    // Les flèches aussi passent par les versions dépliées.
    b.app.playing = false;
    b.app.select_panel(1);
    b.app.cur_frame = 0;
    b.app.step_global(-1);
    assert_eq!(b.app.depli.tete.map(|t| (t.numero, t.frame)), Some((2, 47)), "une image avant le plan 2 : la fin de la V2 dépliée");
}

#[test]
fn choosing_an_unfolded_version_makes_it_current_folds_it_and_unfolds_nothing_in_its_place() {
    let mut doc = film_en_v2(2, 30);
    doc.nouvelle_version(0, 2);
    // Plan 1 : V3 en cours, V1 et V2 rangées, toutes deux dépliées.
    let mut b = banc_film(doc);
    let plan = b.app.doc.panels[0].id;
    b.app.depli_basculer(0, 1);
    b.app.depli_basculer(0, 2);
    b.image(Vec::new(), 1.0 / 60.0);
    let z = zone(&b, 1, Sorte::Nom).unwrap();
    cliquer(&mut b, z.zone.center(), PointerButton::Primary);
    assert_eq!(b.app.doc.numero_de_version(plan), 1, "la V1 dépliée devient la version en cours");
    assert_eq!(b.app.depli.liste, vec![(plan, 2)], "elle se replie, la V3 ne se déplie pas à sa place, la V2 reste dépliée");
}

/// Un son constant de `n` images (à 24 i/s) au niveau `v`, ajouté au document.
fn son(doc: &mut Document, nom: &str, n: usize, v: i16) -> u64 {
    let echantillons = n * 2000;
    let mut b = Vec::new();
    let len = (echantillons * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&len.to_le_bytes());
    for _ in 0..echantillons {
        b.extend_from_slice(&v.to_le_bytes());
    }
    let pcm = crate::audio::decode(&b, "wav").expect("WAV fabriqué");
    doc.add_sound(nom.into(), "wav".into(), std::sync::Arc::new(b), std::sync::Arc::new(pcm))
}

#[test]
fn an_unfolded_version_plays_the_sound_as_it_was_laid_under_it() {
    let travail = crate::fiabilite_fixture::dossier("versions-sons-167");
    let m = travail.join("Film").join("Film.pelure");
    let mut doc = film(3, 48);
    let voix_v1 = son(&mut doc, "voix V1.wav", 20, 8000);
    let voix_v2 = son(&mut doc, "voix V2.wav", 30, 16000);
    // Plan 2 (images 48 à 95) en V1 : la voix V1 posée 6 images après son début.
    doc.place_sound(voix_v1, 54, None).unwrap();
    doc.nouvelle_version(1, 1);
    let plan = doc.panels[1].id;
    assert_eq!(doc.son_de_version(plan, 1).map(<[_]>::to_vec), Some(vec![crate::planvers::SonPose { piste: doc.audio_tracks[0].id, debut: 6, son: voix_v1, decalage: 0, duree: 20 }]), "le son posé sous la V1 est gardé avec elle");
    // En V2, le son est posé autrement : la voix V1 part, la voix V2 arrive au début du plan.
    doc.audio_tracks[0].clips.clear();
    doc.place_sound(voix_v2, 48, None).unwrap();
    let mut b = banc_film(doc);
    b.app.sound_on = true;
    b.app.depli_basculer(1, 1);
    b.image(Vec::new(), 1.0 / 60.0);
    // La partie dépliée joue le son de sa version, là où il était posé ; le film garde le sien.
    let pistes = b.app.depli_pistes(plan, 1).expect("la V1 a son son");
    let c = &pistes[0].clips;
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].sound, c[0].start, c[0].length), (voix_v1, 54, 20), "6 images après le début du plan, comme posé");
    assert_eq!(b.app.doc.audio_tracks[0].clips.len(), 1);
    assert_eq!(b.app.doc.audio_tracks[0].clips[0].sound, voix_v2, "le son du film ne bouge pas");
    let carte = b.app.depli_carte();
    b.app.depli_cliquer(&carte, carte.debut(0) + 10);
    let mix = b.app.depli_mix().expect("la tête est dans la V1 dépliée : son mélange à elle");
    assert_eq!(mix.len(), 1);
    assert!(std::sync::Arc::ptr_eq(&mix[0].pcm, &b.app.doc.sounds[&voix_v1].pcm), "on entend la voix V1, pas la voix V2");
    assert_eq!(b.app.depli_son_global(), Some(48 + 10), "à l'image 10 de la version, le son du film à l'image 10 du plan");
    // Une version sans son gardé rejoue le son du film depuis le début du plan.
    assert!(b.app.depli_pistes(plan, 2).is_none());
    // Enregistré et rouvert : le son de la V1 et son fichier restent, même si plus aucun extrait du film ne le cite.
    crate::project::prepare_save(&b.app.doc, &m, &mut std::collections::HashMap::new(), None).expect("préparation").run().expect("écriture");
    let ouvert = crate::project::load(&m).expect("réouverture");
    assert!(ouvert.warnings.is_empty(), "{:?}", ouvert.warnings);
    assert_eq!(ouvert.doc.son_de_version(plan, 1).map(<[_]>::len), Some(1));
    assert!(ouvert.doc.sounds.contains_key(&voix_v1), "le son de la V1 est gardé dans le projet");
}

#[test]
fn the_soft_label_tints_are_distinct_and_readable() {
    use crate::versiondeplieeui::{couleur, PALETTE};
    assert_eq!(PALETTE.len(), 10);
    for k in 0..PALETTE.len() as u8 {
        let c = couleur(k);
        let encre = if crate::theme::contraste(eframe::egui::Color32::from_gray(24), c) >= crate::theme::contraste(eframe::egui::Color32::WHITE, c) { eframe::egui::Color32::from_gray(24) } else { eframe::egui::Color32::WHITE };
        assert!(crate::theme::contraste(encre, c) >= 4.5, "{} : texte lisible", PALETTE[k as usize].0);
        // Douces : ni trop saturées, ni sombres.
        let (mx, mn) = (c.r().max(c.g()).max(c.b()) as i32, c.r().min(c.g()).min(c.b()) as i32);
        assert!(mx - mn <= 110 && mx >= 140, "{} : douce", PALETTE[k as usize].0);
        for j in 0..k {
            let d = couleur(j);
            let dist = (c.r() as i32 - d.r() as i32).abs() + (c.g() as i32 - d.g() as i32).abs() + (c.b() as i32 - d.b() as i32).abs();
            assert!(dist >= 40, "{} et {} se distinguent", PALETTE[k as usize].0, PALETTE[j as usize].0);
        }
    }
}

#[test]
fn a_label_colour_is_one_undo_step_shows_on_the_plan_name_and_survives_save_and_reopen() {
    let travail = crate::fiabilite_fixture::dossier("versions-couleurs-167");
    let m = travail.join("Film").join("Film.pelure");
    let mut b = banc_film(film_en_v2(2, 30));
    let plan = b.app.doc.panels[0].id;
    let n0 = b.app.history.names().0.len();
    b.app.colorer_la_version(0, 1, Some(9));
    b.app.colorer_la_version(0, 2, Some(6));
    b.app.colorer_la_version(0, 2, Some(6));
    let (undo, _) = b.app.history.names();
    assert_eq!(&undo[n0..], ["Couleur de la V1", "Couleur de la V2"], "une étape par changement de couleur : {undo:?}");
    assert!(b.app.status.contains("V2"), "{}", b.app.status);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.couleur_de_version(plan, 2), None);
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.couleur_de_version(plan, 2), Some(6));
    crate::project::prepare_save(&b.app.doc, &m, &mut std::collections::HashMap::new(), None).expect("préparation").run().expect("écriture");
    let ouvert = crate::project::load(&m).expect("réouverture");
    assert!(ouvert.warnings.is_empty(), "{:?}", ouvert.warnings);
    assert_eq!((ouvert.doc.couleur_de_version(plan, 1), ouvert.doc.couleur_de_version(plan, 2)), (Some(9), Some(6)), "les couleurs sont enregistrées avec les versions");
    let lent = crate::project::load_quick(&m).expect("lecture rapide");
    assert_eq!(lent.doc.couleur_de_version(plan, 1), Some(9));
    // La version d'avant (sans couleur) se relit : on retire le champ du fichier.
    let fichier = m.parent().unwrap().join("plans_versions.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&fichier).unwrap()).unwrap();
    for p in v["plans"].as_array_mut().unwrap() {
        p.as_object_mut().unwrap().remove("couleurs");
    }
    std::fs::write(&fichier, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    let ancien = crate::project::load(&m).expect("un fichier d'avant la partie 167 s'ouvre");
    assert!(ancien.warnings.is_empty(), "{:?}", ancien.warnings);
    assert_eq!(ancien.doc.nombre_de_versions(plan), 2);
    assert_eq!(ancien.doc.couleur_de_version(plan, 1), None);
}