//! Essais des durées de plans (`dureeplans.rs`) : « Durée d'un nouveau plan » (Préférences > Montage) et « Changer la durée des plans choisis… ».
//!
//! Ce qu'ils gardent :
//! - la durée d'origine d'un plan neuf est de deux secondes ; le réglage la change pour les plans créés ensuite (nouveau plan, plan avant,
//!   nouvelle séquence), à la cadence du projet, sans toucher aux plans qui existent ; il s'écrit et se relit, et se garde sain ;
//! - « Changer la durée des plans choisis… » donne à tous les plans choisis la durée tapée (ou un écart), comme « Durée… » pour un plan : la fin
//!   rognée reste en réserve, ou le dernier dessin est tenu ; une seule étape d'annulation ; le son ne bouge pas et la barre d'état prévient ;
//!   refusé sous le verrou du storyboard.

use std::collections::BTreeSet;

use crate::document::{Cel, Document, Layer, LayerKind};
use crate::fiabilite_fixture::{instantane, meme_projet};
use crate::perf_tests::{banc_taille, Banc};
use crate::settings::Settings;

const L: usize = 12;

/// `n` plans de 12 images, un dessin tenu sur tout le plan.
fn film(n: usize, fps: u32) -> Document {
    let mut doc = Document::with_frame(64, 48, fps);
    for i in 0..n.saturating_sub(1) {
        doc.add_scene_after(i);
    }
    for i in 0..n {
        doc.panels[i].layers.clear();
        doc.panels[i].set_length(L);
        let c = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
        let mut l = Layer::new("Dessin", LayerKind::Bitmap);
        l.frames = vec![Some(c); L];
        doc.panels[i].layers.push(l);
    }
    doc
}

fn banc(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 800, 600);
    b.chauffer(2);
    b
}

fn photo(b: &Banc) -> String {
    instantane(&b.app.doc, &BTreeSet::new())
}

fn durees(b: &Banc) -> Vec<usize> {
    b.app.doc.panels.iter().map(|p| p.frame_count()).collect()
}

#[test]
fn a_new_plan_lasts_two_seconds_by_default() {
    assert_eq!(Settings::default().new_plan_secs, 2.0);
    let mut b = banc(film(2, 24));
    b.app.cur_panel = 0;
    b.app.run_action("panneau.nouveau");
    assert_eq!(durees(&b), vec![L, 48, L], "deux secondes à 24 i/s, comme avant le réglage");
    let mut b = banc(film(2, 25));
    b.app.run_action("panneau.nouveau");
    assert_eq!(b.app.doc.panels[1].frame_count(), 50, "deux secondes à 25 i/s");
}

#[test]
fn the_setting_gives_every_new_plan_its_duration_and_leaves_the_others_alone() {
    let mut b = banc(film(2, 24));
    b.app.settings.new_plan_secs = 3.5;
    b.app.cur_panel = 0;
    b.app.run_action("panneau.nouveau");
    assert_eq!(durees(&b), vec![L, 84, L], "3,5 s à 24 i/s = 84 images ; les plans d'avant n'ont pas changé");
    b.app.run_action("plan.nouveau_avant");
    assert!(durees(&b).iter().filter(|&&n| n == 84).count() == 2, "{:?}", durees(&b));
    b.app.run_action("sequence.nouvelle");
    assert!(durees(&b).iter().filter(|&&n| n == 84).count() == 3, "{:?}", durees(&b));
    // Un réglage plus court que le plan qu'on a : toujours au moins une image.
    b.app.settings.new_plan_secs = 0.05;
    b.app.run_action("panneau.nouveau");
    assert!(durees(&b).contains(&1), "{:?}", durees(&b));
    // À 25 images par seconde, 1,5 s se arrondit à l'image la plus proche.
    let mut b = banc(film(1, 25));
    b.app.settings.new_plan_secs = 1.5;
    b.app.run_action("panneau.nouveau");
    assert_eq!(b.app.doc.panels[1].frame_count(), 38);
    // Une étape d'annulation, comme toujours : annuler enlève le plan neuf.
    assert_eq!(b.app.history.undo_len(), 1);
}

