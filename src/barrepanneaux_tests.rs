//! Barre des panneaux : ses icônes, la règle du clic, le réglage qui la cache.

use egui_dock::{DockState, NodeIndex, TabIndex};

use crate::barrepanneaux::{est_devant, pane_de_action, panneaux_d_office, regle_clic, Clic, GROUPES};
use crate::dock::Pane;
use crate::workspace::Espace;

fn panes() -> Vec<Pane> {
    GROUPES.iter().flat_map(|g| g.iter()).map(|e| e.pane).collect()
}

#[test]
fn the_bar_has_the_twelve_side_panels_in_order() {
    assert_eq!(
        panes(),
        vec![
            Pane::ToolOptions,
            Pane::Brushes,
            Pane::Color,
            Pane::Personnages,
            Pane::Layers,
            Pane::Effects,
            Pane::Sheet,
            Pane::XSheet,
            Pane::Comments,
            Pane::History,
            Pane::Bruitages,
            Pane::References,
        ]
    );
    // Trois groupes : le dessin, le plan, puis les commentaires, l'historique, les bruitages et les
    // références. (Les suggestions sonores sont dans le menu Labo, sans icône : HORS_BARRE.)
    assert_eq!(GROUPES.iter().map(|g| g.len()).collect::<Vec<_>>(), vec![4, 4, 4]);
    assert_eq!(pane_de_action("vue.suggestions_sonores"), Some(Pane::SuggestionsSonores));
}

#[test]
fn the_guides_panel_is_gone_from_the_bar_and_from_every_default_set() {
    // Le panneau Guides n'est plus une icône de la barre, ni d'office dans aucun espace de travail (Anthony,
    // 2026-10-21) : les guides sont dans le menu de la règle, au-dessus du dessin.
    assert!(!panes().contains(&Pane::Guides));
    assert_eq!(pane_de_action("vue.guides"), None);
    assert!(crate::keymap::action_info("vue.guides").is_none(), "l'action du panneau n'existe plus");
    for e in Espace::TOUS {
        assert!(!panneaux_d_office(e).contains(&Pane::Guides), "{e:?}");
        assert!(!icones_de(e, true, &[]).contains(&Pane::Guides), "{e:?}");
    }
    // Il n'est plus dans la liste des panneaux (menu Affichage), et une disposition enregistrée qui l'avait le perd.
    assert!(!Pane::ALL.contains(&Pane::Guides));
    let ancienne = crate::dock::DockLayout {
        version: crate::dock::LAYOUT_VERSION,
        principal: crate::dock::DockNode::Row {
            part: 0.8,
            gauche: Box::new(crate::dock::DockNode::Tabs { panneaux: vec![Pane::Canvas], actif: 0 }),
            droite: Box::new(crate::dock::DockNode::Tabs { panneaux: vec![Pane::ToolOptions, Pane::Guides, Pane::Sheet], actif: 1 }),
        },
        fenetres: Vec::new(),
        detachees: Vec::new(),
    };
    let lue = ancienne.sanitized();
    assert!(!lue.contains(Pane::Guides) && lue.contains(Pane::ToolOptions) && lue.contains(Pane::Sheet));
    // Les dispositions d'origine non plus.
    for l in [crate::dock::DockLayout::from_legacy(&crate::workspace::Layout::default()), crate::dock::DockLayout::pour_les_vignettes(), crate::dock::DockLayout::pour_le_montage(290.0)] {
        assert!(!l.contains(Pane::Guides));
    }
}

#[test]
fn the_exposure_sheet_icon_is_only_there_in_animation() {
    use crate::barrepanneaux::icones;
    let panes = |animation: bool| icones(animation).into_iter().flatten().map(|e| e.pane).collect::<Vec<_>>();
    assert_eq!(panes(true), self::panes(), "en animation : les douze icônes");
    let storyboard = panes(false);
    assert_eq!(storyboard.len(), 11);
    assert!(!storyboard.contains(&Pane::XSheet), "pas de feuille d'exposition en storyboard");
    assert!(storyboard.contains(&Pane::Sheet) && storyboard.contains(&Pane::History) && storyboard.contains(&Pane::Comments));
    // Les trois groupes restent là (aucun ne s'est vidé).
    assert_eq!(icones(false).len(), 3);
    assert_eq!(icones(false)[1].len(), 3);
    // L'action reste enregistrée (palette, raccourci) : elle répond d'un mot en storyboard.
    assert!(crate::keymap::action_info("vue.feuille_exposition").is_some_and(|a| a.implemented));
}

