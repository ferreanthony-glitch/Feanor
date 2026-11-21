//! Essais du deuxième tour des retouches vues à l'écran (`docs/INTERFACE.md`, partie 196) : l'aperçu du panneau Préréglages (un tiers
//! plus petit, au milieu, et qui reste en haut quand la liste défile) ; les lignes Séquence (plus fine, sans icônes) et Caméra (sans
//! icônes) de la timeline du Storyboard, et leurs commandes au clic droit ; les lignes Séquence et Plans qui restent en haut de la bande
//! quand les pistes défilent ; la barre de la vue qui commence par la vue caméra, « 2D » et « 3D ». Dans la vraie fenêtre sans écran
//! (`Banc`), chaque élément retrouvé par le nom de sa bulle (`bulles::vus`).

use eframe::egui::{self, pos2, vec2, Event, Modifiers, Pos2, Rect, Shape};

use crate::bulles::vus;
use crate::colonne;
use crate::dock::Pane;
use crate::ligne_sequence_tests::clic_droit;
use crate::montagerows::{SB_CAM_H, SB_SEQ_H};
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::clic;
use crate::workspace::Espace;

fn ou(nom: &str) -> Rect {
    *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

fn textes(b: &Banc) -> Vec<(String, Rect)> {
    fn chercher(s: &Shape, out: &mut Vec<(String, Rect)>) {
        match s {
            Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut out);
    }
    out
}

/// Le texte `mot` peint à l'écran (le plus bas s'il y en a plusieurs : un menu passe par-dessus).
fn texte(b: &Banc, mot: &str) -> Option<Rect> {
    textes(b).into_iter().filter(|(t, _)| t == mot).map(|(_, r)| r).max_by(|a, c| a.top().total_cmp(&c.top()))
}

/// Un film de trois plans de 24 images ; `calques` calques vectoriels dans chacun.
fn film(calques: usize) -> crate::document::Document {
    let noms: Vec<String> = (0..calques).map(|k| format!("C{k}")).collect();
    let refs: Vec<&str> = noms.iter().map(String::as_str).collect();
    crate::pistes_tests::doc_with(&[&refs, &refs, &refs])
}

fn banc_dans(doc: crate::document::Document, espace: Espace, l: u32, h: u32) -> Banc {
    let mut b = banc_taille(doc, false, l, h);
    b.app.set_espace(espace);
    b.garder_formes = true;
    b.chauffer(6);
    b
}

/// La molette vers le bas sur `sur`, douze crans.
fn defiler(b: &mut Banc, sur: Pos2) {
    for _ in 0..12 {
        b.image(vec![Event::PointerMoved(sur)], 1.0 / 60.0);
        b.image(vec![Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -120.0), modifiers: Modifiers::NONE }], 1.0 / 60.0);
    }
    b.chauffer(4);
}

// ------------------------------------------------------------------------------------ 1 et 2. l'aperçu des préréglages

#[test]
fn the_presets_preview_is_a_third_smaller_than_before_and_keeps_sixteen_ninths() {
    for w in [180.0_f32, 300.0, 420.0, 900.0] {
        let a = crate::panneauprereglages::apercu_taille(w);
        // (Partie 195 : toute la largeur, en 16:9, au plus 220 points de haut.)
        let h195 = (w * 9.0 / 16.0).min(220.0);
        assert!((a.y - h195 * 2.0 / 3.0).abs() < 0.01, "{w} : les deux tiers de la hauteur d'avant : {a:?} / {h195}");
        assert!((a.x / a.y - 16.0 / 9.0).abs() < 1e-3, "{w} : 16:9 : {a:?}");
        assert!(a.x <= w * 2.0 / 3.0 + 0.01, "{w} : les deux tiers de la largeur au plus : {a:?}");
    }
}

