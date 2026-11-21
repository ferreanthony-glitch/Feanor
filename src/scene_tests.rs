//! Essais du panneau Scène (partie 197, `scene.rs`, `sceneui.rs`, `docs/INTERFACE.md`) : la pile d'origine du Storyboard, les trois
//! « Depuis » et le fil d'Ariane, l'arbre face à la colonne de la timeline, les dossiers, le calque courant (aussi d'un autre plan),
//! l'œil et le cadenas dans les deux sens, la ligne complète du calque courant, le menu du clic droit, la liste qui ne peint que ce
//! qu'on voit dans un grand projet, l'arbre qui n'est pas refait sans raison, le choix gardé dans les réglages. Dans la vraie fenêtre
//! sans écran (`Banc`).

use eframe::egui::{epaint::Shape, pos2, Modifiers, Rect};

use crate::bulles::vus;
use crate::colonne::{self, PanneauPile};
use crate::document::{Document, Layer, LayerKind};
use crate::dock::Pane;
use crate::ligne_sequence_tests::clic_droit;
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::clic;
use crate::scene::{self, ChoixScene, Ou, Quoi, Racine};
use crate::sceneui::LIGNES_SCENE;
use crate::workspace::Espace;

/// Un film de deux séquences : la première de deux plans (le premier a un dossier « Perso » avec deux calques, et deux calques
/// dehors), la seconde d'un plan de deux calques.
fn film() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.add_scene_after(0);
    doc.add_sequence_after(1);
    let calques = |noms: &[&str]| noms.iter().rev().map(|n| Layer::new(*n, LayerKind::Vector)).collect::<Vec<_>>();
    // Plan 1 : de haut en bas, « Fond », « Perso » (dossier : « Tête », « Corps »), « Notes ».
    let mut p1 = calques(&["Fond", "Tête", "Corps", "Notes"]);
    let mut dossier = Layer::new("Perso", LayerKind::Vector);
    dossier.folder = true;
    let id = dossier.id;
    // (Du bas vers le haut : Notes, Corps, Tête, Perso, Fond ; le dossier au-dessus de ses calques.)
    p1[1].parent = Some(id);
    p1[2].parent = Some(id);
    p1.insert(3, dossier);
    doc.panels[0].layers = p1;
    doc.panels[1].layers = calques(&["Ciel", "Rue"]);
    doc.panels[2].layers = calques(&["Mer", "Plage"]);
    for p in &mut doc.panels {
        p.set_length(24);
    }
    doc
}

fn banc_dans(doc: Document, espace: Espace) -> Banc {
    let mut b = banc_taille(doc, false, 1600, 1200);
    b.app.set_espace(espace);
    if espace == Espace::Animation {
        b.app.enter_animation(0);
    }
    if !b.app.colonne.contient(Pane::Scene) {
        b.app.colonne.panneaux.insert(0, PanneauPile::simple(Pane::Scene));
    }
    // (Assez haut pour tout l'arbre du petit film ; la colonne défile au besoin, partie 196.5.)
    let k = b.app.colonne.rang_de(Pane::Scene).unwrap();
    b.app.colonne.panneaux[k].hauteur = Some(420.0);
    b.garder_formes = true;
    (b.app.cur_panel, b.app.cur_layer) = (0, 0);
    b.chauffer(8);
    b
}

/// Les lignes que le panneau a peintes à la dernière image.
fn lignes() -> Vec<(Quoi, Rect)> {
    LIGNES_SCENE.with(|l| l.borrow().clone())
}

/// La ligne du calque `id` du plan `pi`.
fn ligne_du_calque(pi: usize, id: u64) -> Rect {
    lignes().into_iter().find(|(q, _)| matches!(q, Quoi::Calque { pi: p, id: i, .. } if *p == pi && *i == id)).map(|(_, r)| r).unwrap_or_else(|| panic!("la ligne du calque {id} du plan {pi} : {:?}", lignes()))
}

