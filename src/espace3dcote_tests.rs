//! Essais de la vue 3D à côté de la feuille (`espace3dcote.rs`, `docs/ESPACE3D.md` partie 13) : le partage de la zone,
//! ouvrir / fermer / agrandir, quel geste va à quelle vue, la carte qui suit le trait (par morceaux, pixel pour pixel
//! comme une image refaite), le rythme pendant un trait, la lecture qui continue, le budget par image, le repos.

use eframe::egui::{pos2, Color32, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::document::{CelId, Document, Layer, LayerKind};
use crate::espace3dcote::{self as cote, changement, Changement, Cliche, LARGEUR_CARTE};
use crate::perf_tests::{banc_taille, se_reposer, trait_souris, Banc};
use crate::raster::{Raster, TILE};
use crate::render;

const RED: Color32 = Color32::from_rgb(220, 20, 20);
const BLUE: Color32 = Color32::from_rgb(30, 60, 230);

fn fill_rect(r: &mut Raster, x0: usize, y0: usize, x1: usize, y1: usize, color: Color32) {
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = r.tile_index_of(x, y);
            r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = color;
        }
    }
}

/// Un projet 320 × 180 dont chaque calque a un dessin plein de la couleur donnée, dans le rectangle donné.
fn doc_with_cards(cards: &[(Color32, [usize; 4])]) -> Document {
    let mut doc = Document::new(320, 180, 24);
    doc.panels[0].layers = Vec::new();
    for (k, (color, r)) in cards.iter().enumerate() {
        let mut layer = Layer::new(format!("Carte {k}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        fill_rect(&mut doc.cels.get_mut(&cel).expect("dessin").raster, r[0], r[1], r[2], r[3], *color);
        layer.frames = vec![Some(cel); 12];
        doc.panels[0].layers.push(layer);
    }
    doc
}

/// Une application sans fenêtre, 1280 × 720, Espace 3D à côté ouvert.
fn a_cote(cards: &[(Color32, [usize; 4])]) -> Banc {
    let mut b = banc_taille(doc_with_cards(cards), false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.a_cote");
    b.app.espace3d.show_grid = false;
    b.chauffer(4);
    b
}

fn press(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// L'image entière refaite, comparée à celle d'avant où l'on a recopié le morceau refait.
fn applique(avant: &mut eframe::egui::ColorImage, z: [usize; 4], morceau: &eframe::egui::ColorImage) {
    let w = avant.size[0];
    for y in z[1]..z[3] {
        for x in z[0]..z[2] {
            avant.pixels[y * w + x] = morceau.pixels[(y - z[1]) * (z[2] - z[0]) + (x - z[0])];
        }
    }
}

// ================================================================ l'image d'une carte, refaite par morceaux

#[test]
fn a_card_image_redone_by_pieces_is_the_full_image_pixel_for_pixel() {
    // Feuilles plus petites, à peine plus grandes et bien plus grandes que l'image d'une carte (1024 px), côtés qui ne
    // tombent pas sur les tuiles.
    for (w, h) in [(700usize, 400usize), (1500, 900), (3000, 1700), (1025, 577)] {
        let mut r = Raster::new(w, h);
        fill_rect(&mut r, 10, 10, w / 3, h / 4, RED);
        let mut image = render::cel_image(&r, LARGEUR_CARTE);
        let mut cliche = Cliche::of(&r);
        assert_eq!(changement(&cliche, &r, LARGEUR_CARTE), Changement::Rien, "{w}×{h} : rien n'a changé");
        // Des traits courts un peu partout, sur les bords des tuiles et de la feuille.
        let coups = [(63usize, 63usize, 66usize, 66usize), (w - 5, h - 5, w, h), (0, h / 2, 3, h / 2 + 3), (w / 2 - 1, 0, w / 2 + 70, 2), (128, 128, 129, 129)];
        for (k, &(x0, y0, x1, y1)) in coups.iter().enumerate() {
            let c = Color32::from_rgb(40 * k as u8, 200, 255 - 30 * k as u8);
            fill_rect(&mut r, x0, y0, x1, y1, c);
            match changement(&cliche, &r, LARGEUR_CARTE) {
                Changement::Zone(z) => {
                    let morceau = render::cel_image_region(&r, LARGEUR_CARTE, z);
                    assert_eq!(morceau.size, [z[2] - z[0], z[3] - z[1]]);
                    applique(&mut image, z, &morceau);
                    let size = render::cel_image_size(&r, LARGEUR_CARTE);
                    assert!(Changement::Zone(z).cout(size) * 10 < size[0] * size[1], "{w}×{h}, coup {k} : un petit morceau ({z:?} sur {size:?})");
                }
                autre => panic!("{w}×{h}, coup {k} : un morceau attendu, pas {autre:?}"),
            }
            let plein = render::cel_image(&r, LARGEUR_CARTE);
            assert!(image.pixels == plein.pixels, "{w}×{h}, coup {k} : l'image refaite par morceaux est l'image refaite en entier");
            cliche = Cliche::of(&r);
        }
    }
}

#[test]
fn a_change_outside_the_sheet_or_over_half_of_it_redoes_the_whole_card() {
    let mut r = Raster::new(640, 360);
    let cliche = Cliche::of(&r);
    fill_rect(&mut r, 0, 0, 600, 300, BLUE);
    assert_eq!(changement(&cliche, &r, LARGEUR_CARTE), Changement::Tout, "plus de la moitié : tout");
    // Hors de la feuille (toile infinie).
    let mut r = Raster::new(640, 360);
    let cliche = Cliche::of(&r);
    r.outer.insert((-1, -1), crate::raster::Tile::new([RED; crate::raster::TILE_PIXELS]));
    assert_eq!(changement(&cliche, &r, LARGEUR_CARTE), Changement::Tout, "hors de la feuille : tout");
    // Une autre taille.
    let autre = Raster::new(320, 180);
    assert_eq!(changement(&cliche, &autre, LARGEUR_CARTE), Changement::Tout);
}

// ================================================================ le partage, ouvrir, fermer, agrandir

#[test]
fn the_drawing_zone_is_shared_sheet_left_space_right_and_too_narrow_keeps_the_sheet() {
    let total = Rect::from_min_max(pos2(100.0, 50.0), pos2(1100.0, 650.0));
    let (feuille, barre, vue) = cote::partage(total, 0.5).expect("assez large");
    assert_eq!(feuille.left(), total.left());
    assert_eq!(vue.right(), total.right());
    assert_eq!(feuille.right(), barre.left());
    assert_eq!(barre.right(), vue.left());
    assert!((vue.width() - 500.0).abs() < 1.0, "moitié : {}", vue.width());
    // Bornes de la part, et les deux vues gardent leur largeur minimale.
    let (_, _, petite) = cote::partage(total, 0.01).expect("assez large");
    assert!((petite.width() - 200.0).abs() < 1.0, "part au moins 20 % : {}", petite.width());
    let (f, _, _) = cote::partage(total, 0.99).expect("assez large");
    assert!((f.width() - (1000.0 - 800.0 - cote::BARRE)).abs() < 1.0 && f.width() >= cote::LARGEUR_MIN, "part au plus 80 % : {}", f.width());
    let (_, _, v) = cote::partage(total, f32::NAN).expect("assez large");
    assert!((v.width() - 500.0).abs() < 1.0);
    assert!(cote::partage(Rect::from_min_size(pos2(0.0, 0.0), eframe::egui::vec2(300.0, 400.0)), 0.5).is_none(), "trop étroit : la feuille seule");
}

#[test]
fn side_by_side_opens_and_closes_in_one_gesture_and_the_full_space_still_replaces_the_sheet() {
    let mut b = banc_taille(doc_with_cards(&[(RED, [100, 50, 220, 130])]), false, 1280, 720);
    b.chauffer(3);
    let pleine = b.app.canvas_rect;
    assert!(crate::keymap::action_info("espace3d.a_cote").is_some_and(|a| a.implemented));
    assert!(crate::palette::palette_commands().iter().any(|(id, _)| *id == "espace3d.a_cote"), "dans la palette");

    b.app.run_action("espace3d.a_cote");
    b.chauffer(3);
    assert!(b.app.espace3d.a_cote && !b.app.espace3d.on, "à côté, pas en plein écran");
    let (feuille, vue) = (b.app.canvas_rect, b.app.espace3d.rect);
    assert!(feuille.width() < pleine.width() * 0.6, "la feuille a la moitié gauche : {feuille:?}");
    assert!(vue.left() > feuille.right() && vue.right() <= pleine.right() + 0.5, "la vue 3D à droite : {vue:?}");
    assert!(!feuille.intersects(vue));

    // Agrandir : le plein écran d'avant, à la place de la feuille ; le refermer rend le côte à côte.
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(b.app.espace3d_plein());
    assert_eq!(b.app.espace3d.rect, pleine, "plein écran : toute la zone");
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(!b.app.espace3d.on && b.app.espace3d_a_cote_visible(), "refermé : de nouveau à côté");
    assert_eq!(b.app.canvas_rect, feuille);

    // Depuis le plein écran, « à côté » y remet la vue.
    b.app.run_action("espace3d.basculer");
    b.chauffer(2);
    b.app.run_action("espace3d.a_cote");
    b.chauffer(3);
    assert!(!b.app.espace3d.on && b.app.espace3d_a_cote_visible());

    // Fermer : la feuille reprend toute la zone.
    b.app.run_action("espace3d.a_cote");
    b.chauffer(3);
    assert!(!b.app.espace3d.a_cote);
    assert_eq!(b.app.canvas_rect, pleine);
    assert!(b.app.history.names().0.is_empty(), "ouvrir et fermer n'est pas une étape d'annulation");
}

#[test]
fn the_tools_of_the_space_set_the_side_view_without_going_full_screen() {
    let mut b = a_cote(&[(RED, [100, 50, 220, 130])]);
    for (action, mode) in [("espace3d.outil_deplacer", crate::espace3dgizmo::Mode::Move), ("espace3d.outil_tourner", crate::espace3dgizmo::Mode::Rotate), ("espace3d.outil_choisir", crate::espace3dgizmo::Mode::Select)] {
        b.app.run_action(action);
        b.chauffer(1);
        assert!(!b.app.espace3d.on && b.app.espace3d_a_cote_visible(), "{action} reste à côté");
        assert_eq!(b.app.espace3d.mode, mode);
    }
    b.app.run_action("espace3d.vue_camera");
    b.chauffer(1);
    assert!(!b.app.espace3d.on && b.app.espace3d.through_camera);
    // Sans le côte à côte, comme avant : l'outil ouvre le plein écran.
    b.app.run_action("espace3d.a_cote");
    b.app.run_action("espace3d.outil_deplacer");
    assert!(b.app.espace3d_plein());
}

#[test]
fn the_part_of_the_side_view_is_kept_and_bounded() {
    let mut l = crate::workspace::Layout::default();
    assert_eq!(l.part_espace3d, cote::PART_DEFAUT);
    l.part_espace3d = 0.7;
    let texte = serde_json::to_string(&l).expect("écrit");
    assert!(texte.contains("\"part_espace3d\":0.7"), "{texte}");
    let relu: crate::workspace::Layout = serde_json::from_str(&texte).expect("relu");
    assert_eq!(relu.part_espace3d, 0.7);
    // Une disposition d'avant n'a pas le champ : la part par défaut.
    let ancien: crate::workspace::Layout = serde_json::from_str("{}").expect("relu");
    assert_eq!(ancien.part_espace3d, cote::PART_DEFAUT);
    l.part_espace3d = 5.0;
    assert_eq!(l.clone().sanitized().part_espace3d, cote::PART_MAX);
    // Elle suit d'un espace de travail à l'autre, comme les autres réglages de la vue.
    let mut vue = crate::workspace::Layout::default();
    vue.part_espace3d = 0.3;
    assert_eq!(crate::workspace::Layout::default().avec_les_reglages_de(&vue).part_espace3d, 0.3);
}

// ================================================================ les gestes

#[test]
fn a_gesture_belongs_to_the_view_where_it_starts() {
    let mut b = a_cote(&[(BLUE, [0, 0, 320, 180]), (RED, [100, 50, 220, 130])]);
    // (Le bleu, fond de toute la feuille, plus loin : le rouge est devant.)
    b.app.doc.panels[0].layers[0].depth = 1.5;
    b.app.select_layer(0);
    b.chauffer(2);
    let cels: Vec<(CelId, u64)> = b.app.doc.cels.iter().map(|(id, c)| (*id, c.version)).collect();
    // Un appui dans la vue 3D, outil Choisir, sur la carte rouge (au centre de la vue après Recadrer) : elle est choisie,
    // son calque devient le calque courant ; la feuille ne reçoit pas de trait.
    b.app.espace3d.mode = crate::espace3dgizmo::Mode::Select;
    b.app.espace3d.cam = Some(crate::espace3dmath::Orbite { target: [0.0, 0.0, 0.0], azimuth: 0.0, elevation: 0.0, distance: 12.0, focal: 35.0 });
    b.chauffer(2);
    let vue = b.app.espace3d.rect;
    let centre = vue.center();
    b.image(vec![Event::PointerMoved(centre)], 0.02);
    b.image(vec![press(centre, true)], 0.02);
    b.image(vec![press(centre, false)], 0.02);
    b.chauffer(2);
    let rouge = b.app.layers()[1].id;
    assert_eq!(b.app.espace3d.sel, vec![crate::espace3d::card_selection(rouge)], "la carte rouge est choisie");
    assert_eq!(b.app.layer().id, rouge, "son calque devient le calque courant");
    assert!(b.app.espace3d.clic_3d, "Réglages : ceux de la vue 3D");
    for (id, v) in &cels {
        assert_eq!(b.app.doc.cels[id].version, *v, "un appui dans la vue 3D ne dessine pas");
    }
    // Un trait sur la feuille qui file dans la vue 3D : il dessine, la vue 3D ne choisit rien de neuf.
    b.app.espace3d.sel.clear();
    let feuille = b.app.canvas_rect;
    let debut = feuille.center();
    let fin = pos2(vue.center().x, debut.y);
    b.image(vec![Event::PointerMoved(debut), press(debut, true)], 0.02);
    for k in 1..=10 {
        b.image(vec![Event::PointerMoved(debut + (fin - debut) * (k as f32 / 10.0))], 0.02);
    }
    b.image(vec![press(fin, false)], 0.02);
    b.chauffer(2);
    let cel = b.app.layer().shown_at(b.app.cur_frame).expect("dessin");
    assert!(b.app.doc.cels[&cel].version > cels.iter().find(|(id, _)| *id == cel).map_or(0, |c| c.1), "le trait a dessiné");
    assert!(!b.app.espace3d.clic_3d, "Réglages : de nouveau ceux de l'outil");
    assert!(b.app.espace3d.sel.is_empty() || b.app.espace3d.sel == vec![crate::espace3d::card_selection(rouge)], "le trait n'a rien choisi dans la vue 3D : {:?}", b.app.espace3d.sel);
    assert!(b.app.espace3d.drag.is_none());
}

// ================================================================ la carte suit le trait, sans latence

#[test]
fn a_stroke_on_the_sheet_updates_its_card_by_pieces_at_most_thirty_times_a_second() {
    let mut b = a_cote(&[(BLUE, [0, 0, 160, 90])]);
    se_reposer(&mut b, 40);
    let cel = b.app.layer().shown_at(b.app.cur_frame).expect("dessin");
    assert_eq!(b.app.espace3d.card_textures.get(&cel).map(|t| t.version), Some(b.app.doc.cels[&cel].version), "la carte a son image");
    let feuille = b.app.canvas_rect;
    // Un trait de 60 images à 240 images par seconde (un quart de seconde).
    let (n, dt) = (60usize, 1.0 / 240.0);
    let mut refaites = 0;
    let mut temps = b.app.espace3d.gardee.as_ref().map(|g| g.temps);
    let mut suivies = 0;
    for k in 0..n {
        b.image(trait_souris(k, n, 2, feuille), dt);
        let t = b.app.espace3d.gardee.as_ref().map(|g| g.temps);
        if t != temps {
            refaites += 1;
            temps = t;
            // Refaite : la carte montre le dessin tel qu'il est à cette image.
            if b.app.espace3d.card_textures.get(&cel).map(|t| t.version) == Some(b.app.doc.cels[&cel].version) {
                suivies += 1;
            }
        }
    }
    assert!(refaites >= 4 && refaites <= 10, "un quart de seconde de trait : la vue 3D refaite 4 à 10 fois (30 par seconde au plus), pas {refaites}");
    assert_eq!(suivies, refaites, "chaque vue refaite montre le trait du moment");
    se_reposer(&mut b, 40);
    assert_eq!(b.app.espace3d.card_textures.get(&cel).map(|t| t.version), Some(b.app.doc.cels[&cel].version), "au relâché, la carte a tout le trait");
    assert!(b.app.espace3d.card_textures[&cel].cliche.is_some(), "la carte tracée garde son cliché");
}

#[test]
fn playback_goes_on_side_by_side_and_the_cards_follow_the_frames() {
    let mut doc = doc_with_cards(&[(BLUE, [0, 0, 160, 90])]);
    // Une animation : un dessin par image, à des places différentes.
    let mut frames = Vec::new();
    for k in 0..6 {
        let cel = doc.new_cel(LayerKind::Bitmap);
        fill_rect(&mut doc.cels.get_mut(&cel).expect("dessin").raster, 20 * k, 10, 20 * k + 30, 60, RED);
        frames.push(Some(cel));
    }
    doc.panels[0].layers[0].frames = frames.clone();
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.a_cote");
    b.chauffer(3);
    b.app.run_action("lecture.basculer");
    let mut vus = std::collections::HashSet::new();
    for _ in 0..40 {
        b.image(Vec::new(), 1.0 / 60.0);
        assert!(b.app.espace3d_a_cote_visible());
        if let Some(c) = b.app.espace3d.vu_par_calque.values().next() {
            vus.insert(*c);
        }
    }
    assert!(b.app.playing, "le côte à côte n'arrête pas la lecture");
    assert!(vus.len() >= 4, "la carte suit les images du film : {} dessins vus", vus.len());
}

#[test]
fn many_new_drawings_at_once_are_spread_over_frames_and_none_is_lost() {
    // 8 calques de feuilles entières : à l'ouverture, chacun doit faire son image (plus que le budget d'une image).
    let cards: Vec<(Color32, [usize; 4])> = (0..8).map(|k| (Color32::from_rgb(30 * k as u8, 100, 200), [0, 0, 320, 180])).collect();
    let mut doc = doc_with_cards(&cards);
    doc.width = 1920;
    doc.height = 1080;
    for l in &doc.panels[0].layers.clone() {
        let cel = l.frames[0].expect("dessin");
        let c = doc.cels.get_mut(&cel).expect("dessin");
        c.raster = Raster::new(1920, 1080);
        fill_rect(&mut c.raster, 0, 0, 1900, 1000, RED);
        c.version += 1;
    }
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.a_cote");
    b.image(Vec::new(), 1.0 / 60.0);
    let faites = b.app.espace3d.card_textures.len();
    assert!(faites >= 1 && faites < 8, "une image d'écran n'en prépare qu'une part : {faites}");
    se_reposer(&mut b, 60);
    assert_eq!(b.app.espace3d.card_textures.len(), 8, "toutes les cartes ont leur image");
    assert!(b.app.espace3d.gardee.is_some());
}

#[test]
fn the_application_rests_with_the_side_view_open() {
    let mut b = a_cote(&[(BLUE, [0, 0, 160, 90]), (RED, [100, 50, 220, 130])]);
    let n = se_reposer(&mut b, 60);
    assert!(n < 60, "l'application finit par se reposer, vue 3D à côté ouverte");
}

// ================================================================ mesure : ce que la vue d'à côté coûte pendant un trait

/// Un projet 1920 × 1080 de 10 calques bien remplis ; un trait de 120 images (240 par seconde) sur la feuille, vue 3D
/// d'à côté fermée puis ouverte : le temps de chaque image d'écran (interface, découpage, carte graphique).
fn trait_mesure(a_cote: bool) -> (f64, f64, f64) {
    let mut doc = Document::with_frame(1920, 1080, 24);
    doc.panels[0].layers = Vec::new();
    for k in 0..10usize {
        let mut layer = Layer::new(format!("Carte {k}"), LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        fill_rect(&mut doc.cels.get_mut(&cel).expect("dessin").raster, 100 + k * 80, 100, 900 + k * 80, 900, if k % 2 == 0 { RED } else { BLUE });
        layer.frames = vec![Some(cel); 12];
        layer.depth = k as f32 * 0.3 - 1.0;
        doc.panels[0].layers.push(layer);
    }
    let mut b = banc_taille(doc, true, 1600, 900);
    b.chauffer(2);
    if a_cote {
        b.app.run_action("espace3d.a_cote");
    }
    se_reposer(&mut b, 40);
    let feuille = b.app.canvas_rect;
    let n = 120;
    let mesures: Vec<_> = (0..n).map(|k| b.image(trait_souris(k, n, 2, feuille), 1.0 / 240.0)).collect();
    let part = |f: &dyn Fn(&crate::perf_tests::Mesure) -> f64| mesures.iter().map(f).sum::<f64>() / n as f64;
    println!("PERF détail (à côté : {a_cote}) : interface {:.2} ms, découpage {:.2} ms, carte graphique {:.2} ms, sommets {}", part(&|m| m.construction), part(&|m| m.decoupage), part(&|m| m.carte), mesures.iter().map(|m| m.sommets).sum::<usize>() / n);
    let mut temps: Vec<f64> = mesures.iter().map(|m| m.total()).collect();
    temps.sort_by(|a, b| a.total_cmp(b));
    let moyenne = temps.iter().sum::<f64>() / n as f64;
    (moyenne, temps[n / 2], temps[n * 95 / 100])
}

#[test]
fn a_stroke_with_the_side_view_open_stays_within_the_frame_budget() {
    let (m0, med0, p0) = trait_mesure(false);
    let (m1, med1, p1) = trait_mesure(true);
    println!("PERF Espace 3D à côté, trait sur une feuille 1920 × 1080 de 10 calques : fermée {m0:.2} ms par image (médiane {med0:.2}, 95 % {p0:.2}) ; ouverte {m1:.2} ms (médiane {med1:.2}, 95 % {p1:.2})");
    // L'image d'une carte : entière contre le morceau d'un bout de trait (une tuile).
    let mut r = Raster::new(1920, 1080);
    fill_rect(&mut r, 100, 100, 1500, 900, RED);
    let t = std::time::Instant::now();
    for _ in 0..10 {
        let _ = render::cel_image(&r, LARGEUR_CARTE);
    }
    let entiere = t.elapsed().as_secs_f64() * 100.0;
    let cliche = Cliche::of(&r);
    fill_rect(&mut r, 700, 500, 720, 520, BLUE);
    let t = std::time::Instant::now();
    for _ in 0..10 {
        if let Changement::Zone(z) = changement(&cliche, &r, LARGEUR_CARTE) {
            let _ = render::cel_image_region(&r, LARGEUR_CARTE, z);
        }
    }
    let morceau = t.elapsed().as_secs_f64() * 100.0;
    println!("PERF Espace 3D à côté : image d'une carte 1920 × 1080 refaite en entier {entiere:.2} ms, un bout de trait {morceau:.3} ms");
    assert!(morceau * 10.0 < entiere, "le morceau coûte bien moins que l'image entière");
    assert!(m1 < 8.0, "{m1:.2} ms par image pendant un trait, vue 3D ouverte");
}
