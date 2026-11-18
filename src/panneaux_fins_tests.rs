//! L'interface à panneaux fins (partie 160) : chaque fonction est toujours atteignable à sa nouvelle place, dans la vraie fenêtre
//! sans écran (`Banc`). On lit où chaque élément est posé par le nom de sa bulle (`bulles::vus`).
//!
//! La règle d'Anthony : « ce doit être simple, en plus de ce que nous avons changé pour les calques, rien ne doit bouger pour le
//! reste. Tout doit rester à sa place actuelle et trouver sa place dans cette nouvelle interface ».

use eframe::egui::{Modifiers, Rect};

use crate::bulles::vus;
use crate::dock::Pane;
use crate::perf_tests::{banc, Banc};
use crate::pistes_tests::clic;
use crate::workspace::Espace;

/// Un banc sur un film de deux plans (trois calques, puis un), dans l'espace `espace`.
fn banc_dans(espace: Espace) -> Banc {
    let mut b = banc(crate::pistes_tests::doc_with(&[&["Décor", "Perso", "Ombre"], &["Seul"]]), false);
    b.app.set_espace(espace);
    b.chauffer(6);
    b
}

/// Le premier rectangle de l'élément dont la bulle s'appelle `nom`, à la dernière image.
fn ou(nom: &str) -> Rect {
    *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

fn present(nom: &str) -> bool {
    !vus::ou(nom).is_empty()
}

// ------------------------------------------------------------------------------------ 1. les barres de la vue

#[test]
fn looking_goes_up_and_what_lies_over_the_drawing_stays_down() {
    let mut b = banc_dans(Espace::Storyboard);
    b.chauffer(2);
    let plateau = ou("Vue du plateau");
    let regle = ou("Guides de perspective");
    // En haut, dans cet ordre : les vues, la règle, puis ajuster, zoom, rotation, miroir (montés de la barre du bas).
    let haut = ["Ajuster à la fenêtre", "Zoom", "Rotation de la vue", "Miroir de la vue"].map(ou);
    let mut x = regle.center().x;
    for (k, r) in haut.iter().enumerate() {
        assert!((r.center().y - plateau.center().y).abs() < 2.0, "{k} : dans la barre du haut, {r:?} / {plateau:?}");
        assert!(r.center().x > x, "{k} : dans l'ordre, après la règle");
        x = r.center().x;
    }
    // En bas : ce qui se superpose au dessin.
    let bas = ["Cadrage", "Repères de composition", "Pelure d'oignon", "Table lumineuse", "Passe-partout"].map(ou);
    for (k, r) in bas.iter().enumerate() {
        assert!(r.center().y > plateau.center().y + 100.0, "{k} : dans la barre du bas, {r:?}");
        assert!((r.center().y - bas[0].center().y).abs() < 2.0, "{k} : une seule rangée");
    }
    // Les petits panneaux du zoom et de la rotation s'ouvrent vers le dessin : dessous, en haut ; au-dessus, en bas.
    let ecran = b.ctx.screen_rect();
    assert_eq!(crate::viewbar::sens_du_panneau(haut[1], ecran), eframe::egui::AboveOrBelow::Below);
    assert_eq!(crate::viewbar::sens_du_panneau(bas[2], ecran), eframe::egui::AboveOrBelow::Above);
    // Les pastilles des brosses dédiées ne bougent pas : au bout de la barre du haut, à droite de tout le reste.
    if let Some(p) = vus::ou("Brosses dédiées").first() {
        assert!(p.center().x > haut[3].center().x, "les pastilles restent au bout à droite");
    }
}

// ------------------------------------------------------------------------------------ 2. les outils de l'Espace 3D

#[test]
fn the_3d_view_tools_come_first_in_the_tool_bar_and_its_view_options_stay_up() {
    let mut b = banc_dans(Espace::Storyboard);
    let pinceau = ou(crate::app::Tool::Brush.label());
    assert!(!present("Dessiner sur la carte"), "vue fermée : la barre d'outils est celle d'avant");
    b.app.run_action("espace3d.basculer");
    b.chauffer(4);
    assert!(b.app.outils_3d_dans_la_barre());
    let dessiner = ou("Dessiner sur la carte");
    let pinceau_3d = ou(crate::app::Tool::Brush.label());
    // En tête de la barre d'outils de gauche, en partant du haut, dans l'ordre d'avant.
    let colonne = ["Dessiner sur la carte", "Choisir", "Déplacer", "Tourner", "Mettre à l'échelle"].map(ou);
    for (k, r) in colonne.iter().enumerate() {
        assert!((r.center().x - pinceau_3d.center().x).abs() < 2.0, "{k} : dans la colonne des outils, {r:?} / {pinceau_3d:?}");
        assert!(r.center().y < pinceau_3d.center().y, "{k} : au-dessus des outils de dessin");
        if k > 0 {
            assert!(r.center().y > colonne[k - 1].center().y, "{k} : dans l'ordre");
        }
    }
    // La case « Caméra » de la vue, juste après « Mettre à l'échelle », dans la colonne aussi.
    assert!(vus::ou("Caméra").iter().any(|r| (r.center().x - pinceau_3d.center().x).abs() < 2.0 && r.center().y > colonne[4].center().y && r.center().y < pinceau_3d.center().y), "{:?}", vus::ou("Caméra"));
    assert!(dessiner.center().y < pinceau.center().y + 1.0);
    // Les options de vue de la 3D restent dans la barre au-dessus de l'image.
    let plateau = ou("Vue du plateau");
    for nom in ["Recadrer", "Regarder par la caméra"] {
        let r = ou(nom);
        assert!((r.center().y - plateau.center().y).abs() < 2.0 && r.center().x > plateau.center().x, "{nom} reste en haut");
    }
    // Un clic sur « Déplacer » de la colonne : le mode du manipulateur.
    let deplacer = ou("Déplacer");
    clic(&mut b, deplacer.center(), Modifiers::NONE);
    assert_eq!(b.app.espace3d.mode, crate::espace3dgizmo::Mode::Move);
    // Vue fermée : la barre redevient celle d'avant.
    b.app.run_action("espace3d.basculer");
    b.chauffer(4);
    assert!(!present("Dessiner sur la carte") && !present("Recadrer"));
}

// ------------------------------------------------------------------------------------ 3. les onglets, la barre des panneaux

#[test]
fn the_group_names_are_gone_each_panel_keeps_a_thin_grip_and_the_bar_says_which_is_in_front() {
    let mut b = banc_dans(Espace::Storyboard);
    b.chauffer(2);
    // Chaque panneau ouvert a une fine poignée (sa bulle porte son nom), pas un onglet écrit.
    for pane in [Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario] {
        let nom = b.app.pane_title(pane);
        // (Son icône de la barre des panneaux a la même bulle : on prend la poignée, la fine.)
        let poignees: Vec<Rect> = vus::ou(nom).into_iter().filter(|r| r.height() <= crate::theme::POIGNEE_ONGLET + 1.0).collect();
        assert!(!poignees.is_empty(), "{nom} : une poignée de {} points au plus : {:?}", crate::theme::POIGNEE_ONGLET, vus::ou(nom));
    }
    // L'icône allumée dit quel panneau est devant ; un panneau caché derrière un autre a son point.
    use crate::barrepanneaux::EtatIcone;
    assert_eq!(b.app.etat_dans_la_barre(Pane::ToolOptions), EtatIcone::Devant);
    assert_eq!(b.app.etat_dans_la_barre(Pane::Sheet), EtatIcone::Derriere, "la fiche est en onglet derrière Réglages");
    assert_eq!(b.app.etat_dans_la_barre(Pane::History), EtatIcone::Ferme);
    // Le clic de la barre met devant, puis ferme : la règle d'avant.
    b.app.run_action("vue.fiche");
    b.chauffer(2);
    assert_eq!(b.app.etat_dans_la_barre(Pane::Sheet), EtatIcone::Devant);
    // On fait toujours flotter un panneau : sa poignée l'emmène, et son clic droit le met dans une fenêtre à part.
    assert!(crate::dockui::POIGNEE.contains("Glisser") && crate::dockui::POIGNEE.contains("fenêtre à part"));
}

// ------------------------------------------------------------------------------------ 4 et 5. les calques

/// Les commandes de l'ancien panneau Calques et le nom de la bulle qui les porte à leur nouvelle place (ligne du calque courant ou
/// barre d'actions). Les autres sont vérifiées à part (menus, palette).
const DU_PANNEAU: &[&str] = &[
    "Nouveau calque",
    "Dupliquer les calques choisis",
    "Supprimer les calques choisis",
    "Plus de commandes des calques",
    "Mode de fusion",
    "Opacité du calque",
    "Créer une clé",
    "Afficher / cacher",
    "Verrouiller / déverrouiller",
    "Commandes du calque",
    // (« Pelure d'oignon du calque » : sur la ligne de la timeline ; sur la piste du Storyboard, elle est au clic droit depuis la
    // partie 173, `piste_mode_tests.rs`.)
    crate::accroche::NOM,
    "Aplat sous le trait",
];

/// La colonne des calques et sa barre d'actions : toutes les commandes de l'ancien panneau sont à l'écran, et la barre d'actions
/// est d'une seule rangée, sous la colonne (qui commence à l'abscisse `debut` et finit à `colonne`), plus bas que la ligne du
/// calque courant. Ses cases ne se touchent pas (partie 161 : au moins `barrecalques::ECART` points entre deux) ; elle tient
/// dans la colonne quand la colonne a la place de ses six cases lisibles, sinon elle la déborde juste de ce qu'il faut.
fn la_colonne_porte_tout(debut: f32, colonne: f32, ligne_du_calque: Rect) {
    use crate::barrecalques::{ECART, LARGEUR_LISIBLE};
    for nom in DU_PANNEAU {
        assert!(present(nom), "« {nom} » n'est plus à l'écran : {:?}", vus::noms());
    }
    let barre: Vec<Rect> = crate::barrecalques::NOMS.iter().map(|n| *vus::ou(n).last().unwrap_or_else(|| panic!("« {n} » : {:?}", vus::noms()))).collect();
    // (La barre part du bord gauche de la bande, sous la colonne ; elle va au bout de la colonne, ou de ses six cases lisibles.)
    let fin = colonne.max(barre[0].left() + LARGEUR_LISIBLE);
    assert!(barre[0].left() >= debut - 2.0 && barre[0].left() < colonne, "la barre commence sous la colonne : {barre:?} / {debut}");
    for (k, r) in barre.iter().enumerate() {
        assert!((r.center().y - barre[0].center().y).abs() < 2.0, "une seule rangée : {barre:?}");
        assert!(r.center().y > ligne_du_calque.bottom(), "sous la colonne des calques : {barre:?}");
        assert!(r.right() <= fin + 2.0, "sous la colonne, pas plus large qu'il ne faut : {r:?} / {fin}");
        assert!(r.width() >= crate::barrecalques::CASE[0] - 0.5, "une case lisible : {r:?}");
        if k > 0 {
            assert!(r.center().x > barre[k - 1].center().x, "dans l'ordre : {barre:?}");
            assert!(r.left() - barre[k - 1].right() >= ECART - 0.5, "les cases ne se touchent pas : {barre:?}");
        }
    }
}

#[test]
fn in_the_animation_the_current_layer_carries_everything_and_the_others_are_fine_rows() {
    let mut b = banc_dans(Espace::Animation);
    b.chauffer(2);
    let fusion = vus::ou("Mode de fusion");
    assert_eq!(fusion.len(), 1, "une seule ligne complète, celle du calque courant : {fusion:?}");
    // (La colonne commence au bord gauche de la bande, après la barre d'outils : la barre d'actions part de là.)
    let debut = vus::ou(crate::barrecalques::NOMS[0]).last().copied().expect("la barre d'actions").left();
    la_colonne_porte_tout(debut, debut + crate::layerheads::HEAD_W + 2.0, fusion[0]);
    assert!(present("Pelure d'oignon du calque"), "la pelure reste sur la ligne de la timeline");
    // L'en-tête de la colonne n'a plus ses boutons (ils sont dans la barre d'actions) ni « Tout réduire ».
    let haut_des_lignes = fusion[0].top() - crate::layerheads::ROW_FULL;
    for nom in ["Nouveau dossier", "Groupes de calques", "Opérations sur les calques", "Réduire toutes les lignes", "Agrandir toutes les lignes", "Réduire la ligne", "Agrandir la ligne"] {
        assert!(vus::ou(nom).iter().all(|r| r.top() > haut_des_lignes), "« {nom} » n'est plus en haut de la colonne : {:?}", vus::ou(nom));
    }
    // Un autre calque devient courant : c'est sa ligne qui est complète, l'autre est fine.
    let rows = |b: &Banc| crate::layerheads::rows_of(b.app.layers(), &b.app.doc.groups, b.app.cur_layer);
    assert_eq!(rows(&b).iter().filter(|r| r.h == crate::layerheads::ROW_FULL).count(), 1);
    b.app.select_layer(0);
    b.chauffer(2);
    for r in rows(&b) {
        assert_eq!(r.h, if r.li == 0 { crate::layerheads::ROW_FULL } else { crate::layerheads::ROW_FINE });
    }
}

#[test]
fn in_the_storyboard_the_active_track_carries_everything_on_two_rows() {
    let mut b = banc_dans(Espace::Storyboard);
    b.chauffer(2);
    // La piste du calque actif : trois rangées de 22 (partie 173.5 ; deux depuis la partie 160).
    assert_eq!(crate::montagerows::LAYER_H, 3.0 * crate::pistes::RANGEE_ACTIVE);
    let fusion = vus::ou("Mode de fusion");
    assert_eq!(fusion.len(), 1, "seule la piste du calque actif : {fusion:?}");
    // (Le bord droit de la colonne : sa poignée pour l'élargir.)
    let colonne = ou("Largeur de la colonne").right();
    // Les options de la piste active sont dans la colonne, sur trois rangées (partie 173.5 : le nom, les options, le mode et
    // l'opacité).
    for nom in ["Pelure d'oignon du calque", "Mode de fusion", "Créer une clé", "Opacité du calque", "Commandes du calque", "Verrouiller / déverrouiller"] {
        let r = ou(nom);
        assert!(r.right() <= colonne + 2.0, "« {nom} » dans la colonne : {r:?} / {colonne}");
    }
    let (haut, milieu, bas) = (ou("Commandes du calque"), ou("Pelure d'oignon du calque"), ou("Opacité du calque"));
    let pas = crate::pistes::RANGEE_ACTIVE;
    assert!((milieu.center().y - haut.center().y - pas).abs() < 3.0 && (bas.center().y - milieu.center().y - pas).abs() < 3.0, "trois rangées : {haut:?} {milieu:?} {bas:?}");
    // (La barre du bas de la bande part de son bord gauche, à gauche des en-têtes de la colonne.)
    la_colonne_porte_tout(0.0, colonne, fusion[0]);
    // La barre d'actions est à gauche du zoom de la bande.
    let zoom = ou("Zoom arrière");
    assert!(zoom.left() > ou(crate::barrecalques::NOMS[5]).right(), "le zoom à sa droite");
}

#[test]
fn every_command_of_the_old_layers_panel_has_a_place() {
    // Ce que l'ancien panneau Calques portait et qui n'a pas de bulle à l'écran : par une commande (palette, raccourci) et un menu.
    for id in ["calque.monter", "calque.descendre", "calque.coller", "calque.copier", "calque.cle_opacite", "dossier.nouveau", "calque.dupliquer", "calque.supprimer", "vue.calques"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
    let menu = include_str!("menucalque.rs");
    assert!(menu.contains("\"calque.monter\"") && menu.contains("\"calque.descendre\""), "Monter et Descendre au menu ⋮ (Organiser)");
    assert!(include_str!("barrecalques.rs").contains("\"calque.coller\""), "le clic droit de la barre : coller le calque");
    // Le panneau lui-même n'est plus nulle part : ni d'office, ni dans la barre des panneaux, ni dans Affichage > Panneaux.
    assert!(!Pane::ALL.contains(&Pane::Layers));
    for e in Espace::TOUS {
        assert!(!crate::barrepanneaux::panneaux_d_office(e).contains(&Pane::Layers), "{e:?}");
        assert!(!e.defaut().dock_layout().contains(Pane::Layers), "{e:?}");
    }
    // La commande « Panneau Calques » montre la bande du bas et donne la main à la colonne des calques.
    let mut b = banc_dans(Espace::Storyboard);
    b.app.toggle_pane(Pane::Montage);
    b.chauffer(2);
    assert!(!b.app.pane_shown(Pane::Montage));
    b.app.run_action("vue.calques");
    b.chauffer(2);
    assert!(b.app.pane_shown(Pane::Montage) && b.app.layers_pane_focus, "{}", b.app.status);
    // Le calque Couleur unie : sa couleur sur la ligne du calque courant.
    let mut b = banc_dans(Espace::Animation);
    let li = b.app.cur_layer;
    b.app.layers_mut()[li].unie = Some([120, 120, 120]);
    b.chauffer(3);
    assert!(present("Couleur unie du calque"), "{:?}", vus::noms());
    // Les effets du calque, d'un coup.
    b.app.layers_mut()[li].fx.push(crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur));
    b.chauffer(3);
    assert!(present("Effets allumés"), "{:?}", vus::noms());
}

// ------------------------------------------------------------------------------------ 7. les Vignettes

#[test]
fn the_vignettes_have_one_annotations_line_and_no_word_layer() {
    let mut b = banc_dans(Espace::Vignettes);
    b.app.show_pane(Pane::Vignettes);
    b.garder_textes = true;
    b.chauffer(4);
    assert!(present("Annotations") && present("Dessin de la case"), "{:?}", vus::noms());
    assert!(!b.textes.iter().any(|t| t.contains("Calques d'annotation")), "plus de liste « Calques d'annotation »");
    // Le mot « calque » n'est plus sous le titre de Réglages.
    assert!(!b.textes.iter().any(|t| t == "calque vectoriel" || t == "calque bitmap"), "{:?}", b.textes);
    // L'œil montre ou cache les annotations (un affichage).
    let avant = b.app.vignettes.annot.visible;
    clic(&mut b, ou(if avant { "Cacher les annotations" } else { "Montrer les annotations" }).center(), Modifiers::NONE);
    assert_ne!(b.app.vignettes.annot.visible, avant);
    // « Annotations » prend le stylo ; « Dessin de la case » rend la main au pinceau.
    clic(&mut b, ou("Annotations").center(), Modifiers::NONE);
    assert!(b.app.vignettes.annot.outil.is_some());
    clic(&mut b, ou("Dessin de la case").center(), Modifiers::NONE);
    assert!(b.app.vignettes.annot.outil.is_none());
}

// ------------------------------------------------------------------------------------ partie 161 : amincir

/// Le rectangle de la partie (onglets compris) qui tient `pane` dans la fenêtre principale, et la hauteur de sa barre de poignées.
fn partie_de(b: &Banc, pane: Pane) -> Option<(Rect, f32)> {
    let state = b.app.dock.as_ref()?;
    let (s, n, _) = state.find_tab(&pane)?;
    if s != egui_dock::SurfaceIndex::main() {
        return None;
    }
    match &state[s][n] {
        egui_dock::Node::Leaf { rect, viewport, .. } => Some((*rect, viewport.top() - rect.top())),
        _ => None,
    }
}

/// Deux parties de côté posées côte à côte (ni le dessin ni la bande dans l'une ou l'autre) : il ne doit plus y en avoir.
fn deux_colonnes(n: &crate::dock::DockNode) -> bool {
    n.clone().une_seule_colonne() != *n
}

#[test]
fn every_default_layout_has_one_narrow_column_on_the_right() {
    use crate::dock::{DockLayout, COLONNE};
    // Aucune disposition d'origine n'a deux colonnes de panneaux côte à côte (espaces de travail et dispositions de FEANOR).
    for e in Espace::TOUS {
        let d = e.defaut().dock_layout();
        assert!(!deux_colonnes(&d.principal), "{e:?} : {:?}", d.principal);
    }
    for (id, _, l) in crate::workspace::builtin() {
        assert!(!deux_colonnes(&l.dock_layout().principal), "{id}");
    }
    // Rien ne se perd : dans le Storyboard, les réglages, la fiche, les effets et le scénario sont des onglets du même groupe,
    // la couleur et les brosses dédiées dessous.
    let sb = DockLayout::pour_le_storyboard();
    for p in [Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario, Pane::Color, Pane::Personnages] {
        assert!(sb.contains(p), "{p:?}");
    }
    // À l'écran, dans la vraie fenêtre (1 920 points de large) : la colonne fait `COLONNE` points, quelle que soit la largeur de
    // la fenêtre, et le dessin prend le reste.
    for e in [Espace::Storyboard, Espace::Animation, Espace::Vignettes, Espace::Montage] {
        let mut b = banc_dans(e);
        b.chauffer(3);
        let pane = if e == Espace::Montage { Pane::Sheet } else { Pane::ToolOptions };
        let (col, _) = partie_de(&b, pane).unwrap_or_else(|| panic!("{e:?} : {pane:?} absent"));
        assert!((col.width() - COLONNE).abs() <= 3.0, "{e:?} : la colonne fait {} points", col.width());
        let (dessin, _) = partie_de(&b, Pane::Canvas).expect("le dessin");
        assert!(dessin.width() > b.taille.x - COLONNE - 160.0, "{e:?} : le dessin prend la place, {dessin:?}");
        // Une fenêtre plus large : la colonne ne grandit pas, l'image si.
        b.taille.x += 400.0;
        b.chauffer(3);
        let (col2, _) = partie_de(&b, pane).expect("toujours là");
        let (dessin2, _) = partie_de(&b, Pane::Canvas).expect("le dessin");
        assert!((col2.width() - COLONNE).abs() <= 3.0, "{e:?} : {} points après agrandissement", col2.width());
        assert!(dessin2.width() > dessin.width() + 390.0, "{e:?} : {dessin:?} → {dessin2:?}");
    }
}

#[test]
fn a_saved_two_column_layout_becomes_one_column_and_keeps_every_panel() {
    use crate::dock::DockLayout;
    // La disposition de la partie 160 (version 8) : les effets et le scénario dans une colonne, les réglages et la couleur à côté.
    let ancienne = r#"{"version":8,"principal":{"type":"dessus_dessous","part":0.65,
        "haut":{"type":"cote_a_cote","part":0.6,"gauche":{"type":"onglets","panneaux":["dessin"]},
            "droite":{"type":"cote_a_cote","part":0.4,
                "gauche":{"type":"dessus_dessous","part":0.6,"haut":{"type":"onglets","panneaux":["scenario"]},"bas":{"type":"onglets","panneaux":["effets"]}},
                "droite":{"type":"dessus_dessous","part":0.55,"haut":{"type":"onglets","panneaux":["reglages","fiche"],"actif":0},"bas":{"type":"onglets","panneaux":["couleur","personnages"]}}}},
        "bas":{"type":"onglets","panneaux":["bande"]}}}"#;
    let d: DockLayout = serde_json::from_str(ancienne).expect("disposition lisible");
    let avant = d.panes();
    let l = crate::workspace::Layout { panneaux: Some(d), ..Default::default() };
    let n = l.dock_layout();
    assert_eq!(n.version, crate::dock::LAYOUT_VERSION);
    assert!(!deux_colonnes(&n.principal), "{:?}", n.principal);
    let mut apres = n.panes();
    let mut avant = avant;
    apres.sort_by_key(|p| format!("{p:?}"));
    avant.sort_by_key(|p| format!("{p:?}"));
    assert_eq!(apres, avant, "aucun panneau perdu");
    // Les réglages de l'outil restent devant ; les effets et le scénario sont derrière eux, dans le même groupe.
    let mut b = banc_dans(Espace::Storyboard);
    b.app.set_dock_layout(n);
    b.chauffer(3);
    use crate::barrepanneaux::EtatIcone;
    assert_eq!(b.app.etat_dans_la_barre(Pane::ToolOptions), EtatIcone::Devant);
    assert_eq!(b.app.etat_dans_la_barre(Pane::Effects), EtatIcone::Derriere);
    assert_eq!(b.app.etat_dans_la_barre(Pane::Scenario), EtatIcone::Derriere);
    assert_eq!(b.app.etat_dans_la_barre(Pane::Color), EtatIcone::Devant);
}

