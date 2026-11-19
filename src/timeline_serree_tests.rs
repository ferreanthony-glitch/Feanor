//! Partie 179 : la timeline resserrée (Anthony, au feutre rouge sur une capture de la timeline du Storyboard : « Bouge les éléments
//! pour resserrer les choses et éviter les pertes d'espace. Réduis un peu l'épaisseur des calques. Justifie toutes les icônes à
//! droite. »).
//!
//! 1. La barre de la colonne du Storyboard sur une seule ligne, à toute largeur ; ce qui n'y tient pas est sous « Plus… », les moins
//!    utilisés d'abord ; plus de bande vide sous la règle.
//! 2. Les commandes des calques (nouveau, dossier, effet, dupliquer, supprimer, ⋯) sur la ligne « Calques », et qui marchent ; plus
//!    rien sous la colonne.
//! 3. Le calque actif sur deux rangées, les indicateurs et les colonnes sur la ligne du nom.
//! 4. Les icônes collées à droite : la ligne « Calques », la piste active, les en-têtes Plans et Caméra.
//! 5. Les pistes moins épaisses ; l'espace Montage garde les siennes.
//! 6. La même chose dans la timeline de l'Animation (les commandes des calques sur la ligne « Calques », le calque courant sur deux
//!    rangées, les lignes moins épaisses).
//! 7. La plus petite fenêtre : rien ne déborde.
//!
//! (Ces essais n'emploient que ce qui existait avant la partie 179 : ils ont d'abord été lancés sur le code d'avant, rouges.)

use eframe::egui::{pos2, Event, Modifiers, Rect};

use crate::barres::{Barre, Genre};
use crate::barrecalques::NOMS;
use crate::bulles::vus;
use crate::layerheads::Colonne;
use crate::perf_tests::Banc;
use crate::pistes_tests::{clic, textes};
use crate::timelinesb_tests::{banc_sb, colonne, film, haut, rangees};
use crate::workspace::Espace;

/// Les commandes de la barre du montage du Storyboard (d'origine) qui s'appliquent maintenant : (identifiant, nom de bulle, groupe).
fn commandes(b: &Banc) -> Vec<(&'static str, String, u8)> {
    let reglage = crate::barres::ReglageBarre::default();
    crate::barres::dans_la_barre(Barre::Montage, &reglage)
        .into_iter()
        // (Partie 179.11 : « Nouvelle séquence » est sur la ligne « Séquence », plus dans la barre.)
        .filter(|c| !crate::barres::HORS_DE_LA_BARRE_DE_LECTURE.contains(&c.id))
        .filter_map(|c| match c.genre {
            Genre::Menu => Some((c.id, c.nom.to_string(), c.groupe)),
            Genre::Bouton | Genre::Etat => {
                let v = b.app.vue_de(Barre::Montage, c);
                v.actif.then_some((c.id, v.nom, c.groupe))
            }
            _ => None,
        })
        .collect()
}

/// La ligne des boutons en haut de la colonne (en face de la règle et de la ligne des séquences) : son haut et son bas à l'écran.
fn ligne_de_la_barre(b: &Banc) -> (f32, f32) {
    let h = haut(b);
    // (Partie 179.11 : la barre est en face de la règle seule ; la ligne « Séquence » est dessous.)
    (h, h + rangees(b).regle_h())
}

/// Le bouton `nom` sur la ligne de la barre, dans la colonne (le même nom peut être ailleurs : l'en-tête des plans).
fn sur_la_ligne(b: &Banc, nom: &str) -> Option<Rect> {
    let (y0, y1) = ligne_de_la_barre(b);
    let grip = colonne();
    vus::ou(nom).into_iter().find(|r| r.center().y > y0 && r.center().y < y1 && r.right() <= grip.left() + 1.0 && r.left() >= grip.left() - 600.0)
}

/// Les noms écrits dans le menu « Plus… » de la ligne de la barre, ouvert le temps de les lire (puis refermé).
fn noms_sous_plus(b: &mut Banc) -> Vec<String> {
    let plus = sur_la_ligne(b, "Plus…").expect("« Plus… » sur la ligne de la barre");
    let avant: Vec<String> = textes(b).into_iter().map(|t| t.0).collect();
    clic(b, plus.center(), Modifiers::NONE);
    b.chauffer(3);
    let pendant: Vec<String> = textes(b).into_iter().map(|t| t.0).collect();
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    pendant.into_iter().filter(|t| !avant.contains(t)).collect()
}

