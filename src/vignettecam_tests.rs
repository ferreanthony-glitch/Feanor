//! Essais de la logique des mouvements de caméra des Vignettes (`vignettecam.rs`), sans interface.

use eframe::egui::{pos2, vec2};

use super::*;
use crate::camera::Camera;
use crate::document::Document;

fn doc_avec(keys: &[(usize, f32, f32, f32)]) -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.set_duration(0, 25);
    for &(frame, x, y, zoom) in keys {
        let cam = Camera { x, y, zoom, ..Camera::centered(160, 90) };
        crate::camera::set_key(&mut doc.panels[0].camera, frame, cam);
    }
    doc
}

/// Une case de 120 de large (le cadre de 160 fait 0,75 point de la planche par point de la feuille), une rangée de six cases.
fn place() -> Place {
    Place { card_w: 120.0, thumb_h: 67.5, row_w: 6.0 * 120.0 + 5.0 * 30.0, total: 21.0, max_h: 3.0 * (20.0 + 67.5 + 21.0 + 8.0) }
}

fn empreinte_de(keys: &[(usize, f32, f32, f32)]) -> Empreinte {
    empreinte(&doc_avec(keys), 0, &place()).expect("une empreinte")
}

fn quads(e: &Empreinte) -> Vec<[Pos2; 4]> {
    e.cadres.iter().map(|c| c.coins).collect()
}

fn la_case() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 67.5))
}

#[test]
fn a_plan_without_two_keys_has_no_movement_and_no_footprint() {
    let doc = doc_avec(&[]);
    assert!(cadres(&doc, 0).is_empty());
    assert!(empreinte(&doc, 0, &place()).is_none());
    let doc = doc_avec(&[(0, 80.0, 45.0, 1.0)]);
    assert!(empreinte(&doc, 0, &place()).is_none(), "une seule clé : la caméra est fixe");
}

#[test]
fn a_travelling_widens_the_box_just_as_much_as_needed_at_the_scale_of_the_cases() {
    // Une largeur de cadre vers la droite : A puis B côte à côte, deux cases de large (sans écart : même feuille).
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]);
    assert_eq!(e.cadres[0].rect, la_case(), "A est la case, à son échelle");
    assert!((e.cadres[1].rect.left() - 120.0).abs() < 1e-3 && (e.cadres[1].rect.width() - 120.0).abs() < 1e-3, "{:?}", e.cadres[1].rect);
    assert!((e.x_min - 0.0).abs() < 1e-3 && (e.x_max - 240.0).abs() < 1e-3);
    assert_eq!(e.tuile.rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(240.0, 67.5)));
    // Une demi-largeur : une case et demie, pas deux (juste ce qu'il faut).
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 160.0, 45.0, 1.0)]);
    assert!((e.x_max - 180.0).abs() < 1e-3, "{}", e.x_max);
    // Vers la gauche : la case s'élargit à gauche de A.
    let e = empreinte_de(&[(0, 240.0, 45.0, 1.0), (24, 80.0, 45.0, 1.0)]);
    assert!((e.x_min + 120.0).abs() < 1e-3 && (e.x_max - 120.0).abs() < 1e-3, "{} {}", e.x_min, e.x_max);
    // Trois cadres bout à bout : trois cases.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (12, 240.0, 45.0, 1.0), (24, 400.0, 45.0, 1.0)]);
    assert!((e.x_max - 360.0).abs() < 1e-3);
    assert_eq!(e.cadres.len(), 3);
}

#[test]
fn a_zoom_out_surrounds_the_case_and_its_height_is_limited_to_the_row() {
    // Un cadre un peu plus large (zoom 0,9) : B entoure A, sans être réduit.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 0.9)]);
    let b = &e.cadres[1];
    assert!(!b.reduit);
    assert!((b.rect.height() - 67.5 / 0.9).abs() < 1e-2 && b.rect.contains_rect(e.cadres[0].rect), "{:?}", b.rect);
    // Un zoom arrière de moitié : B voudrait faire 135 de haut, la limite est la case et sa bande de note (67,5 + 21).
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 0.5)]);
    let b = &e.cadres[1];
    assert!(b.reduit, "limite de taille");
    assert!((b.rect.height() - 88.5).abs() < 1e-3, "{}", b.rect.height());
    assert!((b.rect.width() / b.rect.height() - 16.0 / 9.0).abs() < 1e-3, "les proportions du cadre restent");
    // La tuile couvre A et B ; la part de la feuille qu'elle montre est à l'échelle des cases.
    let z = e.tuile.zone;
    assert!((z.echelle - 0.75).abs() < 1e-5);
    assert!((z.taille.x - e.tuile.rect.width() / 0.75).abs() < 1e-3);
    assert!((z.centre - pos2(80.0, 45.0)).length() < 1e-3, "centrée sur A comme B : {:?}", z.centre);
    // Un zoom avant : B est dedans A, dessiné dedans.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 2.0)]);
    let b = &e.cadres[1];
    assert!(e.cadres[0].rect.contains_rect(b.rect) && !b.reduit);
    assert!((b.rect.width() - 60.0).abs() < 1e-3);
    assert_eq!(e.tuile.rect, e.cadres[0].rect, "la case ne grandit pas");
}

