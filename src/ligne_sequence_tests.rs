//! Partie 179.11 (Anthony : « C'est vrai qu'il manque une ligne de séquence. La ligne des icônes de lecture se placerait au-dessus
//! de tout dans ce cas. »).
//!
//! 1. La barre de lecture est la première ligne de la colonne, en face de la règle des temps (seule), au-dessus de toutes les rangées.
//! 2. Une ligne « Séquence » en face de la bande des séquences, au format de Plans et Caméra : le mot à gauche, les icônes collées à
//!    droite, à la taille des icônes de la timeline ; « Nouvelle séquence » y est, et n'est plus dans la barre de lecture.
//! 3. L'espace Montage ne change pas.

use eframe::egui::{Modifiers, Rect};
use egui_phosphor::regular as ph;

use crate::bulles::vus;
use crate::montagerows::SB_SEQ_H;
use crate::perf_tests::Banc;
use crate::pistes_tests::{clic, textes};
use crate::timelinesb_tests::{banc_sb, colonne, film, haut, rangees};
use crate::workspace::Espace;

/// Les noms des icônes de la ligne « Séquence », de gauche à droite.
const ICONES: [&str; 4] = ["Nouvelle séquence", "Renommer la séquence", "Versions de la séquence", "Autres commandes des séquences"];

/// La ligne de la barre de lecture (en face de la règle) et la ligne « Séquence », à l'écran.
fn lignes(b: &Banc) -> ((f32, f32), (f32, f32)) {
    let h = haut(b);
    let rows = rangees(b);
    ((h, h + rows.regle_h()), (h + rows.regle_h(), h + rows.regle_h() + rows.bands_h()))
}

/// Les éléments nommés `nom` entre `y0` et `y1`, dans la colonne.
fn dans(nom: &str, y0: f32, y1: f32) -> Vec<Rect> {
    let grip = colonne();
    vus::ou(nom).into_iter().filter(|r| r.center().y > y0 && r.center().y < y1 && r.right() <= grip.right() + 1.0).collect()
}

#[test]
fn la_barre_de_lecture_est_la_premiere_ligne_en_face_de_la_regle_seule() {
    let b = banc_sb(film(), 1920, 1080, false);
    let ((l0, l1), (s0, _)) = lignes(&b);
    assert!((l1 - l0 - 22.0).abs() < 0.5, "la règle (et la barre en face) : 22 points pour des cases de 20 : {}", l1 - l0);
    for nom in ["Début du montage", "Lire le montage / pause", "Lecture en boucle", "Plus…"] {
        let v = dans(nom, l0, l1);
        assert_eq!(v.len(), 1, "« {nom} » sur la première ligne : {:?}", vus::ou(nom));
        assert!(v[0].top() >= l0 - 0.5 && v[0].bottom() <= s0 + 0.5, "« {nom} » au-dessus de la ligne Séquence : {:?} / {s0}", v[0]);
    }
}

#[test]
fn la_ligne_sequence_a_son_mot_et_ses_icones_a_droite_a_la_taille_des_autres() {
    let b = banc_sb(film(), 1920, 1080, false);
    let (_, (s0, s1)) = lignes(&b);
    assert!(s1 - s0 >= crate::tailleicones::mesure(18.0) + 2.0 - 0.01, "assez haute pour une case d'icône : {}", s1 - s0);
    let mot = textes(&b).into_iter().find(|(t, r)| t == "Séquence" && r.center().y > s0 && r.center().y < s1 && r.right() < colonne().left()).expect("« Séquence » à gauche").1;
    let mut avant = mot.right();
    for nom in ICONES {
        let v = dans(nom, s0, s1);
        assert_eq!(v.len(), 1, "« {nom} » sur la ligne Séquence : {:?}", vus::ou(nom));
        assert!(v[0].left() >= avant - 0.5, "« {nom} » dans l'ordre, après le mot : {:?}", v[0]);
        avant = v[0].right();
    }
    assert!((avant - (colonne().left() - 3.0)).abs() < 1.6, "collées à droite, au bord des autres lignes : {avant}");
    // La taille des icônes : celle de la ligne Plans.
    let rows = rangees(&b);
    let h = haut(&b);
    let hauteur = |g: &str, y0: f32, y1: f32| textes(&b).into_iter().find(|(t, r)| t == g && r.center().y > y0 && r.center().y < y1 && r.right() <= colonne().left()).map(|(_, r)| r.height());
    let seq = hauteur(ph::STACK_PLUS, s0, s1).expect("l'icône de la nouvelle séquence");
    let plans = hauteur(ph::PLUS_SQUARE, h + rows.blocks_top(), h + rows.blocks_top() + rows.block_h).expect("l'icône du nouveau plan");
    assert!((seq - plans).abs() < 0.5, "la même taille que Plans : {seq} / {plans}");
}