/// Le rectangle de la ligne « Calques » (en-tête des pistes) à l'écran.
fn ligne_calques(b: &Banc) -> (f32, f32) {
    let rows = rangees(b);
    let h = haut(b);
    (h + rows.calques_top(), h + rows.calques_top() + crate::montagerows::SB_CALQUES_H)
}

/// Les éléments nommés `nom` entre `y0` et `y1`, dans la colonne.
fn dans_la_ligne(nom: &str, y0: f32, y1: f32) -> Vec<Rect> {
    let grip = colonne();
    vus::ou(nom).into_iter().filter(|r| r.center().y > y0 && r.center().y < y1 && r.right() <= grip.right() + 1.0).collect()
}

/// Le seul élément nommé `nom` entre `y0` et `y1`, dans la colonne.
fn un(nom: &str, y0: f32, y1: f32) -> Rect {
    let grip = colonne();
    let v: Vec<Rect> = vus::ou(nom).into_iter().filter(|r| r.center().y > y0 && r.center().y < y1 && r.right() <= grip.right() + 1.0).collect();
    assert_eq!(v.len(), 1, "« {nom} » : un seul entre {y0} et {y1} : {v:?} ({:?})", vus::ou(nom));
    v[0]
}

/// La piste du calque actif à l'écran (haut, bas).
fn piste_active(b: &Banc) -> (f32, f32) {
    let rows = rangees(b);
    let h = haut(b) + rows.layers_top();
    let l = rows.lanes.iter().find(|l| !l.folded && !l.est_dossier()).expect("une piste dépliée");
    (h + l.top, h + l.top + l.h)
}

// ------------------------------------------------------------------------------------------------ 1. une seule ligne de boutons

#[test]
fn la_barre_du_storyboard_tient_sur_une_seule_ligne_a_toutes_les_largeurs_et_rien_ne_se_perd() {
    let [wmin, hmin] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    for (w, h) in [(1920u32, 1080u32), (wmin as u32, hmin as u32)] {
        let mut b = banc_sb(film(), w, h, false);
        let defaut = b.app.montage_head_w;
        for largeur in [defaut, crate::montagehead::largeur_min_colonne(Barre::Montage), 432.0, crate::montagehead::HEAD_MAX] {
            b.app.montage_head_w = largeur;
            b.chauffer(6);
            let quoi = format!("{w}×{h}, colonne de {:.0} (réglée {largeur:.0})", b.app.montage_head_w);
            let grip = colonne();
            let (y0, y1) = ligne_de_la_barre(&b);
            // Plus de seconde ligne : les calques commencent juste sous la règle, les séquences, les plans, la caméra et « Calques ».
            let rows = rangees(&b);
            // (Partie 179.11 : la règle a la hauteur de la barre de lecture, `Rows::regle_h`.)
            let attendu = rows.regle_h() + rows.bands_h() + rows.block_h + 1.0 + rows.cam_h() + crate::montagerows::SB_CALQUES_H;
            assert!((rows.layers_top() - attendu).abs() < 0.5, "{quoi} : pas de bande vide sous la règle : {} / {attendu}", rows.layers_top());
            // La lecture et le temps toujours sur la ligne ; chaque autre commande sur la ligne ou sous « Plus… ».
            let mut sur: Vec<(&str, u8, Rect)> = Vec::new();
            let mut cachees: Vec<&str> = Vec::new();
            let plus = noms_sous_plus(&mut b);
            for (id, nom, groupe) in commandes(&b) {
                match sur_la_ligne(&b, &nom) {
                    Some(r) => {
                        assert!(r.left() >= grip.left() - b.app.montage_head_w - 1.0 && r.right() <= grip.left() + 1.0, "{quoi} : « {nom} » entier dans la colonne : {r:?}");
                        sur.push((id, groupe, r));
                    }
                    None => {
                        assert!(groupe > 0, "{quoi} : « {nom} » (la lecture, le temps) toujours sur la ligne");
                        // (Dans la plus petite fenêtre, le menu « Plus… » est plus haut que l'écran : ses dernières lignes ne sont pas
                        // peintes ; la liste de ce qui y passe le dit.)
                        let dedans = plus.iter().any(|t| t.contains(&nom)) || (h < 700 && crate::barres::DEBORDENT.with(|d| d.borrow().contains(&id)));
                        assert!(dedans, "{quoi} : « {nom} » est sous « Plus… » : {plus:?}");
                        cachees.push(id);
                    }
                }
            }
            for (_, _, r) in &sur {
                assert!(r.center().y > y0 && r.center().y < y1, "{quoi} : sur la ligne en face de la règle : {r:?} / {y0} {y1}");
            }
            let plus_r = sur_la_ligne(&b, "Plus…").expect("« Plus… »");
            assert!(plus_r.right() <= grip.left() + 1.0, "{quoi} : « Plus… » dans la colonne");
            // Les moins utilisés d'abord sous « Plus… » : aucune commande montrée n'a un rang plus petit qu'une commande cachée.
            for (id, groupe, _) in &sur {
                if *groupe == 0 {
                    continue;
                }
                for c in &cachees {
                    assert!(crate::barres::rang_dans_la_barre(id) >= crate::barres::rang_dans_la_barre(c), "{quoi} : « {id} » montrée mais « {c} » plus utilisée est cachée");
                }
            }
            eprintln!("179 : {quoi} : sur la ligne {:?} ; sous « Plus… » {cachees:?}", sur.iter().map(|s| s.0).collect::<Vec<_>>());
        }
    }
}