#[test]
fn b_can_sit_anywhere_relative_to_a_even_half_a_row_down_with_no_rounding() {
    // Un demi-cadre vers le bas (la moitié de la hauteur d'une case) : B à cheval, exactement à mi-hauteur.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 90.0, 1.0)]);
    assert!((e.cadres[1].rect.top() - 33.75).abs() < 1e-3 && (e.cadres[1].rect.left() - 0.0).abs() < 1e-3, "{:?}", e.cadres[1].rect);
    assert!((e.y_max - (33.75 + 67.5)).abs() < 1e-3, "l'empreinte finit à mi-rangée : {}", e.y_max);
    // N'importe quel décalage, dans les deux sens, sans arrondi à une rangée, une colonne ou un huitième de case.
    for (dx, dy) in [(37.3, 11.1), (-53.7, 61.9), (101.0, -29.5), (0.3, 0.7)] {
        let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0 + dx, 45.0 + dy, 1.0)]);
        let b = e.cadres[1].rect;
        assert!((b.left() - dx * 0.75).abs() < 1e-3 && (b.top() - dy * 0.75).abs() < 1e-3, "({dx}, {dy}) : {b:?}");
    }
    // L'enveloppe est vraiment l'union : A, B et ce qu'il y a entre eux.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 135.0, 1.0)]);
    let b = e.cadres[1].rect;
    assert!(e.enveloppe.len() >= 4);
    for q in quads(&e).iter().flatten() {
        assert!(dans_polygone(&e.enveloppe, *q), "tous les coins sont dans l'enveloppe");
    }
    let entre = pos2(100.0, 90.0);
    assert!(dans_polygone(&e.enveloppe, entre), "l'espace entre A et B est dans la zone de dessin");
    assert!(!la_case().contains(entre) && !b.contains(entre));
    assert!(!dans_polygone(&e.enveloppe, pos2(200.0, 10.0)), "le creux hors de l'enveloppe n'en est pas");
}

#[test]
fn a_rotated_camera_is_a_rotated_frame_and_the_envelope_holds_its_corners() {
    let mut doc = doc_avec(&[(0, 80.0, 45.0, 1.0)]);
    let tournee = Camera { x: 240.0, y: 45.0, angle: 30.0, ..Camera::centered(160, 90) };
    crate::camera::set_key(&mut doc.panels[0].camera, 24, tournee);
    let e = empreinte(&doc, 0, &place()).unwrap();
    let c = &e.cadres[1];
    // Quatre coins d'un rectangle tourné de 30° (côtés égaux, angles droits), plus petit que sa boîte.
    let (a, b, d) = (c.coins[0], c.coins[1], c.coins[3]);
    assert!(((b - a).length() - 120.0).abs() < 1e-2 && ((d - a).length() - 67.5).abs() < 1e-2, "{:?}", c.coins);
    assert!((b - a).dot(d - a).abs() < 1e-2, "des angles droits");
    assert!(((b - a).y / (b - a).length() - 0.5).abs() < 1e-3, "tourné de 30°");
    assert!(c.rect.width() > 120.0 + 10.0 && c.rect.height() > 67.5 + 10.0, "la boîte englobe le cadre tourné");
    for q in c.coins {
        assert!(dans_polygone(&e.enveloppe, q));
    }
    // Les liens suivent les coins tournés : des côtés de l'enveloppe entre A et B, jamais dans l'intérieur.
    let v = liens(&quads(&e), la_case());
    assert!(v.iter().any(|l| matches!(l, Lien::Bord(..))));
    // (La silhouette relie les cadres des clés : ses côtés sont ceux de leur enveloppe, que la rotation entre eux dépasse un peu.)
    let cles = enveloppe_convexe(&quads(&e).iter().flatten().copied().collect::<Vec<_>>());
    for l in &v {
        if let Lien::Bord(p, q) = l {
            assert!(distance_bord(&cles, p.lerp(*q, 0.5)) < 1e-2, "{p:?} {q:?}");
        }
    }
}

#[test]
fn the_footprint_is_never_wider_than_a_row_nor_taller_than_three() {
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 5000.0, 45.0, 1.0)]);
    assert!(e.x_max - e.x_min <= place().row_w + 1e-3, "{} {}", e.x_min, e.x_max);
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 5000.0, 1.0)]);
    assert!(e.y_max - e.y_min <= place().max_h + 1e-3, "{} {}", e.y_min, e.y_max);
    let etroit = Place { row_w: 120.0, ..place() };
    let e = empreinte(&doc_avec(&[(0, 80.0, 45.0, 1.0), (24, 400.0, 45.0, 0.5)]), 0, &etroit).unwrap();
    assert!(e.x_max - e.x_min <= 120.0 + 1e-3, "une colonne : pas de travelling");
    assert!(e.cadres.iter().all(|c| c.rect.width() <= 120.0 + 1e-3));
}