fn id_de(b: &Banc, pi: usize, nom: &str) -> u64 {
    b.app.doc.panels[pi].layers.iter().find(|l| l.name == nom).unwrap_or_else(|| panic!("« {nom} »")).id
}

fn rang_de(b: &Banc, pi: usize, nom: &str) -> usize {
    b.app.doc.panels[pi].layers.iter().position(|l| l.name == nom).unwrap()
}

/// Le nom d'une bulle du panneau dans le registre des essais (`sceneui::PREFIXE_DES_ESSAIS`).
fn ici(nom: &str) -> String {
    format!("{}{nom}", crate::sceneui::PREFIXE_DES_ESSAIS)
}

/// Le seul élément du panneau nommé `nom` dans `r`.
fn dans(nom: &str, r: Rect) -> Rect {
    let nom = ici(nom);
    let v: Vec<Rect> = vus::ou(&nom).into_iter().filter(|x| r.contains(x.center())).collect();
    assert_eq!(v.len(), 1, "« {nom} » dans {r:?} : {:?}", vus::ou(&nom));
    v[0]
}

/// Les textes peints dans `r`, et où.
fn textes_dans(b: &Banc, r: Rect) -> Vec<(String, Rect)> {
    fn chercher(s: &Shape, r: Rect, out: &mut Vec<(String, Rect)>) {
        match s {
            Shape::Text(t) if r.contains(t.visual_bounding_rect().center()) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, r, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, r, &mut out);
    }
    out
}

fn mots_dans(b: &Banc, r: Rect) -> Vec<String> {
    textes_dans(b, r).into_iter().map(|t| t.0).collect()
}

fn racine(b: &mut Banc, r: Racine) {
    b.app.settings.scene.racine = r;
    b.chauffer(3);
}

// ------------------------------------------------------------------------------------------------ la pile d'origine

#[test]
fn the_storyboard_starts_with_scene_colour_and_properties_and_saved_stacks_are_left_alone() {
    let c = Espace::Storyboard.defaut().dock_layout().colonne;
    let titres: Vec<Vec<Pane>> = c.panneaux.iter().map(|p| p.panneaux.clone()).collect();
    assert_eq!(titres[0], vec![Pane::Scene], "la Scène en haut : {titres:?}");
    assert!(titres[1].contains(&Pane::Color), "puis la Couleur : {titres:?}");
    assert!(c.panneaux[2].est_proprietes() && c.panneaux.len() == 3, "puis Propriétés : {titres:?}");
    assert_eq!(c.qui_grandit(), Some(2), "Propriétés grandit toujours");
    // L'Animation ne l'a pas d'office ; « + Panneau » la propose dans le Storyboard et l'Animation, pas dans les Vignettes ni le Montage.
    assert!(!Espace::Animation.defaut().dock_layout().colonne.contient(Pane::Scene));
    for (espace, oui) in [(Espace::Storyboard, true), (Espace::Animation, true), (Espace::Vignettes, false), (Espace::Montage, false)] {
        let p = colonne::propositions(espace, espace == Espace::Animation, false, true, &|_| false);
        assert_eq!(p.iter().any(|x| x.pane == Some(Pane::Scene)), oui, "{espace:?}");
    }
    // Une pile du Storyboard déjà enregistrée (celle d'avant, sans la Scène) ne la reçoit pas en se mettant à jour.
    let mut avant = Espace::Storyboard.defaut().dock_layout();
    avant.colonne.panneaux.remove(0);
    let relue: crate::dock::DockLayout = serde_json::from_str(&serde_json::to_string(&avant).unwrap()).unwrap();
    assert!(!relue.upgraded().colonne.contient(Pane::Scene), "pas de force dans une pile enregistrée");
    // « Rétablir la disposition » la donne.
    let mut b = banc_taille(film(), false, 1600, 1000);
    b.app.set_espace(Espace::Storyboard);
    b.app.colonne.panneaux.retain(|p| !p.panneaux.contains(&Pane::Scene));
    b.chauffer(3);
    assert!(!b.app.colonne.contient(Pane::Scene));
    b.app.run_action("espace.retablir");
    b.chauffer(4);
    assert_eq!(b.app.colonne.panneaux[0].panneaux, vec![Pane::Scene], "rétablie : la Scène en haut");
    // Et « + Panneau » l'ajoute dans l'Animation : un panneau à elle, pas un onglet de Propriétés.
    let mut c = Espace::Animation.defaut().dock_layout().colonne;
    c.ajouter(Pane::Scene);
    assert!(c.contient(Pane::Scene) && !c.en_onglet_de_proprietes(Pane::Scene));
}

