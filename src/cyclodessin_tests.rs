//! Essais du cyclo dessiné (partie 203, `cyclodessin.rs`, `docs/INTERFACE.md` partie 203) : l'ouvrir depuis la galerie (son onglet,
//! la vue de dessin), « Terminer » (le retour où l'on était, une étape d'annulation), la feuille qui n'est un plan nulle part, un trait
//! qui change l'image du cyclo une fois, les fantômes et le raccord des bords, le départ d'un choix ou vide, l'enregistrement, un
//! lecteur d'avant, la copie et la duplication, « Retirer le cyclo dessiné ? » et Ctrl+Z, la mesure.

use std::sync::Arc;

use eframe::egui::{Modifiers, Rect, Shape};

use crate::bulles::vus;
use crate::cyclodessin::{self as cd, Depart, DessinCyclo};
use crate::cyclos::{self, Cyclo, Equirect, Preset};
use crate::cyclosui::ActionCyclo;
use crate::decor3d::{self, Decor3d};
use crate::decor3drendu::Aspect;
use crate::dock::Pane;
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::clic;
use crate::workspace::Espace;

// ------------------------------------------------------------------------------------------------ les aides

/// Le projet aux deux carrés, une carte, et une seconde séquence (sans décor).
fn projet() -> Document {
    let mut doc = crate::decor3d_tests::projet_a_deux_carres();
    let cel = doc.new_cel(LayerKind::Bitmap);
    let n = doc.panels[0].frame_count();
    let mut l = Layer::new("Carte", LayerKind::Bitmap);
    l.frames = vec![Some(cel); n];
    doc.panels[0].layers = vec![l];
    doc.add_sequence_after(0);
    doc
}

/// Peint un rectangle de `couleur` dans le calque « Mon horizon » de la feuille `f` (x0..x1, y0..y1, pixels de la feuille).
fn peindre(f: &mut Document, x0: usize, x1: usize, y0: usize, y1: usize, couleur: [u8; 4]) {
    let li = f.panels[0].layers.len() - 1;
    let cel = f.panels[0].layers[li].frames[0].unwrap();
    let c = f.cels.get_mut(&cel).unwrap();
    let mut rgba = c.raster.to_rgba();
    for y in y0..y1 {
        for x in x0..x1 {
            let o = (y * cd::LARGEUR + x) * 4;
            rgba[o..o + 4].copy_from_slice(&couleur);
        }
    }
    c.raster = crate::raster::Raster::from_rgba(cd::LARGEUR, cd::HAUTEUR, &rgba);
    c.version += 1;
}

/// Pose dans la séquence du plan 0 un cyclo dessiné (sur `base`, la feuille de départ vide ou du choix), après `pinceau`.
fn avec_dessin(doc: &mut Document, base: Cyclo, vide: bool, pinceau: impl FnOnce(&mut Document)) -> Cyclo {
    let mut f = cd::feuille(&base, vide, "Cyclo");
    pinceau(&mut f);
    let v = cd::nouvelle_version();
    let seq = doc.sequence_of(0);
    let d = doc.decors3d.par_sequence.entry(seq).or_insert_with(|| Decor3d::new(Aspect::default()));
    let c = Cyclo { dessin: Some(v), ..base };
    crate::cyclosui::poser(d, Some(c));
    d.dessin_cyclo = Some(Arc::new(DessinCyclo::new(f, v)));
    c
}

/// L'image du cyclo de la séquence du plan 0, prête.
fn image_du_cyclo(doc: &Document) -> Arc<Equirect> {
    let d = &doc.decors3d.par_sequence[&doc.sequence_of(0)];
    let c = d.cyclo.unwrap();
    cyclos::assurer_dessine(&c, d.dessin_cyclo.clone());
    cyclos::attendre();
    cyclos::texture(c.cle()).expect("l'image du cyclo")
}

/// La direction de l'azimut `az` (0 devant, vers −z ; 90 à droite, +x…) et de l'élévation `e` (degrés), dans le repère du décor.
fn direction(az: f32, e: f32) -> [f32; 3] {
    let (a, e) = (az.to_radians(), e.to_radians());
    [e.cos() * a.sin(), e.sin(), -e.cos() * a.cos()]
}

fn ecart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max)
}

