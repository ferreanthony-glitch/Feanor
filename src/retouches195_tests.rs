//! Essais des retouches vues à l'écran de la partie 194 (`docs/INTERFACE.md`, partie 195) : le compteur des bandes du bas (Ctrl + clic,
//! retenu), la règle et le compteur qui restent en haut quand la bande défile, le panneau Préréglages (un aperçu, une liste), plus
//! d'animation d'explication hors du menu Aide, la hauteur d'un panneau ajouté sous Propriétés, les barres de la vue (icônes de la
//! timeline, passe-partout, zoom et inclinaison), les curseurs plus fins, l'en-tête des Réglages. Dans la vraie fenêtre sans écran
//! (`Banc`), chaque élément retrouvé par le nom de sa bulle (`bulles::vus`). (Ctrl + clic dans la vue du dessin : `dessin_notes_tests`.)

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, Pos2, Rect, Shape};

use crate::app::{Mode, Tool};
use crate::bulles::vus;
use crate::colonne;
use crate::dock::Pane;
use crate::perf_tests::{banc, banc_taille, Banc};
use crate::pistes_tests::clic;
use crate::workspace::Espace;

const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };

fn ou(nom: &str) -> Rect {
    *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

fn present(nom: &str) -> bool {
    !vus::ou(nom).is_empty()
}

fn textes(b: &Banc) -> Vec<(String, Rect)> {
    fn chercher(s: &Shape, out: &mut Vec<(String, Rect)>) {
        match s {
            Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut out);
    }
    out
}

/// Un film de trois plans de 24 images ; `calques` calques vectoriels dans chacun.
fn film(calques: usize) -> crate::document::Document {
    let noms: Vec<String> = (0..calques).map(|k| format!("C{k}")).collect();
    let refs: Vec<&str> = noms.iter().map(String::as_str).collect();
    crate::pistes_tests::doc_with(&[&refs, &refs, &refs])
}

fn banc_dans(doc: crate::document::Document, espace: Espace, l: u32, h: u32) -> Banc {
    let mut b = banc_taille(doc, false, l, h);
    b.app.set_espace(espace);
    b.garder_formes = true;
    b.chauffer(6);
    b
}

// ------------------------------------------------------------------------------------ le compteur : Ctrl + clic, retenu

#[test]
fn ctrl_click_on_the_storyboard_counter_switches_to_frame_numbers_and_it_is_remembered() {
    assert_eq!(crate::compteur::texte(62, 528, 24, true), "image 63 / 528");
    assert_eq!(crate::compteur::texte(0, 528, 24, false), "0:00:00 / 0:22:00");
    let mut b = banc_dans(film(1), Espace::Storyboard, 1600, 900);
    assert!(!b.app.settings.compteur_en_images);
    let total = b.app.doc.total_frames();
    let temps = crate::compteur::texte(b.app.global_frame(), total, b.app.doc.fps, false);
    assert!(textes(&b).iter().any(|(t, _)| *t == temps), "le temps d'origine : {temps}");
    // Un clic simple ne change rien.
    clic(&mut b, ou("Position").center(), Modifiers::NONE);
    assert!(!b.app.settings.compteur_en_images);
    // Ctrl + clic : les numéros d'images, et c'est écrit dans les réglages.
    clic(&mut b, ou("Position").center(), CTRL);
    assert!(b.app.settings.compteur_en_images);
    b.chauffer(2);
    let images = format!("image {} / {total}", b.app.global_frame() + 1);
    assert!(textes(&b).iter().any(|(t, _)| *t == images), "{images} : {:?}", textes(&b).iter().map(|t| &t.0).filter(|t| t.contains('/')).collect::<Vec<_>>());
    assert!(crate::settings::Settings::load().compteur_en_images, "retenu dans le fichier des réglages");
    let ecrit = toml::to_string(&b.app.settings).unwrap();
    assert!(ecrit.contains("compteur_en_images = true"), "{ecrit}");
    // Encore : le temps.
    clic(&mut b, ou("Position").center(), CTRL);
    assert!(!b.app.settings.compteur_en_images && !crate::settings::Settings::load().compteur_en_images);
}

#[test]
fn ctrl_click_on_the_animation_counter_switches_to_time_code_and_it_is_remembered() {
    let mut b = banc_dans(film(1), Espace::Storyboard, 1600, 900);
    b.app.enter_animation(0);
    b.chauffer(6);
    assert_eq!(b.app.time_display, crate::cues::TimeDisplay::Image);
    clic(&mut b, ou("Dernière image du plan").center(), CTRL);
    assert_eq!(b.app.time_display, crate::cues::TimeDisplay::TimeCode);
    assert!(b.app.settings.compteur_timeline_en_temps && crate::settings::Settings::load().compteur_timeline_en_temps);
    // Une application qui démarre avec ces réglages : le time code.
    let suivante = banc_taille(film(1), false, 800, 600);
    assert_eq!(suivante.app.time_display, crate::cues::TimeDisplay::TimeCode);
    drop(suivante);
    b.chauffer(2);
    clic(&mut b, ou("Dernière image du plan").center(), CTRL);
    assert_eq!(b.app.time_display, crate::cues::TimeDisplay::Image);
    assert!(!crate::settings::Settings::load().compteur_timeline_en_temps);
}

// ------------------------------------------------------------------------------------ la règle et le compteur restent en haut

/// Fait défiler la bande vers le bas : la molette sur elle, jusqu'à ce qu'elle ait bougé.
fn defiler(b: &mut Banc, sur: Pos2) {
    for _ in 0..12 {
        b.image(vec![Event::PointerMoved(sur)], 1.0 / 60.0);
        b.image(vec![Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -120.0), modifiers: Modifiers::NONE }], 1.0 / 60.0);
    }
    b.chauffer(4);
}

