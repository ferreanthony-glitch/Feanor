//! Partie 179.5 (Anthony, sur la capture du Storyboard après la partie 179 : le groupe +, dossier, effet, dupliquer, supprimer, ⋯
//! de la ligne « ⊖ Calques » entouré, une flèche vers la droite qui arrive sur les icônes des colonnes, barrées : « repositionne les
//! icônes comme sur le screenshot. Les icônes sur la ligne de séquence gardent la même taille que sur le reste des autres lignes.
//! Pareil pour les icônes de l'interface de visionnage au-dessus de la timeline. Tous les icônes doivent avoir cette taille. La taille
//! des icônes et de l'interface doit être réglable dans les préférences. »).
//!
//! 1. La ligne « Calques » : les commandes à droite, au bord de la colonne ; plus d'icônes des colonnes (Storyboard et Animation).
//! 2. Le calque actif montre chaque réglage en icône, pâle quand il est éteint (plus de petit point) : c'est lui qui dit quelle
//!    colonne est laquelle.
//! 3. Une seule taille d'icônes : les lignes Plans, Caméra, Calques, le calque actif et la barre de lecture au-dessus de la timeline ;
//!    les cases de la barre sont plus petites.
//! 4. « Taille des icônes » dans Préférences > Apparence, à côté de « Taille de l'interface » : retenue, tout de suite.

use eframe::egui::{Event, Modifiers, Rect};
use egui_phosphor::regular as ph;

use crate::barrecalques::NOMS;
use crate::bulles::vus;
use crate::layerheads::Colonne;
use crate::perf_tests::Banc;
use crate::pistes_tests::{clic, textes};
use crate::settings::Settings;
use crate::tailleicones::{taille_valide, TAILLES, TAILLE_DEFAUT};
use crate::timelinesb_tests::{banc_sb, colonne, film, haut, rangees};

/// La ligne « Calques » à l'écran (haut, bas).
fn ligne_calques(b: &Banc) -> (f32, f32) {
    let rows = rangees(b);
    let h = haut(b);
    (h + rows.calques_top(), h + rows.calques_top() + crate::montagerows::SB_CALQUES_H)
}

/// La piste du calque actif à l'écran (haut, bas).
fn piste_active(b: &Banc) -> (f32, f32) {
    let rows = rangees(b);
    let h = haut(b) + rows.layers_top();
    let l = rows.lanes.iter().find(|l| !l.folded && !l.est_dossier()).expect("une piste dépliée");
    (h + l.top, h + l.top + l.h)
}

/// La ligne de la barre de lecture, en haut de la colonne (en face de la règle et de la ligne des séquences).
fn ligne_de_la_barre(b: &Banc) -> (f32, f32) {
    let h = haut(b);
    // (Partie 179.11 : la barre est en face de la règle seule.)
    (h, h + rangees(b).regle_h())
}

/// Les éléments nommés `nom` entre `y0` et `y1`, dans la colonne.
fn dans(nom: &str, y0: f32, y1: f32) -> Vec<Rect> {
    let grip = colonne();
    vus::ou(nom).into_iter().filter(|r| r.center().y > y0 && r.center().y < y1 && r.right() <= grip.right() + 1.0).collect()
}

/// La hauteur de la ligne de texte de l'icône `glyphe` peinte entre `y0` et `y1` dans la colonne (elle dit la taille de la police).
fn hauteur_du_glyphe(b: &Banc, glyphe: &str, y0: f32, y1: f32) -> Option<f32> {
    let grip = colonne();
    textes(b).into_iter().find(|(t, r)| t == glyphe && r.center().y > y0 && r.center().y < y1 && r.right() <= grip.left() + 1.0).map(|(_, r)| r.height())
}

// ------------------------------------------------------------------------------------------------ 1. la ligne « Calques »

