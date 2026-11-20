//! Partie 164 : la Couleur, les Effets et la ligne du calque, plus visuels. Chaque dessin, manipulé à la souris dans la vraie
//! fenêtre sans écran (`Banc`), change le projet par le même chemin que le réglage qu'il remplace, en une étape d'annulation que
//! Ctrl+Z défait ; chaque ancien réglage reste joignable (dessin, « Précis », clic droit).

use eframe::egui::{pos2, vec2, Modifiers, Rect};

use crate::app::Tool;
use crate::bulles::vus;
use crate::document::LayerKind;
use crate::dock::Pane;
use crate::fx::EffectKind;
use crate::perf_tests::{banc, Banc};
use crate::pistes_tests::{clic, etapes, glisser};
use crate::vector::{Element, VPoint};
use crate::visuels164 as v;
use crate::vtools::VSelection;
use crate::workspace::Espace;

fn banc_164(espace: Espace) -> Banc {
    let mut doc = crate::pistes_tests::doc_with(&[&["Dessin"], &["Seul"]]);
    doc.panels[0].set_length(24);
    let mut b = banc(doc, false);
    b.app.set_espace(espace);
    // (Partie 197 : cet essai porte sur les autres panneaux de la pile ; la Scène, en haut de celle du Storyboard, en est retirée.)
    crate::colonne_tests::sans_la_scene(&mut b.app);
    b.chauffer(6);
    b
}