// ------------------------------------------------------------------------------------------------ Depuis, le fil d'Ariane

#[test]
fn the_three_roots_show_the_plan_the_sequence_or_the_film_with_their_breadcrumb() {
    let mut b = banc_dans(film(), Espace::Storyboard);
    let doc = &b.app.doc;
    assert_eq!(scene::fil(doc, Racine::Film, 0), vec!["Film"]);
    assert_eq!(scene::fil(doc, Racine::Sequence, 0), vec!["Film".to_string(), format!("Séquence {}", doc.sequence_label(doc.sequence_of(0)))]);
    let f = scene::fil(doc, Racine::Plan, 0);
    assert_eq!(f.len(), 3);
    assert_eq!(f[2], crate::storyboard::panel_title(doc, 0));
    // Plan (d'origine) : les calques du plan courant, sa caméra ; ni séquence ni plan.
    assert_eq!(b.app.settings.scene.racine, Racine::Plan);
    let l = lignes();
    assert!(!l.is_empty() && l.iter().all(|(q, _)| matches!(q, Quoi::Calque { pi: 0, .. } | Quoi::Camera { pi: 0 })), "{l:?}");
    for r in Racine::TOUTES {
        assert_eq!(vus::ou(&ici(&format!("Depuis : {}", r.nom()))).len(), 1, "{r:?}");
    }
    let t = mots_dans(&b, Rect::EVERYTHING);
    // (Partie 206 : « › » est peint avec l'élément qui le suit, pour qu'ils passent à la ligne ensemble.)
    assert!(t.iter().any(|x| *x == format!("› {}", f[2])) && t.iter().any(|x| *x == format!("› {}", f[1])), "le fil d'Ariane est peint : {f:?} {t:?}");
    // Séquence : les deux plans de la séquence ; le courant déplié jusqu'à ses calques, l'autre replié.
    clic(&mut b, vus::ou(&ici("Depuis : Séquence"))[0].center(), Modifiers::NONE);
    assert_eq!(b.app.settings.scene.racine, Racine::Sequence, "le clic choisit la racine");
    let plans: Vec<usize> = lignes().iter().filter_map(|(q, _)| if let Quoi::Plan { pi } = q { Some(*pi) } else { None }).collect();
    assert_eq!(plans, vec![0, 1]);
    assert!(lignes().iter().any(|(q, _)| matches!(q, Quoi::Calque { pi: 0, .. })) && !lignes().iter().any(|(q, _)| matches!(q, Quoi::Calque { pi: 1, .. })));
    // Le second plan se déplie à son triangle.
    let p2 = lignes().into_iter().find(|(q, _)| *q == Quoi::Plan { pi: 1 }).unwrap().1;
    clic(&mut b, dans("Déplier le plan", p2).center(), Modifiers::NONE);
    assert!(lignes().iter().any(|(q, _)| matches!(q, Quoi::Calque { pi: 1, .. })), "le plan 2 déplié");
    assert_eq!(b.app.cur_panel, 0, "déplier ne change pas de plan");
    // Un clic sur le plan 2 y mène.
    let p2 = lignes().into_iter().find(|(q, _)| *q == Quoi::Plan { pi: 1 }).unwrap().1;
    clic(&mut b, pos2(p2.right() - 30.0, p2.center().y), Modifiers::NONE);
    assert_eq!(b.app.cur_panel, 1);
    // Film : les deux séquences ; la courante dépliée avec ses plans ; un clic sur l'autre mène à son premier plan.
    racine(&mut b, Racine::Film);
    let seqs: Vec<(usize, usize)> = lignes().iter().filter_map(|(q, _)| if let Quoi::Sequence { debut, n, .. } = q { Some((*debut, *n)) } else { None }).collect();
    assert_eq!(seqs, vec![(0, 2), (2, 1)]);
    assert!(!lignes().iter().any(|(q, _)| *q == Quoi::Plan { pi: 2 }), "l'autre séquence repliée");
    let s2 = lignes().into_iter().find(|(q, _)| matches!(q, Quoi::Sequence { debut: 2, .. })).unwrap().1;
    clic(&mut b, pos2(s2.right() - 20.0, s2.center().y), Modifiers::NONE);
    assert_eq!(b.app.cur_panel, 2, "son premier plan");
    assert!(lignes().iter().any(|(q, _)| *q == Quoi::Plan { pi: 2 }), "devenue courante, elle est dépliée");
    assert!(mots_dans(&b, Rect::EVERYTHING).iter().any(|x| x == "Film"));
}

