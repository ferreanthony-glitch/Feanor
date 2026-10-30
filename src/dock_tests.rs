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

/// Panneaux d'une disposition par défaut : tous, sauf les icônes des outils (barre verticale), la
/// feuille d'exposition (13g), l'historique, les commentaires, les bruitages, les suggestions sonores et
/// les références (ouverts à la demande).
fn default_panes() -> impl Iterator<Item = Pane> {
    Pane::ALL.into_iter().filter(|p| {
        !matches!(
            p,
            Pane::Tools | Pane::XSheet | Pane::History | Pane::Comments | Pane::Bruitages | Pane::SuggestionsSonores | Pane::References | Pane::Vignettes | Pane::MonPanneau | Pane::Scenario
        )
    })
}

#[test]
fn the_history_opens_as_a_tab_with_the_layers() {
    let mut l = DockLayout::from_legacy(&Layout::default());
    assert_eq!(count(&l, Pane::History), 0, "fermé par défaut");
    l.open(Pane::History);
    assert_eq!(count(&l, Pane::History), 1);
    assert!(format!("{:?}", l.principal).contains("Tabs { panneaux: [Layers, History]"), "{:?}", l.principal);
    // Rouvrir le groupe des calques ne l'ouvre pas.
    l.close(Pane::History);
    l.close(Pane::Layers);
    l.close(Pane::Effects);
    l.toggle_group(Group::Right);
    assert_eq!((count(&l, Pane::Layers), count(&l, Pane::History)), (1, 0));
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
    l.close(Pane::Effects);
    l.toggle_group(Group::Right);
    assert_eq!((count(&l, Pane::Layers), count(&l, Pane::XSheet), count(&l, Pane::Effects)), (1, 0, 1));
}

#[test]
fn only_the_exposure_sheet_is_animation_only() {
    for p in Pane::ALL {
        assert_eq!(p.animation_only(), p == Pane::XSheet, "{p:?}");
    }
}

#[test]
fn the_storyboard_layout_leaves_out_the_exposure_sheet() {
    let mut l = DockLayout::from_legacy(&Layout::default());
    // Fermée : rien à retirer.
    assert!(l.sans_animation_seule().is_none());
    l.open(Pane::XSheet);
    let sans = l.sans_animation_seule().expect("la feuille était ouverte");
    assert_eq!(count(&sans, Pane::XSheet), 0);
    // Le reste de la disposition n'a pas bougé.
    for p in default_panes() {
        assert_eq!(count(&sans, p), 1, "{p:?}");
    }
    // Détachée dans sa fenêtre : retirée aussi.
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.open(Pane::XSheet);
    l.detach(Pane::XSheet);
    assert_eq!(count(&l, Pane::XSheet), 1);
    assert_eq!(count(&l.sans_animation_seule().unwrap(), Pane::XSheet), 0);
}

#[test]
fn back_in_animation_the_exposure_sheet_returns_where_it_was() {
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.open(Pane::XSheet);
    // On l'a rangée ailleurs : détachée dans sa fenêtre.
    let mut saved = l.clone();
    saved.detach(Pane::XSheet);
    let storyboard = saved.sans_animation_seule().unwrap();
    // Rien n'a bougé en storyboard : on retrouve la disposition d'avant, à l'identique.
    assert_eq!(storyboard.retour_animation(&saved), saved);
    // Un panneau fermé entre-temps : la feuille se rouvre à sa place d'ordinaire, le reste est gardé.
    let mut changed = storyboard.clone();
    changed.close(Pane::Sheet);
    let back = changed.retour_animation(&saved);
    assert_eq!(count(&back, Pane::XSheet), 1);
    assert_eq!(count(&back, Pane::Sheet), 0, "ce qu'on a fermé reste fermé");
    assert!(format!("{:?}", back.principal).contains("Tabs { panneaux: [Layers, XSheet]"), "{:?}", back.principal);
    // Une feuille qui n'était pas ouverte ne s'ouvre pas.
    let closed = DockLayout::from_legacy(&Layout::default());
    assert_eq!(closed.retour_animation(&closed), closed);
    // Déjà ouverte (l'utilisatrice l'a rouverte) : rien à faire.
    assert_eq!(l.retour_animation(&l), l);
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
    // Une seule bande (le montage en storyboard, la timeline en animation), à 290 points.
    assert_eq!(**bas, tabs(&[Pane::Montage]));
    assert!((part - (1.0 - 290.0 / 820.0)).abs() < 1e-4, "{part}");
    let DockNode::Row { gauche: center, droite: side, .. } = &**haut else { panic!() };
    assert_eq!(**center, tabs(&[Pane::Canvas]));
    let DockNode::Row { gauche: layers, droite: tools, .. } = &**side else { panic!() };
    // Les calques, les effets dessous (disposition 8, choix d'Anthony).
    assert!(matches!(&**layers, DockNode::Column { haut, bas, .. } if **haut == tabs(&[Pane::Layers]) && **bas == tabs(&[Pane::Effects])), "{layers:?}");
    for p in [Pane::ToolOptions, Pane::Sheet, Pane::Color] {
        assert!(format!("{tools:?}").contains(&format!("{p:?}")), "{p:?} à droite");
    }
    // La fiche est un onglet des réglages (9b).
    assert!(format!("{tools:?}").contains("Tabs { panneaux: [ToolOptions, Sheet]"), "{tools:?}");
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
    assert!(s.contains("Tabs { panneaux: [ToolOptions, Sheet]"), "{s}");
    assert_eq!(count(&d, Pane::Sheet), 1);
}

