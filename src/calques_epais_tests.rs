//! Parties 179.10 à 179.14 : l'épaisseur des calques de la timeline, son option, la hauteur et la couleur de chaque ligne.
//!
//! - 179.13 (Anthony, sur la capture du calque déplié : « Il faut que tous les calques aient cette épaisseur de base. Ajoute dans les
//!   paramètres la possibilité d'ajuster la taille de chaque ligne. Séquence, plan, caméra, calques. ») : d'origine, tous les calques
//!   sont sur deux rangées, le calque actif seulement surligné ; « Replier les calques » les met tous sur une ligne fine ; la hauteur
//!   de chaque ligne se règle dans les Préférences.
//! - 179.14 (« Ajoute aussi dans les réglages la possibilité de donner une couleur à chaque ligne ») : la couleur de chaque ligne
//!   teinte son fond, dans la colonne et dans la bande.
//! - 179.10 : sur la seconde rangée, ‹ ›, le losange et le ⋮ au niveau du mode et de l'opacité.

use eframe::egui::{epaint::Shape, Color32, Event, Modifiers, Rect};
use egui_phosphor::regular as ph;

use crate::barrecalques::NOMS;
use crate::bulles::vus;
use crate::hauteurstimeline::Ligne;
use crate::layerheads::Colonne;
use crate::perf_tests::Banc;
use crate::pistes_tests::{clic, textes};
use crate::settings::Settings;
use crate::timelinesb_tests::{banc_sb, blocs, colonne, film, haut, rangees};
use crate::workspace::Espace;

/// Les pistes de calques du Storyboard (sans les lignes de dossier) : (rangée, hauteur).
fn hauteurs(b: &Banc) -> Vec<(usize, f32)> {
    rangees(b).lanes.iter().filter(|l| !l.est_dossier()).map(|l| (l.row, l.h)).collect()
}

/// La piste du calque actif à l'écran (haut, bas).
fn piste_du_calque_actif(b: &Banc) -> (f32, f32) {
    let rows = rangees(b);
    let h = haut(b) + rows.layers_top();
    let rang = crate::montagerows::row_of_layer(b.app.layers(), b.app.cur_layer).expect("la rangée du calque actif");
    let l = rows.lanes.iter().find(|l| l.row == rang).expect("la piste du calque actif");
    (h + l.top, h + l.top + l.h)
}