// ------------------------------------------------------------------------------------------------ l'arbre face à la timeline

#[test]
fn the_tree_shows_what_the_timeline_column_shows_in_its_order() {
    // Storyboard : les rangées des pistes du plan courant (`montage_rows`), la caméra au-dessus.
    let b = banc_dans(film(), Espace::Storyboard);
    let rows = b.app.montage_rows(300.0);
    let layers = &b.app.doc.panels[0].layers;
    let timeline: Vec<usize> = rows.lanes.iter().filter_map(|l| crate::montagerows::layer_in_row(layers, l.row)).collect();
    let panneau: Vec<usize> = lignes().iter().filter_map(|(q, _)| if let Quoi::Calque { li, .. } = q { Some(*li) } else { None }).collect();
    assert_eq!(panneau, timeline, "le même ordre que les pistes");
    assert_eq!(lignes()[0].0, Quoi::Camera { pi: 0 }, "la caméra d'abord, comme dans le Storyboard");
    // Les calques d'un dossier sont en retrait.
    let arbre = scene::arbre(&b.app.doc, &ChoixScene::default(), b.app.scene_ou());
    let niveau = |nom: &str| arbre.iter().find(|l| matches!(l.quoi, Quoi::Calque { li, .. } if li == rang_de(&b, 0, nom))).unwrap().niveau;
    assert_eq!((niveau("Perso"), niveau("Tête"), niveau("Fond")), (0, 1, 0));
    // Animation : les lignes de sa colonne des calques, la caméra dessous.
    let a = banc_dans(film(), Espace::Animation);
    let timeline: Vec<usize> = a.app.lignes_de_la_timeline().iter().map(|r| r.li).collect();
    let panneau: Vec<usize> = a.app.scene_arbre.lignes.iter().filter_map(|l| if let Quoi::Calque { li, .. } = l.quoi { Some(li) } else { None }).collect();
    assert_eq!(panneau, timeline, "le même ordre que la colonne de l'Animation");
    assert_eq!(a.app.scene_arbre.lignes.last().unwrap().quoi, Quoi::Camera { pi: 0 }, "la caméra en bas, comme dans l'Animation");
    // Un groupe caché de la timeline de l'Animation : ses calques n'y ont pas de ligne, sauf le calque courant.
    let mut doc = film();
    doc.panels[0].layers[0].group = 1;
    doc.groups[1].shown = false;
    let ou = Ou { racine: Racine::Plan, sb: false, cur_panel: 0, cur_layer: 4 };
    let li: Vec<usize> = scene::arbre(&doc, &ChoixScene::default(), ou).iter().filter_map(|l| if let Quoi::Calque { li, .. } = l.quoi { Some(li) } else { None }).collect();
    assert!(!li.contains(&0), "le calque du groupe caché : {li:?}");
    let ou = Ou { cur_layer: 0, ..ou };
    let li: Vec<usize> = scene::arbre(&doc, &ChoixScene::default(), ou).iter().filter_map(|l| if let Quoi::Calque { li, .. } = l.quoi { Some(li) } else { None }).collect();
    assert!(li.contains(&0), "sauf s'il est le calque courant : {li:?}");
}

