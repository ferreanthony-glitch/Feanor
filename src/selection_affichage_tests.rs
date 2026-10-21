//! L'affichage de la sélection : le lasso en cours (« s'étire étrangement », Anthony, 2026-10-22) et
//! l'état choisi (boîte d'1 px orange, poignées de 7 px, pivot fin, halo des traits).
//!
//! La cause du lasso qui s'étirait : egui trace une ligne fermée (`Shape::closed_line`) avec un
//! joint « en pointe » sans limite à chaque sommet (`CUT_OFF_SHARP_CORNERS = false` dans
//! `add_line_loop`) ; quand la main fait demi-tour (deux segments presque opposés, ce qui arrive à
//! chaque début de lasso et à chaque retour sur ses pas, sans compter le segment de fermeture qui
//! revient sur le premier), la pointe fait des centaines de pixels. Le lasso est maintenant une
//! ligne ouverte tracée par notre propre ruban (`contours::lasso_shape`), dont les coins sont bornés.

use eframe::egui::epaint::{Mesh, TessellationOptions, Tessellator};
use eframe::egui::{pos2, Color32, Pos2, Rect, Shape, Stroke};

use crate::contours::{lasso_shape, radius_lines_of, Contours, ANCHORS};
use crate::vector::{Affine, Element, VPoint, VectorDrawing};

fn bounds_of(points: &[Pos2]) -> Rect {
    Rect::from_points(points)
}

fn mesh_bounds(shape: &Shape) -> Rect {
    let Shape::Mesh(m) = shape else { panic!("un maillage") };
    Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>())
}

/// Les demi-tours d'une main : vers le bas puis presque en arrière, sur quelques pixels.
fn hairpins() -> Vec<Vec<Pos2>> {
    vec![
        vec![pos2(100.0, 100.0), pos2(100.0, 140.0), pos2(100.4, 101.0)],
        vec![pos2(100.0, 100.0), pos2(100.0, 102.0), pos2(100.1, 100.5)],
        vec![pos2(50.0, 50.0), pos2(80.0, 50.0), pos2(80.0, 90.0), pos2(80.2, 52.0), pos2(51.0, 50.2)],
        vec![pos2(10.0, 10.0), pos2(10.0, 11.6), pos2(10.0, 13.2), pos2(10.05, 11.0)],
    ]
}

#[test]
fn the_old_closed_line_could_spike_far_outside_the_gesture() {
    // La cause du bug : la ligne fermée d'egui (2,5 px) sort de l'étendue du geste.
    let mut worst = 0.0f32;
    for pts in hairpins() {
        let mut tess = Tessellator::new(1.0, TessellationOptions::default(), [1, 1], Vec::new());
        let mut mesh = Mesh::default();
        tess.tessellate_shape(Shape::closed_line(pts.clone(), Stroke::new(2.5_f32, Color32::from_black_alpha(120))), &mut mesh);
        let (got, want) = (Rect::from_points(&mesh.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()), bounds_of(&pts));
        let out = (want.min.x - got.min.x).max(got.max.x - want.max.x).max(want.min.y - got.min.y).max(got.max.y - want.max.y);
        println!("débord de la ligne fermée d'egui : {out:.0} px");
        worst = worst.max(out);
    }
    assert!(worst > 30.0, "la cause du bug n'est plus reproduite ({worst} px) : à revoir");
}

#[test]
fn every_lasso_vertex_is_finite_and_inside_the_gesture() {
    for pts in hairpins() {
        let shape = lasso_shape(&pts).expect("un lasso de plusieurs points se trace");
        let Shape::Mesh(m) = &shape else { panic!() };
        assert!(m.vertices.iter().all(|v| v.pos.is_finite()), "un sommet n'est pas fini : {pts:?}");
        let (got, want) = (mesh_bounds(&shape), bounds_of(&pts).expand(3.0));
        assert!(want.contains_rect(got), "le lasso sort de son geste : {got:?} hors de {want:?} ({pts:?})");
    }
}

#[test]
fn a_lost_point_does_not_stretch_the_lasso() {
    // Un point à zéro ou à l'infini (NaN, pression perdue) ne trace rien.
    let pts = vec![pos2(100.0, 100.0), pos2(110.0, 104.0), pos2(f32::NAN, 5.0), pos2(120.0, f32::INFINITY), pos2(130.0, 110.0), pos2(140.0, 120.0)];
    let shape = lasso_shape(&pts).expect("trace");
    let got = mesh_bounds(&shape);
    assert!(got.is_finite());
    assert!(Rect::from_min_max(pos2(97.0, 97.0), pos2(143.0, 123.0)).contains_rect(got), "{got:?}");
    assert!(lasso_shape(&[pos2(1.0, 1.0), pos2(f32::NAN, 2.0)]).is_none(), "moins de deux points utiles : rien");
    assert!(lasso_shape(&[pos2(1.0, 1.0)]).is_none());
}

