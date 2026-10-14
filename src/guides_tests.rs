//! Tests des guides de perspective.

use eframe::egui::{pos2, vec2, Rect};

use crate::document::Document;
use crate::guides::{clip_line, project, Guide};
use crate::project;

#[test]
fn directions_follow_the_vanishing_points() {
    let g = Guide {
        name: "2 points".into(),
        points: vec![[0.0, 100.0], [1000.0, 100.0]],
    };
    let s = pos2(500.0, 300.0);
    // Un mouvement vers le haut à gauche suit la fuyante vers le premier point.
    let d = g.best_direction(s, vec2(-10.0, -3.0)).unwrap();
    assert!((d - (pos2(0.0, 100.0) - s).normalized()).length() < 1e-4);
    // Un mouvement presque vertical suit la verticale (2 points : pas de troisième point).
    let d = g.best_direction(s, vec2(1.0, 20.0)).unwrap();
    assert!(d.x.abs() < 1e-4);
    // Le trait reste sur la droite choisie.
    let q = project(s, d, pos2(530.0, 400.0));
    assert!((q.x - 500.0).abs() < 1e-3 && (q.y - 400.0).abs() < 1e-3);
    // 1 point : fuyante, horizontale et verticale.
    let one = Guide::new(1, eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, eframe::egui::vec2(1920.0, 1080.0)));
    assert_eq!(one.directions(pos2(0.0, 0.0)).len(), 3);
    assert_eq!(Guide::new(3, eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, eframe::egui::vec2(1920.0, 1080.0))).directions(pos2(0.0, 0.0)).len(), 3);
}

#[test]
fn lines_are_clipped_to_the_drawing() {
    let frame = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 50.0));
    let (a, b) = clip_line(pos2(-50.0, 25.0), vec2(1.0, 0.0), frame, false).unwrap();
    assert_eq!((a, b), (pos2(0.0, 25.0), pos2(100.0, 25.0)));
    // Segment qui s'arrête avant le bord.
    let (_, b) = clip_line(pos2(10.0, 10.0), vec2(20.0, 0.0), frame, true).unwrap();
    assert_eq!(b, pos2(30.0, 10.0));
    assert!(clip_line(pos2(0.0, 80.0), vec2(1.0, 0.0), frame, false).is_none());
}

#[test]
fn guides_are_saved_with_the_project() {
    let dir = std::env::temp_dir().join(format!("pelure-test-guides-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("G.pelure");
    let mut doc = Document::new(64, 48, 24);
    doc.guides.push(Guide::new(2, eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, eframe::egui::vec2(64.0, 48.0))));
    doc.guides[0].points[1] = [500.0, -20.0];
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    assert_eq!(back.guides, doc.guides);
    std::fs::remove_dir_all(&dir).ok();
}