#[test]
fn the_links_are_exactly_the_hull_edges_between_two_frames_and_never_an_inner_line() {
    let bords = |keys: &[(usize, f32, f32, f32)]| {
        let e = empreinte_de(keys);
        let v = liens(&quads(&e), la_case());
        // Chaque lien d'un bord est un côté de l'enveloppe : ses deux bouts sont sur son bord, et son milieu aussi.
        for l in &v {
            if let Lien::Bord(p, q) = l {
                for pt in [*p, *q, p.lerp(*q, 0.5)] {
                    assert!(distance_bord(&e.enveloppe, pt) < 1e-2, "{p:?}–{q:?} passe dans l'intérieur");
                }
            }
        }
        (e, v.iter().filter_map(|l| if let Lien::Bord(p, q) = l { Some((*p, *q)) } else { None }).collect::<Vec<_>>())
    };
    // Une diagonale (B en bas à droite) : le haut-droit de A au haut-droit de B, le bas-gauche de A au bas-gauche de B. Les deux
    // diagonales qui traversent A et B (haut-gauche à haut-gauche, bas-droit à bas-droit) ont disparu.
    let (e, b) = bords(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 135.0, 1.0)]);
    let (a_, b_) = (e.cadres[0].coins, e.cadres[1].coins);
    assert_eq!(b.len(), 2, "{b:?}");
    let a_ce = |p: Pos2, q: Pos2, x: Pos2, y: Pos2| (p.distance(x) < 1e-2 && q.distance(y) < 1e-2) || (p.distance(y) < 1e-2 && q.distance(x) < 1e-2);
    assert!(b.iter().any(|&(p, q)| a_ce(p, q, a_[1], b_[1])), "haut-droit à haut-droit");
    assert!(b.iter().any(|&(p, q)| a_ce(p, q, a_[3], b_[3])), "bas-gauche à bas-gauche");
    assert!(!b.iter().any(|&(p, q)| a_ce(p, q, a_[0], b_[0]) || a_ce(p, q, a_[2], b_[2])), "pas de ligne dans l'intérieur");
    // Des cadres qui se confondent : rien à relier.
    let (_, b) = bords(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 1.0)]);
    assert!(b.is_empty());
    // Un zoom arrière qui entoure A : A est dans B, aucune ligne ne traverse l'espace entre les deux.
    let (_, b) = bords(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 0.9)]);
    assert!(b.is_empty(), "{b:?}");
    // Trois cadres : seulement des côtés de l'enveloppe entre deux cadres différents.
    let (_, b) = bords(&[(0, 80.0, 45.0, 1.0), (12, 200.0, 105.0, 1.0), (24, 80.0, 165.0, 1.0)]);
    assert!(!b.is_empty());
}

fn fleches(keys: &[(usize, f32, f32, f32)]) -> (Empreinte, Vec<(Pos2, Pos2)>) {
    let e = empreinte_de(keys);
    let f = liens(&quads(&e), la_case()).into_iter().filter_map(|l| if let Lien::Fleche { de, vers } = l { Some((de, vers)) } else { None }).collect();
    (e, f)
}

#[test]
fn a_thin_arrow_follows_the_movement_from_a_to_b_then_b_to_c() {
    // Un travelling : une flèche de A vers B, entre les deux centres, raccourcie à chaque bout.
    let (e, t) = fleches(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]);
    assert_eq!(t.len(), 1, "{t:?}");
    let (a, b) = (e.cadres[0].rect, e.cadres[1].rect);
    assert!(t[0].0.x < t[0].1.x && a.contains(t[0].0) && b.contains(t[0].1), "de A vers B : {t:?}");
    assert!((t[0].0.y - a.center().y).abs() < 1e-3 && (t[0].1.y - b.center().y).abs() < 1e-3);
    assert!(t[0].0.distance(t[0].1) < a.center().distance(b.center()), "raccourcie");
    // Un panoramique vertical, une diagonale, un zoom qui se déplace : la flèche suit le mouvement, dans le bon sens.
    let (_, p) = fleches(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 165.0, 1.0)]);
    assert!(p.len() == 1 && p[0].0.y < p[0].1.y && (p[0].0.x - p[0].1.x).abs() < 1e-3, "{p:?}");
    let (_, d) = fleches(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 135.0, 1.0)]);
    assert!(d.len() == 1 && d[0].0.x < d[0].1.x && d[0].0.y < d[0].1.y, "{d:?}");
    let (_, z) = fleches(&[(0, 240.0, 135.0, 1.0), (24, 80.0, 45.0, 2.0)]);
    assert!(z.len() == 1 && z[0].0.x > z[0].1.x, "{z:?}");
    // A vers B puis B vers C : deux flèches bout à bout, la deuxième part de B.
    let (e, c) = fleches(&[(0, 80.0, 45.0, 1.0), (12, 240.0, 45.0, 1.0), (24, 240.0, 165.0, 1.0)]);
    assert_eq!(c.len(), 2, "{c:?}");
    assert!(e.cadres[1].rect.contains(c[0].1) && e.cadres[1].rect.contains(c[1].0), "B est le bout de l'une et le départ de l'autre");
    assert!(c[1].0.y < c[1].1.y, "B vers C descend");
    // Rien à suivre : un zoom sur place, des cadres qui se confondent.
    assert!(fleches(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 0.9)]).1.is_empty());
    assert!(fleches(&[(0, 80.0, 45.0, 1.0), (24, 80.0, 45.0, 1.0)]).1.is_empty());
    // Discrète : une fine ligne et un petit chevron, jamais de pointe pleine.
    let u = vec2(1.0, 0.0);
    let [h1, h2] = chevron(pos2(100.0, 50.0), u, 5.0);
    assert!(h1.x < 100.0 && h2.x < 100.0 && h1.y != h2.y && (h1 - pos2(100.0, 50.0)).length() <= 5.001, "{h1:?} {h2:?}");
    assert!(FLECHE_ALPHA < 200, "pas d'opacité forte");
}

