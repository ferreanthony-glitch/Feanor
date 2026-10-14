//! Tests des fenêtres modulables (5b) : disposition de Pelure, conversions vers egui_dock et
//! retour, ouverture et fermeture des panneaux, anciennes dispositions.

use crate::dock::{self, DockLayout, DockNode, FloatingPanel, Group, Pane};
use crate::settings::Settings;
use crate::workspace::{self, Layout};

fn tabs(p: &[Pane]) -> DockNode {
    DockNode::Tabs { panneaux: p.to_vec(), actif: 0 }
}

fn count(layout: &DockLayout, pane: Pane) -> usize {
    layout.panes().iter().filter(|p| **p == pane).count()
}

#[test]
fn default_layout_has_every_panel_once() {
    let l = DockLayout::from_legacy(&Layout::default());
    for pane in Pane::ALL {
        assert_eq!(count(&l, pane), 1, "{pane:?}");
    }
    // La bande en bas, sur toute la largeur ; au-dessus, outils | dessin | fiche et calques.
    let DockNode::Column { part, haut, bas } = &l.principal else { panic!("{:?}", l.principal) };
    assert_eq!(**bas, tabs(&[Pane::Strip]));
    assert!((part - (1.0 - 290.0 / 860.0)).abs() < 1e-4, "{part}");
    let DockNode::Row { part: left, gauche, droite } = &**haut else { panic!() };
    assert!((left - 250.0 / 1500.0).abs() < 1e-4);
    let mut left_panes = Vec::new();
    for p in [Pane::Tools, Pane::Color, Pane::ToolOptions] {
        assert!(format!("{gauche:?}").contains(&format!("{p:?}")), "{p:?} à gauche");
        left_panes.push(p);
    }
    let DockNode::Row { gauche: center, droite: right, .. } = &**droite else { panic!() };
    assert_eq!(**center, tabs(&[Pane::Canvas]));
    assert!(format!("{right:?}").contains("Layers") && format!("{right:?}").contains("Sheet"));
}

#[test]
fn builtin_workspaces_keep_their_meaning() {
    let all = workspace::builtin();
    let dessin = all[1].2.dock_layout();
    assert!(!dessin.contains(Pane::Sheet) && !dessin.contains(Pane::Layers) && dessin.contains(Pane::Tools));
    let ensemble = all[3].2.dock_layout();
    assert!(Group::Left.panes().iter().all(|p| !ensemble.contains(*p)));
    assert!(ensemble.contains(Pane::Canvas) && ensemble.contains(Pane::Strip));
    // Timeline : bande plus haute que dans Standard.
    let part = |l: &DockLayout| match &l.principal {
        DockNode::Column { part, .. } => *part,
        _ => 1.0,
    };
    assert!(part(&all[2].2.dock_layout()) < part(&all[0].2.dock_layout()));
}

#[test]
fn layouts_survive_the_trip_through_egui_dock() {
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.close(Pane::Guides);
    l.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::Guides]), x: 300.0, y: 200.0, largeur: 260.0, hauteur: 320.0 });
    // Un groupe de trois onglets dont le deuxième est ouvert.
    if let DockNode::Column { haut, .. } = &mut l.principal {
        if let DockNode::Row { gauche, .. } = &mut **haut {
            if let DockNode::Column { bas, .. } = &mut **gauche {
                if let DockNode::Column { bas, .. } = &mut **bas {
                    if let DockNode::Tabs { actif, .. } = &mut **bas {
                        *actif = 1;
                    }
                }
            }
        }
    }
    let state = dock::to_state(&l);
    let mut back = dock::from_state(&state, &|_| None);
    // Une fenêtre pas encore affichée n'a pas de place connue : on ne compare que l'arbre.
    for w in &mut back.fenetres {
        (w.x, w.y, w.largeur, w.hauteur) = (300.0, 200.0, 260.0, 320.0);
    }
    assert!(back.close_to(&l), "{back:#?}\n{l:#?}");
    for (_, _, layout) in workspace::builtin() {
        let d = layout.dock_layout();
        assert!(dock::from_state(&dock::to_state(&d), &|_| None).close_to(&d));
    }
}

#[test]
fn closing_and_reopening_a_panel_puts_it_back_with_its_group() {
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.close(Pane::Canvas);
    assert!(l.contains(Pane::Canvas), "le dessin ne se ferme pas");
    l.close(Pane::Layers);
    assert!(!l.contains(Pane::Layers) && l.contains(Pane::Sheet));
    l.open(Pane::Layers);
    // Revenu dans le groupe de la fiche.
    assert!(format!("{:?}", l.principal).contains("Tabs { panneaux: [Sheet, Layers]"), "{:?}", l.principal);
    // Plus rien du groupe : une colonne recréée à droite du dessin.
    l.close(Pane::Sheet);
    l.close(Pane::Layers);
    l.open(Pane::Layers);
    let s = format!("{:?}", l.principal);
    assert!(s.contains("Row { part: 0.78, gauche: Tabs { panneaux: [Canvas]"), "{s}");
    l.open(Pane::Sheet);
    assert!(format!("{:?}", l.principal).contains("Tabs { panneaux: [Layers, Sheet]"));
    // La bande revient en bas de tout.
    l.close(Pane::Strip);
    l.open(Pane::Strip);
    assert!(matches!(&l.principal, DockNode::Column { bas, .. } if **bas == tabs(&[Pane::Strip])));
    for pane in Pane::ALL {
        assert_eq!(count(&l, pane), 1, "{pane:?}");
    }
}