// ------------------------------------------------------------------------------------------------ 2. les commandes des calques

#[test]
fn les_commandes_des_calques_sont_sur_la_ligne_calques_et_marchent() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    let (y0, y1) = ligne_calques(&b);
    let mot = textes(&b).into_iter().find(|(t, r)| t == "Calques" && r.center().y > y0 && r.center().y < y1).expect("« Calques »").1;
    // (Partie 179.5 : les icônes des colonnes ont quitté la ligne ; les commandes sont justifiées à droite, au bord de la colonne.)
    let bord = colonne().left() - 2.5;
    let mut avant = mot.right();
    for nom in NOMS {
        let r = un(nom, y0, y1);
        assert!(r.left() >= avant - 0.5, "« {nom} » après « Calques » et le bouton d'avant, sans chevauchement : {r:?} / {avant}");
        assert!(r.right() <= bord + 0.5, "« {nom} » dans la ligne : {r:?} / {bord}");
        avant = r.right();
    }
    // Plus rien sous la colonne : la barre du bas n'a plus que le zoom, à droite de la colonne.
    let grip = colonne();
    for nom in NOMS {
        assert!(vus::ou(nom).iter().all(|r| r.center().y < y1 + 2.0 || r.left() > grip.right()), "« {nom} » n'est plus sous la colonne : {:?}", vus::ou(nom));
    }
    // Ils marchent : un dossier, puis dupliquer le calque courant (une étape d'annulation chacun).
    let n = b.app.layers().len();
    clic(&mut b, un(NOMS[1], y0, y1).center(), Modifiers::NONE);
    b.chauffer(3);
    assert_eq!(b.app.layers().len(), n + 1, "« Nouveau dossier »");
    assert!(b.app.layers().iter().any(|l| l.folder));
    let (y0, y1) = ligne_calques(&b);
    let n = b.app.layers().len();
    b.app.cur_layer = b.app.layers().iter().position(|l| !l.folder).unwrap();
    b.chauffer(3);
    clic(&mut b, un(NOMS[3], y0, y1).center(), Modifiers::NONE);
    b.chauffer(3);
    assert_eq!(b.app.layers().len(), n + 1, "« Dupliquer les calques choisis »");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.layers().len(), n, "Ctrl+Z défait la copie");
}

// ------------------------------------------------------------------------------------------------ 3, 4, 5. le calque actif, à droite