#[test]
fn there_is_no_brushes_tab_in_the_default_layouts_and_the_colour_keeps_its_tabs() {
    // Plus de panneau Pinceaux (2026-10-24) : la couleur et les personnages restent en onglets, et le panneau
    // n'est dans aucune disposition d'origine.
    let d = DockLayout::from_legacy(&Layout::default());
    assert!(format!("{:?}", d.principal).contains("Tabs { panneaux: [Color, Personnages]"), "{:?}", d.principal);
    for l in [d, DockLayout::pour_les_vignettes(), DockLayout::pour_le_montage(290.0)] {
        assert_eq!(count(&l, Pane::Brushes), 0, "{:?}", l.principal);
    }
    for e in crate::workspace::Espace::TOUS {
        assert_eq!(count(&e.defaut().dock_layout(), Pane::Brushes), 0, "{e:?}");
    }
    assert!(!Pane::ALL.contains(&Pane::Brushes));
    // Rouvrir « le panneau » ouvre les réglages de l'outil, où la liste des brosses est maintenant.
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.close(Pane::ToolOptions);
    l.open(Pane::Brushes);
    assert_eq!((count(&l, Pane::Brushes), count(&l, Pane::ToolOptions)), (0, 1));
    // Les personnages se rouvrent comme avant, avec la couleur.
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.close(Pane::Personnages);
    l.open(Pane::Personnages);
    assert!(format!("{:?}", l.principal).contains("Tabs { panneaux: [Color, Personnages]"), "{:?}", l.principal);
}

#[test]
fn a_saved_layout_that_still_has_the_brushes_tab_loads_without_it() {
    // Une disposition d'avant : couleur, pinceaux, personnages en onglets, le panneau des pinceaux actif.
    let mut old = DockLayout::from_legacy(&Layout::default());
    fn with_brushes(n: &mut DockNode) {
        match n {
            DockNode::Tabs { panneaux, actif } => {
                if let Some(i) = panneaux.iter().position(|p| *p == Pane::Color) {
                    panneaux.insert(i + 1, Pane::Brushes);
                    *actif = i + 1;
                }
            }
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => {
                with_brushes(a);
                with_brushes(b);
            }
        }
    }
    with_brushes(&mut old.principal);
    assert_eq!(count(&old, Pane::Brushes), 1);
    // Le fichier garde le nom d'avant (« pinceaux ») : il se lit encore.
    let text = serde_json::to_string(&old).expect("écrite");
    assert!(text.contains("\"pinceaux\""), "{text}");
    let back: DockLayout = serde_json::from_str(&text).expect("lue");
    assert_eq!(back, old);
    // Chargée comme les espaces de travail la chargent : l'onglet part, la couleur et les personnages restent, et
    // l'onglet actif est un onglet qui existe.
    let l = Layout { panneaux: Some(back), ..Layout::default() }.dock_layout();
    assert_eq!(count(&l, Pane::Brushes), 0);
    assert!(format!("{:?}", l.principal).contains("Tabs { panneaux: [Color, Personnages], actif: 0 }"), "{:?}", l.principal);
    // Dans l'état d'egui_dock non plus.
    let state = dock::to_state(&l);
    assert!(state.find_tab(&Pane::Brushes).is_none());
    assert!(state.find_tab(&Pane::Color).is_some() && state.find_tab(&Pane::ToolOptions).is_some());
    // Une version d'avant 11c (3) ou d'après : pas de panneau rouvert « à côté de la couleur ».
    let mut v2 = DockLayout::from_legacy(&Layout::default());
    v2.version = 2;
    assert_eq!(count(&v2.upgraded(), Pane::Brushes), 0);
}