#[test]
fn les_commandes_des_calques_vont_a_droite_et_les_icones_des_colonnes_quittent_la_ligne_calques() {
    let b = banc_sb(film(), 1920, 1080, false);
    let (y0, y1) = ligne_calques(&b);
    for col in Colonne::TOUTES {
        assert!(dans(col.nom(), y0, y1).is_empty(), "« {} » n'est plus sur la ligne « Calques » : {:?}", col.nom(), dans(col.nom(), y0, y1));
    }
    let mot = textes(&b).into_iter().find(|(t, r)| t == "Calques" && r.center().y > y0 && r.center().y < y1).expect("« Calques » à gauche").1;
    let mut avant = mot.right();
    for nom in NOMS {
        let v = dans(nom, y0, y1);
        assert_eq!(v.len(), 1, "« {nom} » une fois sur la ligne : {v:?}");
        assert!(v[0].left() >= avant - 0.5, "« {nom} » après « Calques » et le bouton d'avant : {:?} / {avant}", v[0]);
        avant = v[0].right();
    }
    // Justifiées à droite : « ⋯ » finit au bord droit de la ligne, comme le ⋯ des plans et de la caméra au-dessus.
    let bord = colonne().left() - 3.0;
    assert!((avant - bord).abs() < 1.6, "le « ⋯ » des calques collé à droite : {avant} / {bord}");
    let h = haut(&b);
    let rows = rangees(&b);
    let plans = dans("Autres commandes des plans", h + rows.blocks_top(), h + rows.blocks_top() + rows.block_h);
    assert!((plans[0].right() - avant).abs() < 1.6, "au même bord que le ⋯ des plans : {:?} / {avant}", plans[0]);
    // Une large place vide entre « Calques » et « + » : les commandes ne sont plus collées au mot.
    let plus = dans(NOMS[0], y0, y1)[0];
    assert!(plus.left() - mot.right() > 60.0, "le « + » loin du mot, à droite : {plus:?} / {mot:?}");
}

#[test]
fn dans_l_animation_aussi_les_commandes_vont_a_droite_et_la_rangee_calques_n_a_plus_les_colonnes() {
    let mut b = crate::perf_tests::banc_taille(crate::colonne_calques_tests::doc_174(), false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.settings.calques_replies = false;
    let perso = b.app.layers().iter().position(|l| l.name == "Perso").unwrap();
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, perso, 0);
    b.garder_formes = true;
    b.chauffer(8);
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    let haut = lignes.iter().map(|l| l.1).fold(f32::MAX, f32::min);
    let (e0, e1) = (haut - crate::layerheads::ENTETE_H - 1.0, haut + 0.5);
    let sur = |nom: &str| vus::ou(nom).into_iter().filter(|r| r.center().y > e0 && r.center().y < e1).collect::<Vec<_>>();
    for col in Colonne::TOUTES {
        assert!(sur(col.nom()).is_empty(), "« {} » n'est plus sur la rangée « Calques » : {:?}", col.nom(), sur(col.nom()));
    }
    let replier = sur("Replier tous les calques");
    assert_eq!(replier.len(), 1, "⊖ à gauche");
    let fin = sur(NOMS[5]);
    assert_eq!(fin.len(), 1);
    // Le bord droit de la colonne : celui de la ligne du calque courant (sa dernière colonne, la caméra, y finit à 3 points du bord).
    let (a, z) = lignes.iter().find(|l| l.0 == perso).map(|l| (l.1, l.2)).unwrap();
    let camera = vus::ou(Colonne::Camera.nom()).into_iter().find(|r| r.center().y > a && r.center().y < z).expect("la caméra du calque courant");
    assert!((fin[0].right() - camera.right()).abs() < 1.6, "le « ⋯ » des calques collé à droite, au bord des colonnes : {:?} / {camera:?}", fin[0]);
}

// ------------------------------------------------------------------------------------------------ 2. le calque actif en icônes