#[test]
fn nouvelle_sequence_quitte_la_barre_de_lecture_et_marche_depuis_la_ligne_sequence() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    let ((l0, l1), (s0, s1)) = lignes(&b);
    assert!(dans("Nouvelle séquence", l0, l1).is_empty(), "plus dans la barre de lecture");
    let grip = colonne();
    assert_eq!(vus::ou("Nouvelle séquence").iter().filter(|r| r.right() <= grip.right() + 1.0).count(), 1, "une seule fois dans la colonne");
    // Ni sous « Plus… » de la barre.
    let plus = dans("Plus…", l0, l1)[0];
    clic(&mut b, plus.center(), Modifiers::NONE);
    b.chauffer(2);
    assert!(!textes(&b).iter().any(|(t, _)| t.contains("Nouvelle séquence")), "pas sous « Plus… »");
    b.image(vec![eframe::egui::Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    let n = b.app.doc.sequence_order().len();
    let r = dans("Nouvelle séquence", s0, s1)[0];
    clic(&mut b, r.center(), Modifiers::NONE);
    assert_eq!(b.app.doc.sequence_order().len(), n + 1, "une séquence de plus : {}", b.app.status);
}

#[test]
fn a_la_colonne_la_plus_etroite_la_ligne_sequence_ne_deborde_pas() {
    let [w, h] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let mut b = banc_sb(film(), w as u32, h as u32, false);
    b.app.montage_head_w = crate::montagehead::largeur_min_colonne(crate::barres::Barre::Montage);
    b.chauffer(6);
    let (_, (s0, s1)) = lignes(&b);
    let grip = colonne();
    let gauche = grip.left() - b.app.montage_head_w;
    let mot = textes(&b).into_iter().find(|(t, r)| t == "Séquence" && r.center().y > s0 && r.center().y < s1 && r.right() < grip.left()).expect("« Séquence »").1;
    let mut avant = mot.right();
    for nom in ICONES {
        let v = dans(nom, s0, s1);
        assert_eq!(v.len(), 1, "« {nom} »");
        assert!(v[0].left() >= avant - 0.5 && v[0].right() <= grip.left() + 0.5 && v[0].left() >= gauche - 0.5, "« {nom} » dans la colonne, sans chevauchement : {:?}", v[0]);
        avant = v[0].right();
    }
}

#[test]
fn les_hauteurs_de_l_en_tete_et_l_espace_montage_inchange() {
    let b = banc_sb(film(), 1920, 1080, false);
    let rows = rangees(&b);
    // L'en-tête de la timeline du Storyboard, au-dessus des plans : la règle (et la barre de lecture en face) + la ligne Séquence.
    assert_eq!(rows.regle_h() + rows.bands_h(), 42.0, "22 + 20 (18 + 16 = 34 avant)");
    assert_eq!(SB_SEQ_H, 20.0);
    let mut m = banc_sb(film(), 1920, 1080, false);
    m.app.set_espace(Espace::Montage);
    m.chauffer(6);
    let rm = m.app.montage_rows(300.0);
    assert_eq!(rm.regle_h(), crate::storyboard::RULER_H, "le Montage garde sa règle");
    assert_eq!(rm.bands_h(), crate::storyboard::BANDS_H, "et sa ligne des séquences");
    assert!(vus::ou("Autres commandes des séquences").is_empty(), "pas de ligne « Séquence » dans le Montage");
}