#[test]
fn the_brushes_tab_leaves_no_hole_when_it_was_alone_floating_or_in_its_own_window() {
    let canvas = tabs(&[Pane::Canvas]);
    // Seul dans sa colonne : la colonne disparaît, les deux autres parties se rejoignent.
    let col = |a: DockNode, b: DockNode| DockNode::Column { part: 0.5, haut: Box::new(a), bas: Box::new(b) };
    let row = |a: DockNode, b: DockNode| DockNode::Row { part: 0.7, gauche: Box::new(a), droite: Box::new(b) };
    let old = DockLayout {
        version: dock::LAYOUT_VERSION,
        principal: row(canvas.clone(), col(tabs(&[Pane::ToolOptions]), col(tabs(&[Pane::Brushes]), tabs(&[Pane::Color])))),
        fenetres: vec![
            FloatingPanel { panneaux: tabs(&[Pane::Brushes]), x: 10.0, y: 10.0, largeur: 300.0, hauteur: 300.0 },
            FloatingPanel { panneaux: tabs(&[Pane::Brushes, Pane::Layers]), x: 40.0, y: 40.0, largeur: 300.0, hauteur: 300.0 },
        ],
        detachees: vec![dock::DetachedPanel { panneau: Pane::Brushes, x: 0.0, y: 0.0, largeur: 0.0, hauteur: 0.0 }],
    };
    let l = old.sanitized();
    assert_eq!(count(&l, Pane::Brushes), 0);
    assert_eq!(l.principal, row(canvas, col(tabs(&[Pane::ToolOptions]), tabs(&[Pane::Color]))), "pas de trou : {:?}", l.principal);
    // La fenêtre flottante qui n'avait que lui disparaît ; l'autre garde ses calques.
    assert_eq!(l.fenetres.len(), 1);
    assert_eq!(l.fenetres[0].panneaux, tabs(&[Pane::Layers]));
    assert!(l.detachees.is_empty());
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
    assert!(ensemble.contains(Pane::Canvas) && ensemble.contains(Pane::Montage));
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
    l.close(Pane::Sheet);
    l.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::Sheet]), x: 300.0, y: 200.0, largeur: 260.0, hauteur: 320.0 });
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
    assert!(format!("{:?}", l.principal).contains("Tabs { panneaux: [ToolOptions, Sheet]"), "{:?}", l.principal);
    // Les calques rouverts : au-dessus des effets restés à leur place.
    l.close(Pane::Layers);
    l.open(Pane::Layers);
    let s = format!("{:?}", l.principal);
    assert!(s.contains("Column { part: 0.6, haut: Tabs { panneaux: [Layers], actif: 0 }, bas: Tabs { panneaux: [Effects]"), "{s}");
    // Plus rien du groupe : une colonne recréée à droite du dessin.
    l.close(Pane::Layers);
    l.close(Pane::Effects);
    l.open(Pane::Layers);
    let s = format!("{:?}", l.principal);
    assert!(s.contains("Row { part: 0.78, gauche: Tabs { panneaux: [Canvas]"), "{s}");
    l.open(Pane::Effects);
    // La bande revient en bas de tout ; l'ancienne timeline à part ne s'ouvre plus, c'est la bande.
    l.close(Pane::Montage);
    assert!(!l.contains(Pane::Montage));
    l.open(Pane::Timeline);
    assert!(matches!(&l.principal, DockNode::Column { bas, .. } if **bas == tabs(&[Pane::Montage])), "{:?}", l.principal);
    assert_eq!(count(&l, Pane::Timeline), 0);
    for pane in default_panes() {
        assert_eq!(count(&l, pane), 1, "{pane:?}");
    }
    l.toggle_group(Group::Bottom);
    assert!(!l.contains(Pane::Montage));
    l.toggle_group(Group::Bottom);
    assert!(matches!(&l.principal, DockNode::Column { bas, .. } if **bas == tabs(&[Pane::Montage])), "{:?}", l.principal);
}