#[test]
fn no_empty_band_on_top_of_the_panels() {
    // La barre des poignées n'est plus une bande vide de 26 points au-dessus de chaque panneau : une fine poignée.
    assert!(crate::theme::POIGNEE_ONGLET <= 8.0);
    let mut b = banc_dans(Espace::Storyboard);
    b.chauffer(2);
    for pane in [Pane::ToolOptions, Pane::Color, Pane::Canvas, Pane::Montage] {
        let (_, barre) = partie_de(&b, pane).unwrap_or_else(|| panic!("{pane:?}"));
        assert!(barre <= crate::theme::POIGNEE_ONGLET + 1.0, "{pane:?} : {barre} points au-dessus du panneau");
    }
    // Les panneaux de la colonne sont plus denses : texte de 11,5, lignes à 2 points d'écart.
    let mut style = (*b.ctx.style()).clone();
    crate::theme::densifier_panneau(&mut style);
    assert_eq!(style.text_styles[&eframe::egui::TextStyle::Body].size, crate::theme::TEXTE_PANNEAU);
    assert!(crate::theme::TEXTE_PANNEAU < b.ctx.style().text_styles[&eframe::egui::TextStyle::Body].size);
    assert_eq!(style.spacing.item_spacing.y, crate::theme::LIGNE_PANNEAU);
}