fn banc_animation() -> Banc {
    let mut b = crate::perf_tests::banc_taille(crate::colonne_calques_tests::doc_174(), false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    let perso = b.app.layers().iter().position(|l| l.name == "Perso").unwrap();
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, perso, 0);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

/// Les rectangles pleins de couleur `c` peints à la dernière image.
fn pleins(b: &Banc, c: Color32) -> Vec<Rect> {
    fn chercher(s: &Shape, c: Color32, out: &mut Vec<Rect>) {
        match s {
            Shape::Rect(r) if r.fill == c => out.push(r.rect),
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, c, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, c, &mut out);
    }
    out
}

// ------------------------------------------------------------------------------------------------ 179.13 : tous les calques épais

#[test]
fn d_origine_tous_les_calques_du_storyboard_ont_deux_rangees_et_ne_changent_pas_de_taille() {
    assert!(!Settings::default().replier_calques, "« Replier les calques » éteint d'origine");
    let mut b = banc_sb(film(), 1920, 1080, false);
    let h = hauteurs(&b);
    assert!(h.len() >= 3 && h.iter().all(|(_, h)| (*h - 40.0).abs() < 0.5), "tous les calques sur 40 points : {h:?}");
    let n = b.app.layers().iter().filter(|l| !l.folder).count();
    assert_eq!(vus::ou("Mode de fusion").len(), n, "le mode de chaque calque sur sa ligne");
    assert_eq!(vus::ou("Opacité du calque").len(), n, "l'opacité de chaque calque");
    assert_eq!(vus::ou("Créer une clé").len(), n, "le losange de chaque calque");
    assert_eq!(vus::ou(Colonne::Pelure.nom()).len(), n, "les réglages de chaque calque");
    // Un autre calque actif : aucune hauteur ne change.
    b.app.cur_layer = 1;
    b.chauffer(4);
    assert_eq!(hauteurs(&b), h, "le calque actif ne change pas de taille");
}

#[test]
fn le_mode_et_l_opacite_se_reglent_sur_la_ligne_d_un_calque_qui_n_est_pas_l_actif_et_sur_les_calques_choisis() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    assert_eq!(b.app.cur_layer, 0);
    let rows = rangees(&b);
    let h = haut(&b) + rows.layers_top();
    let autre = rows.lanes.iter().find(|l| !l.est_dossier() && l.row != crate::montagerows::row_of_layer(b.app.layers(), 0).unwrap()).copied().expect("un autre calque");
    let k = crate::montagerows::layer_in_row(b.app.layers(), autre.row).unwrap();
    let (y0, y1) = (h + autre.top, h + autre.top + autre.h);
    let barre = vus::ou("Opacité du calque").into_iter().find(|r| r.center().y > y0 && r.center().y < y1).expect("la barre de l'autre calque");
    clic(&mut b, eframe::egui::pos2(barre.left() + barre.width() * 0.25, barre.center().y), Modifiers::NONE);
    let o = b.app.layers()[k].opacity;
    assert!(o < 0.4, "l'opacité de ce calque-là : {o}");
    assert!((b.app.layers()[0].opacity - 1.0).abs() < 1e-4, "pas celle du calque actif");
    // Deux calques choisis : la barre de l'un règle les deux (« la sélection agit sur tout »).
    b.app.layer_sel = vec![b.app.layers()[0].id, b.app.layers()[k].id];
    b.chauffer(2);
    let barre = vus::ou("Opacité du calque").into_iter().find(|r| r.center().y > y0 && r.center().y < y1).unwrap();
    clic(&mut b, eframe::egui::pos2(barre.left() + barre.width() * 0.75, barre.center().y), Modifiers::NONE);
    let (a, c) = (b.app.layers()[0].opacity, b.app.layers()[k].opacity);
    assert!((a - c).abs() < 0.02 && a > 0.6, "les deux calques choisis : {a} / {c}");
}

#[test]
fn d_origine_dans_l_animation_tous_les_calques_ont_leur_ligne_complete_et_les_dossiers_une_ligne_fine() {
    let b = banc_animation();
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    for (li, a, z) in &lignes {
        let attendu = if b.app.layers()[*li].folder { crate::layerheads::ROW_FINE } else { crate::layerheads::ROW_FULL };
        assert!((z - a - attendu).abs() < 0.5, "calque {li} : {} points, {attendu} attendus : {lignes:?}", z - a);
    }
}

#[test]
fn replier_les_calques_dans_le_menu_de_la_ligne_calques_les_met_tous_sur_une_ligne_fine() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    let rows = rangees(&b);
    let h = haut(&b);
    let (y0, y1) = (h + rows.calques_top(), h + rows.calques_top() + crate::montagerows::SB_CALQUES_H);
    let plus = vus::ou(NOMS[5]).into_iter().find(|r| r.center().y > y0 && r.center().y < y1).expect("le ⋯ de la ligne Calques");
    clic(&mut b, plus.center(), Modifiers::NONE);
    b.chauffer(2);
    let case = textes(&b).into_iter().find(|(t, _)| t == "Replier les calques").expect("« Replier les calques » dans le ⋯").1;
    clic(&mut b, case.center(), Modifiers::NONE);
    assert!(b.app.settings.replier_calques, "allumé");
    assert!(Settings::load().replier_calques, "gardé dans les réglages du poste");
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(4);
    let hs = hauteurs(&b);
    assert!(hs.iter().all(|(_, h)| (*h - crate::pistes::FOLDED_H).abs() < 0.5), "tous sur 15 points : {hs:?}");
    assert!(vus::ou("Mode de fusion").is_empty());
    // Le calque actif replié garde ses réglages en icônes, à droite.
    let (p0, p1) = piste_du_calque_actif(&b);
    assert!(vus::ou(Colonne::Pelure.nom()).iter().any(|r| r.center().y > p0 && r.center().y < p1), "la pelure du calque actif replié");
    // L'Animation suit le même réglage ; éteint de nouveau : tout de suite.
    let mut a = banc_animation();
    a.app.regler_replier_calques(true);
    a.chauffer(4);
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    assert!(lignes.iter().all(|l| (l.2 - l.1 - crate::layerheads::ROW_FINE).abs() < 0.5), "Animation : toutes fines : {lignes:?}");
    b.app.regler_replier_calques(false);
    b.chauffer(4);
    assert!(hauteurs(&b).iter().all(|(_, h)| (*h - 40.0).abs() < 0.5));
}