#[test]
fn layouts_with_a_separate_timeline_come_back_to_one_band() {
    // Version 3 : la bande montrait le montage ou la timeline selon le mode.
    let old = DockLayout {
        version: 3,
        principal: DockNode::Column { part: 0.7, haut: Box::new(tabs(&[Pane::Canvas])), bas: Box::new(tabs(&[Pane::Montage, Pane::XSheet])) },
        fenetres: Vec::new(),
        detachees: Vec::new(),
    };
    let l = old.upgraded();
    assert_eq!(l.version, dock::LAYOUT_VERSION);
    assert_eq!((count(&l, Pane::Montage), count(&l, Pane::Timeline)), (1, 0), "version 3 : rien ne change");
    // Version 4 : l'un sous l'autre, la bande haute de 420 points ; une seule bande, à 290 points.
    let v4 = DockLayout {
        version: 4,
        principal: DockNode::Column {
            part: 1.0 - 420.0 / 820.0,
            haut: Box::new(tabs(&[Pane::Canvas])),
            bas: Box::new(DockNode::Column { part: 0.5, haut: Box::new(tabs(&[Pane::Montage])), bas: Box::new(tabs(&[Pane::Timeline])) }),
        },
        fenetres: Vec::new(),
        detachees: Vec::new(),
    };
    let l = v4.upgraded();
    let DockNode::Column { part, bas, .. } = &l.principal else { panic!("{:?}", l.principal) };
    assert_eq!(**bas, tabs(&[Pane::Montage]));
    assert!((part - (1.0 - 290.0 / 820.0)).abs() < 1e-4, "{part}");
    // Version 5 : en onglets ; la timeline part. Seule en bas : le montage revient à sa place.
    let v5 = DockLayout {
        version: 5,
        principal: DockNode::Column { part: 0.7, haut: Box::new(tabs(&[Pane::Canvas])), bas: Box::new(tabs(&[Pane::Montage, Pane::Timeline])) },
        fenetres: Vec::new(),
        detachees: Vec::new(),
    };
    assert!(matches!(&v5.upgraded().principal, DockNode::Column { bas, .. } if **bas == tabs(&[Pane::Montage])));
    let alone = DockLayout {
        version: 5,
        principal: DockNode::Column { part: 0.7, haut: Box::new(tabs(&[Pane::Canvas])), bas: Box::new(tabs(&[Pane::Timeline])) },
        fenetres: Vec::new(),
        detachees: Vec::new(),
    };
    let l = alone.upgraded();
    assert_eq!((count(&l, Pane::Montage), count(&l, Pane::Timeline)), (1, 0));
    // Le nom enregistré du montage ne change pas (« bande »), la timeline a le sien.
    assert_eq!(serde_json::to_string(&Pane::Montage).unwrap(), "\"bande\"");
    assert_eq!(serde_json::to_string(&Pane::Timeline).unwrap(), "\"timeline\"");
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
    let mut all: Vec<crate::app::Tool> = crate::toolbars::TOOL_GROUPS.iter().flat_map(|g| g.iter().flat_map(|s| s.iter().copied())).collect();
    assert_eq!(all.len(), crate::app::Tool::ALL.len());
    for t in crate::app::Tool::ALL {
        assert_eq!(all.iter().filter(|x| **x == t).count(), 1, "{}", t.label());
    }
    all.dedup();
}

#[test]
fn grouped_tools_share_one_slot() {
    use crate::app::Tool;
    use crate::toolbars::{slot_tool, TOOL_GROUPS};
    // Anthony : la sélection au-dessus du pinceau, avec la sélection par couleur ; les formes dans
    // une seule case, au rectangle.
    let first = TOOL_GROUPS[0];
    assert_eq!(first[0], &[Tool::Select, Tool::SelectColor][..]);
    // (Le Mélangeur, le pinceau en « Lisser », partage la case du pinceau : 2026-10-28.)
    assert_eq!(first[1], &[Tool::Brush, Tool::Mixer][..]);
    let shapes = TOOL_GROUPS.iter().flat_map(|g| g.iter()).find(|s| s.contains(&Tool::Line)).expect("case des formes");
    assert_eq!(shapes.len(), 4);
    assert_eq!(shapes[0], Tool::Rect, "le rectangle au début");
    // Outil montré : le courant, sinon le plus récent de la case, sinon le premier.
    assert_eq!(slot_tool(shapes, Tool::Brush, &[]), Tool::Rect);
    assert_eq!(slot_tool(shapes, Tool::Ellipse, &[Tool::Ellipse]), Tool::Ellipse);
    assert_eq!(slot_tool(shapes, Tool::Brush, &[Tool::Brush, Tool::Polyline, Tool::Line]), Tool::Polyline);
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
    l.close(Pane::Sheet);
    let bad = dock::DetachedPanel { panneau: Pane::Sheet, x: 10.0, y: 10.0, largeur: 5.0, hauteur: f32::NAN };
    l.detachees.push(bad.clone());
    let l = l.sanitized();
    assert_eq!(count(&l, Pane::Sheet), 1);
    assert_eq!((l.detachees[0].largeur, l.detachees[0].hauteur), (0.0, 0.0));
    // En double avec un onglet : l'onglet l'emporte.
    let mut twice = DockLayout::from_legacy(&Layout::default());
    twice.detachees.push(bad);
    let twice = twice.sanitized();
    assert_eq!(count(&twice, Pane::Sheet), 1);
    assert!(twice.detachees.is_empty());
}