#[test]
fn le_calque_actif_tient_sur_deux_rangees_et_ses_icones_sont_collees_a_droite_sous_l_en_tete() {
    let b = banc_sb(film(), 1920, 1080, false);
    let rows = rangees(&b);
    let hauteurs: Vec<f32> = rows.lanes.iter().filter(|l| !l.est_dossier()).map(|l| l.h).collect();
    assert_eq!(hauteurs.iter().filter(|h| **h > 30.0).count(), 1, "un seul calque déplié : {hauteurs:?}");
    for h in &hauteurs {
        assert!(*h == 40.0 || *h == 15.0, "le calque actif sur 40 points (66 avant), les autres sur 15 (17 avant) : {hauteurs:?}");
    }
    let (p0, p1) = piste_active(&b);
    let milieu = p0 + 20.0;
    let (e0, e1) = ligne_calques(&b);
    // Sur la ligne du nom : les indicateurs et les colonnes (sauf l'œil, à gauche comme sur toutes les pistes).
    for nom in ["Pochoir positif", "Verrouiller la position"] {
        let r = un(nom, p0, p1);
        assert!(r.center().y < milieu, "« {nom} » sur la ligne du nom : {r:?} / {p0}");
    }
    // (Partie 179.5 : plus d'icônes des colonnes sur la ligne « Calques » ; chaque colonne garde sa case, `case_de_colonne`.)
    let droite = colonne().left();
    for (k, col) in Colonne::TOUTES.iter().enumerate().skip(1) {
        let r = un(col.nom(), p0, p1);
        assert!(r.center().y < milieu, "« {} » sur la ligne du nom : {r:?}", col.nom());
        let attendu = crate::layerheads::case_de_colonne(droite, k, 0.0);
        assert!((r.center().x - attendu.center().x).abs() < 1.6, "« {} » dans sa colonne : {r:?} / {attendu:?}", col.nom());
        assert!(dans_la_ligne(col.nom(), e0, e1).is_empty(), "« {} » n'est plus sur la ligne « Calques »", col.nom());
    }
    // Sur la seconde : le mode, l'opacité, le ⋮.
    for nom in ["Mode de fusion", "Opacité du calque", "Commandes du calque"] {
        let r = un(nom, p0, p1);
        assert!(r.center().y > milieu, "« {nom} » sur la seconde rangée : {r:?} / {milieu}");
    }
    // Collées à droite : la dernière colonne (la caméra), sur la piste et sur la ligne « Calques », et le dernier bouton des en-têtes
    // Plans et Caméra finissent tous au même bord, à 3 points de la poignée de la colonne.
    let bord = colonne().left() - 3.0;
    let h = haut(&b);
    let fins = [
        ("caméra de la piste", un(Colonne::Camera.nom(), p0, p1).right()),
        // (Partie 179.5 : le « ⋯ » des calques a pris la place de la caméra de l'en-tête.)
        ("⋯ de « Calques »", un(NOMS[5], e0, e1).right()),
        ("⋯ des plans", un("Autres commandes des plans", h + rows.blocks_top(), h + rows.blocks_top() + rows.block_h).right()),
        ("⋯ de la caméra", un("Autres commandes de la caméra", h + rows.camera_top(), h + rows.camera_top() + rows.cam_h()).right()),
    ];
    for (quoi, x) in fins {
        assert!((x - bord).abs() < 1.6, "{quoi} : collé à droite, {x} / {bord} ({fins:?})");
    }
}

#[test]
fn l_espace_montage_garde_ses_pistes_et_sa_barre_du_bas() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    b.app.set_espace(Espace::Montage);
    b.chauffer(8);
    let rows = b.app.montage_rows(300.0);
    let hauteurs: Vec<f32> = rows.lanes.iter().filter(|l| !l.est_dossier()).map(|l| l.h).collect();
    assert!(hauteurs.iter().all(|h| *h == 66.0 || *h == 17.0), "les pistes du Montage : 66 et 17, comme avant : {hauteurs:?}");
    assert!(hauteurs.contains(&66.0));
    // Les commandes des calques restent dans sa barre du bas.
    assert!(!vus::ou(NOMS[0]).is_empty(), "« Nouveau calque » dans la barre du bas du Montage");
}

// ------------------------------------------------------------------------------------------------ 6. l'Animation