#[test]
fn the_presets_preview_is_centred_and_stays_at_the_top_while_the_list_scrolls() {
    let mut b = banc_dans(film(1), Espace::Storyboard, 1400, 640);
    b.app.enter_animation(0);
    b.app.show_pane(Pane::Prereglages);
    crate::colonne_tests::toute_la_place(&mut b.app, Pane::Prereglages);
    crate::panneauprereglages::choisir_onglet(&b.app.ctx, crate::panneauprereglages::Onglet::Camera);
    b.chauffer(6);
    let apercu = crate::panneauprereglages::APERCU.with(|c| c.get()).expect("l'aperçu est peint");
    let liste = crate::panneauprereglages::LISTE.with(|c| c.get()).expect("la liste défile dans sa zone");
    // Les deux tiers de la largeur (ou au plus haut), en 16:9, au milieu du panneau ; la liste dessous.
    assert!((apercu.width() / apercu.height() - 16.0 / 9.0).abs() < 0.02, "16:9 : {apercu:?}");
    assert!(apercu.width() <= liste.width() * 2.0 / 3.0 + 12.0 && apercu.width() >= liste.width() * 0.5, "les deux tiers du panneau : {apercu:?} / {liste:?}");
    assert!((apercu.center().x - liste.center().x).abs() < 8.0, "au milieu : {apercu:?} / {liste:?}");
    assert!(apercu.bottom() <= liste.top() + 0.5, "la liste sous l'aperçu (et sous la phrase d'aide) : {apercu:?} / {liste:?}");
    let aide = textes(&b).into_iter().find(|(t, _)| t.starts_with("Glisse un préréglage")).expect("la phrase d'aide").1;
    assert!(aide.top() >= apercu.bottom() - 0.5 && aide.bottom() <= liste.top() + 0.5, "la phrase entre l'aperçu et la liste : {aide:?}");
    let premiere = crate::camfx::Famille::ALL[0].presets().next().unwrap().label();
    let ligne = |b: &Banc| textes(b).into_iter().filter(|(x, _)| x == premiere).map(|(_, r)| r).last();
    let avant = ligne(&b).expect("la première ligne de la liste");
    // La molette sur la liste : elle défile ; l'aperçu, la phrase et les onglets restent.
    defiler(&mut b, liste.center());
    let apres_apercu = crate::panneauprereglages::APERCU.with(|c| c.get()).expect("toujours peint");
    let apres_liste = crate::panneauprereglages::LISTE.with(|c| c.get()).unwrap();
    assert!((apres_apercu.top() - apercu.top()).abs() < 0.5 && (apres_liste.top() - liste.top()).abs() < 0.5, "l'aperçu ne bouge pas : {apercu:?} → {apres_apercu:?}");
    let aide2 = textes(&b).into_iter().find(|(t, _)| t.starts_with("Glisse un préréglage")).expect("la phrase d'aide, toujours là").1;
    assert!((aide2.top() - aide.top()).abs() < 0.5, "la phrase ne bouge pas");
    match ligne(&b) {
        Some(r) => assert!(r.top() < avant.top() - 10.0 || !liste.intersects(r), "la liste a défilé : {avant:?} → {r:?}"),
        None => {} // sortie de la zone : elle a défilé
    }
    // Les onglets du panneau, au-dessus de l'aperçu, toujours là.
    assert!(texte(&b, "Caméra").is_some_and(|r| r.bottom() <= apercu.top() + 0.5) || vus::ou("Caméra").iter().any(|r| r.bottom() <= apercu.top() + 0.5), "les onglets au-dessus");
}

// ------------------------------------------------------------------------------------ 3 et 5. les lignes Séquence et Caméra

/// Le Storyboard : le haut de la bande, et ses rangées.
fn sb() -> Banc {
    crate::timelinesb_tests::banc_sb(crate::timelinesb_tests::film(), 1920, 1080, false)
}

#[test]
fn the_sequence_row_is_half_as_tall_and_it_and_the_camera_row_have_no_shortcut_buttons() {
    let b = sb();
    let rows = crate::timelinesb_tests::rangees(&b);
    assert_eq!(SB_SEQ_H, 10.0, "la moitié des 20 points d'avant");
    assert_eq!(rows.bands_h(), SB_SEQ_H, "dans la colonne et dans la bande : la même rangée");
    assert_eq!(crate::hauteurstimeline::SEQ_DEFAUT, 10, "la hauteur d'origine des réglages suit");
    let col = crate::timelinesb_tests::colonne();
    let dans_la_colonne = |nom: &str| vus::ou(nom).into_iter().filter(|r| r.right() <= col.right() + 1.0).count();
    for nom in crate::ligne_sequence_tests::ICONES {
        assert_eq!(dans_la_colonne(nom), 0, "« {nom} » n'est plus sur la ligne Séquence");
    }
    for nom in crate::timelinesb_tests::ANCIENNES_ICONES_DE_LA_CAMERA {
        assert_eq!(dans_la_colonne(nom), 0, "« {nom} » n'est plus sur la ligne Caméra");
    }
    // La caméra garde ⊕ (déplier) ; sa hauteur ne change pas. Plans garde ses icônes.
    assert_eq!(dans_la_colonne("Déplier la caméra"), 1);
    assert_eq!(rows.cam_h(), SB_CAM_H);
    let h = crate::timelinesb_tests::haut(&b);
    for nom in ["Nouveau plan", "Dupliquer le plan", "Versions du plan", "Transitions", "Couper le plan à la tête de lecture", "Autres commandes des plans"] {
        crate::timelinesb_tests::ou_dans(nom, h + rows.blocks_top(), h + rows.blocks_top() + rows.block_h);
    }
}