#[test]
fn panel_layouts_are_kept_in_the_settings() {
    let mut s = Settings::default();
    let mut l = DockLayout::from_legacy(&Layout::default());
    l.close(Pane::Sheet);
    l.fenetres.push(FloatingPanel { panneaux: tabs(&[Pane::Sheet]), x: 40.0, y: 50.0, largeur: 300.0, hauteur: 200.0 });
    s.layout = Some(Layout { panneaux: Some(l.clone()), ..Layout::default() });
    workspace::save_workspace(&mut s.workspaces, "Le mien", Layout { panneaux: Some(l.clone()), ..Layout::default() });
    let text = toml::to_string(&s).unwrap();
    let back: Settings = toml::from_str(&text).unwrap();
    assert!(back.layout.as_ref().unwrap().dock_layout().close_to(&l), "{text}");
    assert!(back.workspaces[0].layout.dock_layout().close_to(&l));
    // Une disposition enregistrée avant 5b (sans `panneaux`) reste lisible.
    let old: Layout = toml::from_str("fiche = false\nhauteur_montage = 400.0").unwrap();
    let d = old.dock_layout();
    assert!(!d.contains(Pane::Sheet) && d.contains(Pane::Montage));
}

#[test]
fn the_effects_pane_comes_under_the_layers() {
    // Anthony : « A, avec les Effets sous les Calques » : calques et effets dessous, puis
    // réglages et couleur dessous. Une disposition de ce jour (6 ou 7) prend la nouvelle.
    let v6 = DockLayout {
        version: 6,
        principal: DockNode::Row { part: 0.8, gauche: Box::new(tabs(&[Pane::Canvas])), droite: Box::new(tabs(&[Pane::ToolOptions, Pane::Guides])) },
        fenetres: Vec::new(),
        detachees: Vec::new(),
    };
    let l = v6.upgraded();
    assert_eq!(l.principal, DockLayout::from_legacy(&Layout::default()).principal);
    let under = |l: &DockLayout| format!("{:?}", l.principal).contains("haut: Tabs { panneaux: [Layers], actif: 0 }, bas: Tabs { panneaux: [Effects]");
    assert!(under(&l), "{:?}", l.principal);
    for pane in default_panes() {
        assert_eq!(count(&l, pane), 1, "{pane:?}");
    }
    // Fermé puis rouvert : il revient sous les calques.
    let mut again = l.clone();
    again.close(Pane::Effects);
    assert!(!again.contains(Pane::Effects));
    again.open(Pane::Effects);
    assert!(under(&again), "{:?}", again.principal);
}
#[test]
fn the_old_view_pane_disappears_from_saved_layouts() {
    // Anthony : « ajoute les outils de la fenêtre Vue sous l'espace de travail. La fenêtre Vue
    // devient inutile » : ses outils sont sous le dessin ; le panneau part des dispositions.
    let saved = DockLayout {
        version: dock::LAYOUT_VERSION,
        principal: DockNode::Row { part: 0.8, gauche: Box::new(tabs(&[Pane::Canvas])), droite: Box::new(tabs(&[Pane::ToolOptions, Pane::View])) },
        fenetres: Vec::new(),
        detachees: Vec::new(),
    };
    let l = saved.sanitized();
    assert_eq!(count(&l, Pane::View), 0);
    assert_eq!(count(&l, Pane::ToolOptions), 1);
    assert!(!Pane::ALL.contains(&Pane::View), "plus dans la liste des panneaux");
    assert_eq!(count(&DockLayout::from_legacy(&Layout::default()), Pane::View), 0);
}