#[test]
fn dans_l_animation_les_commandes_des_calques_montent_et_le_calque_courant_tient_sur_deux_rangees() {
    let mut b = crate::perf_tests::banc_taille(crate::colonne_calques_tests::doc_174(), false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.settings.calques_replies = false;
    let perso = b.app.layers().iter().position(|l| l.name == "Perso").unwrap();
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, perso, 0);
    b.garder_formes = true;
    b.chauffer(8);
    b.image(vec![Event::PointerMoved(pos2(960.0, 200.0))], 1.0 / 60.0);
    b.chauffer(4);
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    for (li, a, z) in &lignes {
        let attendu = if *li == perso { 40.0 } else { 18.0 };
        assert!((z - a - attendu).abs() < 0.5, "calque {li} : {} points ({attendu} attendus : 60 et 22 avant)", z - a);
    }
    let haut = lignes.iter().map(|l| l.1).fold(f32::MAX, f32::min);
    let (e0, e1) = (haut - crate::layerheads::ENTETE_H - 1.0, haut + 0.5);
    let mot = textes(&b).into_iter().find(|(t, r)| t == "Calques" && r.center().y > e0 && r.center().y < e1).expect("« Calques » sur la rangée d'en-tête").1;
    // (Partie 179.5 : plus d'icônes des colonnes sur la rangée ; les commandes vont jusqu'au bord droit de la colonne, celui des
    // colonnes du calque courant.)
    let (a0, z0) = lignes.iter().find(|l| l.0 == perso).map(|l| (l.1, l.2)).unwrap();
    let bord = vus::ou(Colonne::Camera.nom()).into_iter().find(|r| r.center().y > a0 && r.center().y < z0).expect("la caméra du calque courant").right();
    let mut avant = mot.right();
    for nom in NOMS {
        let r = vus::ou(nom).into_iter().find(|r| r.center().y > e0 && r.center().y < e1).unwrap_or_else(|| panic!("« {nom} » sur la rangée « Calques » : {:?}", vus::ou(nom)));
        assert!(r.left() >= avant - 0.5 && r.right() <= bord + 0.5, "« {nom} » entre « Calques » et le bord : {r:?}");
        avant = r.right();
        assert_eq!(vus::ou(nom).len(), 1, "« {nom} » une seule fois (plus dans la barre du bas) : {:?}", vus::ou(nom));
    }
    // Le calque courant : les colonnes et les indicateurs sur la rangée du nom ; le mode, l'opacité et le ⋮ sur la seconde.
    let (a, z) = lignes.iter().find(|l| l.0 == perso).map(|l| (l.1, l.2)).unwrap();
    let dedans = |nom: &str| vus::ou(nom).into_iter().find(|r| r.center().y > a && r.center().y < z).unwrap_or_else(|| panic!("« {nom} » sur la ligne du calque courant"));
    for nom in ["Pochoir positif", "Verrouiller la position", Colonne::Pelure.nom(), Colonne::Camera.nom()] {
        assert!(dedans(nom).center().y < a + 20.0, "« {nom} » sur la rangée du nom");
    }
    for nom in ["Mode de fusion", "Opacité du calque", "Commandes du calque"] {
        assert!(dedans(nom).center().y > a + 20.0, "« {nom} » sur la seconde rangée");
    }
    // « Nouveau dossier » de la rangée « Calques » marche.
    let n = b.app.layers().len();
    let r = vus::ou(NOMS[1]).into_iter().find(|r| r.center().y > e0 && r.center().y < e1).unwrap();
    clic(&mut b, r.center(), Modifiers::NONE);
    b.chauffer(3);
    assert_eq!(b.app.layers().len(), n + 1, "« Nouveau dossier » de la rangée « Calques »");
}

// ------------------------------------------------------------------------------------------------ 7. la plus petite fenêtre

