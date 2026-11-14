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
    "Pelure d'oignon du calque",
    crate::accroche::NOM,
    "Aplat sous le trait",
];

/// La colonne des calques et sa barre d'actions : toutes les commandes de l'ancien panneau sont à l'écran, et la barre d'actions
/// est d'une seule rangée, sous la colonne (qui finit à l'abscisse `colonne`), plus bas que la ligne du calque courant.
fn la_colonne_porte_tout(colonne: f32, ligne_du_calque: Rect) {
    for nom in DU_PANNEAU {
        assert!(present(nom), "« {nom} » n'est plus à l'écran : {:?}", vus::noms());
    }
    let barre: Vec<Rect> = crate::barrecalques::NOMS.iter().map(|n| *vus::ou(n).last().unwrap_or_else(|| panic!("« {n} » : {:?}", vus::noms()))).collect();
    for (k, r) in barre.iter().enumerate() {
        assert!((r.center().y - barre[0].center().y).abs() < 2.0, "une seule rangée : {barre:?}");
        assert!(r.center().y > ligne_du_calque.bottom(), "sous la colonne des calques : {barre:?}");
        assert!(r.right() <= colonne + 2.0, "sous la colonne, pas plus large : {r:?} / {colonne}");
        if k > 0 {
            assert!(r.center().x > barre[k - 1].center().x, "dans l'ordre : {barre:?}");
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
    la_colonne_porte_tout(debut + crate::layerheads::HEAD_W + 2.0, fusion[0]);
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
    // La fenêtre des trois calques grandit de deux rangées de 22 : 134 points.
    assert_eq!(crate::montagerows::LAYER_H, 2.0 * crate::pistes::RANGEE_ACTIVE);
    assert_eq!(crate::pistes::MARGE_H * 2.0 + crate::montagerows::LAYER_H, 134.0);
    let fusion = vus::ou("Mode de fusion");
    assert_eq!(fusion.len(), 1, "seule la piste du calque actif : {fusion:?}");
    // (Le bord droit de la colonne : sa poignée pour l'élargir.)
    let colonne = ou("Largeur de la colonne").right();
    // Les options de la piste active sont dans la colonne, sur deux rangées.
    for nom in ["Pelure d'oignon du calque", "Mode de fusion", "Créer une clé", "Opacité du calque", "Commandes du calque", "Verrouiller / déverrouiller"] {
        let r = ou(nom);
        assert!(r.right() <= colonne + 2.0, "« {nom} » dans la colonne : {r:?} / {colonne}");
    }
    let (haut, bas) = (ou("Commandes du calque"), ou("Opacité du calque"));
    assert!(bas.center().y - haut.center().y > 12.0 && bas.center().y - haut.center().y < 30.0, "deux rangées : {haut:?} {bas:?}");
    la_colonne_porte_tout(colonne, fusion[0]);
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