/// Les panneaux dont la barre montre l'icône dans `espace`, `ouverts` étant les panneaux ouverts.
fn icones_de(espace: Espace, animation: bool, ouverts: &[Pane]) -> Vec<Pane> {
    crate::barrepanneaux::icones_de(espace, animation, &|p| ouverts.contains(&p)).into_iter().flatten().map(|e| e.pane).collect()
}

#[test]
fn each_workspace_shows_only_the_panels_useful_for_its_work() {
    // Vignettes : « la plus épurée possible, rien de distraction, que du dessin et des références ».
    // (Et le panneau « Vignettes », sens de lecture et textes, qui a son icône dans cet espace seulement, en tête.)
    assert_eq!(icones_de(Espace::Vignettes, false, &[]), vec![Pane::Vignettes, Pane::ToolOptions, Pane::Brushes, Pane::Color, Pane::References]);
    // Montage : la fiche du plan et ce qui sert à la relecture et au son.
    assert_eq!(icones_de(Espace::Montage, false, &[]), vec![Pane::Sheet, Pane::Comments, Pane::History, Pane::Bruitages]);
    // Storyboard : le dessin, les calques, la fiche ; pas de feuille d'exposition, de commentaires ni d'historique.
    let storyboard = icones_de(Espace::Storyboard, false, &[]);
    assert_eq!(storyboard.len(), 8, "{storyboard:?}");
    for absent in [Pane::XSheet, Pane::Comments, Pane::History, Pane::Bruitages] {
        assert!(!storyboard.contains(&absent), "{absent:?} n'est pas dans la barre du storyboard");
    }
    // Animation : la feuille d'exposition en plus, sans les personnages.
    let animation = icones_de(Espace::Animation, true, &[]);
    assert!(animation.contains(&Pane::XSheet) && !animation.contains(&Pane::Personnages), "{animation:?}");
    // Chaque espace a moins d'icônes que les douze du catalogue.
    for e in Espace::TOUS {
        assert!(icones_de(e, true, &[]).len() < 12, "{e:?}");
    }
    // Rien n'est perdu : chaque panneau de la barre est d'office dans au moins un espace, et garde son action
    // (palette Ctrl+K) et sa case dans le menu Affichage.
    for p in panes() {
        assert!(Espace::TOUS.iter().any(|e| panneaux_d_office(*e).contains(&p)), "{p:?} n'est d'office dans aucun espace");
    }
}

#[test]
fn a_panel_opened_by_hand_keeps_its_icon_while_it_is_open() {
    // Vignettes : l'historique ouvert à la main a son icône (la barre ne se bat pas contre l'utilisateur) ; refermé,
    // elle part. Chaque espace retient sa disposition, donc le panneau ouvert revient avec l'espace.
    let sans = icones_de(Espace::Vignettes, false, &[]);
    let avec = icones_de(Espace::Vignettes, false, &[Pane::History]);
    assert!(!sans.contains(&Pane::History) && avec.contains(&Pane::History));
    assert_eq!(avec.len(), sans.len() + 1);
    // Les panneaux d'office restent, ouverts ou non.
    assert!(sans.contains(&Pane::Color));
    // La feuille d'exposition, elle, n'existe qu'en animation, même ouverte.
    assert!(!icones_de(Espace::Storyboard, false, &[Pane::XSheet]).contains(&Pane::XSheet));
}

#[test]
fn the_panels_not_in_the_bar_stay_reachable_by_the_palette_and_the_display_menu() {
    for e in Espace::TOUS {
        let visibles = icones_de(e, true, &[]);
        for p in panes().into_iter().filter(|p| !visibles.contains(p)) {
            let entree = GROUPES.iter().flat_map(|g| g.iter()).find(|x| x.pane == p).unwrap();
            // Palette : l'action existe ; menu Affichage : le panneau est dans `Pane::ALL` (une case chacun).
            assert!(crate::keymap::action_info(entree.action).is_some_and(|a| a.implemented), "{p:?}");
            assert!(Pane::ALL.contains(&p));
        }
    }
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