#[test]
fn the_lasso_stays_open_and_has_amber_dots() {
    // Un arc presque fermé : aucun segment ne revient au premier point (le jour reste vide).
    let arc: Vec<Pos2> = (0..=90).map(|i| {
        let a = i as f32 / 100.0 * std::f32::consts::TAU;
        pos2(200.0 + 100.0 * a.cos(), 200.0 + 100.0 * a.sin())
    }).collect();
    let shape = lasso_shape(&arc).unwrap();
    let Shape::Mesh(m) = &shape else { panic!() };
    let (first, last) = (arc[0], *arc.last().unwrap());
    let middle = first.lerp(last, 0.5);
    assert!(m.vertices.iter().all(|v| v.pos.distance(middle) > 3.0), "un trait ferme le lasso");
    let amber = crate::theme::LASSO_POINTS;
    let dots = m.vertices.iter().filter(|v| v.color == amber).count() / 4;
    let length: f32 = arc.windows(2).map(|w| w[0].distance(w[1])).sum();
    assert!(dots as f32 > length / 10.0 && (dots as f32) < length / 4.0, "{dots} points pour {length:.0} px");
    assert!(m.vertices.iter().any(|v| v.color == crate::theme::LASSO_TRAIT), "la ligne beige");
}

#[test]
fn a_very_long_lasso_stays_cheap() {
    let pts: Vec<Pos2> = (0..50_000).map(|i| pos2(i as f32 * 0.05, ((i as f32) * 0.01).sin() * 80.0)).collect();
    let start = std::time::Instant::now();
    let shape = lasso_shape(&pts).unwrap();
    let Shape::Mesh(m) = &shape else { panic!() };
    assert!(m.vertices.len() < 40_000, "{} sommets", m.vertices.len());
    assert!(start.elapsed().as_millis() < 20, "{} ms", start.elapsed().as_millis());
}

fn one_stroke(r: f32) -> VectorDrawing {
    VectorDrawing {
        elements: vec![Element::Stroke {
            points: (0..20).map(|i| VPoint { x: 50.0 + i as f32 * 10.0, y: 100.0 + (i as f32 * 0.4).sin() * 20.0, r }).collect(),
            color: [0, 0, 0],
            opacity: 1.0,
            hardness: 1.0,
            brush: None,
            trace: Default::default(),
        }],
    }
}

#[test]
fn the_halo_hugs_the_stroke_edge_and_fades_outward() {
    let d = one_stroke(4.0);
    let lines = radius_lines_of(&d, &[0]);
    assert_eq!(lines.len(), 1);
    let id = |p: Pos2| p;
    let anchors = ANCHORS.map(id);
    let halo = Contours::build_halo(1, &lines, id, 1.0, anchors, crate::theme::SELECTION_BOITE);
    let shape = halo.shape(&Affine::IDENTITY);
    let Shape::Mesh(m) = &shape else { panic!() };
    assert!(m.vertices.iter().all(|v| v.pos.is_finite()));
    // Les sommets pleins sont à moins de r (4) du trait ; les transparents à 4 + 2,8 de lui.
    let centre: Vec<Pos2> = d.elements[0].outline()[0].clone();
    let dist = |p: Pos2| centre.windows(2).map(|s| crate::selbox::segment_distance(p, s[0], s[1])).fold(f32::MAX, f32::min);
    for v in &m.vertices {
        let near = dist(v.pos);
        if v.color == Color32::TRANSPARENT {
            assert!(near > 4.0 + 2.0 && near < 4.0 + 3.6, "bord transparent à {near}");
        } else {
            assert!(near > 3.0 && near < 4.0 + 2.2, "bord plein à {near}");
            assert_eq!(v.color, crate::theme::SELECTION_BOITE);
        }
    }
}

#[test]
fn a_halo_hairpin_does_not_spike() {
    // Un trait qui fait demi-tour sur lui-même : le halo reste près du trait.
    let d = VectorDrawing {
        elements: vec![Element::Stroke {
            points: vec![VPoint { x: 100.0, y: 100.0, r: 3.0 }, VPoint { x: 100.0, y: 160.0, r: 3.0 }, VPoint { x: 100.3, y: 101.0, r: 3.0 }],
            color: [0; 3],
            opacity: 1.0,
            hardness: 1.0,
            brush: None,
            trace: Default::default(),
        }],
    };
    let id = |p: Pos2| p;
    let halo = Contours::build_halo(2, &radius_lines_of(&d, &[0]), id, 1.0, ANCHORS.map(id), crate::theme::SELECTION_BOITE);
    let got = mesh_bounds(&halo.shape(&Affine::IDENTITY));
    // (Le coin s'allonge au plus de deux fois la largeur : 160 + 2 × (3 + 2,8) au bout.)
    assert!(Rect::from_min_max(pos2(90.0, 90.0), pos2(110.0, 175.0)).contains_rect(got), "{got:?}");
}