#[test]
fn les_reglages_sont_ecrits_seulement_quand_ils_changent_et_sont_dans_les_preferences() {
    let mut s = Settings::default();
    let t = toml::to_string(&s).unwrap();
    for cle in ["replier_calques", "hauteur_ligne_", "couleur_ligne_", "deplier_calque_actif"] {
        assert!(!t.contains(cle), "« {cle} » absent d'origine : {t}");
    }
    s.replier_calques = true;
    s.hauteur_plans = 40;
    s.couleur_ligne_camera = Some([200, 40, 40]);
    let t = toml::to_string(&s).unwrap();
    assert!(t.contains("replier_calques = true") && t.contains("hauteur_ligne_plans = 40") && t.contains("couleur_ligne_camera"), "{t}");
    let relu: Settings = toml::from_str(&t).unwrap();
    assert!(relu.replier_calques && relu.hauteur_plans == 40 && relu.couleur_ligne_camera == Some([200, 40, 40]));
    // Un ancien `deplier_calque_actif` n'est pas repris (pas d'erreur non plus).
    let ancien: Settings = toml::from_str("deplier_calque_actif = true").unwrap();
    assert!(!ancien.replier_calques);
    for id in ["montage.replier_calques", "apparence.ligne_sequence", "apparence.ligne_plans", "apparence.ligne_camera", "apparence.ligne_calques"] {
        assert!(crate::prefsonglets::REGLAGES.iter().any(|r| r.id == id), "« {id} » dans les Préférences");
    }
}

#[test]
fn l_espace_montage_garde_ses_hauteurs_et_ses_couleurs() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    b.app.settings.hauteur_calques = 56;
    b.app.settings.hauteur_camera = 36;
    b.app.set_espace(Espace::Montage);
    b.chauffer(8);
    let rows = b.app.montage_rows(300.0);
    assert!(rows.lanes.iter().any(|l| l.h == 66.0), "le Montage : sa piste de 66 : {:?}", rows.lanes);
    assert_eq!(rows.cam_h(), crate::montagerows::CAM_H, "sa caméra");
}

// ------------------------------------------------------------------------------------------------ 179.13 : la hauteur des lignes

#[test]
fn la_hauteur_de_chaque_ligne_se_regle_et_les_blocs_la_suivent() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    (b.app.settings.hauteur_sequence, b.app.settings.hauteur_plans, b.app.settings.hauteur_camera, b.app.settings.hauteur_calques) = (28, 48, 30, 56);
    b.chauffer(6);
    let rows = rangees(&b);
    assert_eq!((rows.bands_h(), rows.block_h, rows.cam_h()), (28.0, 48.0, 30.0), "séquence, plans, caméra");
    let h = hauteurs(&b);
    assert!(h.iter().all(|(_, h)| (*h - 56.0).abs() < 0.5), "les calques : {h:?}");
    assert!(blocs().iter().all(|r| (r.height() - 46.0).abs() < 0.6), "les plans suivent leur rangée : {:?}", blocs());
    // Plus bas que les icônes : jamais (icônes de 16 points : une ligne de 21 au moins, un calque de 42).
    (b.app.settings.hauteur_sequence, b.app.settings.hauteur_camera, b.app.settings.hauteur_calques) = (18, 18, 36);
    b.app.settings.taille_icones = 16;
    b.chauffer(6);
    let rows = rangees(&b);
    // (Partie 196 : la ligne Séquence n'a plus d'icônes : elle garde ses 18 points ; la caméra garde ⊕.)
    assert!(rows.bands_h() == 18.0 && rows.cam_h() >= 21.0, "{} {}", rows.bands_h(), rows.cam_h());
    assert!(hauteurs(&b).iter().all(|(_, h)| *h >= 42.0 - 0.5), "{:?}", hauteurs(&b));
    crate::tailleicones::poser(crate::tailleicones::TAILLE_ESSAIS);
}