#[test]
fn every_panel_of_the_column_is_reached_from_the_panel_bar() {
    use crate::barrepanneaux::EtatIcone;
    // Chaque panneau d'office de chaque espace s'ouvre ou passe devant d'un clic sur son icône, dans la colonne unique.
    for e in [Espace::Vignettes, Espace::Storyboard, Espace::Animation, Espace::Montage] {
        let mut b = banc_dans(e);
        for &pane in crate::barrepanneaux::panneaux_d_office(e) {
            if pane == Pane::References || pane == Pane::Bibliotheque {
                continue; // (Ils s'ouvrent aussi, mais peuvent partir dans leur fenêtre à part.)
            }
            if b.app.etat_dans_la_barre(pane) != EtatIcone::Devant {
                b.app.clic_panneau(pane);
            }
            b.chauffer(2);
            assert_eq!(b.app.etat_dans_la_barre(pane), EtatIcone::Devant, "{e:?} : {pane:?}");
            let layout = b.app.dock_layout();
            assert!(!deux_colonnes(&layout.principal), "{e:?} : {pane:?} a refait une deuxième colonne : {:?}", layout.principal);
        }
    }
}

#[test]
fn only_the_side_columns_next_to_the_drawing_are_merged() {
    use crate::dock::DockNode;
    let t = |p: &[Pane]| DockNode::Tabs { panneaux: p.to_vec(), actif: 0 };
    let row = |part: f32, a: DockNode, b: DockNode| DockNode::Row { part, gauche: Box::new(a), droite: Box::new(b) };
    let col = |part: f32, a: DockNode, b: DockNode| DockNode::Column { part, haut: Box::new(a), bas: Box::new(b) };
    // Une rangée rangée à la main sous le dessin (commentaires | historique) reste comme elle est ; la colonne double à côté du
    // dessin, elle, se fond.
    let n = col(0.6, row(0.6, t(&[Pane::Canvas]), row(0.4, t(&[Pane::Effects]), t(&[Pane::ToolOptions]))), row(0.5, t(&[Pane::Comments]), t(&[Pane::History])));
    let attendu = col(0.6, row(0.6, t(&[Pane::Canvas]), t(&[Pane::ToolOptions, Pane::Effects])), row(0.5, t(&[Pane::Comments]), t(&[Pane::History])));
    assert_eq!(n.une_seule_colonne(), attendu);
}
// ------------------------------------------------------------------------------------ partie 161.4 : ce qui débordait