// ------------------------------------------------------------------------------------------------ les dossiers

#[test]
fn folders_fold_in_the_panel_and_in_the_timeline_together() {
    let mut b = banc_dans(film(), Espace::Storyboard);
    let (perso, tete) = (id_de(&b, 0, "Perso"), id_de(&b, 0, "Tête"));
    let fp = rang_de(&b, 0, "Perso");
    clic(&mut b, dans("Replier le dossier", ligne_du_calque(0, perso)).center(), Modifiers::NONE);
    assert!(b.app.doc.panels[0].layers[fp].collapsed, "le dossier est fermé dans le projet");
    assert!(!lignes().iter().any(|(q, _)| matches!(q, Quoi::Calque { id, .. } if *id == tete)), "ses calques n'ont plus de ligne");
    let rows = b.app.montage_rows(300.0);
    let layers = &b.app.doc.panels[0].layers;
    let k = rang_de(&b, 0, "Tête");
    assert!(!rows.lanes.iter().any(|l| crate::montagerows::layer_in_row(layers, l.row) == Some(k)), "ni de piste dans la timeline");
    // Et dans l'autre sens : la timeline le rouvre, le panneau suit.
    b.app.basculer_dossier(fp);
    b.chauffer(3);
    assert!(lignes().iter().any(|(q, _)| matches!(q, Quoi::Calque { id, .. } if *id == tete)), "rouvert depuis la timeline");
}

// ------------------------------------------------------------------------------------------------ le calque courant

#[test]
fn a_click_makes_a_layer_current_even_in_another_plan() {
    let mut b = banc_dans(film(), Espace::Storyboard);
    let fond = id_de(&b, 0, "Fond");
    let r = ligne_du_calque(0, fond);
    clic(&mut b, pos2(r.center().x, r.center().y), Modifiers::NONE);
    assert_eq!(b.app.cur_layer, rang_de(&b, 0, "Fond"), "{}", b.app.status);
    // Un calque d'un autre plan : on va à ce plan, il y devient le calque courant.
    racine(&mut b, Racine::Sequence);
    let id2 = b.app.doc.panels[1].id;
    b.app.settings.scene.plans_ouverts.push(id2);
    b.chauffer(3);
    let rue = id_de(&b, 1, "Rue");
    clic(&mut b, ligne_du_calque(1, rue).center(), Modifiers::NONE);
    assert_eq!((b.app.cur_panel, b.app.cur_layer), (1, rang_de(&b, 1, "Rue")));
    // La timeline le montre : la piste du calque actif est la sienne.
    assert_eq!(b.app.layers()[b.app.cur_layer].id, rue);
}

// ------------------------------------------------------------------------------------------------ l'œil et le cadenas

#[test]
fn eye_and_padlock_follow_the_timeline_both_ways_and_work_on_another_plan() {
    let mut b = banc_dans(film(), Espace::Storyboard);
    let fond = id_de(&b, 0, "Fond");
    let li = rang_de(&b, 0, "Fond");
    let e = b.app.history.undo_len();
    clic(&mut b, dans("Afficher / cacher", ligne_du_calque(0, fond)).center(), Modifiers::NONE);
    assert!(!b.app.doc.panels[0].layers[li].visible, "caché depuis le panneau");
    assert_eq!(b.app.history.undo_len(), e + 1, "une étape, comme dans la timeline");
    assert!(mots_dans(&b, ligne_du_calque(0, fond)).iter().any(|t| t == egui_phosphor::regular::EYE_SLASH), "l'œil barré dans le panneau");
    clic(&mut b, dans("Verrouiller / déverrouiller", ligne_du_calque(0, fond)).center(), Modifiers::NONE);
    assert!(b.app.doc.panels[0].layers[li].locked);
    // La timeline (son code, `agir_colonne`) : le panneau suit.
    b.app.agir_colonne(crate::layerheads::Colonne::Afficher, li);
    b.app.agir_colonne(crate::layerheads::Colonne::Verrouiller, li);
    b.chauffer(3);
    let t = mots_dans(&b, ligne_du_calque(0, fond));
    assert!(t.iter().any(|x| x == egui_phosphor::regular::EYE) && t.iter().any(|x| x == egui_phosphor::regular::LOCK_SIMPLE_OPEN), "{t:?}");
    // Un calque d'un autre plan : son œil se change sans quitter le plan courant.
    racine(&mut b, Racine::Sequence);
    let id2 = b.app.doc.panels[1].id;
    b.app.settings.scene.plans_ouverts.push(id2);
    b.chauffer(3);
    let ciel = id_de(&b, 1, "Ciel");
    clic(&mut b, dans("Afficher / cacher", ligne_du_calque(1, ciel)).center(), Modifiers::NONE);
    assert!(!b.app.doc.panels[1].layers[rang_de(&b, 1, "Ciel")].visible);
    assert_eq!(b.app.cur_panel, 0, "on reste sur le plan courant");
}

