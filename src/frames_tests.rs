//! Tests des cadres de composition (9d) : format de diffusion et zones de sécurité.

use eframe::egui::{pos2, vec2, Rect};

use crate::frames::{self, format_rect, safe_rect};
use crate::workspace::Layout;

fn size(r: Rect) -> (f32, f32) {
    (r.width(), r.height())
}

#[test]
fn format_frame_is_the_largest_centred_rectangle() {
    let hd = [1920, 1080];
    assert_eq!(format_rect(hd, [16.0, 9.0]), Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1080.0)));
    assert_eq!(size(format_rect(hd, [1.0, 1.0])), (1080.0, 1080.0));
    assert_eq!(size(format_rect(hd, [4.0, 5.0])), (864.0, 1080.0));
    let vertical = format_rect(hd, [9.0, 16.0]);
    assert!((vertical.width() - 607.5).abs() < 1e-3 && vertical.height() == 1080.0);
    assert_eq!(vertical.center(), pos2(960.0, 540.0), "centré dans le cadre");
    // Plus large que le cadre : toute la largeur, bandes en haut et en bas.
    let scope = format_rect(hd, [2.39, 1.0]);
    assert!(scope.width() == 1920.0 && (scope.height() - 1920.0 / 2.39).abs() < 1e-2);
    // Cadre vertical (9:16) et format 16:9 dedans.
    assert_eq!(size(format_rect([1080, 1920], [16.0, 9.0])), (1080.0, 607.5));
}

#[test]
fn safe_zones_shrink_around_the_centre() {
    let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1080.0));
    let action = safe_rect(full, frames::ACTION_SAFE);
    let title = safe_rect(full, frames::TITLE_SAFE);
    assert!((action.min.x - 1920.0 * 0.035).abs() < 1e-3 && (action.min.y - 1080.0 * 0.035).abs() < 1e-3);
    assert!((title.min.x - 96.0).abs() < 1e-3 && (title.min.y - 54.0).abs() < 1e-3);
    assert!(action.contains_rect(title), "les titres sont dans la zone d'action");
    assert_eq!(action.center(), full.center());
}

#[test]
fn formats_are_named_and_checked() {
    assert_eq!(frames::label([16.0, 9.0]), "16:9");
    assert_eq!(frames::label([2.39, 1.0]), "2.39:1");
    assert!(frames::is_preset([9.0, 16.0]) && !frames::is_preset([1.9, 1.0]));
    assert!(frames::is_preset([1.85, 1.0]) && frames::is_preset([4.0, 3.0]));
    assert!(frames::is_preset([2.39, 1.0]), "le 2,39:1 de l'objectif anamorphique est proposé dans la liste des formats");
    assert_eq!(frames::sanitized([0.0, 1.0]), None);
    assert_eq!(frames::sanitized([f32::NAN, 1.0]), None);
    assert_eq!(frames::sanitized([4.0, 5.0]), Some([4.0, 5.0]));
}

#[test]
fn the_dimmed_band_is_the_sheet_minus_the_shot() {
    use crate::viewbar::{clip_to_rect, nested_band};
    use eframe::egui::{Color32, Shape};
    let area = |p: &[eframe::egui::Pos2]| {
        0.5 * (0..p.len()).map(|i| p[i].x * p[(i + 1) % p.len()].y - p[(i + 1) % p.len()].x * p[i].y).sum::<f32>().abs()
    };
    let sheet = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 200.0));
    // Un trapèze qui déborde de la feuille en haut à droite.
    let shot = [pos2(150.0, 60.0), pos2(520.0, -80.0), pos2(480.0, 150.0), pos2(170.0, 140.0)];
    let inside = clip_to_rect(&shot, sheet);
    assert!(inside.iter().all(|p| sheet.expand(1e-3).contains(*p)), "{inside:?}");
    assert!(inside.len() >= 4);
    let outer = [sheet.left_top(), sheet.right_top(), sheet.right_bottom(), sheet.left_bottom()];
    let Shape::Mesh(mesh) = nested_band(&outer, &inside, Color32::BLACK) else { panic!() };
    let covered: f32 = mesh
        .indices
        .chunks(3)
        .map(|t| area(&[mesh.vertices[t[0] as usize].pos, mesh.vertices[t[1] as usize].pos, mesh.vertices[t[2] as usize].pos]))
        .sum();
    assert!((covered - (area(&outer) - area(&inside))).abs() < 1.0, "{covered} ≠ {}", area(&outer) - area(&inside));
    // Cadre hors de la feuille : toute la feuille est estompée.
    let away = clip_to_rect(&[pos2(500.0, 0.0), pos2(600.0, 0.0), pos2(600.0, 50.0), pos2(500.0, 50.0)], sheet);
    assert!(away.is_empty());
}

#[test]
fn frames_are_kept_with_the_layout() {
    let l = Layout { safe_frames: true, frame_format: Some([4.0, 5.0]), ..Layout::default() };
    let text = toml::to_string(&l).unwrap();
    assert!(text.contains("cadres_securite = true") && text.contains("cadre_format"), "{text}");
    let back: Layout = toml::from_str(&text).unwrap();
    assert_eq!(back, l);
    // Rien d'écrit sans format ; une disposition d'avant 9d s'ouvre sans cadres.
    assert!(!toml::to_string(&Layout::default()).unwrap().contains("cadre_format"));
    let old: Layout = toml::from_str("outils = true").unwrap();
    assert!(!old.safe_frames && old.frame_format.is_none());
    // Format illisible (fichier modifié à la main) : retiré.
    let bad = Layout { frame_format: Some([-1.0, 2.0]), ..Layout::default() }.sanitized();
    assert_eq!(bad.frame_format, None);
}