#[test]
fn a_la_plus_petite_fenetre_rien_ne_deborde_ni_ne_se_chevauche() {
    let [w, h] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let mut b = banc_sb(film(), w as u32, h as u32, false);
    for largeur in [b.app.montage_head_w, crate::montagehead::largeur_min_colonne(Barre::Montage)] {
        b.app.montage_head_w = largeur;
        b.chauffer(6);
        let quoi = format!("{w}×{h}, colonne de {:.0}", b.app.montage_head_w);
        let grip = colonne();
        let gauche = grip.left() - b.app.montage_head_w;
        // La ligne « Calques » : ⊖, « Calques », les commandes (celles qui tiennent), les colonnes : dans l'ordre, sans chevauchement.
        let (e0, e1) = ligne_calques(&b);
        let mut ligne: Vec<(String, Rect)> = std::iter::once("Replier la rangée Calques")
            .chain(NOMS.iter().copied())
            .chain(Colonne::TOUTES.iter().map(|c| c.nom()))
            .filter_map(|n| vus::ou(n).into_iter().find(|r| r.center().y > e0 && r.center().y < e1 && r.right() <= grip.right() + 1.0).map(|r| (n.to_string(), r)))
            .collect();
        assert!(ligne.iter().any(|(n, _)| n == NOMS[0]) && ligne.iter().any(|(n, _)| n == NOMS[5]), "{quoi} : « + » et « ⋯ » toujours là : {ligne:?}");
        ligne.sort_by(|p, q| p.1.left().total_cmp(&q.1.left()));
        for k in 1..ligne.len() {
            assert!(ligne[k].1.left() >= ligne[k - 1].1.right() - 0.5, "{quoi} : {} touche {} : {ligne:?}", ligne[k].0, ligne[k - 1].0);
        }
        assert!(ligne.iter().all(|(_, r)| r.left() >= gauche - 0.5 && r.right() <= grip.left() + 0.5), "{quoi} : la ligne « Calques » dans la colonne : {ligne:?}");
        // La piste active : chaque rangée sans chevauchement, dans la colonne.
        let (p0, p1) = piste_active(&b);
        let noms = ["Afficher / cacher", "Pochoir positif", "Verrouiller la position", "Mode de fusion", "Opacité du calque", "Commandes du calque"];
        let mut piste: Vec<(String, Rect)> = noms
            .iter()
            .map(|n| n.to_string())
            .chain(Colonne::TOUTES[1..].iter().map(|c| c.nom().to_string()))
            .filter_map(|n| vus::ou(&n).into_iter().find(|r| r.center().y > p0 && r.center().y < p1 && r.right() <= grip.right() + 1.0).map(|r| (n, r)))
            .collect();
        assert!(piste.len() >= noms.len() + 5, "{quoi} : tout est là sur la piste active : {piste:?}");
        piste.sort_by(|p, q| p.1.left().total_cmp(&q.1.left()));
        for (k, (n, r)) in piste.iter().enumerate() {
            assert!(r.left() >= gauche - 0.5 && r.right() <= grip.left() + 0.5 && r.top() >= p0 - 0.5 && r.bottom() <= p1 + 0.5, "{quoi} : « {n} » dans la piste : {r:?}");
            for (m, s) in &piste[k + 1..] {
                let meme_rangee = (r.center().y - s.center().y).abs() < 8.0;
                assert!(!meme_rangee || s.left() >= r.right() - 0.5, "{quoi} : « {n} » et « {m} » se chevauchent : {r:?} / {s:?}");
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ captures

/// Les images de contrôle (partie 179), dans `SHOT_CAPTURE_DIR`, à taille réelle : la timeline du Storyboard au cadrage de la capture
/// d'Anthony (colonne de 432 points, sa largeur), la même à la colonne la plus étroite, un gros plan de la colonne, la timeline de
/// l'Animation. `SHOT_CAPTURE_PROJET` : le projet à ouvrir (une copie), enrichi en mémoire (plusieurs calques, un dossier, des plans).
#[test]
#[ignore = "écrit des captures : SHOT_CAPTURE_DIR"]
fn timeline_serree_179_captures() {
    use eframe::egui::vec2;
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let prefixe = std::env::var("SHOT_CAPTURE_PREFIXE").unwrap_or_else(|_| "179".into());
    let charger = || {
        let mut doc = match std::env::var_os("SHOT_CAPTURE_PROJET") {
            Some(p) => crate::project::load(std::path::Path::new(&p)).expect("le projet").doc,
            None => film(),
        };
        while doc.panels.len() < 3 {
            let last = doc.panels.len() - 1;
            doc.add_scene_after(last);
        }
        // Le plan 0 : six calques vectoriels dessinés, dont deux rangés dans un dossier « Dossier 1 ».
        let len = doc.panels[0].frame_count().max(24);
        let mut layers = Vec::new();
        let dos = crate::document::Layer::new_folder("Dossier 1");
        for k in (1..=6).rev() {
            let cel = doc.new_cel(crate::document::LayerKind::Vector);
            let mut l = crate::document::Layer::new(&format!("Vectoriel {k}"), crate::document::LayerKind::Vector);
            l.frames = (0..len).map(|_| Some(cel)).collect();
            if k <= 2 {
                l.parent = Some(dos.id);
            }
            layers.push(l);
        }
        layers.insert(2, dos);
        crate::folders::normalize(&mut layers);
        doc.panels[0].layers = layers;
        doc
    };
    let sauver = |b: &mut Banc, nom: &str, zone: Rect, taille: [u32; 2]| {
        b.chauffer(4);
        let Some(rgba) = b.capturer() else { return println!("CAPTURE pas de carte graphique : {nom}") };
        let img = image::RgbaImage::from_raw(taille[0], taille[1], rgba).expect("image");
        let z = zone.intersect(Rect::from_min_size(pos2(0.0, 0.0), vec2(taille[0] as f32, taille[1] as f32)));
        let crop = image::imageops::crop_imm(&img, z.left() as u32, z.top() as u32, z.width() as u32, z.height() as u32).to_image();
        crop.save(dossier.join(nom)).expect("png");
        println!("CAPTURE {nom} {}x{}", crop.width(), crop.height());
    };
    let storyboard = |doc, w: u32, h: u32| {
        let mut b = crate::perf_tests::banc_taille(doc, true, w, h);
        b.app.set_espace(Espace::Storyboard);
        b.app.settings.storyboard_camera_depliee = false;
        b.app.settings.storyboard_calques_replies = false;
        b.app.montage_zoom = 8.0;
        b.app.cur_panel = 0;
        b.app.cur_layer = b.app.layers().len() - 1;
        // (Partie 179.10 : `SHOT_CAPTURE_DEPLIER=1`, « Déplier le calque actif » allumé ; éteint d'origine.)
        b.app.settings.deplier_calque_actif = std::env::var("SHOT_CAPTURE_DEPLIER").is_ok_and(|v| v == "1");
        // (Partie 179.5 : `SHOT_CAPTURE_ICONES`, une autre taille d'icônes.)
        if let Some(t) = std::env::var("SHOT_CAPTURE_ICONES").ok().and_then(|t| t.parse().ok()) {
            b.app.settings.taille_icones = t;
        }
        b.garder_formes = true;
        b.chauffer(12);
        b
    };
    // La timeline du Storyboard, colonne de 432 points (celle de son poste), au cadrage de sa capture : la colonne et le début de la
    // bande (793 × 360 environ, de la règle à la barre d'état).
    let mut b = storyboard(charger(), 1920, 1080);
    b.app.montage_head_w = 432.0;
    b.chauffer(8);
    let grip = colonne();
    let haut_bande = haut(&b);
    let bas = (haut_bande + rangees(&b).height(b.app.doc.audio_tracks.len()) + 40.0).min(1080.0);
    let gauche = (grip.right() - b.app.montage_head_w - 20.0).max(0.0);
    let z = Rect::from_min_max(pos2(gauche, (haut_bande - 40.0).max(0.0)), pos2(gauche + 793.0, bas));
    sauver(&mut b, &format!("{prefixe}-1-storyboard-colonne-432.png"), z, [1920, 1080]);
    // Partie 179.10 : la piste du calque actif dépliée, en gros plan (sa seconde rangée).
    if b.app.settings.deplier_calque_actif {
        let rows = rangees(&b);
        let rang = crate::montagerows::row_of_layer(b.app.layers(), b.app.cur_layer).unwrap_or(0);
        if let Some(l) = rows.lanes.iter().find(|l| l.row == rang) {
            let y = haut_bande + rows.layers_top() + l.top;
            let z = Rect::from_min_max(pos2(gauche, y - 4.0), pos2(grip.right() + 4.0, y + l.h + 4.0));
            sauver(&mut b, &format!("{prefixe}-7-calque-actif-deplie.png"), z, [1920, 1080]);
        }
    }
    // Gros plan de la colonne.
    let z = Rect::from_min_max(pos2(gauche, (haut_bande - 4.0).max(0.0)), pos2(grip.right() + 30.0, bas));
    sauver(&mut b, &format!("{prefixe}-3-gros-plan-colonne.png"), z, [1920, 1080]);
    // La largeur d'origine (340).
    b.app.montage_head_w = crate::montagehead::LARGEUR_STORYBOARD;
    b.chauffer(8);
    let grip = colonne();
    let gauche = (grip.right() - b.app.montage_head_w - 20.0).max(0.0);
    let z = Rect::from_min_max(pos2(gauche, (haut_bande - 40.0).max(0.0)), pos2(gauche + 793.0, bas));
    sauver(&mut b, &format!("{prefixe}-2a-storyboard-colonne-340.png"), z, [1920, 1080]);
    // La plus étroite.
    b.app.montage_head_w = crate::montagehead::largeur_min_colonne(Barre::Montage);
    b.chauffer(8);
    let grip = colonne();
    let gauche = (grip.right() - b.app.montage_head_w - 20.0).max(0.0);
    let z = Rect::from_min_max(pos2(gauche, (haut_bande - 40.0).max(0.0)), pos2(gauche + 793.0, bas));
    sauver(&mut b, &format!("{prefixe}-2b-storyboard-colonne-la-plus-etroite.png"), z, [1920, 1080]);
    // La plus petite fenêtre, toute l'image.
    let [w, hh] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let mut p = storyboard(charger(), w as u32, hh as u32);
    sauver(&mut p, &format!("{prefixe}-4-plus-petite-fenetre.png"), Rect::from_min_size(pos2(0.0, 0.0), vec2(w, hh)), [w as u32, hh as u32]);
    // La timeline de l'Animation (le plan 0), de la colonne des calques au début de la grille.
    let mut a = crate::perf_tests::banc_taille(charger(), true, 1920, 1080);
    a.app.set_mode(crate::app::Mode::Animation);
    a.app.settings.calques_replies = false;
    a.app.cur_panel = 0;
    a.app.cur_layer = a.app.layers().len() - 1;
    if let Some(t) = std::env::var("SHOT_CAPTURE_ICONES").ok().and_then(|t| t.parse().ok()) {
        a.app.settings.taille_icones = t;
    }
    a.app.settings.deplier_calque_actif = std::env::var("SHOT_CAPTURE_DEPLIER").is_ok_and(|v| v == "1");
    a.garder_formes = true;
    a.chauffer(12);
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    let haut_l = lignes.iter().map(|l| l.1).fold(f32::MAX, f32::min);
    let bas_l = lignes.iter().map(|l| l.2).fold(0.0, f32::max);
    // (Partie 179.10 : repliés d'origine, le ⊕ s'appelle « Déplier les calques ».)
    let gauche = vus::ou("Replier tous les calques").iter().chain(vus::ou("Déplier les calques").iter()).map(|r| r.left()).fold(f32::MAX, f32::min) - 8.0;
    let z = Rect::from_min_max(pos2(gauche.max(0.0), (haut_l - 90.0).max(0.0)), pos2(gauche.max(0.0) + 793.0, (bas_l + 60.0).min(1080.0)));
    sauver(&mut a, &format!("{prefixe}-5-animation.png"), z, [1920, 1080]);
    // Partie 179.5 : Préférences > Apparence, le groupe « Tailles ».
    let mut p = crate::perf_tests::banc_taille(charger(), true, 1920, 1080);
    p.app.settings.prefs_onglet = crate::prefsonglets::Onglet::Apparence;
    p.app.run_action("fichier.preferences");
    p.garder_formes = true;
    p.chauffer(10);
    if let Some(r) = crate::pistes_tests::textes(&p).into_iter().find(|(t, _)| t == "Taille des icônes").map(|(_, r)| r) {
        let z = Rect::from_min_max(pos2((r.left() - 30.0).max(0.0), (r.top() - 90.0).max(0.0)), pos2(r.left() + 560.0, r.bottom() + 50.0));
        sauver(&mut p, &format!("{prefixe}-6-preferences-tailles.png"), z, [1920, 1080]);
    }
    crate::tailleicones::poser(crate::tailleicones::TAILLE_DEFAUT);
}