/// Partie 196 : chaque commande des icônes enlevées est au clic droit de sa ligne (et les séquences, aussi au menu Storyboard).
#[test]
fn every_removed_button_still_has_its_action_on_the_right_click_of_its_row() {
    let mut b = sb();
    let h = crate::timelinesb_tests::haut(&b);
    let rows = crate::timelinesb_tests::rangees(&b);
    let col = crate::timelinesb_tests::colonne();
    let seq = pos2(col.left() - 12.0, h + rows.regle_h() + rows.bands_h() * 0.5);
    clic_droit(&mut b, seq);
    for mot in ["Nouvelle séquence", "Renommer la séquence…", "Versions de la séquence", "Dupliquer la séquence", "Nouvelle séquence avec les plans choisis", "Couper la séquence ici (le plan courant la commence)"] {
        assert!(texte(&b, mot).is_some(), "« {mot} » au clic droit de la ligne Séquence");
    }
    b.image(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    let cam = pos2(col.left() - 12.0, h + rows.camera_top() + rows.cam_h() * 0.5);
    clic_droit(&mut b, cam);
    for mot in ["Clé de caméra ici", "Préréglages de caméra…", "Remettre la caméra à zéro", "Convertir les préréglages en clés", "Copier les clés de caméra choisies", "Coller les clés de caméra"] {
        assert!(texte(&b, mot).is_some(), "« {mot} » au clic droit de la ligne Caméra : {:?}", textes(&b).into_iter().map(|t| t.0).collect::<Vec<_>>());
    }
    b.image(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    // Les actions des séquences ont aussi leur raccourci ou leur place dans les menus (la palette des commandes les trouve).
    let palette: Vec<String> = b.app.palette_commandes_ici().into_iter().map(|(a, _)| a.to_string()).collect();
    for action in ["sequence.nouvelle", "sequence.renommer", "sequence.dupliquer", "sequence.nouvelle_avec_plans", "sequence.couper_ici", "camera.cle_ici", "camera.reinitialiser", "camera.cles_copier", "camera.cles_coller", "camera.effet_appliquer"] {
        assert!(palette.iter().any(|a| a == action), "« {action} » dans la palette des commandes");
    }
}

// ------------------------------------------------------------------------------------ 4. Séquence et Plans restent en haut

#[test]
fn the_sequence_and_plans_rows_stay_at_the_top_of_the_band_when_the_tracks_scroll() {
    let mut b = banc_dans(film(14), Espace::Storyboard, 1400, 560);
    b.app.settings.montage_grands = false;
    b.chauffer(4);
    let compteur = ou("Position");
    let (contenu, vue) = crate::montagehauteur::DEFILEMENT.with(|c| c.get());
    assert!(contenu > vue + 20.0, "le banc a une bande qui défile : {contenu} dans {vue}");
    assert!(crate::storyboard::HAUT_COLLANT_MONTAGE.with(|c| c.get()).is_none(), "pas défilée : rien n'est recopié");
    assert!(crate::montagerows::HAUT_COLLANT_COLONNE.with(|c| c.get()).is_none());
    defiler(&mut b, pos2(900.0, compteur.bottom() + 120.0));
    let rows = b.app.montage_rows(300.0);
    let zone = crate::storyboard::HAUT_COLLANT_MONTAGE.with(|c| c.get()).expect("la bande a défilé : le haut reste");
    let regle = crate::storyboard::REGLE_COLLANTE_MONTAGE.with(|c| c.get()).expect("et la règle (partie 195)");
    assert!((zone.top() - regle.top()).abs() < 0.5, "la règle en haut de la zone : {zone:?} / {regle:?}");
    assert!((zone.height() - (rows.blocks_top() + rows.block_h)).abs() < 1.0, "la règle, les séquences et les plans : {zone:?} / {}", rows.blocks_top() + rows.block_h);
    assert!(regle.top() <= compteur.top() + 4.0, "sous la règle et le compteur de la partie 195 : {regle:?} / {compteur:?}");
    // Dans la colonne : « Séquence » et « Plans » en face, Plans avec ses icônes.
    let (seq, plans) = crate::montagerows::HAUT_COLLANT_COLONNE.with(|c| c.get()).expect("les en-têtes restent en haut de la colonne");
    assert!((seq.top() - (zone.top() + rows.regle_h() + 1.0)).abs() < 1.0, "Séquence sous la règle : {seq:?} / {zone:?}");
    assert!((plans.top() - (zone.top() + rows.blocks_top() + 1.0)).abs() < 1.0, "Plans sous Séquence : {plans:?}");
    assert!(textes(&b).iter().any(|(t, r)| t == "Séquence" && seq.expand(1.0).contains(r.center())), "le mot « Séquence » en haut");
    let np: Vec<Rect> = vus::ou("Nouveau plan").into_iter().filter(|r| plans.contains(r.center())).collect();
    assert_eq!(np.len(), 1, "« Nouveau plan » sur la ligne Plans restée en haut : {:?} / {plans:?}", vus::ou("Nouveau plan"));
    // Un clic sur un plan de la ligne restée en haut le choisit ; « Nouveau plan » y marche.
    let x = crate::timelinesb_tests::blocs()[1].center().x;
    clic(&mut b, pos2(x, zone.top() + rows.blocks_top() + rows.block_h * 0.5), Modifiers::NONE);
    assert_eq!(b.app.cur_panel, 1, "le plan 2 choisi depuis la ligne restée en haut : {}", b.app.status);
    let n = b.app.doc.panels.len();
    clic(&mut b, np[0].center(), Modifiers::NONE);
    assert_eq!(b.app.doc.panels.len(), n + 1, "nouveau plan depuis l'en-tête resté en haut : {}", b.app.status);
}

// ------------------------------------------------------------------------------------ 6. la barre de la vue

#[test]
fn the_view_bar_starts_with_the_camera_view_then_2d_then_3d_in_cells_of_the_same_size() {
    let mut b = banc_dans(film(1), Espace::Storyboard, 1600, 900);
    let (camera, d2, d3) = (ou("Vue caméra"), ou("Vue du plateau"), ou("Espace 3D"));
    assert!(camera.right() <= d2.left() + 0.5 && d2.right() <= d3.left() + 0.5, "dans l'ordre : {camera:?} {d2:?} {d3:?}");
    for (nom, r) in [("Vue caméra", camera), ("2D", d2), ("3D", d3)] {
        assert!((r.width() - camera.width()).abs() < 0.6 && (r.height() - camera.height()).abs() < 0.6, "« {nom} » : la case des autres : {r:?} / {camera:?}");
        assert!((r.center().y - camera.center().y).abs() < 0.6, "« {nom} » sur la même ligne");
    }
    for nom in ["Ajuster à la fenêtre", "Miroir de la vue", "Espace 3D à côté"] {
        if let Some(r) = vus::ou(nom).into_iter().find(|r| (r.center().y - camera.center().y).abs() < 3.0) {
            assert!(r.left() >= d3.right() - 0.5, "« {nom} » après « 3D » : {r:?}");
        }
    }
    // La vue caméra est la première case de la barre.
    let barre_g = b.app.canvas_rect.left();
    assert!(camera.left() - barre_g < 12.0, "la vue caméra en premier, au bord gauche : {camera:?} / {barre_g}");
    // Les mots « 2D » et « 3D », peints dans leurs cases, à la taille des icônes.
    for (mot, r) in [("2D", d2), ("3D", d3)] {
        let t = textes(&b).into_iter().find(|(t, z)| t == mot && r.contains(z.center())).unwrap_or_else(|| panic!("« {mot} » dans sa case")).1;
        assert!(t.height() <= crate::tailleicones::icone() + 0.5 && t.height() >= crate::tailleicones::icone() * 0.5, "« {mot} » à la taille des icônes : {t:?}");
    }
    // Les bulles gardent les noms entiers et les raccourcis (l'action de chaque case).
    for (r, nom, action) in [(d2, "Vue du plateau", "camera.vue"), (d3, "Espace 3D", "espace3d.basculer")] {
        b.image(vec![Event::PointerMoved(r.center() + vec2(0.0, 1.0))], 1.0 / 60.0);
        b.image(vec![Event::PointerMoved(r.center())], 1.0 / 60.0);
        b.chauffer(50);
        let t = textes(&b);
        assert!(t.iter().any(|(x, z)| x.contains(nom) && z.top() >= r.bottom() - 1.0), "la bulle dit « {nom} » : {:?}", t.iter().map(|x| &x.0).collect::<Vec<_>>());
        let raccourci = b.app.keymap.label(action);
        if !raccourci.is_empty() {
            assert!(t.iter().any(|(x, _)| x.contains(&raccourci)), "la bulle de « {nom} » dit son raccourci « {raccourci} »");
        }
    }
    b.image(vec![Event::PointerMoved(pos2(800.0, 500.0))], 1.0 / 60.0);
    b.chauffer(3);
    // L'état allumé : la 2D au départ ; un clic sur la vue caméra la passe ; un clic sur « 2D » la ramène.
    assert!(!b.app.camera_view);
    clic(&mut b, camera.center(), Modifiers::NONE);
    assert!(b.app.camera_view, "la vue caméra");
    clic(&mut b, ou("Vue du plateau").center(), Modifiers::NONE);
    assert!(!b.app.camera_view && !b.app.espace3d.on, "le plateau, à plat");
}

// ------------------------------------------------------------------------------------ 196.5 la Couleur écrasée dans la pile

/// La pile du Storyboard d'origine (Couleur, Propriétés), Préréglages ajouté en bas, tous ouverts, dans une fenêtre de `l` × `h`
/// à 90 % (la taille d'origine de l'interface).
fn pile_a_90(l: u32, h: u32) -> Banc {
    let mut b = banc_dans(film(1), Espace::Storyboard, l, h);
    // (Partie 197 : cet essai porte sur les autres panneaux de la pile ; la Scène, en haut de celle du Storyboard, en est retirée.)
    crate::colonne_tests::sans_la_scene(&mut b.app);
    b.app.settings.echelle = 90;
    b.chauffer(4);
    b.app.colonne.ajouter(Pane::Prereglages);
    b.chauffer(10);
    assert!(b.app.colonne.panneaux.iter().all(|p| !p.replie), "tous ouverts");
    b
}

/// Le corps du panneau `k` de la pile (sa tranche sans l'en-tête ni le bord du bas), et sa tranche à l'écran.
fn corps(b: &Banc, k: usize) -> (f32, (f32, f32)) {
    let t = b.app.colonne_etat.tranches[k];
    let bord = if Some(k) == b.app.colonne.qui_grandit() { 0.0 } else { colonne::BORD };
    (t.1 - t.0 - colonne::ENTETE - bord, t)
}

#[test]
fn the_column_heights_keep_content_panels_whole_and_only_lists_give_way_to_properties() {
    use colonne::Voulue;
    let fixe = 3.0 * colonne::ENTETE + 2.0 * colonne::BORD;
    // De la place : la Couleur garde son contenu, la liste ses 230 points, Propriétés le reste.
    let h = colonne::hauteurs(700.0, &[Some(Voulue::ferme(200.0)), Some(Voulue::souple_au_moins(230.0, 150.0)), Some(Voulue::ferme(50.0))], Some(2));
    assert_eq!((h[0], h[1]), (200.0, 230.0));
    assert!((h[2] - (700.0 - fixe - 430.0)).abs() < 0.01, "{h:?}");
    // Un peu juste : la liste cède ce qui manque à Propriétés pour ses 120 points, pas plus ; la Couleur ne bouge pas.
    let total = fixe + 200.0 + 230.0 + colonne::GRANDIT_MIN - 50.0;
    let h = colonne::hauteurs(total, &[Some(Voulue::ferme(200.0)), Some(Voulue::souple_au_moins(230.0, 150.0)), Some(Voulue::ferme(50.0))], Some(2));
    assert_eq!(h[0], 200.0);
    assert!((h[1] - 180.0).abs() < 0.01 && (h[2] - colonne::GRANDIT_MIN).abs() < 0.01, "{h:?}");
    // Trop juste : la liste s'arrête à son plancher, Propriétés à son minimum, la Couleur entière : la colonne défile.
    let h = colonne::hauteurs(fixe + 300.0, &[Some(Voulue::ferme(200.0)), Some(Voulue::souple_au_moins(230.0, 150.0)), Some(Voulue::ferme(50.0))], Some(2));
    assert_eq!((h[0], h[1], h[2]), (200.0, 150.0, colonne::GRANDIT_MIN), "{h:?}");
    assert!(h.iter().sum::<f32>() > 300.0, "plus haut que la colonne : elle défile");
}

/// Partie 196.5 (la capture d'Anthony : la Couleur écrasée à 55 points, Préréglages coupé) : 1384 × 868 à 90 %, Propriétés,
/// Couleur et Préréglages ouverts.
#[test]
fn at_1384_by_868_and_90_percent_colour_keeps_its_content_height_no_panel_is_cut_and_the_column_scrolls() {
    let mut b = pile_a_90(1384, 868);
    let zone = b.app.colonne_etat.zone.expect("la colonne");
    let couleur = b.app.colonne.rang_de(Pane::Color).expect("la Couleur");
    let prop = b.app.colonne.proprietes().expect("Propriétés");
    let prer = b.app.colonne.rang_de(Pane::Prereglages).expect("Préréglages");
    // La Couleur : toute la hauteur de son contenu (et ses marges).
    let contenu = *b.app.colonne_etat.contenus.get(&Pane::Color).expect("la hauteur du contenu de la Couleur");
    let (c, _) = corps(&b, couleur);
    assert!(c >= contenu + 6.0 - 0.5, "la Couleur entière : {c} pour un contenu de {contenu}");
    assert!(c > 150.0, "pas écrasée : {c}");
    // Propriétés : au moins son minimum ; Préréglages : son aperçu, sa phrase et au moins trois lignes de sa liste.
    assert!(corps(&b, prop).0 >= colonne::GRANDIT_MIN - 0.5, "Propriétés : {}", corps(&b, prop).0);
    let plancher = crate::panneauprereglages::plancher(&b.app.ctx).expect("le plancher des Préréglages");
    assert!(corps(&b, prer).0 >= plancher - 0.5, "Préréglages pas coupé : {} / {plancher}", corps(&b, prer).0);
    // (Audit du 2026-11-21 : dans le Storyboard la colonne prend maintenant toute la hauteur de la fenêtre. À cette taille, la pile
    // qui débordait de 480 points de colonne y tient : plus besoin de défiler pour voir Préréglages en entier.)
    let bas = b.app.colonne_etat.tranches.last().unwrap().1;
    assert!(bas <= zone.bottom() + 1.0, "la pile tient dans la colonne : {bas} / {zone:?}");
    let (_, (p0, p1)) = corps(&b, prer);
    assert!(p1 <= zone.bottom() + 1.0 && p0 >= zone.top() - 1.0, "Préréglages entier à l'écran, en bas : {p0}..{p1} / {zone:?}");
    let apercu = crate::panneauprereglages::APERCU.with(|c| c.get()).unwrap();
    let liste = crate::panneauprereglages::LISTE.with(|c| c.get()).unwrap();
    assert!(apercu.top() >= p0 && apercu.bottom() <= p1, "l'aperçu entier : {apercu:?}");
    assert!(liste.height() >= 3.0 * crate::panneauprereglages::LIGNE_H, "au moins trois lignes de la liste : {liste:?}");
    // Et la Couleur n'a pas bougé de hauteur.
    assert!((corps(&b, couleur).0 - c).abs() < 0.5);
}

#[test]
fn with_room_properties_takes_the_rest_a_new_list_starts_at_230_and_nothing_scrolls() {
    let b = pile_a_90(1384, 1400);
    let zone = b.app.colonne_etat.zone.expect("la colonne");
    let prer = b.app.colonne.rang_de(Pane::Prereglages).unwrap();
    let prop = b.app.colonne.proprietes().unwrap();
    let plancher = crate::panneauprereglages::plancher(&b.app.ctx).unwrap() + 7.0;
    let (h, _) = corps(&b, prer);
    assert!((h - colonne::HAUTEUR_DEFAUT.max(plancher)).abs() < 0.5, "Préréglages à sa hauteur d'origine : {h} (plancher {plancher})");
    let bas = b.app.colonne_etat.tranches.last().unwrap().1;
    assert!((bas - zone.bottom()).abs() < 2.0, "la pile remplit la colonne sans la dépasser : {bas} / {zone:?}");
    assert!(corps(&b, prop).0 > colonne::GRANDIT_MIN + 100.0, "Propriétés prend le reste : {}", corps(&b, prop).0);
}