#[test]
fn the_setting_is_written_read_and_kept_sane() {
    // D'origine : rien d'écrit.
    let texte = toml::to_string(&Settings::default()).unwrap();
    assert!(!texte.contains("duree_nouveau_plan"), "{texte}");
    // Écrit quand il change, relu tel quel.
    let s = Settings { new_plan_secs: 3.5, ..Default::default() };
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("duree_nouveau_plan = 3.5"), "{texte}");
    let relu: Settings = toml::from_str(&texte).unwrap();
    assert_eq!(relu.new_plan_secs, 3.5);
    // Absent d'un ancien fichier : deux secondes.
    let ancien: Settings = toml::from_str("copie_secours_minutes = 3").unwrap();
    assert_eq!(ancien.new_plan_secs, 2.0);
    // Sain : un nombre dans les bornes.
    use crate::dureeplans::{secondes_valides, SECONDES_MAX, SECONDES_MIN};
    assert_eq!(secondes_valides(f32::NAN), 2.0);
    assert_eq!(secondes_valides(f32::INFINITY), 2.0);
    assert_eq!(secondes_valides(-3.0), SECONDES_MIN);
    assert_eq!(secondes_valides(1e9), SECONDES_MAX);
    assert_eq!(secondes_valides(4.0), 4.0);
}

#[test]
fn the_duration_of_the_chosen_plans_changes_all_at_once() {
    let mut b = banc(film(4, 24));
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let avant = photo(&b);
    b.app.board_sel = vec![ids[0], ids[2]];
    b.app.cur_panel = 0;
    // L'action ouvre la fenêtre, le champ prérempli de la durée commune des plans choisis (partie 137 ; avant : celle d'un plan neuf).
    b.app.run_action("montage.duree_plans");
    let dlg = b.app.duree_plans.as_ref().expect("la fenêtre s'ouvre");
    assert_eq!(dlg.ids, vec![ids[0], ids[2]]);
    assert_eq!(dlg.text, L.to_string());
    b.chauffer(2);
    assert_eq!(b.app.change_plans_duration(&[ids[0], ids[2]], "20"), Some(2));
    assert_eq!(durees(&b), vec![20, L, 20, L], "seuls les plans choisis changent");
    assert_eq!(b.app.history.undo_len(), 1, "une seule étape d'annulation");
    // Le dernier dessin est tenu jusqu'à la nouvelle fin.
    let l = &b.app.doc.panels[0].layers[0];
    assert_eq!((l.cel_at(L - 1), l.cel_at(19)), (l.cel_at(0), l.cel_at(0)));
    // Une seule annulation rend tout.
    b.app.run_action("edition.annuler");
    meme_projet(&avant, &photo(&b)).expect("annuler rend les durées d'avant");
    b.app.run_action("edition.retablir");
    assert_eq!(durees(&b), vec![20, L, 20, L]);
}

#[test]
fn a_shorter_duration_clips_the_end_and_keeps_it_in_reserve() {
    let mut b = banc(film(3, 24));
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let dessins = b.app.doc.used_cels();
    assert_eq!(b.app.change_plans_duration(&[ids[0], ids[1]], "5"), Some(2));
    assert_eq!(durees(&b), vec![5, 5, L]);
    assert_eq!(b.app.doc.hidden(0), (0, L - 5), "la fin est en réserve, pas détruite");
    assert!(dessins.iter().all(|c| b.app.doc.used_cels().contains(c)), "aucun dessin perdu");
    // Rallonger la rend (la réserve revient d'abord).
    assert_eq!(b.app.change_plans_duration(&[ids[0], ids[1]], "12"), Some(2));
    assert_eq!(durees(&b), vec![L, L, L]);
    assert_eq!(b.app.doc.hidden(0), (0, 0));
}