// ------------------------------------------------------------------------------------------------ la ligne complète du calque courant

#[test]
fn the_current_layer_unfolds_into_the_full_timeline_line() {
    let mut b = banc_dans(film(), Espace::Storyboard);
    let li = rang_de(&b, 0, "Fond");
    b.app.select_layer(li);
    b.chauffer(3);
    let r = ligne_du_calque(0, id_de(&b, 0, "Fond"));
    assert!((r.height() - crate::layerheads::ROW_FULL).abs() < 0.5, "la hauteur de la ligne complète : {r:?}");
    for nom in ["Opacité du calque", "Pelure d'oignon du calque", crate::accroche::NOM, "Commandes du calque", "Afficher / cacher"] {
        dans(nom, r);
    }
    // Ses réglages agissent : la pelure du calque, par le code de la timeline.
    let avant = b.app.doc.panels[0].layers[li].onion;
    clic(&mut b, dans("Pelure d'oignon du calque", r).center(), Modifiers::NONE);
    assert_ne!(b.app.doc.panels[0].layers[li].onion, avant, "{}", b.app.status);
    // Les autres calques gardent leur ligne fine.
    let autre = ligne_du_calque(0, id_de(&b, 0, "Notes"));
    assert!((autre.height() - scene::H_CALQUE).abs() < 0.5);
}

// ------------------------------------------------------------------------------------------------ le menu du clic droit

#[test]
fn right_click_opens_the_timeline_layer_menu() {
    let mut b = banc_dans(film(), Espace::Storyboard);
    let notes = id_de(&b, 0, "Notes");
    clic_droit(&mut b, ligne_du_calque(0, notes).center());
    let t = mots_dans(&b, Rect::EVERYTHING).join("|");
    for mot in ["Dupliquer", "Renommer…", "Supprimer", "Nouveau calque"] {
        assert!(t.contains(mot), "« {mot} » dans le menu : {t}");
    }
    assert_eq!(b.app.cur_layer, rang_de(&b, 0, "Notes"), "le clic droit vise ce calque, comme dans la timeline");
    // Le menu agit sur ce calque : dupliquer.
    let n = b.app.doc.panels[0].layers.len();
    let pos = textes_dans(&b, Rect::EVERYTHING).into_iter().find(|(x, _)| x.contains("Dupliquer")).unwrap().1.center();
    clic(&mut b, pos, Modifiers::NONE);
    assert_eq!(b.app.doc.panels[0].layers.len(), n + 1, "{}", b.app.status);
}

// ------------------------------------------------------------------------------------------------ un grand projet

/// 20 séquences de 15 plans, 15 calques par plan (300 plans).
fn grand_film() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    for s in 0..20 {
        if s > 0 {
            let last = doc.panels.len() - 1;
            doc.add_sequence_after(last);
        }
        for _ in 1..15 {
            let last = doc.panels.len() - 1;
            doc.add_scene_after(last);
        }
    }
    for p in &mut doc.panels {
        p.layers = (0..15).map(|k| Layer::new(&format!("C{k}"), LayerKind::Vector)).collect();
        p.set_length(12);
    }
    doc
}