#[test]
fn the_animation_ruler_and_counter_stay_at_the_top_when_the_layers_scroll() {
    let mut b = banc_dans(film(14), Espace::Storyboard, 1400, 640);
    b.app.enter_animation(0);
    b.chauffer(6);
    assert!(crate::timeline::REGLE_COLLANTE.with(|c| c.get()).is_none(), "la timeline n'a pas défilé : la règle est à sa place");
    let compteur = ou("Dernière image du plan");
    let grille = b.app.dock.as_ref().and_then(|s| s.find_tab(&Pane::Montage)).map(|(s, n, _)| match &b.app.dock.as_ref().unwrap()[s][n] {
        egui_dock::Node::Leaf { rect, .. } => *rect,
        _ => unreachable!(),
    });
    let bande = grille.expect("la bande du bas");
    defiler(&mut b, pos2(bande.center().x + 200.0, bande.bottom() - 60.0));
    let regle = crate::timeline::REGLE_COLLANTE.with(|c| c.get()).expect("la timeline a défilé : la règle reste en haut");
    assert!(regle.top() >= bande.top() && regle.top() < bande.top() + 30.0, "en haut de la bande : {regle:?} / {bande:?}");
    // Le compteur aussi, à la même hauteur qu'avant le défilement.
    let apres = ou("Dernière image du plan");
    assert!((apres.top() - compteur.top()).abs() < 4.0, "le compteur n'a pas bougé : {compteur:?} → {apres:?}");
    // Un clic sur la règle restée en haut mène la tête de lecture.
    let avant = b.app.cur_frame;
    let x = regle.left() + 260.0;
    clic(&mut b, pos2(x, regle.top() + 6.0), Modifiers::NONE);
    assert_ne!(b.app.cur_frame, avant, "la tête de lecture a suivi le clic sur la règle");
}

#[test]
fn the_storyboard_ruler_and_counter_stay_at_the_top_when_the_tracks_scroll() {
    let mut b = banc_dans(film(14), Espace::Storyboard, 1400, 560);
    b.app.settings.montage_grands = false;
    b.chauffer(4);
    let compteur = ou("Position");
    let (contenu, vue) = crate::montagehauteur::DEFILEMENT.with(|c| c.get());
    assert!(contenu > vue + 20.0, "le banc a une bande qui défile : {contenu} dans {vue}");
    assert!(crate::storyboard::REGLE_COLLANTE_MONTAGE.with(|c| c.get()).is_none());
    defiler(&mut b, pos2(900.0, compteur.bottom() + 120.0));
    let regle = crate::storyboard::REGLE_COLLANTE_MONTAGE.with(|c| c.get()).expect("la bande a défilé : la règle reste en haut");
    assert!(regle.top() <= compteur.top() && compteur.bottom() <= regle.bottom() + 1.0, "en haut, en face du compteur : {regle:?} / {compteur:?}");
    let apres = ou("Position");
    assert!((apres.top() - compteur.top()).abs() < 4.0, "le compteur reste : {compteur:?} → {apres:?}");
    let avant = b.app.global_frame();
    let x = regle.left() + 6.0 + 50.0 * b.app.montage_zoom;
    clic(&mut b, pos2(x, regle.center().y), Modifiers::NONE);
    assert_ne!(b.app.global_frame(), avant, "la tête de lecture a suivi le clic sur la règle");
}

