//! Contours de la sélection (`contours.rs`) : des centaines de traits choisis ne ralentissent plus
//! l'affichage (Anthony, 2026-10-18), et le contour reste lisible : même ligne d'1 px, à la
//! résolution de l'écran.

use eframe::egui::epaint::{Mesh, TessellationOptions, Tessellator};
use eframe::egui::{pos2, vec2, Color32, Pos2, Shape, Stroke};

use crate::affichage_tests::griffonnage;
use crate::contours::{fingerprint, lines_of, simplify, Contours, ANCHORS, MAX_POINTS};
use crate::vector::{Affine, VectorDrawing};

const BLUE: Color32 = Color32::from_rgb(60, 140, 217);

fn identity_view(p: Pos2) -> Pos2 {
    p
}

fn anchors(f: impl Fn(Pos2) -> Pos2) -> [Pos2; 4] {
    ANCHORS.map(f)
}

/// Distance d'un point à une ligne brisée.
fn distance_to_line(p: Pos2, line: &[Pos2]) -> f32 {
    line.windows(2)
        .map(|s| {
            let ab = s[1] - s[0];
            let t = if ab.length_sq() > 0.0 { ((p - s[0]).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
            (s[0] + ab * t).distance(p)
        })
        .fold(f32::MAX, f32::min)
}

/// « Encre » d'un maillage : somme des surfaces des triangles, pondérées par l'opacité moyenne de
/// leurs sommets.
fn encre(mesh: &Mesh) -> f32 {
    mesh.indices
        .chunks_exact(3)
        .map(|t| {
            let v = [mesh.vertices[t[0] as usize], mesh.vertices[t[1] as usize], mesh.vertices[t[2] as usize]];
            let area = ((v[1].pos - v[0].pos).x * (v[2].pos - v[0].pos).y - (v[2].pos - v[0].pos).x * (v[1].pos - v[0].pos).y).abs() / 2.0;
            area * v.iter().map(|x| x.color.a() as f32 / 255.0).sum::<f32>() / 3.0
        })
        .sum()
}

fn mesh_of(shape: &Shape) -> &Mesh {
    match shape {
        Shape::Mesh(m) => m,
        _ => panic!("un maillage"),
    }
}

#[test]
fn simplifying_keeps_the_shape_within_the_tolerance() {
    let curve: Vec<Pos2> = (0..2000).map(|i| pos2(i as f32 * 0.5, (i as f32 * 0.02).sin() * 60.0 + (i as f32 * 0.7).sin() * 0.2)).collect();
    for tol in [0.45f32, 1.5, 6.0] {
        let s = simplify(&curve, tol);
        assert_eq!((s[0], *s.last().unwrap()), (curve[0], *curve.last().unwrap()), "les extrémités restent");
        assert!(s.len() < curve.len() / 3, "tolérance {tol} : {} points gardés sur {}", s.len(), curve.len());
        let worst = curve.iter().map(|&p| distance_to_line(p, &s)).fold(0.0, f32::max);
        // (La distance se mesure à la droite des extrémités, au segment pour nous : un peu de marge.)
        assert!(worst <= tol * 1.2, "tolérance {tol} : un point s'écarte de {worst}");
    }
    // Une droite : deux points. Un point seul ou deux : tels quels.
    let line: Vec<Pos2> = (0..100).map(|i| pos2(i as f32, i as f32 * 0.5)).collect();
    assert_eq!(simplify(&line, 0.45).len(), 2);
    assert_eq!(simplify(&line[..2], 0.45).len(), 2);
    // Une boucle fermée (début = fin) garde sa forme.
    let ring: Vec<Pos2> = (0..=360).map(|i| pos2(100.0 + 80.0 * (i as f32).to_radians().cos(), 100.0 + 80.0 * (i as f32).to_radians().sin())).collect();
    let s = simplify(&ring, 0.45);
    assert!(s.len() > 20 && s.len() < ring.len());
    assert!(ring.iter().all(|&p| distance_to_line(p, &s) <= 0.55));
}

#[test]
fn the_ribbon_is_a_valid_mesh_with_the_line_through_its_middle_vertices() {
    let line = vec![pos2(10.0, 10.0), pos2(60.0, 12.0), pos2(90.0, 50.0), pos2(40.0, 80.0)];
    let c = Contours::build(1, &[line.clone()], identity_view, anchors(identity_view), BLUE);
    let shape = c.shape(&Affine::IDENTITY);
    let mesh = mesh_of(&shape);
    assert!(mesh.is_valid());
    assert_eq!(mesh.vertices.len(), 3 * line.len(), "trois sommets par point");
    assert_eq!(mesh.indices.len(), 12 * (line.len() - 1));
    for (i, p) in line.iter().enumerate() {
        let mid = mesh.vertices[3 * i + 1];
        assert_eq!((mid.pos, mid.color), (*p, BLUE), "le milieu est sur le point");
        assert_eq!(mesh.vertices[3 * i].color, Color32::TRANSPARENT);
        let edge = mesh.vertices[3 * i].pos.distance(*p);
        assert!((0.99..=2.01).contains(&edge), "bord à 1 px (jusqu'à 2 px aux coins vifs) : {edge}");
    }
    // Lignes dégénérées : rien, sans panique.
    let none = Contours::build(2, &[vec![pos2(5.0, 5.0)], vec![pos2(5.0, 5.0), pos2(5.0, 5.0)]], identity_view, anchors(identity_view), BLUE);
    assert_eq!(none.vertex_count(), 0);
}

#[test]
fn the_contour_has_about_the_ink_of_the_line_egui_used_to_draw() {
    let line: Vec<Pos2> = (0..400).map(|i| pos2(20.0 + i as f32 * 1.5, 100.0 + (i as f32 * 0.05).sin() * 50.0)).collect();
    let c = Contours::build(1, &[line.clone()], identity_view, anchors(identity_view), BLUE);
    let ours = encre(mesh_of(&c.shape(&Affine::IDENTITY)));
    let mut tess = Tessellator::new(1.0, TessellationOptions::default(), [1, 1], Vec::new());
    let mut old = Mesh::default();
    tess.tessellate_shape(Shape::line(line, Stroke::new(1.0_f32, BLUE)), &mut old);
    let before = encre(&old);
    println!("encre : {ours:.1} contre {before:.1} pour le tracé d'egui");
    assert!(ours > before * 0.7 && ours < before * 1.4, "{ours} contre {before}");
}

#[test]
fn hundreds_of_long_strokes_stay_within_the_point_budget_and_none_disappears() {
    let elements = griffonnage(300, 500, 1920, 1080, 17);
    let d = VectorDrawing { elements };
    let items: Vec<usize> = (0..300).collect();
    let lines = lines_of(&d, &items);
    let view = |p: Pos2| pos2(40.0 + p.x * 0.55, 30.0 + p.y * 0.55);
    let c = Contours::build(1, &lines, view, anchors(view), BLUE);
    let s = c.stats;
    println!("{} lignes, {} points -> {} points (tolérance {:.2} px), {} sommets, {} indices", s.lines, s.points_in, s.points_out, s.tolerance, c.vertex_count(), c.index_count());
    assert_eq!(s.points_in, 150_000);
    assert!(s.points_out <= MAX_POINTS, "{} points", s.points_out);
    assert_eq!(s.lines, 300, "chaque trait garde son contour");
    assert_eq!(c.vertex_count(), 3 * s.points_out);
    assert!(c.vertex_count() <= 3 * MAX_POINTS && c.vertex_count() < 4 * 150_000 / 4, "bien moins de sommets que le tracé d'egui (600 000)");
    // Un trait seul, long ou court, n'est pas simplifié au point de changer d'allure.
    let few = Contours::build(2, &lines[..5], view, anchors(view), BLUE);
    assert!(few.stats.tolerance <= 0.45 + 1e-6, "peu de traits : la finesse de l'écran");
}

#[test]
fn the_cached_mesh_follows_the_view_and_the_gesture_by_a_transform_and_is_reused_at_rest() {
    let line: Vec<Pos2> = (0..50).map(|i| pos2(100.0 + i as f32 * 8.0, 200.0 + (i as f32 * 0.3).sin() * 40.0)).collect();
    let view = |p: Pos2| pos2(40.0 + p.x * 0.6, 30.0 + p.y * 0.6);
    let c = Contours::build(7, &[line], view, anchors(view), BLUE);
    // Au repos : identité, et le même maillage (rien de copié).
    let t = c.transform_to(&anchors(view)).expect("affine");
    let (a, b) = (c.shape(&t), c.shape(&Affine::IDENTITY));
    match (&a, &b) {
        (Shape::Mesh(x), Shape::Mesh(y)) => assert!(std::sync::Arc::ptr_eq(x, y), "le maillage gardé est rendu tel quel"),
        _ => panic!("maillages"),
    }
    // Le dessin déplacé de (30, -20) par un geste : la vue de la sélection translatée.
    let moved = |p: Pos2| view(p + vec2(30.0, -20.0));
    let t = c.transform_to(&anchors(moved)).expect("affine");
    assert!((t.tx - 18.0).abs() < 1e-2 && (t.ty + 12.0).abs() < 1e-2 && (t.a - 1.0).abs() < 1e-4, "{t:?}");
    let shape = c.shape(&t);
    let (m0, m1) = (mesh_of(&b), mesh_of(&shape));
    for (v0, v1) in m0.vertices.iter().zip(&m1.vertices) {
        assert!((v1.pos - v0.pos - vec2(18.0, -12.0)).length() < 1e-2);
    }
    // Rotation et échelle du geste : retrouvées, et l'échelle dit quand refaire.
    let turned = |p: Pos2| view(Affine::rotate_about(pos2(300.0, 300.0), 0.5).then(&Affine::scale_about(pos2(300.0, 300.0), 1.1, 1.1)).apply(p));
    let t = c.transform_to(&anchors(turned)).expect("affine");
    assert!((t.width_factor() - 1.1).abs() < 1e-3);
    assert!(!Contours::too_scaled(&t));
    let big = |p: Pos2| view(Affine::scale_about(pos2(0.0, 0.0), 1.6, 1.6).apply(p));
    assert!(Contours::too_scaled(&c.transform_to(&anchors(big)).unwrap()));
    let small = |p: Pos2| view(Affine::scale_about(pos2(0.0, 0.0), 0.7, 0.7).apply(p));
    assert!(Contours::too_scaled(&c.transform_to(&anchors(small)).unwrap()));
    // Une vue en perspective n'est pas affine : on refait.
    let persp = |p: Pos2| {
        let w = 1.0 + p.x * 0.0004;
        pos2(view(p).x / w, view(p).y / w)
    };
    assert!(c.transform_to(&anchors(persp)).is_none());
}

#[test]
fn the_fingerprint_changes_when_the_selection_or_the_drawing_does_and_not_otherwise() {
    let mut d = VectorDrawing { elements: griffonnage(6, 200, 1920, 1080, 3) };
    let items: Vec<usize> = vec![0, 2, 3];
    let base = fingerprint(1, 5, &d, &items);
    assert_eq!(base, fingerprint(1, 5, &d, &items), "stable");
    assert_ne!(base, fingerprint(1, 6, &d, &items), "version du dessin");
    assert_ne!(base, fingerprint(2, 5, &d, &items), "autre dessin");
    assert_ne!(base, fingerprint(1, 5, &d, &[0, 2]), "autre sélection");
    assert_ne!(base, fingerprint(1, 5, &d, &[0, 3, 2]), "autre ordre");
    // Un élément qui n'est pas choisi peut changer sans refaire les contours.
    d.elements[1] = d.elements[1].transformed(&Affine::translate(vec2(5.0, 5.0)));
    assert_eq!(base, fingerprint(1, 5, &d, &items));
    // Un élément choisi qui bouge les refait.
    d.elements[2] = d.elements[2].transformed(&Affine::translate(vec2(5.0, 5.0)));
    assert_ne!(base, fingerprint(1, 5, &d, &items));
}
