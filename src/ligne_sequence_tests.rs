//! Partie 179.11 (Anthony : « C'est vrai qu'il manque une ligne de séquence. La ligne des icônes de lecture se placerait au-dessus
//! de tout dans ce cas. »).
//!
//! 1. La barre de lecture est la première ligne de la colonne, en face de la règle des temps (seule), au-dessus de toutes les rangées.
//! 2. Une ligne « Séquence » en face de la bande des séquences, au format de Plans et Caméra : le mot à gauche, les icônes collées à
//!    droite, à la taille des icônes de la timeline ; « Nouvelle séquence » y est, et n'est plus dans la barre de lecture.
//! 3. L'espace Montage ne change pas.
//!
//! Partie 196 (Anthony : « La ligne Séquence perd ses raccourcis et devient plus fine (−50 %) ») : la ligne « Séquence » n'a plus
//! que son mot, sur 10 points ; ses commandes sont au clic droit de la ligne (`TimelineSb::menu_sequences_sb`).

use eframe::egui::{epaint::Shape, pos2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::bulles::vus;
use crate::montagerows::SB_SEQ_H;
use crate::perf_tests::Banc;
use crate::pistes_tests::{clic, textes};
use crate::timelinesb_tests::{banc_sb, colonne, film, haut, rangees};
use crate::workspace::Espace;

/// Les noms des anciennes icônes de la ligne « Séquence » (partie 179.11), qui n'y sont plus (partie 196).
pub(crate) const ICONES: [&str; 4] = ["Nouvelle séquence", "Renommer la séquence", "Versions de la séquence", "Autres commandes des séquences"];

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

/// Le rectangle des glyphes du mot « Séquence » de la colonne (ce qui est vraiment peint, pas la boîte du texte).
fn glyphes_du_mot(b: &Banc, y0: f32, y1: f32) -> Option<Rect> {
    let grip = colonne();
    b.formes.iter().find_map(|s| match &s.shape {
        Shape::Text(t) if t.galley.job.text == "Séquence" => {
            let r = t.galley.mesh_bounds.translate(t.pos.to_vec2());
            (r.center().y > y0 && r.center().y < y1 && r.right() < grip.left()).then_some(r)
        }
        _ => None,
    })
}

/// Le clic droit en `p`.
pub(crate) fn clic_droit(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    for pressed in [true, false] {
        b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    }
    b.chauffer(3);
}

/// Le texte `mot` peint à l'écran (le plus bas s'il y en a plusieurs : un menu passe par-dessus).
fn texte(b: &Banc, mot: &str) -> Option<Rect> {
    textes(b).into_iter().filter(|(t, _)| t == mot).map(|(_, r)| r).max_by(|a, c| a.top().total_cmp(&c.top()))
}

/// Les commandes de l'ancienne ligne d'icônes, au clic droit de la ligne « Séquence ».
pub(crate) const MENU: [&str; 5] = ["Nouvelle séquence", "Renommer la séquence…", "Versions de la séquence", "Dupliquer la séquence", "Nouvelle séquence avec les plans choisis"];

#[test]
fn la_ligne_sequence_n_a_plus_que_son_mot_et_moitie_moins_haute() {
    let b = banc_sb(film(), 1920, 1080, false);
    let (_, (s0, s1)) = lignes(&b);
    assert!((s1 - s0 - 10.0).abs() < 0.01, "10 points (20 avant) : {}", s1 - s0);
    assert_eq!(SB_SEQ_H, 10.0);
    textes(&b).into_iter().find(|(t, r)| t == "Séquence" && r.center().y > s0 && r.center().y < s1 && r.right() < colonne().left()).expect("« Séquence » à gauche");
    let g = glyphes_du_mot(&b, s0, s1).expect("les glyphes de « Séquence »");
    assert!(g.top() >= s0 - 0.5 && g.bottom() <= s1 + 0.5, "le mot tient dans la ligne, accent et jambage compris : {g:?} / {s0}..{s1}");
    for nom in ICONES {
        assert!(dans(nom, f32::MIN, f32::MAX).is_empty(), "« {nom} » n'est plus dans la colonne : {:?}", vus::ou(nom));
    }
    // Les noms des séquences dans la bande, en face : leurs glyphes tiennent aussi dans la ligne.
    let grip = colonne();
    let noms: Vec<Rect> = b
        .formes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.job.text.starts_with("Séquence ") => Some(t.galley.mesh_bounds.translate(t.pos.to_vec2())),
            _ => None,
        })
        .filter(|r| r.left() > grip.right() && r.center().y > s0 - 2.0 && r.center().y < s1 + 2.0)
        .collect();
    assert!(!noms.is_empty(), "les noms des séquences dans la bande");
    for r in noms {
        assert!(r.top() >= s0 - 0.5 && r.bottom() <= s1 + 0.5, "un nom de séquence dans sa ligne : {r:?} / {s0}..{s1}");
    }
}

#[test]
fn nouvelle_sequence_quitte_la_barre_de_lecture_et_marche_au_clic_droit_de_la_ligne_sequence() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    let ((l0, l1), (s0, s1)) = lignes(&b);
    assert!(dans("Nouvelle séquence", l0, l1).is_empty(), "plus dans la barre de lecture");
    // Ni sous « Plus… » de la barre.
    let plus = dans("Plus…", l0, l1)[0];
    clic(&mut b, plus.center(), Modifiers::NONE);
    b.chauffer(2);
    assert!(!textes(&b).iter().any(|(t, _)| t.contains("Nouvelle séquence")), "pas sous « Plus… »");
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    // Partie 196 : au clic droit de la ligne « Séquence ».
    let n = b.app.doc.sequence_order().len();
    clic_droit(&mut b, pos2(colonne().left() - 12.0, (s0 + s1) * 0.5));
    for mot in MENU.iter().copied().chain(["Couper la séquence ici (le plan courant la commence)"]) {
        assert!(texte(&b, mot).is_some(), "« {mot} » au clic droit de la ligne Séquence : {:?}", textes(&b).into_iter().map(|t| t.0).collect::<Vec<_>>());
    }
    let p = texte(&b, "Nouvelle séquence").unwrap().center();
    clic(&mut b, p, Modifiers::NONE);
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
    assert!(mot.left() >= gauche - 0.5 && mot.right() <= grip.left() + 0.5, "« Séquence » dans la colonne : {mot:?} / {gauche}..{}", grip.left());
    for nom in ICONES {
        assert!(dans(nom, s0, s1).is_empty(), "« {nom} » n'y est plus");
    }
}

#[test]
fn les_hauteurs_de_l_en_tete_et_l_espace_montage_inchange() {
    let b = banc_sb(film(), 1920, 1080, false);
    let rows = rangees(&b);
    // L'en-tête de la timeline du Storyboard, au-dessus des plans : la règle (et la barre de lecture en face) + la ligne Séquence.
    assert_eq!(rows.regle_h() + rows.bands_h(), 32.0, "22 + 10 (22 + 20 = 42 avant la partie 196)");
    let mut m = banc_sb(film(), 1920, 1080, false);
    m.app.set_espace(Espace::Montage);
    m.chauffer(6);
    let rm = m.app.montage_rows(300.0);
    assert_eq!(rm.regle_h(), crate::storyboard::RULER_H, "le Montage garde sa règle");
    assert_eq!(rm.bands_h(), crate::storyboard::BANDS_H, "et sa ligne des séquences");
    assert!(vus::ou("Autres commandes des séquences").is_empty(), "pas de ligne « Séquence » dans le Montage");
}