#[test]
fn les_preferences_montrent_la_hauteur_et_la_couleur_de_chaque_ligne_et_agissent_tout_de_suite() {
    let mut b = crate::perf_tests::banc_taille(crate::document::Document::with_frame(320, 180, 24), false, 1600, 900);
    b.garder_formes = true;
    b.chauffer(6);
    b.app.settings.prefs_onglet = crate::prefsonglets::Onglet::Apparence;
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    for l in Ligne::TOUTES {
        assert!(textes(&b).iter().any(|(t, _)| *t == format!("Ligne {}", l.nom())), "« Ligne {} »", l.nom());
        assert_eq!(vus::ou(&format!("Couleur de la ligne {}", l.nom())).len(), 1, "la pastille de la ligne {}", l.nom());
        assert_eq!(vus::ou(&format!("Ligne {}", l.nom())).len(), 1, "la liste de la ligne {}", l.nom());
    }
    // (Partie 179.17 : chaque hauteur se choisit dans la liste de sa ligne.)
    for l in Ligne::TOUTES {
        let v = l.choix()[l.choix().len() - 1];
        let ouvert = crate::tailleicones_tests::choisir_dans_la_liste(&mut b, &format!("Ligne {}", l.nom()), &format!("{v} pt"));
        for &c in l.choix() {
            let t = if c == l.defaut() { format!("{c} pt (d'origine)") } else { format!("{c} pt") };
            assert!(ouvert.contains(&t), "{} : « {t} » dans la liste : {ouvert:?}", l.nom());
        }
        assert_eq!(l.reglee(&b.app.settings), v, "{} : la liste choisit {v}", l.nom());
    }
    crate::tailleicones_tests::choisir_dans_la_liste(&mut b, "Ligne Plans", "40 pt");    assert_eq!(b.app.settings.hauteur_plans, 40);
    let disque = std::fs::read_to_string(crate::donnees::roaming().unwrap().join("reglages.toml")).unwrap();
    assert!(disque.contains("hauteur_ligne_plans = 40"), "{disque}");
    // « Par défaut » n'est là que quand la ligne a une couleur ; il la rend.
    assert!(vus::ou("Couleur d'origine de la ligne Caméra").is_empty());
    b.app.settings.couleur_ligne_camera = Some([30, 160, 60]);
    b.chauffer(3);
    let r = vus::ou("Couleur d'origine de la ligne Caméra")[0];
    clic(&mut b, r.center(), Modifiers::NONE);
    assert_eq!(b.app.settings.couleur_ligne_camera, None, "revenue à celle du thème");
}

// ------------------------------------------------------------------------------------------------ 179.14 : la couleur des lignes