/// Les éléments du panneau Planche des Vignettes, dans l'ordre où il les montre (partie 163.3 : les dessins de la planche, puis sous
/// « Précis » les séquences et les numéros).
const DE_LA_PLANCHE: &[&str] = &[
    "Annotations",
    "Dessin de la case",
    "En lignes",
    "À la japonaise",
    "Cases par ligne",
    "Action",
    "Dialogue",
    "Notes",
    "Nouvelle séquence",
    "Toutes",
    "Seulement la courante",
    "Aucune autre",
    "Cacher la séquence",
    "Numéros de plan",
];

#[test]
fn the_board_panel_fits_in_the_column_and_the_vignettes_suite_does_not_cover_it() {
    let mut b = banc_dans(Espace::Vignettes);
    b.app.show_pane(Pane::Vignettes);
    // Une séquence au long nom (jusqu'à 200 signes) : sa ligne tient dans la colonne.
    assert!(!b.app.doc.sequences.is_empty(), "le film a une séquence");
    b.app.doc.sequences[0].name = "Une séquence au nom très long, comme un intitulé de scénario : INT. CUISINE DE LA FERME, NUIT, ORAGE".into();
    crate::panneauxvisuels::ouvrir_precis(&b.ctx, "Planche", true);
    b.chauffer(4);
    let (col, _) = partie_de(&b, Pane::Vignettes).expect("le panneau Planche est dans la colonne");
    assert!((col.width() - crate::dock::COLONNE).abs() <= 3.0, "{col:?}");
    let nom = b.app.doc.sequence_label(b.app.doc.sequences[0].id);
    let mut noms: Vec<&str> = DE_LA_PLANCHE.to_vec();
    noms.push(&nom);
    let mut y = f32::MIN;
    for n in noms.iter().take(DE_LA_PLANCHE.len()) {
        let r = *vus::ou(n).iter().find(|r| col.contains_rect(r.shrink(0.5)) || (r.center().x > col.left() && r.center().y > col.top() && r.center().y < col.bottom())).unwrap_or_else(|| panic!("« {n} » n'est pas dans le panneau Planche : {:?}", vus::ou(n)));
        assert!(r.center().y >= y - 1.0, "« {n} » : dans le même ordre");
        y = y.max(r.top());
    }
    for n in &noms {
        for r in vus::ou(n).into_iter().filter(|r| r.center().x > col.left() && r.center().y > col.top() && r.center().y < col.bottom()) {
            assert!(r.left() >= col.left() - 0.5 && r.right() <= col.right() + 0.5, "« {n} » déborde de la colonne de {} points : {r:?} / {col:?}", r.right() - col.right());
        }
    }
    // La suite des Vignettes joue sous l'onglet « Vignettes » (en haut à droite) : sa fenêtre se pose à côté de la colonne, sans
    // passer par-dessus le panneau Planche ni par-dessus la barre des panneaux.
    let t0 = b.ctx.input(|i| i.time);
    assert!(b.app.aide_revoir(crate::aideflottante::VIGNETTES, t0));
    b.chauffer(2);
    let onglet = crate::aideflottante::ancre(&b.ctx, crate::aideflottante::VIGNETTES).expect("l'onglet Vignettes");
    let f = b.app.confort.aide.fenetre().expect("la fenêtre joue");
    assert!(f.right() <= col.left(), "la fenêtre de l'aide passe par-dessus la colonne : {f:?} / {col:?}");
    assert!(f.top() >= onglet.bottom() && b.ctx.screen_rect().contains_rect(f), "sous l'onglet, dans l'écran : {f:?} / {onglet:?}");
}

