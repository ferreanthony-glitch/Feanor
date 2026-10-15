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

/// Panneaux d'une disposition par défaut : tous, sauf les icônes des outils (barre verticale) et
/// la feuille d'exposition (ouverte à la demande, 13g).
fn default_panes() -> impl Iterator<Item = Pane> {
    Pane::ALL.into_iter().filter(|p| !matches!(p, Pane::Tools | Pane::XSheet))
}

#[test]
fn the_exposure_sheet_opens_next_to_the_layers() {
    let mut l = DockLayout::from_legacy(&Layout::default());
    assert_eq!(count(&l, Pane::XSheet), 0, "fermée par défaut");
    l.open(Pane::XSheet);
    assert_eq!(count(&l, Pane::XSheet), 1);
    assert!(format!("{:?}", l.principal).contains("Tabs { panneaux: [Layers, XSheet]"), "{:?}", l.principal);
    // Sans les calques : une colonne à elle.
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.close(Pane::Layers);
    l.open(Pane::XSheet);
    assert_eq!(count(&l, Pane::XSheet), 1);
    // Rouvrir le groupe des calques ne l'ouvre pas.
    l.close(Pane::XSheet);
    l.toggle_group(Group::Right);
    assert_eq!((count(&l, Pane::Layers), count(&l, Pane::XSheet)), (1, 0));
}

#[test]
fn default_layout_has_every_panel_once() {
    let l = DockLayout::from_legacy(&Layout::default());
    for pane in default_panes() {
        assert_eq!(count(&l, pane), 1, "{pane:?}");
    }
    assert_eq!(count(&l, Pane::Tools), 0, "les outils sont dans la barre verticale");
    assert_eq!(l.version, dock::LAYOUT_VERSION);
    // La bande en bas, sur toute la largeur ; au-dessus : dessin | calques et fiche | réglages
    // et couleur.
    let DockNode::Column { part, haut, bas } = &l.principal else { panic!("{:?}", l.principal) };
    assert_eq!(**bas, tabs(&[Pane::Strip]));
    assert!((part - (1.0 - 290.0 / 820.0)).abs() < 1e-4, "{part}");
    let DockNode::Row { gauche: center, droite: side, .. } = &**haut else { panic!() };
    assert_eq!(**center, tabs(&[Pane::Canvas]));
    let DockNode::Row { gauche: layers, droite: tools, .. } = &**side else { panic!() };
    assert_eq!(**layers, tabs(&[Pane::Layers]), "les calques sur toute la hauteur");
    for p in [Pane::ToolOptions, Pane::View, Pane::Guides, Pane::Color] {
        assert!(format!("{tools:?}").contains(&format!("{p:?}")), "{p:?} à droite");
    }
    // La fiche est un onglet des réglages (9b).
    assert!(format!("{tools:?}").contains("Tabs { panneaux: [ToolOptions, View, Guides, Sheet]"), "{tools:?}");
}

#[test]
fn layouts_from_before_9b_put_the_sheet_with_the_settings() {
    // Disposition 6a : la fiche sous les calques.
    let mut old = DockLayout::from_legacy(&Layout::default());
    old.close(Pane::Sheet);
    let mut v1 = old.clone();
    v1.version = 1;
    // On remet la fiche sous les calques, à la main.
    fn put_below(n: &mut DockNode) -> bool {
        match n {
            DockNode::Tabs { panneaux, .. } if panneaux == &vec![Pane::Layers] => {
                *n = DockNode::Column {
                    part: 0.6,
                    haut: Box::new(DockNode::Tabs { panneaux: vec![Pane::Layers], actif: 0 }),
                    bas: Box::new(DockNode::Tabs { panneaux: vec![Pane::Sheet], actif: 0 }),
                };
                true
            }
            DockNode::Tabs { .. } => false,
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => put_below(a) || put_below(b),
        }
    }
    assert!(put_below(&mut v1.principal));
    let l = Layout { panneaux: Some(v1), ..Layout::default() };
    assert!(!l.is_before_6a());
    let d = l.dock_layout();
    assert_eq!(d.version, dock::LAYOUT_VERSION);
    let s = format!("{:?}", d.principal);
    assert!(s.contains("Tabs { panneaux: [ToolOptions, View, Guides, Sheet]"), "{s}");
    assert_eq!(count(&d, Pane::Sheet), 1);
}