fn banc_3d(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1600, 1800);
    // (Une vraie carte graphique accepte une texture de 4096 de côté ; egui seul, 2048.)
    b.cote_max = Some(8192);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    b.app.set_espace(Espace::TroisD);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn ou(nom: &str) -> Rect {
    *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

fn etapes(b: &Banc) -> Vec<String> {
    b.app.history.names().0.into_iter().map(String::from).collect()
}

fn annuler(b: &mut Banc) {
    b.app.run_action("edition.annuler");
    b.chauffer(2);
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

/// Montre la section Cyclo de Propriétés > Monde (la ligne « Cyclo » de Scène 3D y fait défiler).
fn section_cyclo(b: &mut Banc) {
    b.app.colonne.montrer(Pane::Lumieres3d);
    b.chauffer(2);
    b.app.scene3d_choisir(crate::onglet3d::Ligne3d::Ciel, false);
    b.chauffer(4);
}

/// Attend la bande aplatie de l'onglet du cyclo (sur un autre fil), à l'empreinte de la feuille.
fn attendre_la_bande(b: &mut Banc) {
    let t0 = std::time::Instant::now();
    while b.app.cyclo_edition.as_ref().is_some_and(|e| e.vue.as_ref().is_none_or(|(s, _)| *s != cd::signature(&b.app.doc))) {
        b.chauffer(1);
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(t0.elapsed().as_secs() < 30, "la bande ne vient pas");
    }
}

// ------------------------------------------------------------------------------------------------ 1. l'ouvrir, le terminer

#[test]
fn drawing_my_cyclo_from_the_gallery_opens_its_own_tab_and_terminer_brings_back_the_plan_the_tab_and_the_tool() {
    let mut b = banc_3d(projet());
    b.app.select_panel(0);
    b.app.set_tool(crate::app::Tool::Fill);
    section_cyclo(&mut b);
    let (n_onglets, plan) = (b.app.onglets.len(), b.app.cur_panel);
    let r = ou("Dessiner mon cyclo");
    assert!(r.max.y <= b.taille.y, "à l'écran : {r:?}");
    clic(&mut b, r.center(), Modifiers::NONE);
    b.chauffer(4);
    // L'onglet du cyclo : la feuille de 360°, la vue de dessin, deux calques, un outil qui dessine.
    assert!(b.app.cyclo_edition.is_some(), "{}", b.app.status);
    assert_eq!(b.app.onglets.len(), n_onglets + 1);
    assert_eq!((b.app.doc.width, b.app.doc.height, b.app.doc.panels.len()), (cd::LARGEUR, cd::HAUTEUR, 1));
    assert_eq!(b.app.layers().iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Fond du cyclo", "Mon horizon"]);
    assert_eq!((b.app.espace, b.app.board_shown(), b.app.cur_layer), (Espace::Storyboard, false, 1));
    assert_eq!(b.app.tool, crate::app::Tool::Brush);
    let info = b.app.infos_onglet(b.app.actif);
    assert!(info.nom == "Cyclo de la séquence 1" && !info.modifie, "{info:?}");
    ou("Terminer le cyclo");
    // Un trait, puis « Terminer ».
    crate::calqueannot_tests::trait_au_milieu(&mut b, 0.0);
    assert!(b.app.modified);
    clic(&mut b, ou("Terminer le cyclo").center(), Modifiers::NONE);
    b.chauffer(4);
    assert!(b.app.cyclo_edition.is_none());
    assert_eq!(b.app.onglets.len(), n_onglets);
    assert_eq!((b.app.espace, b.app.tool, b.app.cur_panel), (Espace::TroisD, crate::app::Tool::Fill, plan), "où l'on était");
    let d = &b.app.doc.decors3d.par_sequence[&b.app.doc.sequence_of(0)];
    assert!(d.cyclo.is_some_and(|c| c.dessin.is_some()) && d.dessin_cyclo.is_some());
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Dessiner le cyclo"));
    // La galerie : « Mon cyclo », « Redessiner ».
    section_cyclo(&mut b);
    ou("Cyclo : Mon cyclo");
    ou("Redessiner le cyclo");
    // « Redessiner » rouvre la même feuille ; « Terminer » sans trait ne pose rien.
    let n = etapes(&b).len();
    clic(&mut b, ou("Redessiner le cyclo").center(), Modifiers::NONE);
    b.chauffer(4);
    assert!(b.app.cyclo_edition.is_some());
    assert!(b.app.terminer_cyclo());
    assert_eq!(etapes(&b).len(), n, "rien n'a changé : pas d'étape");
    // Fermer l'onglet du cyclo (sa croix), c'est « Terminer ».
    assert!(b.app.dessiner_mon_cyclo(None));
    let i = b.app.actif;
    b.app.fermer_onglet(i);
    assert!(b.app.cyclo_edition.is_none() && b.app.onglets.len() == n_onglets);
    // Ctrl+Z : plus de cyclo dessiné.
    annuler(&mut b);
    assert!(b.app.cyclo_courant().is_none_or(|c| c.dessin.is_none()));
}

#[test]
fn the_storyboard_menu_offers_to_draw_my_cyclo() {
    let mut b = banc_3d(projet());
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    let r = ou("Cyclo de la séquence");
    clic(&mut b, r.center(), Modifiers::NONE);
    b.chauffer(3);
    let dessiner = textes(&b).into_iter().find(|(t, _)| t == "Dessiner mon cyclo…").expect("« Dessiner mon cyclo… » dans le menu").1;
    clic(&mut b, dessiner.center(), Modifiers::NONE);
    b.chauffer(3);
    assert!(b.app.cyclo_edition.is_some());
    assert!(b.app.terminer_cyclo());
    assert_eq!(b.app.espace, Espace::Storyboard);
    assert!(b.app.cyclo_courant().is_some_and(|c| c.dessin.is_some()), "un dessin neuf se pose même sans trait");
}

#[test]
fn closing_the_window_or_saving_from_the_cyclo_tab_first_brings_the_drawing_into_its_project() {
    // La croix de la fenêtre, l'onglet du cyclo derrière : le dessin va dans son projet, la fenêtre reste (le projet est modifié).
    let mut b = banc_3d(projet());
    b.app.modified = false;
    assert!(b.app.dessiner_mon_cyclo(Some(Depart::Vide)));
    b.chauffer(3);
    crate::calqueannot_tests::trait_au_milieu(&mut b, 0.0);
    let origine = b.app.cyclo_edition.as_ref().unwrap().origine;
    assert!(b.app.activer_onglet(b.app.rang_onglet(origine).unwrap()));
    assert!(b.app.cyclos_en_cours());
    assert!(b.fermer_la_fenetre(), "la fenêtre reste");
    assert!(!b.app.cyclos_en_cours());
    assert!(b.app.cyclo_courant().is_some_and(|c| c.dessin.is_some()) && b.app.modified);
    // Ctrl+S dans l'onglet du cyclo : « Terminer » d'abord, puis le projet s'enregistre, son cyclo dessiné avec.
    b.app.pending = None;
    b.app.groupe = None;
    let dir = crate::decor3d_tests::dossier("cyclodessin203-ctrls");
    b.app.path = Some(dir.join("Film.shot"));
    assert!(b.app.dessiner_mon_cyclo(None));
    b.chauffer(2);
    assert!(b.app.save());
    assert!(b.app.wait_save());
    assert!(b.app.cyclo_edition.is_none() && cd::chemin(&dir, b.app.doc.sequence_of(0)).exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_gpu_limited_to_2048_gets_a_smaller_sheet_and_nothing_stops() {
    let mut b = banc_3d(projet());
    b.cote_max = Some(2048);
    b.chauffer(2);
    assert_eq!(cd::largeur_pour(2048), 2048);
    assert_eq!((cd::largeur_pour(8192), cd::largeur_pour(3000), cd::largeur_pour(100)), (cd::LARGEUR, 2048, 512));
    // « Dessiner mon cyclo… » depuis la galerie : une feuille de 2048 × 512.
    section_cyclo(&mut b);
    clic(&mut b, ou("Dessiner mon cyclo").center(), Modifiers::NONE);
    b.chauffer(4);
    assert!(b.app.cyclo_edition.is_some(), "{}", b.app.status);
    assert_eq!((b.app.doc.width, b.app.doc.height), (2048, 512));
    assert!(b.app.status.contains("2048"), "{}", b.app.status);
    attendre_la_bande(&mut b);
    // Un trait, ses fantômes (256 pixels), « Terminer ».
    crate::calqueannot_tests::trait_au_milieu(&mut b, 0.0);
    assert!(b.app.modified);
    attendre_la_bande(&mut b);
    let bande = b.app.cyclo_edition.as_ref().unwrap().vue.clone().unwrap().1;
    assert_eq!((bande.largeur, cd::fantomes(&bande).0.len()), (2048, 256 * 512 * 4));
    clic(&mut b, ou("Terminer le cyclo").center(), Modifiers::NONE);
    b.chauffer(4);
    assert!(b.app.cyclo_edition.is_none());
    let img = image_du_cyclo(&b.app.doc);
    assert!(b.app.cyclo_courant().is_some_and(|c| c.dessin.is_some()));
    // Sa vignette, et l'image réduite pour une carte graphique de 2048 (jamais d'arrêt).
    let petite = cyclos::reduire(&img, 2048);
    assert_eq!((petite.largeur, petite.hauteur), (2048, 1024));
    section_cyclo(&mut b);
    ou("Cyclo : Mon cyclo");
    // Un cyclo dessiné sur une feuille de 4096 ne se rouvre pas ici : un message, pas d'arrêt.
    let mut doc = projet();
    avec_dessin(&mut doc, Preset::Jour.cyclo(), false, |f| peindre(f, 0, 10, 0, 10, [9, 9, 9, 255]));
    let mut b = banc_3d(doc);
    b.cote_max = Some(2048);
    b.chauffer(2);
    assert!(!b.app.dessiner_mon_cyclo(None));
    assert!(b.app.status.contains("4096"), "{}", b.app.status);
}

#[test]
fn the_cyclo_tab_opens_and_fits_with_its_ghosts_in_view() {
    let mut b = banc_3d(projet());
    assert!(b.app.dessiner_mon_cyclo(Some(Depart::Vide)));
    b.chauffer(4);
    let r = b.app.canvas_rect;
    let (w, h, l) = (cd::LARGEUR as f32, cd::HAUTEUR as f32, cd::FANTOME as f32);
    let vu = |b: &Banc| {
        let (g, d) = (b.app.layer_to_screen(eframe::egui::pos2(-l, h / 2.0)), b.app.layer_to_screen(eframe::egui::pos2(w + l, h / 2.0)));
        (r.contains(g) && r.contains(d), g, d)
    };
    let (ok, g, d) = vu(&b);
    assert!(ok, "les deux fantômes à l'écran : {g:?} {d:?} dans {r:?}");
    // On zoome, puis « Ajuster » : de nouveau la feuille et ses fantômes.
    b.app.view.zoom *= 3.0;
    b.chauffer(2);
    assert!(!vu(&b).0);
    b.app.run_action("vue.ajuster");
    b.chauffer(3);
    assert!(vu(&b).0, "« Ajuster » montre les fantômes");
    assert!(b.app.terminer_cyclo());
}

// ------------------------------------------------------------------------------------------------ 2. pas un plan

#[test]
fn the_cyclo_sheet_is_not_a_plan_anywhere() {
    let mut doc = projet();
    let compte = |d: &Document| (d.panels.len(), d.scenes.len(), d.sequences.len(), d.total_frames(), d.cels.len(), d.sequence_order(), d.board.strokes.len() + d.board.texts.len());
    let avant = compte(&doc);
    avec_dessin(&mut doc, Preset::Ville.cyclo(), false, |f| peindre(f, 100, 300, 500, 700, [200, 30, 30, 255]));
    assert_eq!(avant, compte(&doc), "ni plan, ni scène, ni séquence, ni image, ni dessin du film de plus");
    // L'export du film : la même image (le cadre), le cyclo derrière.
    let img = crate::decor3d_tests::image_exportee(&doc);
    assert_eq!(img.len(), doc.frame[0] * doc.frame[1]);
    // Dans l'onglet du dessin : son projet n'a qu'un plan, le film n'en voit rien.
    let mut b = banc_3d(doc);
    let n = b.app.doc.panels.len();
    assert!(b.app.dessiner_mon_cyclo(None));
    assert_eq!(b.app.doc.panels.len(), 1);
    assert!(b.app.terminer_cyclo());
    assert_eq!(compte(&b.app.doc), avant);
    assert_eq!(b.app.doc.panels.len(), n);
}

// ------------------------------------------------------------------------------------------------ 3. un trait, une image

#[test]
fn a_stroke_changes_the_cyclo_image_and_only_once() {
    let mut b = banc_3d(projet());
    b.app.changer_cyclo("Cyclo", Some(Preset::Jour.cyclo()));
    assert!(b.app.dessiner_mon_cyclo(Some(Depart::Vide)));
    b.chauffer(4);
    attendre_la_bande(&mut b);
    let aplaties = b.app.cyclo_edition.as_ref().unwrap().aplaties;
    // Un trait au milieu de la feuille (devant, un peu au-dessus de l'horizon) : la bande se refait une fois, au lâcher.
    crate::calqueannot_tests::trait_au_milieu(&mut b, 0.0);
    attendre_la_bande(&mut b);
    b.chauffer(20);
    assert_eq!(b.app.cyclo_edition.as_ref().unwrap().aplaties, aplaties + 1, "une fois par trait, rien au repos");
    // Où est l'encre ? (pour lire l'image du cyclo au même endroit)
    let ed_bande = b.app.cyclo_edition.as_ref().unwrap().vue.clone().unwrap().1;
    let fond = cd::feuille(&Preset::Jour.cyclo(), true, "x");
    let base = cd::aplatir(&fond);
    let encre = (0..ed_bande.rgba.len() / 4).find(|&i| (0..3).any(|k| (ed_bande.rgba[i * 4 + k] as i32 - base.rgba[i * 4 + k] as i32).abs() > 60)).expect("l'encre du trait");
    let (x, y) = (encre % cd::LARGEUR, encre / cd::LARGEUR);
    assert!(b.app.terminer_cyclo());
    let d = &b.app.doc.decors3d.par_sequence[&b.app.doc.sequence_of(0)];
    let c = d.cyclo.unwrap();
    let img = image_du_cyclo(&b.app.doc);
    assert_eq!((img.largeur, img.hauteur), (cyclos::LARGEUR * 2, cyclos::HAUTEUR * 2));
    // L'encre du trait est dans l'image du cyclo ; le ciel de fond seul (sans la bande) ne l'a pas.
    let sans = cyclos::generer_avec(&c, img.largeur, img.hauteur, None);
    let az = (x as f32 + 0.5) / cd::LARGEUR as f32 * 360.0 - 180.0;
    let e = cd::E_HAUT - (y as f32 + 0.5) / cd::HAUTEUR as f32 * (cd::E_HAUT - cd::E_BAS);
    let dir = direction(az, e);
    assert!(ecart(img.vers(dir, 0.0), sans.vers(dir, 0.0)) > 0.2, "le trait ({x}, {y}) : {:?} / {:?}", img.vers(dir, 0.0), sans.vers(dir, 0.0));
    // Faite une fois ; l'image des plans et l'export ne la refont pas.
    for _ in 0..3 {
        let _ = crate::decor3d_tests::image_exportee(&b.app.doc);
        b.chauffer(2);
    }
    assert_eq!(cyclos::faites_de(c.cle()), 1);
}

// ------------------------------------------------------------------------------------------------ 4. les bords

#[test]
fn the_ghosts_show_the_opposite_edge_and_the_band_joins_across_the_seam() {
    let mut f = cd::feuille(&Preset::Jour.cyclo(), true, "Cyclo");
    // Un trait rouge qui sort à droite, son bout repris à gauche.
    peindre(&mut f, cd::LARGEUR - 40, cd::LARGEUR, 600, 640, [220, 20, 20, 255]);
    peindre(&mut f, 0, 40, 600, 640, [220, 20, 20, 255]);
    let bande = cd::aplatir(&f);
    let (g, d) = cd::fantomes(&bande);
    let l = cd::FANTOME;
    // Le fantôme de gauche est le bord droit de la feuille (et inversement).
    for y in [600, 620, 639] {
        assert_eq!(&g[(y * l + l - 1) * 4..(y * l + l) * 4], &bande.rgba[(y * cd::LARGEUR + cd::LARGEUR - 1) * 4..(y * cd::LARGEUR + cd::LARGEUR) * 4]);
        assert_eq!(&d[(y * l) * 4..(y * l + 1) * 4], &bande.rgba[(y * cd::LARGEUR) * 4..(y * cd::LARGEUR + 1) * 4]);
        assert_eq!(g[(y * l + l - 1) * 4], 220, "le rouge du bord droit, à gauche");
    }
    // Dans l'image du cyclo, derrière (le raccord) : le rouge des deux côtés du raccord, sans couture.
    let c = Cyclo { dessin: Some(1), ..Preset::Jour.cyclo() };
    let img = cyclos::generer_avec(&c, 4096, 2048, Some(&bande));
    let e = cd::E_HAUT - 620.0 / cd::HAUTEUR as f32 * (cd::E_HAUT - cd::E_BAS);
    let (a, z) = (img.vers(direction(179.5, e), 0.0), img.vers(direction(180.5, e), 0.0));
    assert!(a[0] > 0.5 && z[0] > 0.5 && ecart(a, z) < 0.05, "le raccord : {a:?} / {z:?}");
    // Les directions de la feuille : 0° au milieu (devant), 90° aux trois quarts (à droite), 180° aux bords, 270° au quart.
    assert_eq!((cd::x_de_l_azimut(0.0), cd::x_de_l_azimut(90.0), cd::x_de_l_azimut(180.0), cd::x_de_l_azimut(270.0)), (2048.0, 3072.0, 0.0, 1024.0));
    let mut droite = cd::feuille(&Preset::Jour.cyclo(), true, "Cyclo");
    peindre(&mut droite, 3060, 3084, 600, 640, [20, 20, 220, 255]);
    let img = cyclos::generer_avec(&c, 4096, 2048, Some(&cd::aplatir(&droite)));
    let (d90, d270) = (img.vers(direction(90.0, e), 0.0), img.vers(direction(270.0, e), 0.0));
    assert!(d90[0] < 0.1 && d90[2] > 0.5, "le bleu peint à 90° est à droite (+x) : {d90:?}");
    assert!(d270[0] > 0.3, "et pas à gauche : {d270:?}");
}

#[test]
fn a_stroke_across_the_right_edge_comes_back_on_the_left() {
    let mut b = banc_3d(projet());
    assert!(b.app.dessiner_mon_cyclo(Some(Depart::Vide)));
    b.chauffer(4);
    attendre_la_bande(&mut b);
    // Un trait qui passe le bord droit de la feuille, à la hauteur de l'horizon.
    let (w, y) = (cd::LARGEUR as f32, cd::horizon_y() - 100.0);
    let (de, a) = (b.app.layer_to_screen(eframe::egui::pos2(w - 150.0, y)), b.app.layer_to_screen(eframe::egui::pos2(w + 150.0, y)));
    crate::traits_continus_tests::tracer(&mut b, de, a);
    b.chauffer(3);
    attendre_la_bande(&mut b);
    let li = b.app.doc.panels[0].layers.len() - 1;
    let cel = b.app.doc.panels[0].layers[li].frames[0].unwrap();
    let r = &b.app.doc.cels[&cel].raster;
    let encre = |x: usize| (y as usize - 8..y as usize + 8).any(|yy| r.get(x, yy).a() > 100);
    assert!(encre(cd::LARGEUR - 20), "le trait avant le bord");
    assert!(encre(40), "son bout revient à gauche");
    assert!(r.outer.keys().all(|&(_, ty)| !(0..(cd::HAUTEUR / crate::raster::TILE) as i32).contains(&ty)), "plus rien hors de la feuille à hauteur de la bande");
    // La bande aplatie (et l'image du cyclo) l'ont des deux côtés.
    let bande = b.app.cyclo_edition.as_ref().unwrap().vue.clone().unwrap().1;
    assert!(bande.rgba[((y as usize) * cd::LARGEUR + 40) * 4] < 100, "à gauche, dans la bande : l'encre (sombre)");
    assert!(b.app.terminer_cyclo());
}

#[test]
fn the_tab_paints_its_ghosts_horizon_and_directions() {
    let mut b = banc_3d(projet());
    assert!(b.app.dessiner_mon_cyclo(Some(Depart::Choix(Preset::Montagne))));
    b.chauffer(4);
    attendre_la_bande(&mut b);
    b.chauffer(4);
    let t: Vec<String> = textes(&b).into_iter().map(|(t, _)| t).collect();
    for mot in ["Horizon", "0° devant", "90° droite", "180° derrière", "270° gauche"] {
        assert!(t.iter().any(|x| x == mot), "« {mot} » : {t:?}");
    }
    assert!(b.app.terminer_cyclo());
}

// ------------------------------------------------------------------------------------------------ 5. le départ

#[test]
fn starting_from_a_preset_paints_its_band_and_vide_paints_only_its_sky() {
    let montagne = cd::feuille(&Preset::Montagne.cyclo(), false, "Cyclo");
    let vide = cd::feuille(&Preset::Montagne.cyclo(), true, "Cyclo");
    for f in [&montagne, &vide] {
        assert_eq!(f.panels[0].layers.len(), 2);
        let fond = f.cels[&f.panels[0].layers[0].frames[0].unwrap()].raster.to_rgba();
        assert!(fond.chunks(4).all(|p| p[3] == 255), "le fond est peint partout");
        assert!(f.cels[&f.panels[0].layers[1].frames[0].unwrap()].raster.to_rgba().chunks(4).all(|p| p[3] == 0), "« Mon horizon » est vide");
    }
    // Juste au-dessus de l'horizon : les crêtes de la montagne varient d'une colonne à l'autre ; le vide est un dégradé lisse.
    let ligne = |f: &Document| {
        let r = f.cels[&f.panels[0].layers[0].frames[0].unwrap()].raster.to_rgba();
        let y = cd::horizon_y() as usize - 60;
        (0..cd::LARGEUR).step_by(16).map(|x| r[(y * cd::LARGEUR + x) * 4] as f32).collect::<Vec<_>>()
    };
    let ecart_type = |v: Vec<f32>| {
        let m = v.iter().sum::<f32>() / v.len() as f32;
        (v.iter().map(|x| (x - m) * (x - m)).sum::<f32>() / v.len() as f32).sqrt()
    };
    let (sm, sv) = (ecart_type(ligne(&montagne)), ecart_type(ligne(&vide)));
    assert!(sm > sv + 5.0 && sv < 3.0, "les crêtes : {sm} ; le vide : {sv}");
    // La bande du choix est bien le cyclo du choix : la même couleur, dans la même direction.
    let c = Preset::Jour.cyclo();
    let bande = cd::bande_du_choix(&c, false);
    let img = cyclos::generer_avec(&c, 4096, 2048, None);
    let (x, y) = (cd::x_de_l_azimut(90.0) as usize, 300usize);
    let e = cd::E_HAUT - (y as f32 + 0.5) / cd::HAUTEUR as f32 * (cd::E_HAUT - cd::E_BAS);
    let lu = img.vers(direction(90.0, e), 0.0);
    let o = (y * cd::LARGEUR + x) * 4;
    let b = crate::couleurs3d::lineaire([bande[o], bande[o + 1], bande[o + 2]]);
    assert!(ecart(lu, b) < 0.03, "{lu:?} / {b:?}");
}

// ------------------------------------------------------------------------------------------------ 6. l'enregistrement

#[test]
fn the_drawing_is_saved_with_the_project_read_back_written_only_when_it_changes_and_removed_with_its_cyclo() {
    let dir = crate::decor3d_tests::dossier("cyclodessin203");
    let manifeste = dir.join("Film.shot");
    let mut doc = projet();
    let c = avec_dessin(&mut doc, Preset::Coucher.cyclo(), false, |f| peindre(f, 1000, 1400, 500, 700, [30, 200, 60, 255]));
    let seq = doc.sequence_of(0);
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let ch = cd::chemin(&dir, seq);
    assert!(ch.exists(), "{}", ch.display());
    let texte = std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap();
    assert!(texte.contains("\"dessin\""), "{texte}");
    // Relu : le même cyclo, le même dessin (sa bande, au pixel près).
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let d = &relu.doc.decors3d.par_sequence[&seq];
    assert_eq!(d.cyclo, Some(c));
    let avant = doc.decors3d.par_sequence[&seq].dessin_cyclo.as_ref().unwrap().bande();
    assert!(d.dessin_cyclo.as_ref().unwrap().bande().rgba == avant.rgba, "la bande relue");
    assert_eq!(relu.doc.panels.len(), doc.panels.len());
    // Réenregistré sans changement : le dessin n'est pas réécrit.
    let date = std::fs::metadata(&ch).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    crate::project::save(&relu.doc, &manifeste, &mut Default::default()).unwrap();
    assert_eq!(std::fs::metadata(&ch).unwrap().modified().unwrap(), date);
    // Plus de cyclo dessiné : son dossier part.
    let mut sans = relu.doc;
    crate::cyclosui::poser(sans.decors3d.par_sequence.get_mut(&seq).unwrap(), Some(Preset::Coucher.cyclo()));
    crate::project::save(&sans, &manifeste, &mut Default::default()).unwrap();
    assert!(!ch.exists() && !dir.join(cd::DOSSIER).join(seq.to_string()).exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_old_feanor_still_opens_the_project_and_loses_nothing_else() {
    let dir = crate::decor3d_tests::dossier("cyclodessin203-avant");
    let manifeste = dir.join("Film.shot");
    let mut doc = projet();
    avec_dessin(&mut doc, Preset::Ville.cyclo(), true, |f| peindre(f, 10, 50, 10, 50, [1, 2, 3, 255]));
    let seq = doc.sequence_of(0);
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifeste).unwrap();
    assert!(texte.contains("\"version\": 61"), "le format reste 61");
    // Un FEANOR de la partie 202 lit `decors3d.json` (son cyclo sans la clé `dessin`) ; un d'avant, sans cyclo.
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct Cyclo202 {
        preset: String,
        #[serde(default)]
        heure: f32,
        #[serde(default)]
        graine: u32,
    }
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct Sequence202 {
        sequence: u64,
        objets: Vec<serde_json::Value>,
        #[serde(default)]
        cyclo: Option<Cyclo202>,
    }
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap()).unwrap();
    assert_eq!(v["version"], serde_json::json!(2));
    for s in v["decors"].as_array().unwrap() {
        let s: Sequence202 = serde_json::from_value(s.clone()).expect("lu par un FEANOR d'avant");
        assert_eq!(s.cyclo.map(|c| c.preset), Some("ville".into()));
    }
    // Le projet sans le dossier des cyclos : tout le reste s'ouvre, le cyclo montre son choix ; le message le dit.
    std::fs::remove_dir_all(dir.join(cd::DOSSIER)).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert_eq!((relu.doc.panels.len(), relu.doc.sequences.len()), (doc.panels.len(), doc.sequences.len()));
    assert_eq!(relu.doc.decors3d.par_sequence[&seq].objets.len(), 2);
    assert!(relu.doc.decors3d.par_sequence[&seq].dessin_cyclo.is_none());
    assert!(relu.warnings.iter().any(|w| w.contains("cyclo")), "{:?}", relu.warnings);
    let _ = crate::decor3d_tests::image_exportee(&relu.doc);
    // Réenregistré par ce FEANOR-ci sans le dessin : la clé reste (le dessin peut revenir), rien d'autre ne bouge.
    crate::project::save(&relu.doc, &manifeste, &mut Default::default()).unwrap();
    assert!(std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap().contains("\"dessin\""));
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------------------ 7. copier, dupliquer

#[test]
fn copying_the_decor_and_duplicating_the_sequence_carry_the_drawing() {
    let mut doc = projet();
    let c = avec_dessin(&mut doc, Preset::Mer.cyclo(), false, |f| peindre(f, 0, 10, 0, 10, [9, 9, 9, 255]));
    let o = doc.sequence_order();
    let (a, b) = (o[0], o[1]);
    assert!(doc.copier_decor_de_sequence(a, b));
    let (da, db) = (&doc.decors3d.par_sequence[&a], &doc.decors3d.par_sequence[&b]);
    assert_eq!(db.cyclo, Some(c));
    assert!(Arc::ptr_eq(da.dessin_cyclo.as_ref().unwrap(), db.dessin_cyclo.as_ref().unwrap()), "le même dessin, partagé");
    doc.dupliquer_sequence(a).expect("la copie");
    let neuve = *doc.sequence_order().iter().find(|s| **s != a && **s != b).unwrap();
    assert!(doc.decors3d.par_sequence[&neuve].dessin_cyclo.is_some() && doc.decors3d.par_sequence[&neuve].cyclo == Some(c));
    // Enregistrés : un dossier par séquence.
    let dir = crate::decor3d_tests::dossier("cyclodessin203-copie");
    crate::project::save(&doc, &dir.join("Film.shot"), &mut Default::default()).unwrap();
    for s in [a, b, neuve] {
        assert!(cd::chemin(&dir, s).exists());
    }
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------------------ 8. retirer, et Ctrl+Z

#[test]
fn choosing_another_cyclo_asks_before_removing_the_drawing_and_undo_brings_it_back() {
    let mut doc = projet();
    avec_dessin(&mut doc, Preset::Jour.cyclo(), false, |f| peindre(f, 0, 10, 0, 10, [9, 9, 9, 255]));
    let mut b = banc_3d(doc);
    let seq = b.app.doc.sequence_of(0);
    let dessin = b.app.doc.decors3d.par_sequence[&seq].dessin_cyclo.clone().unwrap();
    // Un choix : la question, rien n'a changé.
    b.app.demander_cyclo(ActionCyclo::Choisir(Some(Preset::Nuit.cyclo())));
    b.chauffer(3);
    assert!(b.app.o3.cyclo_a_jeter.is_some());
    assert!(b.app.cyclo_courant().is_some_and(|c| c.dessin.is_some()));
    // « Annuler » : rien.
    let annule = textes(&b).into_iter().rev().find(|(t, _)| t == "Annuler").expect("« Annuler »").1;
    clic(&mut b, annule.center(), Modifiers::NONE);
    b.chauffer(2);
    assert!(b.app.o3.cyclo_a_jeter.is_none() && b.app.cyclo_courant().is_some_and(|c| c.dessin.is_some()));
    // « Retirer le dessin » : la Nuit, sans dessin, une étape.
    b.app.demander_cyclo(ActionCyclo::Choisir(Some(Preset::Nuit.cyclo())));
    b.chauffer(3);
    let retirer = textes(&b).into_iter().find(|(t, _)| t == "Retirer le dessin").expect("« Retirer le dessin »").1;
    clic(&mut b, retirer.center(), Modifiers::NONE);
    b.chauffer(2);
    let d = &b.app.doc.decors3d.par_sequence[&seq];
    assert!(d.cyclo.is_some_and(|c| c.preset == Preset::Nuit && c.dessin.is_none()) && d.dessin_cyclo.is_none());
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Retirer le cyclo dessiné"));
    // Ctrl+Z : le dessin revient, le même.
    annuler(&mut b);
    let d = &b.app.doc.decors3d.par_sequence[&seq];
    assert!(Arc::ptr_eq(d.dessin_cyclo.as_ref().unwrap(), &dessin));
    // Les réglages (l'heure…) gardent le dessin, sans rien demander.
    let c = b.app.cyclo_courant().unwrap();
    b.app.retoucher_cyclo(Cyclo { heure: 18.0, ..c });
    b.app.decor3d_retouche_finir();
    assert!(b.app.doc.decors3d.par_sequence[&seq].dessin_cyclo.is_some() && b.app.o3.cyclo_a_jeter.is_none());
    // « Dessiner mon cyclo… » par-dessus un dessin : la question aussi.
    b.app.demander_cyclo(ActionCyclo::Dessiner(Depart::Vide));
    assert!(b.app.o3.cyclo_a_jeter.is_some() && b.app.cyclo_edition.is_none());
    b.app.o3.cyclo_a_jeter = None;
}

// ------------------------------------------------------------------------------------------------ 9. la mesure

/// La mesure de la partie 203 : aplatir la feuille (4096 × 1024), faire l'image d'un cyclo dessiné (4096 × 2048), et, dans l'onglet,
/// le temps entre le lâcher d'un trait et ses fantômes.
#[test]
fn mesure_du_cyclo_dessine() {
    let mut f = cd::feuille(&Preset::Montagne.cyclo(), false, "Cyclo");
    peindre(&mut f, 500, 3500, 400, 800, [120, 80, 40, 255]);
    let mut t = Vec::new();
    for _ in 0..3 {
        let t0 = std::time::Instant::now();
        let _ = cd::aplatir(&f);
        t.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    t.sort_by(f64::total_cmp);
    eprintln!("MESURE 203 : aplatir la feuille (4096 × 1024, deux calques) : médiane {:.0} ms", t[1]);
    let bande = cd::aplatir(&f);
    let c = Cyclo { dessin: Some(7), ..Preset::Montagne.cyclo() };
    let t0 = std::time::Instant::now();
    let _ = cyclos::generer_avec(&c, 4096, 2048, Some(&bande));
    eprintln!("MESURE 203 : l'image du cyclo dessiné (4096 × 2048) : {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    let t0 = std::time::Instant::now();
    let _ = cd::feuille(&Preset::Montagne.cyclo(), false, "Cyclo");
    eprintln!("MESURE 203 : la feuille de départ (la bande de la Montagne) : {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    // Dans l'onglet : du lâcher d'un trait à ses fantômes.
    let mut b = banc_3d(projet());
    assert!(b.app.dessiner_mon_cyclo(Some(Depart::Vide)));
    b.chauffer(4);
    attendre_la_bande(&mut b);
    crate::calqueannot_tests::trait_au_milieu(&mut b, 0.0);
    let t0 = std::time::Instant::now();
    attendre_la_bande(&mut b);
    eprintln!("MESURE 203 : dans l'onglet, du lâcher d'un trait à ses fantômes : {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    let mut t = Vec::new();
    for _ in 0..10 {
        let t0 = std::time::Instant::now();
        let _ = b.image(Vec::new(), 1.0 / 60.0);
        t.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    t.sort_by(f64::total_cmp);
    eprintln!("MESURE 203 : une image d'écran de l'onglet au repos (banc sans carte graphique) : médiane {:.1} ms", t[5]);
    assert!(b.app.terminer_cyclo());
    let t0 = std::time::Instant::now();
    let _ = image_du_cyclo(&b.app.doc);
    eprintln!("MESURE 203 : après « Terminer », l'image du cyclo (bande déjà aplatie) : {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
}

/// (Pour regarder : `FEANOR_CYCLOS_PNG=<dossier>` écrit l'écran de l'onglet d'un cyclo (départ Montagne, un trait), puis celui du
/// Storyboard après « Terminer », et l'image du cyclo.)
#[test]
fn ecrire_l_onglet_du_cyclo_en_png() {
    let Ok(dir) = std::env::var("FEANOR_CYCLOS_PNG") else { return };
    if crate::gpu_tests::headless().is_none() {
        return;
    }
    let mut b = banc_taille(projet(), true, 1600, 1000);
    b.cote_max = Some(8192);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(6);
    assert!(b.app.dessiner_mon_cyclo(Some(Depart::Choix(Preset::Montagne))));
    b.chauffer(6);
    crate::calqueannot_tests::trait_au_milieu(&mut b, 0.0);
    attendre_la_bande(&mut b);
    b.chauffer(6);
    let (w, h) = (b.taille.x as u32, b.taille.y as u32);
    image::save_buffer(format!("{dir}/onglet-cyclo.png"), &b.capturer().unwrap(), w, h, image::ColorType::Rgba8).unwrap();
    assert!(b.app.terminer_cyclo());
    b.chauffer(10);
    crate::decor3d::wait_idle();
    cyclos::attendre();
    b.app.renderer.invalidate_all();
    b.chauffer(8);
    image::save_buffer(format!("{dir}/storyboard-cyclo.png"), &b.capturer().unwrap(), w, h, image::ColorType::Rgba8).unwrap();
    let img = image_du_cyclo(&b.app.doc);
    let petite: Vec<u8> = (0..512).flat_map(|y| (0..1024).flat_map(move |x| [x * 4, y * 4])).collect::<Vec<_>>().chunks(2).flat_map(|p| {
        let o = (p[1] * img.largeur + p[0]) * 4;
        img.rgba[o..o + 4].to_vec()
    }).collect();
    image::save_buffer(format!("{dir}/image-cyclo.png"), &petite, 1024, 512, image::ColorType::Rgba8).unwrap();
}