// ------------------------------------------------------------------------------------ ce que la caméra balaie

/// Les cadres que la caméra montre vraiment, à très petits pas entre deux clés, posés sur la planche comme `empreinte` le fait (sans
/// resserrement, cadres de taille normale : `g` et `f` valent 1 dans ces essais) : leurs polygones.
fn cadres_reels(doc: &Document, e: &Empreinte) -> Vec<Vec<Pos2>> {
    let a = &cadres(doc, 0)[0];
    let echelle = place().card_w / taille_cadre(doc, &a.cam).x;
    let (ra, centre_thumb) = (boite(&a.coins).center(), la_case().center());
    let keys = &doc.panels[0].camera;
    let mut out = Vec::new();
    for w in keys.windows(2) {
        for k in 0..=600 {
            let cam = Camera::lerp(&w[0].cam, &w[1].cam, k as f32 / 600.0);
            out.push(cam.corners(doc.frame).map(|c| centre_thumb + (c - ra) * echelle).to_vec());
        }
    }
    assert!(e.cadres.iter().all(|c| !c.reduit), "essai sans cadre réduit");
    out
}

fn grille(r: Rect, pas: f32) -> impl Iterator<Item = Pos2> {
    let (nx, ny) = ((r.width() / pas) as usize + 1, (r.height() / pas) as usize + 1);
    (0..ny).flat_map(move |j| (0..nx).map(move |i| pos2(r.left() + i as f32 * pas, r.top() + j as f32 * pas)))
}

fn dans_balayage(e: &Empreinte, p: Pos2) -> bool {
    e.balayage.iter().any(|q| dans_polygone(q, p)) || la_case().contains(p)
}

#[test]
fn the_swept_zone_holds_every_frame_the_camera_really_shows_with_a_rotation_and_a_zoom() {
    let mut doc = doc_avec(&[(0, 80.0, 45.0, 1.0)]);
    let b = Camera { x: 260.0, y: 100.0, zoom: 1.5, angle: 70.0, ..Camera::centered(160, 90) };
    crate::camera::set_key(&mut doc.panels[0].camera, 24, b);
    let e = empreinte(&doc, 0, &place()).expect("une empreinte");
    let reels = cadres_reels(&doc, &e);
    // L'enveloppe de deux cadres ne contient pas tout ce que balaie la rotation (elle sort de l'enveloppe des clés) : le balayage, si.
    let hull_cles = enveloppe_convexe(&quads(&e).iter().flatten().copied().collect::<Vec<_>>());
    let dehors = reels.iter().flatten().filter(|p| !dans_polygone(&hull_cles, **p)).count();
    assert!(dehors > 0, "la rotation sort de l'enveloppe des deux cadres");
    // Pixel par pixel (pas d'un point de la planche) : tout point d'un vrai cadre est dans l'ouverture, à un point près du bord.
    let (mut vrais, mut manques, mut trop) = (0, 0, 0);
    for p in grille(e.tuile.rect, 1.0) {
        let vrai = reels.iter().any(|q| dans_polygone(q, p));
        let ouvert = dans_balayage(&e, p);
        vrais += vrai as usize;
        if vrai && !ouvert && e.balayage.iter().map(|q| distance_bord(q, p)).fold(f32::MAX, f32::min) > 1.0 {
            manques += 1;
        }
        // Hors de l'enveloppe : jamais ouvert.
        if ouvert && !dans_polygone(&e.enveloppe, p) && !la_case().contains(p) {
            trop += 1;
        }
    }
    assert!(vrais > 2000, "{vrais}");
    assert_eq!(manques, 0, "des points vus par la caméra sont masqués");
    assert_eq!(trop, 0, "l'ouverture sort de l'enveloppe");
    // Et elle n'est pas gloutonne : elle ne dépasse pas de beaucoup ce que la caméra voit.
    let ouvert = grille(e.tuile.rect, 1.0).filter(|p| dans_balayage(&e, *p)).count();
    assert!((ouvert as f32) < vrais as f32 * 1.25, "ouverture {ouvert} points pour {vrais} points vus");
    // Les cadres A et B (les clés) sont ouverts en entier, au centre.
    for c in &e.cadres {
        assert!(dans_balayage(&e, c.rect.center()), "le cadre {} est ouvert", c.rang);
    }
}