// ------------------------------------------------------------------------------------ le panneau Préréglages : un aperçu, une liste

#[test]
fn the_presets_panel_is_one_preview_over_a_list_of_names() {
    let mut b = banc_dans(film(1), Espace::Storyboard, 1600, 1000);
    b.app.enter_animation(0);
    b.app.show_pane(Pane::Prereglages);
    crate::colonne_tests::toute_la_place(&mut b.app, Pane::Prereglages);
    crate::panneauprereglages::choisir_onglet(&b.app.ctx, crate::panneauprereglages::Onglet::Camera);
    b.chauffer(6);
    // Chaque préréglage a sa ligne, son nom ; un seul aperçu le dit (« ▶ … »).
    let t = textes(&b);
    for p in crate::camfx::Famille::ALL[0].presets().take(3) {
        assert!(t.iter().any(|(x, _)| x == p.label()), "« {} » dans la liste", p.label());
    }
    let premiere = crate::camfx::Famille::ALL[0].presets().next().unwrap().label();
    assert_eq!(crate::panneauprereglages::choisi(&b.app.ctx, crate::panneauprereglages::Onglet::Camera), Some(premiere), "d'origine, le premier");
    assert_eq!(t.iter().filter(|(x, _)| x.starts_with('▶')).count(), 1, "un seul écran d'aperçu");
    let ligne = |b: &Banc, nom: &str| textes(b).into_iter().filter(|(x, _)| x == nom).map(|(_, r)| r).last().unwrap_or_else(|| panic!("« {nom} »"));
    // Le survol d'une ligne : l'aperçu la joue.
    let zoom = crate::camfx::Preset::ZoomOut.label();
    let r = ligne(&b, zoom);
    b.image(vec![Event::PointerMoved(r.center())], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(crate::panneauprereglages::montre(&b.app.ctx), Some(zoom));
    // Ailleurs : le choisi.
    b.image(vec![Event::PointerMoved(pos2(400.0, 300.0))], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(crate::panneauprereglages::montre(&b.app.ctx), Some(premiere));
    // Un clic : il est choisi, et posé à la tête de lecture (un bloc de plus sur la piste Caméra).
    let avant = b.app.doc.panels[0].effets_camera.len();
    let r = ligne(&b, zoom);
    clic(&mut b, r.center(), Modifiers::NONE);
    assert_eq!(crate::panneauprereglages::choisi(&b.app.ctx, crate::panneauprereglages::Onglet::Camera), Some(zoom));
    assert_eq!(b.app.doc.panels[0].effets_camera.len(), avant + 1, "{}", b.app.status);
    b.image(vec![Event::PointerMoved(pos2(400.0, 300.0))], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(crate::panneauprereglages::montre(&b.app.ctx), Some(zoom), "le pointeur parti : l'aperçu montre le choisi");
}

// ------------------------------------------------------------------------------------ plus d'animation d'explication hors du menu Aide

#[test]
fn no_explanation_video_plays_beside_a_tool_it_is_only_in_the_help_menu() {
    assert!(!crate::aideanim::AIDE_HORS_DU_MENU);
    let mut b = banc(crate::document::Document::with_frame(400, 300, 24), false);
    b.app.confort.aide.auto = true;
    b.app.settings.explications_vues.clear();
    b.app.settings.afficher_explications = true;
    for t in [Tool::Brush, Tool::Select, Tool::Eraser, Tool::Fill, Tool::Camera] {
        b.app.set_tool(t);
        b.chauffer(3);
        assert!(!b.app.confort.aide.joue(), "{t:?} : rien ne joue à côté de l'outil");
    }
    assert!(!present("Revoir l'explication"), "plus de ▶ dans les Réglages");
    // Le menu Aide : le registre, où elles sont toutes ; l'ancienne commande y mène aussi.
    b.app.run_action("aide.explications");
    assert!(b.app.confort.aide.registre);
    assert!(crate::aideanim::toutes().count() > 10);
}

// ------------------------------------------------------------------------------------ un panneau ajouté sous Propriétés

#[test]
fn a_panel_added_under_properties_leaves_it_usable() {
    // (Partie 196.5 : Propriétés ne garde plus le tiers de force ; il prend ce qui reste, au moins `GRANDIT_MIN`, et une liste ajoutée
    // à sa hauteur d'origine cède ce qu'il faut pour qu'il les ait.) Le calcul : une liste ajoutée sous la Couleur, dans une colonne
    // basse : la Couleur garde son contenu, la liste rapetisse, Propriétés a son minimum.
    use crate::colonne::Voulue;
    let h = colonne::hauteurs(560.0, &[Some(Voulue::ferme(230.0)), Some(Voulue::souple(colonne::HAUTEUR_DEFAUT)), Some(Voulue::ferme(100.0))], Some(2));
    let reste = 560.0 - 3.0 * colonne::ENTETE - 2.0 * colonne::BORD;
    assert_eq!(h[0], 230.0, "{h:?}");
    assert!(h[1] < colonne::HAUTEUR_DEFAUT && h[1] >= colonne::HAUTEUR_MIN, "la liste cède : {h:?}");
    assert!((h[2] - colonne::GRANDIT_MIN).abs() < 0.5 && h.iter().sum::<f32>() <= reste + 0.5, "{h:?}");
    // À l'écran, dans une fenêtre de 870 de haut, la Couleur repliée : Préréglages ajouté en bas de la pile du Storyboard garde sa
    // hauteur d'origine (230 points, ou son aperçu et trois lignes s'il lui en faut plus), Propriétés le reste, au moins son minimum,
    // sans que la colonne ait à défiler.
    let mut b = banc_dans(film(1), Espace::Storyboard, 1500, 870);
    let couleur = b.app.colonne.rang_de(Pane::Color).expect("la Couleur dans la pile");
    b.app.colonne.panneaux[couleur].replie = true;
    b.app.colonne.ajouter(Pane::Prereglages);
    b.chauffer(8);
    let colonne_rect = b.app.colonne_etat.zone.expect("la colonne");
    let i = b.app.colonne.proprietes().unwrap();
    let (haut, bas) = b.app.colonne_etat.tranches[i];
    assert!(bas - haut >= colonne::GRANDIT_MIN + colonne::ENTETE - 0.5, "Propriétés reste utilisable : {} sur {}", bas - haut, colonne_rect.height());
    let k = b.app.colonne.rang_de(Pane::Prereglages).unwrap();
    let (h0, h1) = b.app.colonne_etat.tranches[k];
    let voulu = colonne::HAUTEUR_DEFAUT.max(crate::panneauprereglages::plancher(&b.app.ctx).unwrap() + 7.0);
    assert!((h1 - h0 - (voulu + colonne::ENTETE + colonne::BORD)).abs() < 1.0, "Préréglages à sa hauteur d'origine : {} / {voulu}", h1 - h0);
    assert!(h1 <= colonne_rect.bottom() + 1.0, "tout tient, la colonne ne défile pas : {h1} / {colonne_rect:?}");
}

// ------------------------------------------------------------------------------------ les barres de la vue

#[test]
fn the_view_bars_have_the_timeline_icon_size_the_new_passe_partout_and_zoom_and_tilt_sliders() {
    let mut b = banc_dans(film(1), Espace::Storyboard, 1600, 900);
    // Des icônes de la taille de celles de la timeline (13 points d'origine, des cases de 20) ; des barres plus minces (26 et 24).
    assert_eq!(crate::tailleicones::icone(), 13.0);
    assert_eq!(crate::viewbar::view_bar_h(), 26.0);
    assert_eq!(crate::viewbar::tools_bar_h(), 24.0);
    for nom in ["Vue du plateau", "Vue caméra", "Ajuster à la fenêtre", "Miroir de la vue", "Cadrage", "Table lumineuse", "Passe-partout"] {
        let r = ou(nom);
        assert!((r.height() - 20.0).abs() < 0.6 && (r.width() - 20.0).abs() < 0.6, "« {nom} » : une case de 20 : {r:?}");
    }
    // Sous le dessin, de gauche à droite après les superpositions : le passe-partout, le zoom, l'inclinaison, la remise droite.
    let suite = ["Table lumineuse", "Passe-partout", "Zoom de la vue", "Inclinaison de la vue", "Remettre la vue droite"].map(ou);
    for k in 1..suite.len() {
        assert!(suite[k].left() > suite[k - 1].left(), "dans l'ordre : {suite:?}");
        assert!((suite[k].center().y - suite[0].center().y).abs() < 3.0, "sur la même ligne");
    }
    // Le zoom par son curseur : la vue suit ; l'inclinaison aussi, et la remise droite la ramène à 0°.
    let z = b.app.view.zoom;
    b.app.zoom_by(2.0);
    b.chauffer(2);
    assert!((b.app.view.zoom - 2.0 * z).abs() < 1e-3);
    b.app.rotate_view(30.0);
    b.chauffer(2);
    clic(&mut b, ou("Remettre la vue droite").center(), Modifiers::NONE);
    assert!(b.app.view.rotation.abs() < 1e-4, "{}", b.app.view.rotation);
    // L'opacité du passe-partout est dans le petit panneau de sa flèche.
    assert!(present("Opacité du passe-partout"));
}

#[test]
fn at_the_16_pt_default_the_timeline_icons_and_both_view_bars_grow_together_with_the_same_margins() {
    // Partie 195 : 16 points d'origine (les essais sans fenêtre gardent 13, la taille de leurs mesures) ; « Par défaut » remet 16.
    assert_eq!(crate::tailleicones::TAILLE_DEFAUT, 16);
    assert_eq!(crate::settings::Settings::default().taille_icones, 16);
    let mut b = banc_dans(film(1), Espace::Storyboard, 1600, 900);
    assert_eq!(b.app.settings.taille_icones, crate::tailleicones::TAILLE_ESSAIS);
    let (barre_13, bas_13) = (crate::viewbar::view_bar_h(), crate::viewbar::tools_bar_h());
    b.app.retablir_taille_icones();
    b.chauffer(4);
    assert_eq!(b.app.settings.taille_icones, 16);
    assert_eq!(crate::tailleicones::icone(), 16.0);
    // Une case de 23 pour une icône de 16 ; les barres gardent leurs marges : 2 points de part et d'autre de la case.
    let case = crate::tailleicones::case_de_barre();
    assert_eq!(case, [23.0, 23.0]);
    assert_eq!(crate::viewbar::tools_bar_h(), case[1] + 4.0);
    assert_eq!(crate::viewbar::view_bar_h(), case[1] + 4.0);
    assert!(crate::viewbar::tools_bar_h() > bas_13 && crate::viewbar::view_bar_h() > barre_13);
    for nom in ["Vue du plateau", "Ajuster à la fenêtre", "Cadrage", "Table lumineuse", "Passe-partout", "Remettre la vue droite"] {
        let r = ou(nom);
        assert!((r.height() - 23.0).abs() < 0.6, "« {nom} » : une case de 23 : {r:?}");
    }
    crate::tailleicones::poser(crate::tailleicones::TAILLE_ESSAIS);
}

#[test]
fn sliders_are_thinner_everywhere() {
    let b = banc(crate::document::Document::with_frame(64, 48, 24), false);
    let style = b.ctx.style();
    assert_eq!(style.spacing.slider_rail_height, crate::theme::RAIL_CURSEUR);
    assert!(crate::theme::RAIL_CURSEUR < 8.0);
    assert_eq!(style.visuals.handle_shape, egui::style::HandleShape::Rect { aspect_ratio: crate::theme::POIGNEE_CURSEUR });
}

// ------------------------------------------------------------------------------------ l'en-tête des Réglages

#[test]
fn the_brush_title_and_its_layer_kind_do_not_touch_in_the_column() {
    let mut b = banc_dans(film(1), Espace::Storyboard, 1600, 900);
    b.app.set_tool(Tool::Brush);
    b.chauffer(4);
    let t = textes(&b);
    let titre = t.iter().find(|(x, _)| x.ends_with(Tool::Brush.label())).map(|(_, r)| *r).expect("le titre « Pinceau »");
    let sous = t.iter().find(|(x, _)| x == "calque vectoriel" || x == "calque bitmap").map(|(_, r)| *r).expect("« calque … »");
    assert!(sous.left() >= titre.right() + 3.0, "« Pinceau » et « calque … » séparés : {titre:?} / {sous:?}");
    let colonne = b.app.colonne_etat.zone.expect("la colonne");
    assert!(sous.right() <= colonne.right() + 0.5, "dans la colonne : {sous:?} / {colonne:?}");
    let _ = Mode::Storyboard;
    let _ = PointerButton::Primary;
}
