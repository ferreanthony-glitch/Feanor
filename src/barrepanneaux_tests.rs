//! Barre des panneaux : ses icônes, la règle du clic, le réglage qui la cache.

use egui_dock::{DockState, NodeIndex, TabIndex};

use crate::barrepanneaux::{est_devant, pane_de_action, regle_clic, Clic, GROUPES};
use crate::dock::Pane;

fn panes() -> Vec<Pane> {
    GROUPES.iter().flat_map(|g| g.iter()).map(|e| e.pane).collect()
}

#[test]
fn the_bar_has_the_nine_side_panels_in_order() {
    assert_eq!(
        panes(),
        vec![
            Pane::ToolOptions,
            Pane::Brushes,
            Pane::Color,
            Pane::Guides,
            Pane::Layers,
            Pane::Effects,
            Pane::Sheet,
            Pane::XSheet,
            Pane::History,
        ]
    );
    // Trois groupes : le dessin, le plan, l'historique.
    assert_eq!(GROUPES.iter().map(|g| g.len()).collect::<Vec<_>>(), vec![4, 4, 1]);
}

#[test]
fn the_bar_leaves_out_the_canvas_the_strip_the_tools_and_the_old_panes() {
    for p in [Pane::Canvas, Pane::Montage, Pane::Tools, Pane::View, Pane::Timeline] {
        assert!(!panes().contains(&p), "{p:?} n'a pas d'icône dans la barre");
    }
}

#[test]
fn every_icon_has_a_registered_action_a_name_and_a_description() {
    let mut seen = std::collections::BTreeSet::new();
    for e in GROUPES.iter().flat_map(|g| g.iter()) {
        let info = crate::keymap::action_info(e.action).unwrap_or_else(|| panic!("{} n'est pas dans keymap::ACTIONS", e.action));
        assert!(info.implemented, "{} doit être implémentée", e.action);
        assert!(seen.insert(e.action), "{} sert deux icônes", e.action);
        assert!(!e.nom.is_empty() && !e.description.is_empty(), "{:?} : bulle incomplète", e.pane);
        assert!(!e.pane.icon().is_empty());
        assert_eq!(pane_de_action(e.action), Some(e.pane));
    }
    assert_eq!(pane_de_action("vue.panneau_outils"), None, "les groupes ne sont pas des icônes");
    assert_eq!(pane_de_action("fichier.nouveau"), None);
    // La barre elle-même est une action de la palette.
    assert!(crate::keymap::action_info("vue.barre_panneaux").is_some_and(|a| a.implemented));
}

#[test]
fn the_click_opens_brings_forward_or_closes() {
    // Fermé : on l'ouvre (que « devant » veuille dire ce qu'il veut).
    assert_eq!(regle_clic(false, false), Clic::Ouvrir);
    assert_eq!(regle_clic(false, true), Clic::Ouvrir);
    // Ouvert derrière un autre onglet : on le met devant, on ne le ferme pas.
    assert_eq!(regle_clic(true, false), Clic::Avancer);
    // Ouvert et visible : on le ferme.
    assert_eq!(regle_clic(true, true), Clic::Fermer);
}

#[test]
fn a_tab_is_in_front_only_when_it_is_the_active_one_of_its_group() {
    let mut state = DockState::new(vec![Pane::Canvas]);
    state.main_surface_mut().split_right(NodeIndex::root(), 0.8, vec![Pane::Layers, Pane::Effects, Pane::History]);
    // Le premier onglet du groupe est actif au départ.
    assert!(est_devant(&state, Pane::Layers));
    assert!(!est_devant(&state, Pane::Effects));
    assert!(!est_devant(&state, Pane::History));
    // Le dessin, dans son propre groupe, est devant.
    assert!(est_devant(&state, Pane::Canvas));
    // Un panneau absent n'est pas devant.
    assert!(!est_devant(&state, Pane::Color));
    // On met les effets devant : les calques passent derrière.
    let at = state.find_tab(&Pane::Effects).unwrap();
    state.set_active_tab(at);
    assert!(est_devant(&state, Pane::Effects));
    assert!(!est_devant(&state, Pane::Layers));
    assert_eq!(state.find_tab(&Pane::Effects).map(|(_, _, t)| t), Some(TabIndex(1)));
}

#[test]
fn the_bar_is_shown_by_default_and_by_old_settings() {
    let s = crate::settings::Settings::default();
    assert!(s.panel_bar, "la barre est là d'office");
    let old: crate::settings::Settings = toml::from_str("copie_secours_minutes = 1").unwrap();
    assert!(old.panel_bar, "des réglages d'avant la barre : elle est montrée");
    let hidden: crate::settings::Settings = toml::from_str("barre_panneaux = false").unwrap();
    assert!(!hidden.panel_bar);
    let text = toml::to_string(&hidden).unwrap();
    assert!(!toml::from_str::<crate::settings::Settings>(&text).unwrap().panel_bar, "le choix est retenu");
}