#[test]
fn a_travelling_opens_exactly_the_hull_of_its_two_frames_in_one_piece() {
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 135.0, 1.0)]);
    assert_eq!(e.balayage.len(), 1, "une seule pièce entre deux clés qui glissent");
    let coque = enveloppe_convexe(&quads(&e).iter().flatten().copied().collect::<Vec<_>>());
    assert!((tour(&e.balayage[0]).abs() - tour(&coque).abs()).abs() < 1e-2, "la pièce est l'enveloppe des deux cadres");
    // Et pas le creux : hors de la silhouette A–B, rien.
    assert!(!dans_balayage(&e, pos2(200.0, 10.0)) && dans_balayage(&e, pos2(100.0, 90.0)));
}

#[test]
fn a_path_through_several_keys_does_not_open_the_corner_it_never_shows() {
    // A, puis B à droite, puis C en dessous de B : le coin bas-gauche entre A et C n'est jamais filmé.
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (12, 400.0, 45.0, 1.0), (24, 400.0, 245.0, 1.0)]);
    assert_eq!(e.balayage.len(), 2, "une pièce par intervalle de clés");
    let coin = pos2(150.0, 100.0);
    assert!(dans_polygone(&e.enveloppe, coin), "dans l'enveloppe convexe de tous les cadres");
    assert!(!dans_balayage(&e, coin), "mais pas dans ce que la caméra balaie");
    // Chaque cadre et l'entre-deux d'un tronçon sont ouverts.
    for c in &e.cadres {
        assert!(dans_balayage(&e, c.rect.center()));
    }
    assert!(dans_balayage(&e, pos2(240.0, 33.0)) && dans_balayage(&e, pos2(300.0, 100.0)));
}

#[test]
fn a_plan_without_movement_has_nothing_swept() {
    assert!(empreinte(&doc_avec(&[(0, 80.0, 45.0, 1.0)]), 0, &place()).is_none());
}
#[test]
fn the_zone_of_the_tile_maps_the_sheet_onto_the_board_at_the_scale_of_the_cases() {
    let e = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]);
    let z = e.tuile.zone;
    // La tuile, posée sur la planche à (10, 20), montre la feuille : le point central du cadre A est au centre de A.
    let tuile = Rect::from_min_size(pos2(10.0, 20.0), e.tuile.rect.size());
    let vue = z.vue(tuile, vec2(160.0, 90.0));
    assert!((vue.zoom - 0.75).abs() < 1e-5, "{}", vue.zoom);
    assert_eq!(z.centre, pos2(160.0, 45.0));
    // Un point du dessin de la feuille entre A et B tombe où la tuile le montre : l'enveloppe en coordonnées de la feuille.
    assert_eq!(e.tuile.masque.len(), e.enveloppe.len());
    assert!(dans_polygone(&e.tuile.masque, pos2(0.0, 0.0)) && dans_polygone(&e.tuile.masque, pos2(320.0, 90.0)));
    assert!(!dans_polygone(&e.tuile.masque, pos2(-5.0, 0.0)));
    // Le dessin vit sur la feuille : déplacer B ne change pas ce que montre la tuile en un point de la feuille entre A et B.
    let e2 = empreinte_de(&[(0, 80.0, 45.0, 1.0), (24, 240.0, 75.0, 1.0)]);
    assert!(dans_polygone(&e.tuile.masque, pos2(160.0, 45.0)) && dans_polygone(&e2.tuile.masque, pos2(160.0, 45.0)));
}

#[test]
fn polygon_helpers_hull_inside_overlap_and_span() {
    let pts = [pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(5.0, 0.0), pos2(10.0, 10.0), pos2(0.0, 10.0), pos2(5.0, 5.0)];
    let h = enveloppe_convexe(&pts);
    assert_eq!(h.len(), 4, "les points alignés et ceux de l'intérieur s'en vont : {h:?}");
    assert!(dans_polygone(&h, pos2(5.0, 5.0)) && !dans_polygone(&h, pos2(11.0, 5.0)));
    let autre = [pos2(8.0, 8.0), pos2(20.0, 8.0), pos2(20.0, 20.0), pos2(8.0, 20.0)];
    assert!(se_recouvrent(&h, &autre, 0.1));
    let loin = [pos2(10.0, 0.0), pos2(20.0, 0.0), pos2(20.0, 10.0), pos2(10.0, 10.0)];
    assert!(!se_recouvrent(&h, &loin, 0.1), "qui se touchent seulement");
    assert_eq!(intervalle_a(&h, 5.0), Some((0.0, 10.0)));
    assert_eq!(intervalle_a(&h, 11.0), None);
}