#[test]
fn a_big_project_draws_only_the_visible_rows_and_does_not_rebuild_the_tree_every_frame() {
    let doc = grand_film();
    assert_eq!(doc.panels.len(), 300);
    let mut b = banc_dans(doc, Espace::Storyboard);
    // Le film, toutes les séquences et tous les plans dépliés : des milliers de lignes.
    b.app.settings.scene.racine = Racine::Film;
    b.app.settings.scene.sequences_ouvertes = b.app.doc.sequence_order();
    b.app.settings.scene.plans_ouverts = b.app.doc.panels.iter().map(|p| p.id).collect();
    b.chauffer(4);
    let n = b.app.scene_arbre.lignes.len();
    assert!(n > 5000, "{n} lignes");
    let vues = lignes();
    let haut = vues.iter().map(|(_, r)| r.top()).fold(f32::MAX, f32::min);
    let bas = vues.iter().map(|(_, r)| r.bottom()).fold(f32::MIN, f32::max);
    let visibles = ((bas - haut) / scene::H_CALQUE).ceil() as usize + 2;
    assert!(b.app.scene_arbre.peintes <= visibles && b.app.scene_arbre.peintes < 60, "{} lignes peintes pour {visibles} visibles", b.app.scene_arbre.peintes);
    // Rien ne change : l'arbre n'est pas refait.
    let r = b.app.scene_arbre.reconstructions;
    let t = std::time::Instant::now();
    b.chauffer(10);
    assert_eq!(b.app.scene_arbre.reconstructions, r, "pas refait sans raison");
    eprintln!("197 : 10 images, le film tout déplié ({n} lignes) : {:?}", t.elapsed());
    // L'œil d'un calque (lu à chaque image) : pas refait non plus ; un calque de plus : refait, une fois.
    b.app.agir_colonne(crate::layerheads::Colonne::Afficher, 0);
    b.chauffer(2);
    assert_eq!(b.app.scene_arbre.reconstructions, r);
    b.app.doc.panels[0].layers.push(Layer::new("Neuf", LayerKind::Vector));
    b.chauffer(2);
    assert_eq!(b.app.scene_arbre.reconstructions, r + 1, "la structure a changé : refait une fois");
    // L'arbre seul : refait seulement quand son empreinte change ; les lignes d'une tranche de 200 points.
    let mut a = scene::Arbre::default();
    let ou = b.app.scene_ou();
    assert!(a.a_jour(&b.app.doc, &b.app.settings.scene, ou));
    assert!(!a.a_jour(&b.app.doc, &b.app.settings.scene, ou));
    let range = a.visibles(1000.0, 1200.0);
    assert!(!range.is_empty() && range.len() <= 12, "{range:?}");
}

// ------------------------------------------------------------------------------------------------ le choix, gardé

#[test]
fn the_root_and_the_folds_are_kept_in_the_settings() {
    let mut c = ChoixScene::default();
    assert!(c.est_d_origine());
    c.racine = Racine::Film;
    c.basculer_plan(42, false);
    c.basculer_plan(7, true);
    c.basculer_sequence(3, false);
    assert!(c.plan_ouvert(42, false) && !c.plan_ouvert(7, true) && c.sequence_ouverte(3, false) && c.sequence_ouverte(9, true));
    let s = crate::settings::Settings { scene: c.clone(), ..Default::default() };
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("panneau_scene") && texte.contains("racine = \"film\""), "{texte}");
    let relu: crate::settings::Settings = toml::from_str(&texte).unwrap();
    assert_eq!(relu.scene, c);
    // D'origine, rien n'est écrit.
    let texte = toml::to_string(&crate::settings::Settings::default()).unwrap();
    assert!(!texte.contains("panneau_scene"));
    // Un clic sur « Depuis : Séquence » s'écrit dans les réglages.
    let mut b = banc_dans(film(), Espace::Storyboard);
    clic(&mut b, vus::ou(&ici("Depuis : Séquence"))[0].center(), Modifiers::NONE);
    assert_eq!(crate::settings::Settings::load().scene.racine, Racine::Sequence);
}

