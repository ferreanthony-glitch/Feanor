//! Flips (13h) : limites, checkpoints, pas et lecture.

use crate::document::{Document, Layer, LayerKind};
use crate::flips::{self, Checkpoints, Flip, FlipKind, FlipScope, FlipSettings, Limits};
use crate::roles::Role;

/// Un plan de 20 images ; un calque : A (0–3), vide (4–5), B (6–9), C (10–13) ; B marqué en
/// couleur 2 ; signets aux images 2, 8 et 15 ; rôle clé aux images 0 et 10, intervalle à 5.
fn doc() -> Document {
    let mut doc = Document::new(16, 16, 24);
    doc.panels[0].set_length(20);
    let [a, b, c] = [0; 3].map(|_| doc.new_cel(LayerKind::Bitmap));
    let mut l = Layer::new("L", LayerKind::Bitmap);
    l.frames = [vec![Some(a); 4], vec![None; 2], vec![Some(b); 4], vec![Some(c); 4]].concat();
    l.color_marks.insert(b, 2);
    l.roles = [(0, Role::Key), (5, Role::Inbetween), (10, Role::Key)].into_iter().collect();
    doc.panels[0].layers = vec![l];
    doc.panels[0].cues.bookmarks = vec![2, 8, 15];
    doc
}

fn scope(doc: &Document) -> FlipScope<'_> {
    FlipScope { panel: &doc.panels[0], layer: &doc.panels[0].layers[0], len: doc.panels[0].frame_count() }
}

#[test]
fn limits_around_the_start() {
    let d = doc();
    let s = scope(&d);
    assert_eq!(s.limits(Limits::Plan, 7), (0, 20));
    assert_eq!(s.limits(Limits::Layer, 7), (0, 14));
    assert_eq!(s.limits(Limits::Bookmarks, 7), (2, 9), "du signet 2 au signet 8 compris");
    assert_eq!(s.limits(Limits::Bookmarks, 16), (15, 20));
    assert_eq!(s.limits(Limits::Marks { couleur: Some(2) }, 12), (6, 20), "depuis le dessin marqué");
    assert_eq!(s.limits(Limits::Role { role: Role::Key }, 4), (0, 11));
    assert_eq!(s.limits(Limits::Custom { gauche: 3, droite: 2 }, 7), (4, 10));
    assert_eq!(s.limits(Limits::Custom { gauche: 30, droite: 30 }, 7), (0, 20));
}

#[test]
fn checkpoints_inside_the_limits() {
    let d = doc();
    let s = scope(&d);
    let flip = |limits, checkpoints| Flip { limits, checkpoints, ..Flip::default() };
    assert_eq!(s.points(&flip(Limits::Layer, Checkpoints::Instances), 7), vec![0, 6, 10]);
    assert_eq!(s.points(&flip(Limits::Plan, Checkpoints::Bookmarks), 7), vec![2, 8, 15]);
    assert_eq!(s.points(&flip(Limits::Plan, Checkpoints::Marks { couleur: None }), 7), vec![6]);
    assert_eq!(s.points(&flip(Limits::Plan, Checkpoints::Role { role: Role::Key }), 7), vec![0, 10]);
    assert_eq!(s.points(&flip(Limits::Plan, Checkpoints::Role { role: Role::Inbetween }), 7), vec![5], "même sur une image vide");
    assert_eq!(s.points(&flip(Limits::Custom { gauche: 1, droite: 1 }, Checkpoints::Images), 7), vec![6, 7, 8]);
}

#[test]
fn steps_and_playback() {
    let pts = [0, 6, 10];
    assert_eq!(flips::index_of(&pts, 7), 1);
    assert_eq!(flips::step(&pts, 2, 1, false), 2, "au bout, on reste");
    assert_eq!(flips::step(&pts, 2, 1, true), 0, "en boucle, on repart");
    assert_eq!(flips::step(&pts, 0, -1, true), 2);
    assert_eq!(flips::next_point(&pts, 7, 1, false), Some(10));
    assert_eq!(flips::next_point(&pts, 7, -1, false), Some(6), "d'une image entre deux : la précédente");
    assert_eq!(flips::next_point(&pts, 10, 1, false), Some(10));
    assert_eq!(flips::next_point(&pts, 10, 1, true), Some(0));
    assert_eq!(flips::next_point(&[], 3, 1, true), None);
    // 12 flips par seconde depuis le 2e checkpoint : 0,1 s plus tard, on est au 3e.
    assert_eq!(flips::played(&pts, 1, 0.1, 12.0, false), 2);
    assert_eq!(flips::played(&pts, 1, 1.0, 12.0, false), 2, "sans boucle : on reste au bout");
    assert_eq!(flips::played(&pts, 0, 0.25, 12.0, true), 0, "3 pas en boucle sur 3");
}

#[test]
fn flips_are_saved_with_the_app_settings() {
    let mut set = crate::settings::Settings::default();
    assert_eq!(set.flips, FlipSettings::default());
    assert_eq!(set.flips.current().kind, FlipKind::Manual, "le flip d'Anthony : manuel");
    set.flips.flips.push(Flip {
        name: "Clés".into(),
        kind: FlipKind::Directional,
        limits: Limits::Marks { couleur: Some(3) },
        checkpoints: Checkpoints::Role { role: Role::Key },
        vertical: true,
        ..Flip::default()
    });
    set.flips.current = 1;
    let text = toml::to_string(&set).unwrap();
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.flips, set.flips, "{text}");
}
