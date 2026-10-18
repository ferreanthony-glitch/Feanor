//! Panneaux qui s'ajustent : la hauteur donnée à chaque partie d'une colonne.

use crate::ajuste::{ease, fits, flex_min, fraction_for, measured, top_height, Want, MIN_FIT};
use crate::dock::Pane;

fn flex(min: f32) -> Want {
    Want::Flex { min, content: None }
}

#[test]
fn a_settings_pane_takes_its_content_height_and_the_color_the_rest() {
    // Réglages de l'outil (180 points de contenu) sur la couleur, dans 700 points.
    assert_eq!(top_height(700.0, 4.0, Want::Fit(180.0), flex(200.0)), Some(180.0));
    // Les effets (120 points) sous les calques : les calques ont le reste.
    assert_eq!(top_height(700.0, 4.0, flex(140.0), Want::Fit(120.0)), Some(576.0));
}

#[test]
fn the_flexible_pane_keeps_its_minimum() {
    // Des réglages très longs ne mangent pas la couleur : elle garde ses 200 points.
    assert_eq!(top_height(700.0, 4.0, Want::Fit(900.0), flex(200.0)), Some(496.0));
    assert_eq!(top_height(700.0, 4.0, flex(140.0), Want::Fit(900.0)), Some(140.0));
}

#[test]
fn when_both_overflow_the_color_gets_its_share() {
    // Réglages du pinceau (500) et couleur (500) dans 604 points : moitié-moitié.
    let color = Want::Flex { min: 200.0, content: Some(500.0) };
    assert_eq!(top_height(604.0, 4.0, Want::Fit(500.0), color), Some(300.0));
    // Ils tiennent : les réglages ont leur hauteur, la couleur le reste.
    assert_eq!(top_height(704.0, 4.0, Want::Fit(100.0), color), Some(100.0));
    // Un contenu immense compte pour la colonne entière, pas plus.
    let huge = Want::Flex { min: 140.0, content: Some(5000.0) };
    assert_eq!(top_height(604.0, 4.0, Want::Fit(600.0), huge), Some(300.0));
}

#[test]
fn two_fitting_panes_share_when_they_do_not_fit() {
    assert_eq!(top_height(500.0, 0.0, Want::Fit(100.0), Want::Fit(150.0)), Some(100.0), "ils tiennent : celui du haut a sa hauteur");
    assert_eq!(top_height(500.0, 0.0, Want::Fit(400.0), Want::Fit(600.0)), Some(200.0), "chacun sa part");
}

#[test]
fn nothing_moves_between_two_flexible_panes_or_in_a_tiny_column() {
    assert_eq!(top_height(700.0, 4.0, flex(240.0), flex(160.0)), None, "le dessin et la bande : à la main");
    assert_eq!(top_height(100.0, 4.0, Want::Fit(20.0), flex(200.0)), None);
    // Jamais moins qu'une barre d'onglets et une ligne.
    assert_eq!(top_height(700.0, 4.0, Want::Fit(10.0), flex(200.0)), Some(MIN_FIT));
}

#[test]
fn fraction_and_easing() {
    // egui_dock place le bord au milieu de son épaisseur.
    assert!((fraction_for(700.0, 4.0, 180.0) - 182.0 / 700.0).abs() < 1e-6);
    assert_eq!(fraction_for(700.0, 4.0, 10_000.0), 0.95);
    // Le bord glisse, puis s'arrête sur la cible.
    let mut h = 300.0;
    let mut steps = 0;
    while h != 180.0 {
        h = ease(h, 180.0);
        steps += 1;
        assert!(steps < 20);
    }
    assert!(steps > 2, "pas d'un seul coup");
}

#[test]
fn edges_dragged_by_hand_are_kept_in_the_settings() {
    let mut s = crate::settings::Settings::default();
    assert!(s.panes_fit, "les panneaux s'ajustent d'office");
    s.panes_fixed.push((Pane::ToolOptions, Pane::Color));
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("panneaux_tires_a_la_main"), "{text}");
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.panes_fixed, vec![(Pane::ToolOptions, Pane::Color)]);
    // Des réglages d'avant : rien de tiré, ajustés.
    let old: crate::settings::Settings = toml::from_str("copie_secours_minutes = 1").unwrap();
    assert!(old.panes_fit && old.panes_fixed.is_empty());
}

#[test]
fn which_panes_fit() {
    for p in [Pane::ToolOptions, Pane::Guides, Pane::Sheet, Pane::Effects, Pane::Tools] {
        assert!(fits(p) && measured(p), "{p:?}");
    }
    for p in [Pane::Color, Pane::Brushes, Pane::Layers, Pane::Canvas, Pane::Montage, Pane::XSheet] {
        assert!(!fits(p), "{p:?}");
        assert!(flex_min(p) >= 140.0, "{p:?}");
    }
    // La couleur est mesurée ; les calques remplissent toujours leur panneau, on ne les mesure pas.
    assert!(measured(Pane::Color) && !measured(Pane::Layers));
}