#[test]
fn le_calque_actif_montre_ses_reglages_eteints_en_icones_pales() {
    let b = banc_sb(film(), 1920, 1080, false);
    let (p0, p1) = piste_active(&b);
    let l = &b.app.layers()[b.app.cur_layer];
    let grip = colonne();
    for col in &Colonne::TOUTES[1..] {
        let Some(on) = col.etat(l) else { continue };
        let ici = textes(&b).into_iter().any(|(t, r)| t == col.icone() && r.center().y > p0 && r.center().y < p0 + 20.0 && r.right() <= grip.left() + 1.0);
        assert!(ici, "« {} » ({}) : son icône sur la rangée du nom, même éteinte", col.nom(), if on { "allumé" } else { "éteint" });
    }
    let _ = p1;
}

#[test]
fn dans_l_animation_le_calque_courant_montre_aussi_ses_reglages_eteints() {
    let mut b = crate::perf_tests::banc_taille(crate::colonne_calques_tests::doc_174(), false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.settings.calques_replies = false;
    let perso = b.app.layers().iter().position(|l| l.name == "Perso").unwrap();
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, perso, 0);
    b.app.layers_mut()[perso].onion = false;
    b.garder_formes = true;
    b.chauffer(8);
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    let (a, _) = lignes.iter().find(|l| l.0 == perso).map(|l| (l.1, l.2)).unwrap();
    let ici = textes(&b).into_iter().any(|(t, r)| t == Colonne::Pelure.icone() && r.center().y > a && r.center().y < a + 20.0);
    assert!(ici, "la pelure éteinte du calque courant : son icône pâle");
}

// ------------------------------------------------------------------------------------------------ 3. une seule taille

#[test]
fn toutes_les_icones_de_la_timeline_du_storyboard_ont_la_meme_taille_barre_de_lecture_comprise() {
    let b = banc_sb(film(), 1920, 1080, false);
    let h = haut(&b);
    let rows = rangees(&b);
    let (l0, l1) = ligne_de_la_barre(&b);
    let (c0, c1) = ligne_calques(&b);
    let (p0, p1) = piste_active(&b);
    let mesures = [
        ("lire, barre de lecture", hauteur_du_glyphe(&b, ph::PLAY, l0, l1)),
        ("début, barre de lecture", hauteur_du_glyphe(&b, ph::SKIP_BACK, l0, l1)),
        ("nouveau plan, barre de lecture", hauteur_du_glyphe(&b, ph::PLUS_SQUARE, l0, l1)),
        ("nouveau plan, ligne Plans", hauteur_du_glyphe(&b, ph::PLUS_SQUARE, h + rows.blocks_top(), h + rows.blocks_top() + rows.block_h)),
        ("couper, ligne Plans", hauteur_du_glyphe(&b, ph::SCISSORS, h + rows.blocks_top(), h + rows.blocks_top() + rows.block_h)),
        ("remise à zéro, ligne Caméra", hauteur_du_glyphe(&b, ph::ARROW_COUNTER_CLOCKWISE, h + rows.camera_top(), h + rows.camera_top() + rows.cam_h())),
        ("nouveau dossier, ligne Calques", hauteur_du_glyphe(&b, ph::FOLDER_PLUS, c0, c1)),
        ("supprimer, ligne Calques", hauteur_du_glyphe(&b, ph::TRASH, c0, c1)),
        // (Partie 179.15 : plus de punaise sur la ligne ; le losange de l'opacité.)
        ("losange, calque actif", hauteur_du_glyphe(&b, ph::DIAMOND, p0, p1)),
        ("pelure, calque actif", hauteur_du_glyphe(&b, Colonne::Pelure.icone(), p0, p1)),
    ];
    let reference = mesures[3].1.expect("le nouveau plan de la ligne Plans");
    for (quoi, m) in mesures {
        let m = m.unwrap_or_else(|| panic!("{quoi} : l'icône est peinte"));
        assert!((m - reference).abs() < 0.5, "{quoi} : {m} points de haut, comme les icônes de la ligne Plans ({reference})");
    }
    // Les cases de la barre de lecture : plus petites (20 points d'origine ; 26 avant), dans la ligne.
    let lire = dans("Lire le montage / pause", l0, l1);
    assert_eq!(lire.len(), 1, "« Lire » sur la ligne de la barre");
    assert!(lire[0].height() <= 21.0 && lire[0].height() >= 17.0, "la case de « Lire » : {:?}", lire[0]);
}