// ------------------------------------------------------------------------------------------------ partie 206 : la Scène entière

/// Le corps du panneau `k` de la pile, à l'écran (sa tranche sans l'en-tête ni le bord du bas).
fn corps_de(b: &Banc, k: usize) -> Rect {
    let (haut, bas) = b.app.colonne_etat.tranches[k];
    let bord = if Some(k) == b.app.colonne.qui_grandit() { 0.0 } else { colonne::BORD };
    let zone = b.app.colonne_etat.zone.expect("la colonne");
    Rect::from_min_max(pos2(zone.left(), haut + colonne::ENTETE), pos2(zone.right(), bas - bord))
}

/// Partie 206 (vu à l'écran, 868 points de haut à 90 %) : avec l'Historique, puis les Préréglages, ajoutés à la pile d'origine du
/// Storyboard (Scène, Couleur, Propriétés), la Scène était coupée (sa deuxième ligne de calque à moitié), puis réduite à sa barre et à
/// une ligne ; l'Historique à une étape. La Scène garde maintenant la hauteur de son arbre quand il tient dans sa hauteur d'origine,
/// l'Historique ses boutons et trois étapes ; et le fil d'Ariane passe à la ligne entre ses éléments, jamais au milieu de « Séquence 1 ».
#[test]
fn the_scene_panel_keeps_its_tree_whole_and_history_three_steps_in_a_full_storyboard_column() {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.add_scene_after(0);
    for p in &mut doc.panels {
        p.layers = vec![Layer::new("Dessin", LayerKind::Vector), Layer::new("Décor", LayerKind::Vector)];
        p.set_length(24);
    }
    let mut b = banc_taille(doc, false, 1384, 868);
    b.app.set_espace(Espace::Storyboard);
    b.app.settings.echelle = 90;
    b.garder_formes = true;
    (b.app.cur_panel, b.app.cur_layer) = (0, 0);
    b.chauffer(6);
    assert_eq!(b.app.colonne.panneaux[0].panneaux, vec![Pane::Scene], "la pile d'origine, la Scène en haut");
    for ajout in [Pane::History, Pane::Prereglages] {
        b.app.colonne.ajouter(ajout);
        b.chauffer(10);
        assert!(b.app.colonne.panneaux.iter().all(|p| !p.replie), "tous ouverts");
        let scene = corps_de(&b, b.app.colonne.rang_de(Pane::Scene).unwrap());
        let l = lignes();
        assert!(l.len() >= 3, "{l:?}");
        for (q, r) in &l {
            assert!(r.bottom() <= scene.bottom() + 0.5, "avec {ajout:?} : la ligne {q:?} ({r:?}) coupée par le bas de la Scène ({scene:?})");
        }
        let contenu = b.app.scene_arbre.barre + b.app.scene_arbre.hauteur;
        assert!(scene.height() >= contenu, "avec {ajout:?} : la Scène ({}) garde tout son arbre ({contenu})", scene.height());
        let histo = corps_de(&b, b.app.colonne.rang_de(Pane::History).unwrap());
        assert!(histo.height() >= 4.0 * 22.0, "avec {ajout:?} : l'Historique garde ses boutons et trois étapes : {}", histo.height());
    }
    // Le fil d'Ariane : chacun de ses éléments sur une seule ligne.
    let mut vus = 0;
    fn chercher(s: &Shape, vus: &mut usize) {
        match s {
            Shape::Text(t) if t.galley.text().starts_with("› ") => {
                *vus += 1;
                assert_eq!(t.galley.rows.len(), 1, "« {} » coupé en {} lignes", t.galley.text(), t.galley.rows.len());
            }
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, vus)),
            _ => {}
        }
    }
    for s in &b.formes {
        chercher(&s.shape, &mut vus);
    }
    assert_eq!(vus, 2, "« › Séquence 1 » et « › Plan 1 » sont peints");
}