#[test]
fn the_field_reads_seconds_and_a_difference() {
    let mut b = banc(film(3, 24));
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    // 1:12 = une seconde et douze images = 36.
    b.app.change_plans_duration(&[ids[0], ids[1]], "1:12");
    assert_eq!(durees(&b), vec![36, 36, L]);
    // Un écart s'ajoute à la durée de chaque plan.
    b.app.change_plans_duration(&[ids[1], ids[2]], "+4");
    assert_eq!(durees(&b), vec![36, 40, 16]);
    // Pas une durée : rien ne bouge, la phrase le dit.
    let n = b.app.history.undo_len();
    assert_eq!(b.app.change_plans_duration(&[ids[0]], "abc"), None);
    assert_eq!(b.app.change_plans_duration(&[ids[0]], "0"), None);
    assert_eq!(b.app.history.undo_len(), n);
    assert_eq!(durees(&b), vec![36, 40, 16]);
    assert!(b.app.status.contains("à retaper"), "{}", b.app.status);
    // La même durée : rien à changer, pas d'étape.
    assert_eq!(b.app.change_plans_duration(&[ids[0]], "36"), Some(0));
    assert_eq!(b.app.history.undo_len(), n);
}

#[test]
fn sound_does_not_move_and_the_status_warns() {
    let mut doc = film(3, 24);
    doc.add_audio_track();
    let pcm = crate::audio::Pcm::from_interleaved(&vec![0.1f32; 2 * 48_000], 2, 48_000);
    let s = doc.add_sound("a.wav".into(), "wav".into(), std::sync::Arc::new(Vec::new()), std::sync::Arc::new(pcm));
    doc.add_clip(0, s, 14, 0, 8).unwrap();
    let mut b = banc(doc);
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let clip_avant: Vec<(usize, usize)> = b.app.doc.audio_tracks[0].clips.iter().map(|c| (c.start, c.end())).collect();
    assert_eq!(b.app.change_plans_duration(&[ids[0], ids[1]], "20"), Some(2));
    let clip_apres: Vec<(usize, usize)> = b.app.doc.audio_tracks[0].clips.iter().map(|c| (c.start, c.end())).collect();
    assert_eq!(clip_avant, clip_apres, "le son reste en place");
    assert!(b.app.status.contains("par rapport au son") && b.app.status.contains("+16 images"), "{}", b.app.status);
    // Sans son derrière les plans changés : pas d'avertissement.
    let mut b = banc(film(3, 24));
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.app.change_plans_duration(&[ids[0]], "20");
    assert!(!b.app.status.contains("son"), "{}", b.app.status);
}

#[test]
fn the_storyboard_lock_refuses_it() {
    let mut doc = film(3, 24);
    doc.storyboard_locked = true;
    let mut b = banc(doc);
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    let avant = photo(&b);
    assert_eq!(b.app.change_plans_duration(&[ids[0], ids[1]], "20"), None);
    b.app.run_action("montage.duree_plans");
    assert!(b.app.duree_plans.is_none());
    assert_eq!(b.app.history.undo_len(), 0);
    meme_projet(&avant, &photo(&b)).expect("rien n'a changé");
}

#[test]
fn the_window_validates_with_the_button_and_shows_the_total() {
    let mut b = banc(film(3, 24));
    b.garder_textes = true;
    let ids: Vec<u64> = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.app.board_sel = vec![ids[0], ids[1]];
    b.app.cur_panel = 0;
    b.app.run_action("montage.duree_plans");
    b.chauffer(3);
    let vus = b.textes.join("\n");
    for t in ["Durée des plans choisis", "2 plans choisis", "Valider", "Annuler", "Nouvelle durée en tout"] {
        assert!(vus.contains(t), "« {t} » manque :\n{vus}");
    }
    // Un champ vide ou faux : un avertissement, pas de validation.
    b.app.duree_plans.as_mut().unwrap().text = "xyz".into();
    b.chauffer(3);
    assert!(b.textes.join("\n").contains("Durée à retaper"), "{}", b.textes.join("\n"));
}