#[test]
fn the_magnet_acts_only_very_close_to_an_alignment_and_never_jumps() {
    let grille = Grille { xs: vec![100.0], ys: vec![50.0] };
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 30.0));
    let seuil = 6.0;
    // Loin d'un alignement : le cadre suit exactement, aucun arrondi.
    for d in [vec2(13.7, 4.3), vec2(31.1, -9.2), vec2(0.0, 0.0), vec2(50.0, 20.0)] {
        // (Le bord droit est à 40 + dx : il faut dx à plus de 6 de 60, ni le bord gauche à 100, ni le centre à 80…)
        let r = rect.translate(d);
        let loin = [r.left(), r.center().x, r.right()].iter().all(|x| (x - 100.0).abs() > seuil) && [r.top(), r.center().y, r.bottom()].iter().all(|y| (y - 50.0).abs() > seuil);
        if loin {
            assert_eq!(aimante(rect, d, &grille, seuil), d, "{d:?}");
        }
    }
    // Tout près : le bord gauche à 2 de la colonne x = 100 est ramené vers elle, mais pas au-delà de ce qui rapproche.
    let d = vec2(98.0, 2.0);
    let a = aimante(rect, d, &grille, seuil);
    assert!(a.x > d.x && a.x <= 100.0, "attiré vers la colonne : {a:?}");
    assert_eq!(a.y, d.y, "l'axe y est libre : les bords sont loin de 50");
    assert!((rect.translate(a).left() - 100.0).abs() < (rect.translate(d).left() - 100.0).abs());
    // Pile dessus : exactement dessus. Au bord du seuil : le pointeur, sans saut.
    assert_eq!(aimante(rect, vec2(100.0, 2.0), &grille, seuil).x, 100.0);
    let bord = aimante(rect, vec2(94.0 + 1e-3, 2.0), &grille, seuil).x;
    assert!((bord - (94.0 + 1e-3)).abs() < 0.01, "pas de saut en entrant dans l'aimant : {bord}");
    // La correction ne dépasse jamais le seuil, et croît sans à-coup quand on s'approche.
    let mut dernier = None::<f32>;
    for k in 0..=120 {
        let x = 90.0 + k as f32 * 0.1;
        let sortie = aimante(rect, vec2(x, 2.0), &grille, seuil).x;
        assert!((sortie - x).abs() <= seuil + 1e-4, "{x} → {sortie}");
        if let Some(p) = dernier {
            assert!((sortie - p).abs() < 0.5, "un saut en {x} : {p} → {sortie}");
        }
        dernier = Some(sortie);
    }
}

#[test]
fn keys_are_spread_evenly_over_the_plan() {
    assert_eq!(repartition(2, 25), vec![0, 24]);
    assert_eq!(repartition(3, 25), vec![0, 12, 24]);
    assert_eq!(repartition(4, 25), vec![0, 8, 16, 24]);
    let doc = doc_avec(&[(0, 80.0, 45.0, 1.0), (12, 160.0, 45.0, 1.0), (24, 240.0, 45.0, 1.0)]);
    assert!(est_repartie(&doc.panels[0].camera, 25));
    assert!(!est_repartie(&doc.panels[0].camera, 30));
}

#[test]
fn a_new_frame_makes_a_at_the_start_and_b_at_the_end_then_spreads_c() {
    let a = Camera::centered(160, 90);
    let b = a.shifted(160.0, 0.0);
    let keys = avec_nouveau_cadre(&[], a, b, 25).unwrap();
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 24]);
    assert_eq!((keys[0].cam, keys[1].cam), (a, b));
    assert!(!keys[1].smooth && keys[1].ease_in == 0.0, "vitesse constante, comme au montage");
    let c = b.shifted(160.0, 0.0);
    let keys = avec_nouveau_cadre(&keys, a, c, 25).unwrap();
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 12, 24]);
    assert_eq!((keys[0].cam, keys[1].cam, keys[2].cam), (a, b, c));
    let d = c.shifted(0.0, 90.0);
    let keys = avec_nouveau_cadre(&keys, a, d, 25).unwrap();
    assert_eq!(keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 8, 16, 24]);
    assert_eq!(keys[3].cam, d);
}

#[test]
fn keys_placed_by_hand_are_respected() {
    let a = Camera::centered(160, 90);
    // A à 0, B à 5 (loin de la fin) : C va à la dernière image, rien d'autre ne bouge.
    let k = vec![CameraKey::new(0, a), CameraKey::new(5, a.shifted(50.0, 0.0))];
    let out = avec_nouveau_cadre(&k, a, a.shifted(100.0, 0.0), 25).unwrap();
    assert_eq!(out.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 5, 24]);
    // Des clés à la main dont la dernière est à la fin : elle recule à mi-chemin, la nouvelle prend la fin.
    let k = vec![CameraKey::new(0, a), CameraKey::new(6, a.shifted(50.0, 0.0)), CameraKey::new(24, a.shifted(80.0, 0.0))];
    let out = avec_nouveau_cadre(&k, a, a.shifted(100.0, 0.0), 25).unwrap();
    assert_eq!(out.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 6, 15, 24]);
    // Un plan d'une image ne porte pas de mouvement.
    assert!(avec_nouveau_cadre(&[], a, a, 1).is_none());
}