#[test]
fn the_whole_camera_of_the_3d_space_panel_shows_without_scrolling() {
    // La fenêtre des captures d'Anthony (2 576 × 1 048, la barre de titre en moins), le Storyboard, la vue Espace 3D ouverte, six
    // cartes (comme « retakes brawlydays »), l'aimantation allumée (d'origine) ; la Couleur sous le panneau Espace 3D.
    let mut doc = crate::pistes_tests::doc_with(&[&["Dessin", "Vectoriel 13", "Vectoriel 8", "Vectoriel 9", "Vectoriel 10", "Vectoriel 11"], &["Seul"]]);
    doc.panels[0].set_length(24);
    let mut b = crate::perf_tests::banc_taille(doc, false, 2576, 1010);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    b.app.run_action("espace3d.basculer");
    b.chauffer(6);
    assert!(b.app.espace3d.snap, "l'aimantation d'origine");
    // (Partie 162 : le panneau est contextuel ; la caméra choisie, il montre ses graphiques.)
    b.app.espace3d.sel = vec![crate::espace3dgizmo::CAMERA_ID];
    b.chauffer(4);
    let state = b.app.dock.as_ref().expect("le dock");
    let (s, n, _) = state.find_tab(&Pane::Espace3d).expect("le panneau Espace 3D");
    let vue = match &state[s][n] {
        egui_dock::Node::Leaf { viewport, .. } => *viewport,
        _ => panic!("une feuille"),
    };
    assert!(b.app.pane_shown(Pane::Color), "la Couleur dessous");
    assert!((vue.width() - crate::dock::COLONNE).abs() <= 3.0, "la colonne de 290 points : {vue:?}");
    // Toute la caméra, du haut (l'objectif) au bas (« Précis ») : rien sous le bas du panneau, rien hors de la colonne.
    for nom in ["Objectif 18 mm", "Objectif 135 mm", "Champ de la caméra", "Hauteur et plongée", "Panoramique", "Horizon", "Regarder par la caméra", "Viser la sélection", "Précis"] {
        let r = *vus::ou(nom).iter().find(|r| r.center().x > vue.left() && r.center().x < vue.right()).unwrap_or_else(|| panic!("« {nom} » n'est pas dans le panneau : {:?}", vus::noms()));
        assert!(r.bottom() <= vue.bottom() + 0.5, "« {nom} » dépasse le bas du panneau de {} points : {r:?} / {vue:?}", r.bottom() - vue.bottom());
        assert!(r.left() >= vue.left() - 0.5 && r.right() <= vue.right() + 0.5, "« {nom} » déborde de la colonne : {r:?} / {vue:?}");
    }
}