#[test]
fn la_couleur_d_une_ligne_teinte_sa_case_et_sa_rangee_sans_toucher_aux_autres() {
    let mut b = banc_sb(film(), 1920, 1080, false);
    let rouge = [220, 40, 40];
    let case = crate::theme::etiquette().lerp_to_gamma(Color32::from_rgb(220, 40, 40), crate::hauteurstimeline::PART_DE_LA_COULEUR);
    let rangee = crate::theme::rangee_sombre().lerp_to_gamma(Color32::from_rgb(220, 40, 40), crate::hauteurstimeline::PART_DE_LA_COULEUR);
    assert!(pleins(&b, case).is_empty() && pleins(&b, rangee).is_empty(), "d'origine : les couleurs du thème");
    b.app.settings.couleur_ligne_camera = Some(rouge);
    b.chauffer(4);
    let rows = rangees(&b);
    let h = haut(&b);
    let (c0, c1) = (h + rows.camera_top(), h + rows.camera_top() + rows.cam_h());
    let grip = colonne();
    let cases = pleins(&b, case);
    assert!(cases.iter().any(|r| r.center().y > c0 && r.center().y < c1 && r.right() <= grip.right()), "la case « Caméra » teintée : {cases:?}");
    let rangees_ = pleins(&b, rangee);
    assert!(rangees_.iter().any(|r| r.center().y > c0 && r.center().y < c1 && r.left() >= grip.left() - 1.0), "la rangée de la caméra teintée : {rangees_:?}");
    assert!(cases.iter().chain(rangees_.iter()).all(|r| r.center().y > c0 && r.center().y < c1), "seule la ligne Caméra");
    // Quatre couleurs, quatre lignes ; les blocs des plans gardent les leurs.
    b.app.settings.couleur_ligne_sequence = Some([40, 90, 220]);
    b.app.settings.couleur_ligne_plans = Some([230, 200, 40]);
    b.app.settings.couleur_ligne_calques = Some([40, 200, 90]);
    let avant = blocs();
    b.chauffer(4);
    assert_eq!(blocs(), avant, "les plans gardent leurs blocs");
    for (l, c) in [(Ligne::Sequence, [40, 90, 220]), (Ligne::Plans, [230, 200, 40]), (Ligne::Calques, [40, 200, 90])] {
        let t = crate::theme::etiquette().lerp_to_gamma(Color32::from_rgb(c[0], c[1], c[2]), crate::hauteurstimeline::PART_DE_LA_COULEUR);
        assert!(!pleins(&b, t).is_empty(), "la case de la ligne {} teintée", l.nom());
    }
}

/// Le centre vertical de l'encre du texte `t` peint sur `y0..y1` à gauche de `droite` : l'union des rectangles des glyphes eux-mêmes
/// (pas la boîte de la ligne de texte, qui est la même pour une icône et pour un mot alors que leur dessin n'est pas au même niveau).
fn centre_de_l_encre(b: &Banc, t: &str, y0: f32, y1: f32, droite: f32) -> Option<f32> {
    use eframe::egui::epaint::Shape;
    fn chercher(s: &Shape, t: &str, out: &mut Vec<Rect>) {
        match s {
            Shape::Text(x) if x.galley.job.text == t => {
                let mut encre = Rect::NOTHING;
                for row in &x.galley.rows {
                    for g in &row.glyphs {
                        if g.uv_rect.size.x > 0.0 && g.uv_rect.size.y > 0.0 {
                            let min = x.pos + g.pos.to_vec2() + g.uv_rect.offset;
                            encre = encre.union(Rect::from_min_size(min, g.uv_rect.size));
                        }
                    }
                }
                if encre.is_positive() {
                    out.push(encre);
                }
            }
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, t, out)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, t, &mut v);
    }
    v.into_iter().find(|r| r.center().y > y0 && r.center().y < y1 && r.right() <= droite + 1.0).map(|r| r.center().y)
}

/// Les centres de l'encre de ‹, ›, du losange et du ⋮, moins le milieu de la case de la liste du mode (celui de la barre
/// d'opacité), sur `y0..y1`, à gauche de `droite`.
fn ecarts(b: &Banc, y0: f32, y1: f32, droite: f32) -> Vec<(&'static str, f32)> {
    let mode = vus::ou("Mode de fusion").into_iter().find(|r| r.center().y > y0 && r.center().y < y1).expect("la liste du mode");
    let barre = vus::ou("Opacité du calque").into_iter().find(|r| r.center().y > y0 && r.center().y < y1).expect("la barre d'opacité");
    assert!((barre.center().y - mode.center().y).abs() < 0.6, "la liste et la barre au même niveau : {mode:?} / {barre:?}");
    let reference = mode.center().y;
    [("flèche ‹", ph::CARET_LEFT), ("flèche ›", ph::CARET_RIGHT), ("losange de l'opacité", ph::DIAMOND), ("⋮ du calque", ph::DOTS_THREE_VERTICAL)]
        .into_iter()
        .filter_map(|(q, g)| centre_de_l_encre(b, g, y0, y1, droite).map(|c| (q, c - reference)))
        .collect()
}