#[test]
fn removing_a_frame_respreads_the_keys_and_removing_b_of_two_removes_the_move() {
    let a = Camera::centered(160, 90);
    let deux = vec![CameraKey::new(0, a), CameraKey::new(24, a.shifted(160.0, 0.0))];
    let out = sans_le_cadre(&deux, 1, 25).unwrap();
    assert_eq!(out.len(), 1, "il ne reste que A : plus de mouvement");
    assert_eq!(out[0].cam, a);
    let trois = vec![CameraKey::new(0, a), CameraKey::new(12, a.shifted(160.0, 0.0)), CameraKey::new(24, a.shifted(320.0, 0.0))];
    let out = sans_le_cadre(&trois, 1, 25).unwrap();
    assert_eq!(out.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 24], "les clés restantes se répartissent");
    assert_eq!(out[1].cam, trois[2].cam);
    let out = sans_le_cadre(&trois, 0, 25).unwrap();
    assert_eq!(out[0].cam, trois[1].cam, "retirer A : B devient le début");
    // Des clés posées à la main restent où elles sont.
    let main = vec![CameraKey::new(0, a), CameraKey::new(5, a.shifted(50.0, 0.0)), CameraKey::new(24, a.shifted(80.0, 0.0))];
    assert_eq!(sans_le_cadre(&main, 1, 25).unwrap().iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 24]);
    assert!(sans_le_cadre(&main, 3, 25).is_none());
}

#[test]
fn the_added_frame_sits_half_a_frame_beside_the_last() {
    let a = Camera::centered(160, 90);
    let t = Vec2::new(160.0, 90.0);
    let c = cadre_suivant([160, 90], a.shifted(0.0, 0.0), t);
    assert!(c.x > a.x && c.y == a.y && c.zoom == a.zoom, "{c:?}");
}

fn carre(x: f32, y: f32, w: f32, h: f32) -> [Pos2; 4] {
    [pos2(x, y), pos2(x + w, y), pos2(x + w, y + h), pos2(x, y + h)]
}

#[test]
fn the_grab_on_a_plain_case_makes_b_moved_or_zoomed_from_a_corner_wherever_it_starts() {
    let un = [carre(0.0, 0.0, 100.0, 60.0)];
    assert_eq!(choisit(&un, true, pos2(50.0, 30.0), 6.0, false).map(|c| c.prise), Some(Prise::Nouveau));
    assert_eq!(choisit(&un, true, pos2(98.0, 2.0), 6.0, false), Some(Choix { prise: Prise::NouveauZoom, rang: 0, coin: 1 }));
    // (Même hors de la case : sa marge, son empreinte — un appui de caméra prend toujours quelque chose.)
    assert_eq!(choisit(&un, true, pos2(150.0, 30.0), 6.0, false).map(|c| c.prise), Some(Prise::Nouveau));
    assert_eq!(choisit(&[], true, pos2(0.0, 0.0), 6.0, false), None);
}