#[test]
fn the_brushes_panel_sits_with_the_colour() {
    // Disposition par défaut : couleur et pinceaux en onglets (11c).
    let d = DockLayout::from_legacy(&Layout::default());
    assert!(format!("{:?}", d.principal).contains("Tabs { panneaux: [Color, Brushes]"), "{:?}", d.principal);
    // Disposition enregistrée avant 11c (sans le panneau) : il arrive à côté de la couleur.
    let mut v2 = d.clone();
    v2.close(Pane::Brushes);
    v2.version = 2;
    let l = Layout { panneaux: Some(v2), ..Layout::default() };
    let up = l.dock_layout();
    assert_eq!(up.version, dock::LAYOUT_VERSION);
    assert_eq!(count(&up, Pane::Brushes), 1);
    assert!(format!("{:?}", up.principal).contains("Tabs { panneaux: [Color, Brushes]"), "{:?}", up.principal);
    // Fermé puis rouvert : il revient avec la couleur ; fermé par l'utilisateur, il le reste.
    let mut closed = up.clone();
    closed.close(Pane::Brushes);
    assert!(!closed.clone().upgraded().contains(Pane::Brushes), "fermé par l'utilisateur : il le reste");
    closed.open(Pane::Brushes);
    assert!(format!("{:?}", closed.principal).contains("Tabs { panneaux: [Color, Brushes]"));
}

#[test]
fn layouts_from_before_6a_lose_the_tools_panel() {
    let mut old = DockLayout::from_legacy(&Layout::default());
    old.open(Pane::Tools);
    old.version = 0;
    let l = Layout { panneaux: Some(old), ..Layout::default() };
    assert!(l.is_before_6a());
    let d = l.dock_layout();
    assert!(!d.contains(Pane::Tools) && d.version == dock::LAYOUT_VERSION);
    // Les outils en panneau restent possibles : rouverts au-dessus des réglages.
    let mut again = d.clone();
    again.open(Pane::Tools);
    assert_eq!(count(&again, Pane::Tools), 1);
}

#[test]
fn builtin_workspaces_keep_their_meaning() {
    let all = workspace::builtin();
    let dessin = all[1].2.dock_layout();
    assert!(!dessin.contains(Pane::Sheet) && !dessin.contains(Pane::Layers) && dessin.contains(Pane::ToolOptions));
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
    // Un groupe de plusieurs onglets dont le deuxième est ouvert.
    fn second_tab(n: &mut DockNode) -> bool {
        match n {
            DockNode::Tabs { panneaux, actif } if panneaux.len() >= 2 => {
                *actif = 1;
                true
            }
            DockNode::Tabs { .. } => false,
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => {
                second_tab(a) || second_tab(b)
            }
        }
    }
    assert!(second_tab(&mut l.principal));
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
    l.close(Pane::Sheet);
    assert!(!l.contains(Pane::Sheet) && l.contains(Pane::Layers));
    l.open(Pane::Sheet);
    // Revenue avec les onglets des réglages (pas avec la couleur).
    assert!(format!("{:?}", l.principal).contains("Tabs { panneaux: [ToolOptions, View, Guides, Sheet]"), "{:?}", l.principal);
    // Plus rien du groupe : une colonne recréée à droite du dessin.
    l.close(Pane::Layers);
    l.open(Pane::Layers);
    let s = format!("{:?}", l.principal);
    assert!(s.contains("Row { part: 0.78, gauche: Tabs { panneaux: [Canvas]"), "{s}");
    // La bande revient en bas de tout.
    l.close(Pane::Strip);
    l.open(Pane::Strip);
    assert!(matches!(&l.principal, DockNode::Column { bas, .. } if **bas == tabs(&[Pane::Strip])));
    for pane in default_panes() {
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
        version: dock::LAYOUT_VERSION,
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
fn the_vertical_tool_bar_has_every_tool_once() {
    let mut all: Vec<crate::app::Tool> = crate::toolbars::TOOL_GROUPS.iter().flat_map(|g| g.iter().copied()).collect();
    assert_eq!(all.len(), crate::app::Tool::ALL.len());
    for t in crate::app::Tool::ALL {
        assert_eq!(all.iter().filter(|x| **x == t).count(), 1, "{}", t.label());
    }
    all.dedup();
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