#[test]
fn dans_l_animation_la_barre_de_lecture_a_la_meme_taille_d_icones_et_des_lignes_moins_hautes() {
    let mut b = crate::perf_tests::banc_taille(crate::colonne_calques_tests::doc_174(), false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.settings.calques_replies = false;
    b.garder_formes = true;
    b.chauffer(8);
    let lire = vus::ou("Lecture / pause");
    assert!(!lire.is_empty(), "« Lecture / pause » dans la barre de la timeline");
    assert!(lire[0].height() <= 21.0, "la case de « Lecture / pause » : {:?}", lire[0]);
    let lire_glyphe = textes(&b).into_iter().find(|(t, r)| t == ph::PLAY && (r.center().y - lire[0].center().y).abs() < 6.0).expect("l'icône de lecture").1;
    let dossier = textes(&b).into_iter().filter(|(t, _)| t == ph::FOLDER_PLUS).map(|(_, r)| r).next().expect("le nouveau dossier de la rangée « Calques »");
    assert!((lire_glyphe.height() - dossier.height()).abs() < 0.5, "la même taille : {lire_glyphe:?} / {dossier:?}");
    // Les deux lignes de boutons : 24 points chacune (30 et 28 avant).
    assert_eq!(crate::timeline::LIGNE1_H, 24.0);
    assert_eq!(crate::timeline::LIGNE2_H, 24.0);
}

// ------------------------------------------------------------------------------------------------ 4. le réglage

#[test]
fn la_taille_des_icones_est_retenue_dans_les_reglages_et_l_origine_ne_laisse_pas_de_trace() {
    assert_eq!(TAILLES, [11, 12, 13, 14, 15, 16]);
    assert_eq!(TAILLE_DEFAUT, 13);
    for (ecrit, valide) in [(0, 11), (11, 11), (13, 13), (14, 14), (16, 16), (40, 16)] {
        assert_eq!(taille_valide(ecrit), valide, "{ecrit}");
    }
    let mut s = Settings::default();
    assert_eq!(s.taille_icones, 13);
    assert!(!toml::to_string(&s).unwrap().contains("taille_icones"), "13 : la clé n'est pas écrite");
    s.taille_icones = 15;
    let ecrit = toml::to_string(&s).unwrap();
    assert!(ecrit.contains("taille_icones = 15"), "{ecrit}");
    let relu: Settings = toml::from_str(&ecrit).unwrap();
    assert_eq!(relu.taille_icones, 15);
    let ancien: Settings = toml::from_str("").unwrap();
    assert_eq!(ancien.taille_icones, 13, "des réglages d'avant : 13");
}

#[test]
fn une_autre_taille_d_icones_agit_tout_de_suite_sur_toute_la_timeline_et_rien_ne_deborde() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    let mesure = |b: &Banc| {
        let h = haut(b);
        let rows = rangees(b);
        let (l0, l1) = ligne_de_la_barre(b);
        let (c0, c1) = ligne_calques(b);
        let plans = hauteur_du_glyphe(b, ph::PLUS_SQUARE, h + rows.blocks_top(), h + rows.blocks_top() + rows.block_h).expect("ligne Plans");
        let barre = hauteur_du_glyphe(b, ph::PLAY, l0, l1).expect("barre");
        let calques = hauteur_du_glyphe(b, ph::TRASH, c0, c1).expect("ligne Calques");
        let lire = dans("Lire le montage / pause", l0, l1)[0];
        (plans, barre, calques, lire)
    };
    let (p13, b13, c13, l13) = mesure(&b);
    for t in [16u8, 11] {
        b.app.settings.taille_icones = t;
        b.chauffer(6);
        let (p, barre, c, lire) = mesure(&b);
        let plus_grand = t > 13;
        assert_eq!(p > p13 + 0.5, plus_grand, "{t} : la ligne Plans suit : {p} / {p13}");
        assert_eq!(p < p13 - 0.5, !plus_grand, "{t} : la ligne Plans suit : {p} / {p13}");
        assert!((barre - p).abs() < 0.5 && (c - p).abs() < 0.5, "{t} : la barre et la ligne Calques ont la même taille : {barre} {c} / {p}");
        assert_eq!(lire.height() > l13.height(), plus_grand, "{t} : la case de « Lire » suit : {lire:?} / {l13:?}");
        // Rien ne sort de la colonne : le « ⋯ » des calques et celui des plans restent dans la ligne, au bord.
        let (y0, y1) = ligne_calques(&b);
        let fin = dans(NOMS[5], y0, y1);
        assert_eq!(fin.len(), 1, "{t} : le « ⋯ » des calques");
        assert!(fin[0].right() <= colonne().left() + 0.5, "{t} : dans la colonne : {:?}", fin[0]);
    }
    let _ = (b13, c13);
    // (La taille est retenue pour le fil : les essais suivants de ce fil la retrouvent d'origine.)
    crate::tailleicones::poser(TAILLE_DEFAUT);
}