#[test]
fn a_whole_group_toggles_at_once() {
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.toggle_group(Group::Left);
    assert!(Group::Left.panes().iter().all(|p| !l.contains(*p)));
    l.toggle_group(Group::Left);
    assert!(Group::Left.panes().iter().all(|p| l.contains(*p)));
    // Un seul panneau du groupe affiché : le bouton du groupe le masque.
    l.close(Pane::Tools);
    l.close(Pane::Color);
    l.toggle_group(Group::Left);
    assert!(Group::Left.panes().iter().all(|p| !l.contains(*p)));
}

#[test]
fn damaged_layouts_are_repaired() {
    let bad = DockLayout {
        principal: DockNode::Row {
            part: 7.0,
            gauche: Box::new(DockNode::Tabs { panneaux: vec![Pane::Tools, Pane::Tools], actif: 9 }),
            droite: Box::new(tabs(&[])),
        },
        fenetres: vec![
            FloatingPanel { panneaux: tabs(&[Pane::Canvas, Pane::Color]), x: f32::NAN, y: 0.0, largeur: 10.0, hauteur: 5.0 },
            FloatingPanel { panneaux: tabs(&[Pane::Tools]), x: 0.0, y: 0.0, largeur: 0.0, hauteur: 0.0 },
        ],
        detachees: Vec::new(),
    }
    .sanitized();
    assert_eq!(count(&bad, Pane::Tools), 1);
    assert_eq!(count(&bad, Pane::Canvas), 1, "dessin remis dans la fenêtre principale");
    assert_eq!(bad.fenetres.len(), 1, "fenêtre vide retirée");
    assert_eq!(bad.fenetres[0].panneaux, tabs(&[Pane::Color]), "le dessin ne flotte pas");
    assert_eq!(bad.fenetres[0].largeur, 0.0, "taille absurde : à replacer");
    let s = format!("{:?}", bad.principal);
    assert!(s.contains("actif: 0") && !s.contains("part: 7"), "{s}");
}

#[test]
fn a_panel_can_have_its_own_window() {
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.detach(Pane::Layers);
    assert_eq!(count(&l, Pane::Layers), 1, "dans sa fenêtre, plus dans un onglet");
    assert!(!format!("{:?}", l.principal).contains("Layers"));
    assert_eq!(l.detachees[0].panneau, Pane::Layers);
    l.detach(Pane::Canvas);
    assert!(l.detachees.iter().all(|d| d.panneau != Pane::Canvas), "le dessin reste dans la fenêtre principale");
    // Rouvrir ne le double pas ; le fermer le retire de sa fenêtre.
    l.open(Pane::Layers);
    assert_eq!(count(&l, Pane::Layers), 1);
    l.close(Pane::Layers);
    assert!(!l.contains(Pane::Layers) && l.detachees.is_empty());
    // Une fenêtre à part de taille absurde sera replacée.
    l.close(Pane::View);
    let bad = dock::DetachedPanel { panneau: Pane::View, x: 10.0, y: 10.0, largeur: 5.0, hauteur: f32::NAN };
    l.detachees.push(bad.clone());
    let l = l.sanitized();
    assert_eq!(count(&l, Pane::View), 1);
    assert_eq!((l.detachees[0].largeur, l.detachees[0].hauteur), (0.0, 0.0));
    // En double avec un onglet : l'onglet l'emporte.
    let mut twice = DockLayout::from_legacy(&Layout::default());
    twice.detachees.push(bad);
    let twice = twice.sanitized();
    assert_eq!(count(&twice, Pane::View), 1);
    assert!(twice.detachees.is_empty());
}

#[test]
fn panel_layouts_are_kept_in_the_settings() {
    let mut s = Settings::default();
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.close(Pane::View);
    l.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::View]), x: 40.0, y: 50.0, largeur: 300.0, hauteur: 200.0 });
    s.layout = Some(Layout { panneaux: Some(l.clone()), ..Layout::default() });
    workspace::save_workspace(&mut s.workspaces, "Le mien", Layout { panneaux: Some(l.clone()), ..Layout::default() });
    let text = toml::to_string(&s).unwrap();
    let back: Settings = toml::from_str(&text).unwrap();
    assert!(back.layout.as_ref().unwrap().dock_layout().close_to(&l), "{text}");
    assert!(back.workspaces[0].layout.dock_layout().close_to(&l));
    // Une disposition enregistrée avant 5b (sans `panneaux`) reste lisible.
    let old: Layout = toml::from_str("fiche = false\nhauteur_montage = 400.0").unwrap();
    let d = old.dock_layout();
    assert!(!d.contains(Pane::Sheet) && d.contains(Pane::Strip));
}