#[test]
fn sur_la_seconde_rangee_les_fleches_le_losange_et_le_menu_sont_au_niveau_du_mode_et_de_l_opacite() {
    let mut tout: Vec<(String, Vec<(&'static str, f32)>)> = Vec::new();
    for echelle in [100u16, 125, 150] {
        // Storyboard (tous les calques sur deux rangées d'origine, partie 179.13), la colonne large (les flèches ‹ › n'y sont que quand la place y est).
        let mut b = banc_sb(film(), 1920, 1080, false);
        b.app.settings.echelle = echelle;
        b.app.montage_head_w = 520.0;
        b.chauffer(8);
        let (p0, p1) = piste_du_calque_actif(&b);
        let e = ecarts(&b, p0 + crate::pistes::RANGEE_ACTIVE, p1, colonne().left());
        eprintln!("179.10 Storyboard {echelle} % : {e:?}");
        assert_eq!(e.len(), 4, "{echelle} % : les quatre sont là : {e:?}");
        tout.push((format!("Storyboard {echelle} %"), e));
        // Animation : la ligne du calque courant (sur deux rangées d'origine).
        let mut a = banc_animation();
        a.app.settings.echelle = echelle;
        a.chauffer(8);
        let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
        let cur = a.app.cur_layer;
        let (h0, h1) = lignes.iter().find(|l| l.0 == cur).map(|l| (l.1, l.2)).unwrap();
        let e = ecarts(&a, h0 + 20.0, h1, f32::MAX);
        eprintln!("179.10 Animation {echelle} % : {e:?}");
        assert!(e.len() >= 3, "{echelle} % : le losange, le ⋮ (et les flèches s'il y a la place) : {e:?}");
        tout.push((format!("Animation {echelle} %"), e));
    }
    // Le centre de l'encre de chacune au milieu de la case du mode et de la barre d'opacité, à moins de 0,75 point, et jamais plus bas
    // que lui (au point près, l'arrondi au pixel ; mesuré avant : ‹ › et le losange 0,5 point plus haut, le ⋮ de l'Animation 1,5 à 2
    // points plus haut).
    assert!(tout.iter().all(|(_, e)| e.iter().all(|(_, d)| d.abs() <= 0.75 && *d <= 0.3)), "écart au milieu du mode et de l'opacité : {tout:?}");
}
// ------------------------------------------------------------------------------------------------ 179.17 : les pastilles alignées

#[test]
fn les_pastilles_et_les_boutons_par_defaut_des_lignes_sont_alignes_en_colonne() {
    // (Anthony, sur les pastilles des « Lignes de la timeline » : « Aligne les ».)
    let mut b = crate::perf_tests::banc_taille(crate::document::Document::with_frame(320, 180, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.settings.couleur_ligne_sequence = Some([70, 110, 220]);
    b.app.settings.couleur_ligne_calques = Some([60, 190, 110]);
    b.chauffer(6);
    b.app.settings.prefs_onglet = crate::prefsonglets::Onglet::Apparence;
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    let x: Vec<f32> = Ligne::TOUTES.iter().map(|l| vus::ou(&format!("Couleur de la ligne {}", l.nom()))[0].left()).collect();
    assert!(x.iter().all(|v| (v - x[0]).abs() < 0.5), "les quatre pastilles au même x : {x:?}");
    let d: Vec<f32> = ["Séquence", "Calques"].iter().map(|n| vus::ou(&format!("Couleur d'origine de la ligne {n}"))[0].left()).collect();
    assert!((d[0] - d[1]).abs() < 0.5, "les « Par défaut » au même x : {d:?}");
    // Les six listes de la même largeur.
    let w: Vec<f32> = ["Taille de l'interface", "Taille des icônes", "Ligne Séquence", "Ligne Plans", "Ligne Caméra", "Ligne Calques"].iter().map(|n| vus::ou(n)[0].width()).collect();
    assert!(w.iter().all(|v| (v - w[0]).abs() < 0.5), "les listes de la même largeur : {w:?}");
}