#[test]
fn les_preferences_montrent_la_taille_de_l_interface_et_la_taille_des_icones_cote_a_cote() {
    let mut b = crate::perf_tests::banc_taille(crate::document::Document::with_frame(320, 180, 24), false, 1600, 900);
    b.garder_formes = true;
    b.chauffer(6);
    b.app.settings.prefs_onglet = crate::prefsonglets::Onglet::Apparence;
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    let t = textes(&b);
    let interface = t.iter().find(|(x, _)| x == "Taille de l'interface").expect("« Taille de l'interface »").1;
    let icones = t.iter().find(|(x, _)| x == "Taille des icônes").expect("« Taille des icônes »").1;
    assert!(icones.top() > interface.top() && icones.top() - interface.bottom() < 30.0, "l'une sous l'autre : {interface:?} / {icones:?}");
    for t in TAILLES {
        let nom = if t == TAILLE_DEFAUT { format!("Icônes de {t} points (d'origine)") } else { format!("Icônes de {t} points") };
        assert_eq!(vus::ou(&nom).len(), 1, "« {nom} »");
    }
    // La taille de l'interface, juste au-dessus, marche toujours (puis revient à 100 %). (Sans un premier clic dans la fenêtre, le
    // banc ne livrait pas le clic sur la première icône : la fenêtre ne prenait la main qu'au clic suivant.)
    let e = vus::ou("125 %").into_iter().find(|q| (q.center().y - interface.center().y).abs() < 8.0).expect("« 125 % » sur sa ligne");
    clic(&mut b, e.center(), Modifiers::NONE);
    assert_eq!(b.app.settings.echelle, 125);
    b.app.settings.echelle = 100;
    b.chauffer(3);
    let r = vus::ou("Icônes de 15 points")[0];
    clic(&mut b, r.center(), Modifiers::NONE);
    assert_eq!(b.app.settings.taille_icones, 15, "le clic choisit 15 : {r:?}, la ligne {icones:?}");
    assert_eq!(crate::tailleicones::icone(), 15.0, "tout de suite");
    let disque = std::fs::read_to_string(crate::donnees::roaming().unwrap().join("reglages.toml")).unwrap();
    assert!(disque.contains("taille_icones = 15"), "{disque}");
    let r = vus::ou("Icônes de 13 points (d'origine)")[0];
    clic(&mut b, r.center(), Modifiers::NONE);
    let disque = std::fs::read_to_string(crate::donnees::roaming().unwrap().join("reglages.toml")).unwrap();
    assert!(!disque.contains("taille_icones"), "13 : plus de clé : {disque}");
    b.image(vec![Event::PointerMoved(eframe::egui::pos2(5.0, 5.0))], 1.0 / 60.0);
    crate::tailleicones::poser(TAILLE_DEFAUT);
}