#[test]
fn a_grab_inside_an_existing_frame_moves_that_frame_and_the_smallest_one_wins() {
    // B décalé à droite, recouvrant la moitié de A.
    let deux = [carre(0.0, 0.0, 100.0, 60.0), carre(60.0, 0.0, 100.0, 60.0)];
    assert_eq!(choisit(&deux, false, pos2(130.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(1)), "dedans B : on déplace B, rien ne naît");
    assert_eq!(choisit(&deux, false, pos2(30.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(0)), "dedans A, hors de B : on déplace A");
    // B dans A : dedans B on déplace B, dans A hors de B on déplace A.
    let dedans = [carre(0.0, 0.0, 100.0, 60.0), carre(30.0, 15.0, 40.0, 30.0)];
    assert_eq!(choisit(&dedans, false, pos2(50.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(1)));
    assert_eq!(choisit(&dedans, false, pos2(10.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(0)));
    // Un coin de n'importe quel cadre change son zoom.
    assert_eq!(choisit(&deux, false, pos2(160.0, 60.0), 4.0, false), Some(Choix { prise: Prise::Zoom(1), rang: 1, coin: 2 }), "coin de B");
    assert_eq!(choisit(&deux, false, pos2(0.0, 0.0), 4.0, false), Some(Choix { prise: Prise::Zoom(0), rang: 0, coin: 0 }), "coin de A");
    // Hors de tous les cadres : le plus proche, jamais rien.
    assert_eq!(choisit(&deux, false, pos2(200.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(1)));
    assert_eq!(choisit(&deux, false, pos2(-30.0, 30.0), 4.0, false).map(|c| c.prise), Some(Prise::Deplace(0)));
}

#[test]
fn inside_b_or_at_its_corners_b_wins_over_the_corners_of_a() {
    // B recouvre le coin bas-droit de A (60..160 contre 0..100 : le coin (100, 60) de A est dans B, près du coin haut-gauche de B).
    let deux = [carre(0.0, 0.0, 100.0, 60.0), carre(60.0, 20.0, 100.0, 60.0)];
    // Le centre de B est le coin bas-droit de A (même taille de cadre) : on déplace B.
    let centre_b = pos2(110.0, 50.0);
    assert_eq!(choisit(&deux, false, centre_b, 6.0, false).map(|c| c.prise), Some(Prise::Deplace(1)));
    // Pile sur le coin bas-droit de A, qui est dans B : B quand même.
    assert_eq!(choisit(&deux, false, pos2(100.0, 60.0), 6.0, false).map(|c| c.prise), Some(Prise::Deplace(1)), "un coin de A dans B : B");
    // Un coin de B à portée prend le zoom de B, même si un coin de A est tout près aussi.
    assert_eq!(choisit(&deux, false, pos2(62.0, 22.0), 6.0, false), Some(Choix { prise: Prise::Zoom(1), rang: 1, coin: 0 }));
    // Dans A hors de B, près d'un coin de A : A.
    assert_eq!(choisit(&deux, false, pos2(2.0, 2.0), 6.0, false), Some(Choix { prise: Prise::Zoom(0), rang: 0, coin: 0 }));
    // À égalité d'aire et de portée (le coin bas-droit de A au centre d'un B identique) : le dernier cadre.
    let memes = [carre(0.0, 0.0, 100.0, 60.0), carre(50.0, 30.0, 100.0, 60.0)];
    assert_eq!(choisit(&memes, false, pos2(100.0, 60.0), 6.0, false).map(|c| c.prise), Some(Prise::Deplace(1)));
}

#[test]
fn alt_asks_for_the_next_frame_from_the_last_one() {
    let deux = [carre(0.0, 0.0, 100.0, 60.0), carre(60.0, 0.0, 100.0, 60.0)];
    // Où que l'appui tombe, avec Alt : un nouveau cadre déplacé depuis le dernier, ou zoomé à son coin.
    assert_eq!(choisit(&deux, false, pos2(30.0, 30.0), 4.0, true), Some(Choix { prise: Prise::Nouveau, rang: 1, coin: 0 }));
    assert_eq!(choisit(&deux, false, pos2(130.0, 30.0), 4.0, true).map(|c| c.prise), Some(Prise::Nouveau));
    assert_eq!(choisit(&deux, false, pos2(160.0, 60.0), 4.0, true), Some(Choix { prise: Prise::NouveauZoom, rang: 1, coin: 2 }));
}

#[test]
fn the_frame_under_a_right_click_is_the_smallest_that_contains_the_point() {
    let dedans = [carre(0.0, 0.0, 100.0, 60.0), carre(30.0, 15.0, 40.0, 30.0)];
    assert_eq!(cadre_sous(&dedans, pos2(50.0, 30.0)), Some(1));
    assert_eq!(cadre_sous(&dedans, pos2(10.0, 30.0)), Some(0));
    assert_eq!(cadre_sous(&dedans, pos2(150.0, 30.0)), None);
}

#[test]
fn the_geste_moves_and_zooms_from_the_keys_before_it_and_never_accumulates() {
    let frame = [160, 90];
    let a = Camera::centered(160, 90);
    let k0 = vec![CameraKey::new(0, a), CameraKey::new(24, a.shifted(160.0, 0.0))];
    let centre = centre_de(frame, &k0[1].cam);
    let coin = coin_de(frame, &k0[1].cam, 1);
    for _ in 0..3 {
        let out = applique(frame, &k0, a, Prise::Deplace(1), vec2(10.0, 5.0), coin, centre, 25).unwrap();
        assert_eq!((out[1].cam.x, out[1].cam.y), (250.0, 50.0));
        assert_eq!(out[0].cam, a);
    }
    // Déplacer A (le cadre du début) : B ne bouge pas.
    let out = applique(frame, &k0, a, Prise::Deplace(0), vec2(10.0, 5.0), coin, centre, 25).unwrap();
    assert_eq!((out[0].cam.x, out[0].cam.y), (90.0, 50.0));
    assert_eq!(out[1].cam, k0[1].cam);
    assert_eq!(out.len(), 2, "rien ne naît");
    // Tirer le coin vers l'extérieur élargit (zoom plus petit).
    let out = applique(frame, &k0, a, Prise::Zoom(1), (coin - centre).normalized() * 40.0, coin, centre, 25).unwrap();
    assert!(out[1].cam.zoom < 1.0, "{}", out[1].cam.zoom);
    // Un nouveau cadre depuis le dernier.
    let out = applique(frame, &k0, a, Prise::Nouveau, vec2(160.0, 0.0), coin, centre, 25).unwrap();
    assert_eq!(out.len(), 3);
    assert_eq!(out[2].cam.x, 400.0);
}

#[test]
fn step_names_follow_the_gesture() {
    assert_eq!(nom_etape(Prise::Nouveau, 0), "Mouvement de caméra");
    assert_eq!(nom_etape(Prise::NouveauZoom, 0), "Mouvement de caméra");
    assert_eq!(nom_etape(Prise::Nouveau, 2), "Cadre C");
    assert_eq!(nom_etape(Prise::Nouveau, 3), "Cadre D");
    assert_eq!(nom_etape(Prise::Deplace(1), 2), "Cadre B");
    assert_eq!(nom_etape(Prise::Zoom(1), 2), "Zoom du cadre B");
    assert_eq!(lettre(0), 'A');
}