/// Le rectangle de l'élément dont la bulle s'appelle `nom`, à la dernière image (le plus à droite : la colonne des panneaux).
fn ou(nom: &str) -> Rect {
    vus::ou(nom).into_iter().max_by(|a, b| a.center().x.total_cmp(&b.center().x)).unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

fn present(nom: &str) -> bool {
    !vus::ou(nom).is_empty()
}


fn annuler(b: &mut Banc) {
    b.app.run_action("edition.annuler");
    b.chauffer(3);
}

// ------------------------------------------------------------------------------------ les calculs

#[test]
fn the_force_of_an_effect_and_the_bar() {
    assert_eq!(v::force_de(EffectKind::Glow), Some((1, 0.0, 200.0)), "la lueur : son intensité");
    assert_eq!(v::force_de(EffectKind::Grain).map(|f| f.0), Some(0), "le grain : sa quantité");
    assert_eq!(v::force_de(EffectKind::Vignette).map(|f| f.0), Some(0));
    assert_eq!(v::force_de(EffectKind::GaussianBlur), Some((0, 0.0, 40.0)), "le flou : son rayon, jusqu'à 40 px sur la barre");
    assert!(v::force_de(EffectKind::Wave).is_none(), "une onde n'a pas de force");
    let r = Rect::from_min_size(pos2(100.0, 0.0), vec2(200.0, 50.0));
    assert_eq!(v::force_a(100.0, r), 1.0, "la barre tout à gauche : tout l'après, la force entière");
    assert_eq!(v::force_a(300.0, r), 0.0);
    assert!((v::force_a(200.0, r) - 0.5).abs() < 1e-6);
    assert_eq!(v::fraction(100.0, 0.0, 200.0), 0.5);
    // Chaque vignette d'« Ajouter » est un effet que FEANOR sait faire, avec un nom court.
    for k in v::EN_VIGNETTES {
        assert!(EffectKind::ALL.contains(&k));
        assert!(!v::nom_court(k).is_empty());
    }
}

#[test]
fn before_now_keeps_the_colour_of_before_the_last_finished_choice() {
    let mut am = v::AvantMaintenant::default();
    am.suivre([1, 1, 1], true);
    assert_eq!(am.avant, None, "au début : pas encore d'ancienne");
    am.suivre([2, 2, 2], false);
    am.suivre([3, 3, 3], false);
    assert_eq!(am.avant, None, "pendant le geste, rien ne bouge");
    am.suivre([3, 3, 3], true);
    assert_eq!(am.avant, Some([1, 1, 1]), "le geste fini : la couleur d'avant le geste");
    am.suivre([1, 1, 1], true);
    assert_eq!(am.avant, Some([3, 3, 3]), "revenir à l'ancienne : l'autre devient l'ancienne");
}

// ------------------------------------------------------------------------------------ 164.1 : la Couleur

/// Un dessin vectoriel sur le calque du plan 0 : un trait rouge, choisi avec l'outil Sélection (le choix d'une couleur le recolore,
/// une étape).
fn avec_un_trait_choisi(b: &mut Banc) -> crate::document::CelId {
    let cel = b.app.doc.new_cel(LayerKind::Vector);
    b.app.doc.panels[0].layers[0].frames = vec![Some(cel); 24];
    let trait_ = Element::Stroke { points: vec![VPoint { x: 4.0, y: 10.0, r: 2.0 }, VPoint { x: 50.0, y: 10.0, r: 2.0 }], color: [200, 0, 0], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() };
    b.app.doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements = vec![trait_];
    b.app.doc.cels.get_mut(&cel).unwrap().rerender();
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.tool = Tool::Select;
    b.app.vsel = Some(VSelection { cel, items: vec![0] });
    b.chauffer(4);
    cel
}

fn couleur_du_trait(b: &Banc, cel: crate::document::CelId) -> [u8; 3] {
    b.app.drawing(cel).unwrap().elements[0].color()
}

#[test]
fn the_wheel_changes_the_colour_in_one_step_and_the_old_spot_comes_back() {
    let mut b = banc_164(Espace::Storyboard);
    let cel = avec_un_trait_choisi(&mut b);
    b.app.color = [200, 0, 0];
    b.app.couleurs.prefs.wheel_shown = true;
    b.chauffer(4);
    assert!(b.app.pane_shown(Pane::Color));
    let roue = ou("Roue chromatique");
    assert!(roue.width() >= 80.0 && roue.width() <= 112.5, "la roue en tête, à côté d'« avant · maintenant » : {roue:?}");
    let avant = etapes(&b);
    // Un clic dans l'anneau, à la teinte verte : la couleur change, le trait choisi la prend.
    let vert = crate::roue::Wheel::in_rect(roue).ring_point(120.0);
    clic(&mut b, vert, Modifiers::NONE);
    let hsv = crate::colorui::rgb_to_hsv(b.app.color);
    assert!((hsv[0] - 120.0).abs() < 8.0, "la teinte de l'anneau : {:?} {hsv:?}", b.app.color);
    assert_eq!(couleur_du_trait(&b, cel), b.app.color, "le trait choisi a pris la couleur");
    assert_eq!(etapes(&b), avant + 1, "une seule étape pour le clic");
    // Glisser dans le triangle : un geste, une étape.
    let centre = roue.center();
    glisser(&mut b, centre, centre + vec2(6.0, 8.0));
    assert_eq!(etapes(&b), avant + 2, "un glissé, une étape");
    annuler(&mut b);
    annuler(&mut b);
    assert_eq!(couleur_du_trait(&b, cel), [200, 0, 0], "Ctrl+Z rend la couleur du trait");
    // (Partie 195 : plus de taches « Avant · maintenant » ; la couleur d'avant reste suivie, sans tache à l'écran.)
    b.app.vsel = None;
    b.app.choose_front([10, 20, 200]);
    b.chauffer(3);
    b.app.choose_front([240, 200, 10]);
    b.chauffer(3);
    assert_eq!(b.app.couleurs.avant_maintenant.avant, Some([10, 20, 200]));
    assert!(!present("Couleur d'avant") && !present("Couleur de maintenant"));
}

#[test]
fn the_old_spot_is_gone_from_the_colour_panel() {
    // Partie 195 (Anthony, sur la capture du panneau Couleur) : les taches « Avant · maintenant » et la ligne « clic sur l'ancienne
    // pour revenir » sont retirées ; les deux carrés d'avant-plan et d'arrière-plan restent.
    let mut b = banc_164(Espace::Storyboard);
    b.app.color = [200, 0, 0];
    b.chauffer(3);
    b.app.choose_front([0, 0, 200]);
    b.chauffer(3);
    assert!(!present("Couleur d'avant") && !present("Couleur de maintenant"), "{:?}", vus::noms());
    assert!(present("Couleur d'avant-plan") && present("Couleur d'arrière-plan"));
}

#[test]
fn every_old_colour_setting_is_still_reached() {
    let mut b = banc_164(Espace::Storyboard);
    b.app.couleurs.prefs.more_open = false;
    b.app.couleurs.prefs.wheel_shown = true;
    b.chauffer(4);
    // Toujours visibles : la roue, les deux taches, les deux carrés, les neutres et le nuancier.
    // (Partie 195 : sans les taches « Avant · maintenant ».)
    for nom in ["Roue chromatique", "Couleur d'avant-plan", "Couleur d'arrière-plan", "Précis : Couleur"] {
        assert!(present(nom), "« {nom} » : {:?}", vus::noms());
    }
    assert!(!present("Code hexadécimal") && !present("Nuancier montré"), "les chiffres et les commandes du nuancier sous Précis, replié");
    clic(&mut b, ou("Précis : Couleur").center(), Modifiers::NONE);
    assert!(b.app.couleurs.prefs.more_open, "Précis s'ouvre, retenu dans les réglages de l'application (l'ancien « Plus de couleurs »)");
    b.chauffer(2);
    for nom in [
        "Nuancier montré",
        "Commandes du nuancier",
        "Ajouter au nuancier",
        "Retirer du nuancier",
        "Curseurs",
        "Carré",
        "Triangle",
        "Teinte",
        "Saturation",
        "Valeur (clarté)",
        "Code hexadécimal",
        "Copier le code",
        "Couleurs récentes",
        "Déplier « Palette de mélange »",
    ] {
        assert!(present(nom), "« {nom} » sous Précis : {:?}", vus::noms());
    }
    // Les curseurs à la place de la roue : en haut, et toujours « avant · maintenant ».
    b.app.couleurs.prefs.wheel_shown = false;
    b.chauffer(3);
    assert!(vus::ou("Roue chromatique").iter().all(|r| r.width() < 40.0), "un seul à la fois : la roue n'est plus dessinée");
    for nom in ["Curseurs TSV", "Curseurs RVB", "Curseurs TSL", "Couleur d'avant-plan"] {
        assert!(present(nom), "« {nom} » : {:?}", vus::noms());
    }
}

// ------------------------------------------------------------------------------------ 164.2 : les Effets

fn effets(b: &mut Banc) {
    b.app.show_pane(Pane::Effects);
    b.chauffer(4);
    assert!(present("Ajouter : Lueur") || present("Lueur"), "les vignettes d'effets : {:?}", vus::noms());
}

#[test]
fn an_effect_thumbnail_adds_it_and_the_strength_bar_sets_it_in_one_step() {
    let mut b = banc_164(Espace::Storyboard);
    effets(&mut b);
    let avant = etapes(&b);
    clic(&mut b, ou("Lueur").center(), Modifiers::NONE);
    let fx = &b.app.layers()[b.app.cur_layer].fx;
    assert_eq!(fx.iter().map(|e| e.kind).collect::<Vec<_>>(), vec![EffectKind::Glow], "la vignette a ajouté la lueur");
    assert_eq!(etapes(&b), avant + 1, "une étape");
    // Un deuxième clic sur la vignette allumée ne l'ajoute pas deux fois : il la choisit.
    clic(&mut b, ou("Lueur").center(), Modifiers::NONE);
    assert_eq!(b.app.layers()[b.app.cur_layer].fx.len(), 1);
    assert_eq!(etapes(&b), avant + 1);
    // L'avant | après : tirer la barre vers la gauche donne plus de place à l'après, la lueur est plus forte.
    let r = ou("Force de l'effet");
    let intensite = |b: &Banc| b.app.layers()[b.app.cur_layer].fx[0].values_at(b.app.cur_frame)[1];
    assert_eq!(intensite(&b), 100.0, "la force d'origine (100 %, la moitié de la barre)");
    let x = r.left() + r.width() * 0.5;
    let avant = etapes(&b);
    glisser(&mut b, pos2(x, r.center().y), pos2(r.left() + r.width() * 0.2, r.center().y));
    let attendu = 200.0 * (1.0 - 0.2);
    assert!((intensite(&b) - attendu).abs() < 4.0, "la barre règle l'intensité : {} / {attendu}", intensite(&b));
    assert_eq!(etapes(&b), avant + 1, "un geste, une étape");
    annuler(&mut b);
    assert_eq!(intensite(&b), 100.0, "Ctrl+Z rend la force");
    // Un appui immobile quelques images, puis un glissé : toujours une seule étape.
    let avant = etapes(&b);
    let (a, z) = (pos2(x, r.center().y), pos2(r.left() + r.width() * 0.3, r.center().y));
    b.image(vec![eframe::egui::Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![crate::pistes_tests::pointer_button(a, true, Modifiers::NONE)], 1.0 / 60.0);
    for _ in 0..4 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    for k in 1..=6 {
        b.image(vec![eframe::egui::Event::PointerMoved(a + (z - a) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    b.image(vec![crate::pistes_tests::pointer_button(z, false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(3);
    assert_eq!(etapes(&b), avant + 1, "appui immobile puis glissé : une étape");
    annuler(&mut b);
    assert_eq!(intensite(&b), 100.0);
    annuler(&mut b);
    assert!(b.app.layers()[b.app.cur_layer].fx.is_empty(), "Ctrl+Z retire l'effet ajouté par la vignette");
}

#[test]
fn every_old_effect_setting_is_still_reached() {
    let mut b = banc_164(Espace::Storyboard);
    effets(&mut b);
    clic(&mut b, ou("Grain de film").center(), Modifiers::NONE);
    assert_eq!(b.app.layers()[b.app.cur_layer].fx[0].kind, EffectKind::Grain);
    // Visibles : les effets posés (allumer, choisir, retirer) et l'avant | après.
    for nom in ["Effet actif", "Supprimer l'effet", "Force de l'effet", "Précis : Effets"] {
        assert!(present(nom), "« {nom} » : {:?}", vus::noms());
    }
    assert!(present("Choisir l'effet") || present("Effet choisi"));
    assert!(!present("Création automatique des clés") && !present("Ajouter un effet"), "sous Précis, replié");
    // (Le titre « Précis » peut être sous le bas visible du panneau, qui défile : on l'ouvre sans clic, comme le ferait un clic.)
    crate::panneauxvisuels::ouvrir_precis(&b.ctx, "Effets", true);
    b.chauffer(3);
    for nom in [
        "Ajouter un effet",
        "Préréglages d'effets",
        "Tout réinitialiser",
        "Tout supprimer",
        "Effets du trait",
        "Effets de matière",
        "Création automatique des clés",
        "Clés d'effets dans la timeline et le montage",
        "Replier l'effet",
        "Créer une clé",
    ] {
        assert!(present(nom), "« {nom} » sous Précis : {:?}", vus::noms());
    }
    // Sur un calque bitmap, pas de vignette de tremblement (il ne va que sur le vectoriel).
    b.app.doc.panels[0].layers[0].kind = LayerKind::Bitmap;
    b.chauffer(3);
    assert!(!present("Tremblement du trait") && present("Lueur"), "{:?}", vus::noms());
}

// ------------------------------------------------------------------------------------ 164.3 : retirée (partie 165.3)

// (Les pastilles des modes de fusion et l'opacité sur la miniature sont retirées à la demande d'Anthony : la ligne du calque a
// retrouvé « 100 % » et « Normal ▾ », `avantapres165_tests::the_layer_rows_show_100_and_normal_again`.)

// ------------------------------------------------------------------------------------ 164.4 : un seul « Précis »

#[test]
fn one_precis_title_everywhere() {
    // Les replis des parties 162, 163 et 164 passent par le même titre peint (un triangle, pas un signe d'une police).
    for (nom, source) in [("espace3dgraphes.rs", include_str!("espace3dgraphes.rs")), ("panneauxvisuels.rs", include_str!("panneauxvisuels.rs")), ("colorui.rs", include_str!("colorui.rs")), ("fxui.rs", include_str!("fxui.rs"))] {
        assert!(source.contains("titre_precis(") || source.contains("precis_entete("), "{nom}");
        assert!(!source.contains("} Précis"), "{nom} : un titre « Précis » écrit avec une flèche de police");
    }
}
