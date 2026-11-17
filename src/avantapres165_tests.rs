//! Partie 165 : l'avant | après des Effets montre le vrai calque, calculé en petit au fil de fond, jamais sur celui de l'interface ;
//! le schéma de la partie 164 reste le repli d'un calque vide.

use eframe::egui::{pos2, vec2, Event, Modifiers, Rect};

use crate::avantapres165 as a;
use crate::bulles::vus;
use crate::document::{Document, Layer, LayerKind};
use crate::dock::Pane;
use crate::fx::EffectKind;
use crate::perf_tests::{banc, Banc};
use crate::pistes_tests::{clic, etapes, glisser, pointer_button};
use crate::vector::{Element, VPoint};
use crate::workspace::Espace;

/// Un plan de 960 × 540, un calque vectoriel « Dessin » ; avec `trait_`, un gros trait rouge en travers de la feuille.
fn banc_165(trait_: bool) -> Banc {
    let mut doc = Document::with_frame(960, 540, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Vector)];
    doc.panels[0].set_length(24);
    if trait_ {
        let cel = doc.new_cel(LayerKind::Vector);
        doc.panels[0].layers[0].frames = vec![Some(cel); 24];
        let points = (0..=20).map(|k| VPoint { x: 200.0 + 28.0 * k as f32, y: 270.0 + 60.0 * ((k as f32) * 0.3).sin(), r: 9.0 }).collect();
        let t = Element::Stroke { points, color: [210, 20, 20], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() };
        doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements = vec![t];
        doc.cels.get_mut(&cel).unwrap().rerender();
    }
    let mut b = banc(doc, false);
    b.app.set_espace(Espace::Storyboard);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.chauffer(6);
    b.app.show_pane(Pane::Effects);
    b.chauffer(4);
    b
}

fn ou(nom: &str) -> Rect {
    vus::ou(nom).into_iter().max_by(|a, b| a.center().x.total_cmp(&b.center().x)).unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

/// Laisse le fil de fond finir et l'écran se refaire, jusqu'à ce que l'aperçu soit à jour (ou qu'on abandonne).
fn attendre(b: &mut Banc) -> bool {
    for _ in 0..40 {
        crate::fxcache::settle();
        b.chauffer(2);
        if b.app.effets_ui.apercu.a_jour() && b.app.effets_ui.apercu.images().is_some() {
            return true;
        }
    }
    false
}

/// Les pixels rouges (ceux du trait) d'une image.
fn rouges(im: &eframe::egui::ColorImage) -> usize {
    im.pixels.iter().filter(|c| c.a() > 150 && c.r() > 120 && c.g() < 90 && c.b() < 90).count()
}

/// L'opacité totale d'une image (une lueur en ajoute autour du trait).
fn encre(im: &eframe::egui::ColorImage) -> u64 {
    im.pixels.iter().map(|c| c.a() as u64).sum()
}

fn ajouter_lueur(b: &mut Banc) {
    clic(b, ou("Lueur").center(), Modifiers::NONE);
    assert_eq!(b.app.layers()[0].fx.iter().map(|e| e.kind).collect::<Vec<_>>(), vec![EffectKind::Glow]);
}

#[test]
fn the_before_after_shows_the_real_layer_and_the_after_side_follows_the_strength() {
    let mut b = banc_165(true);
    ajouter_lueur(&mut b);
    assert!(attendre(&mut b), "l'aperçu du vrai calque est arrivé du fil de fond");
    let (avant, apres) = b.app.effets_ui.apercu.images().map(|(x, y)| (x.clone(), y.clone())).unwrap();
    assert!(rouges(&avant) > 50, "l'avant montre les pixels du trait rouge du calque, pas le schéma (un trait gris) : {}", rouges(&avant));
    assert!(avant.size[0] >= 150 && avant.size[0] <= 1000, "une image à peu près de la largeur du panneau : {:?}", avant.size);
    assert!(encre(&apres) > encre(&avant), "l'après a la lueur autour du trait");
    // Plus fort : l'après change, l'avant non.
    b.app.layers_mut()[0].fx[0].set_param(1, 200.0);
    b.chauffer(2);
    assert!(attendre(&mut b));
    let (avant2, apres2) = b.app.effets_ui.apercu.images().map(|(x, y)| (x.clone(), y.clone())).unwrap();
    // (Le cadre peut avoir bougé d'un point avec la largeur du panneau : on compare l'après à l'avant de la même image.)
    assert!(rouges(&avant2) > 50, "l'avant reste le calque sans l'effet");
    let gain = |av: &eframe::egui::ColorImage, ap: &eframe::egui::ColorImage| encre(ap) as f64 / encre(av).max(1) as f64;
    assert!(gain(&avant2, &apres2) > gain(&avant, &apres) * 1.05, "le côté « après » suit la force : {} / {}", gain(&avant2, &apres2), gain(&avant, &apres));
}

#[test]
fn dragging_the_bar_never_renders_on_the_interface_thread_and_keeps_the_last_image() {
    let mut b = banc_165(true);
    ajouter_lueur(&mut b);
    assert!(attendre(&mut b));
    let (avant, apres) = b.app.effets_ui.apercu.images().map(|(x, y)| (x.clone(), y.clone())).unwrap();
    let r = ou("Force de l'effet");
    let (x0, z) = (pos2(r.center().x, r.center().y), pos2(r.left() + r.width() * 0.1, r.center().y));
    // (L'aperçu seul : la vue de dessin de ce banc d'essai, sans carte graphique, compose encore la lueur du calque sur ce fil.)
    let calculs = a::calculs_de_l_apercu();
    let pas = etapes(&b);
    b.image(vec![Event::PointerMoved(x0)], 1.0 / 60.0);
    b.image(vec![pointer_button(x0, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(x0 + (z - x0) * (k as f32 / 8.0))], 1.0 / 60.0);
        assert!(b.app.effets_ui.apercu.images().is_some(), "pendant le glissé, la dernière image prête reste montrée (jamais le schéma)");
    }
    b.image(vec![pointer_button(z, false, Modifiers::NONE)], 1.0 / 60.0);
    assert_eq!(a::calculs_de_l_apercu(), calculs, "aucun effet ni aucune réduction calculés sur le fil de l'interface pendant le geste");
    assert_eq!(etapes(&b) , pas + 1, "le geste reste une seule étape d'annulation");
    assert!(attendre(&mut b), "l'image du nouveau réglage arrive après le geste");
    let (avant2, apres2) = b.app.effets_ui.apercu.images().unwrap();
    let gain = |av: &eframe::egui::ColorImage, ap: &eframe::egui::ColorImage| encre(ap) as f64 / encre(av).max(1) as f64;
    assert!(gain(avant2, apres2) > gain(&avant, &apres) * 1.05, "la lueur plus forte se voit");
}

#[test]
fn an_empty_layer_keeps_the_drawing_of_the_effect() {
    let mut b = banc_165(false);
    ajouter_lueur(&mut b);
    crate::fxcache::settle();
    b.chauffer(4);
    assert!(b.app.effets_ui.apercu.images().is_none(), "rien de dessiné : le schéma de la partie 164 reste");
    assert!(!vus::ou("Force de l'effet").is_empty(), "et la barre se tire toujours");
}

// ------------------------------------------------------------------------------------ 165.2 : la piste du calque, vue Storyboard

fn present(nom: &str) -> bool {
    !vus::ou(nom).is_empty()
}

fn annuler(b: &mut Banc) {
    b.app.run_action("edition.annuler");
    b.chauffer(3);
}

#[test]
fn in_the_storyboard_sliding_on_the_track_thumbnail_sets_the_opacity_in_one_step() {
    let mut b = banc_165(true);
    let r = ou("Opacité du calque");
    assert!(r.width() > 15.0 && r.width() < 40.0, "la miniature de la piste, plus la barre : {r:?}");
    let avant = etapes(&b);
    glisser(&mut b, r.center(), r.center() - vec2(60.0, 0.0));
    let o = b.app.layers()[0].opacity_at(b.app.cur_frame);
    assert!((o - 0.5).abs() < 0.02, "60 points à gauche : la moitié de l'opacité ({o})");
    assert_eq!(etapes(&b), avant + 1, "un geste, une étape");
    clic(&mut b, ou("Opacité du calque").center(), Modifiers::NONE);
    assert_eq!(etapes(&b), avant + 1, "un simple clic ne change rien");
    annuler(&mut b);
    assert_eq!(b.app.layers()[0].opacity_at(b.app.cur_frame), 1.0, "Ctrl+Z rend l'opacité");
}

#[test]
fn in_the_storyboard_a_blend_pastille_of_the_track_sets_the_mode_in_one_step() {
    let mut b = banc_165(true);
    assert_eq!(vus::ou("Mode de fusion").len(), 1, "la pastille de la piste active seule");
    assert!(!present("Mode : Produit"), "les pastilles s'ouvrent au clic");
    clic(&mut b, ou("Mode de fusion").center(), Modifiers::NONE);
    for m in crate::blend::BlendMode::ALL {
        assert!(present(&format!("Mode : {}", m.label())), "{} : {:?}", m.label(), vus::noms());
    }
    let avant = etapes(&b);
    clic(&mut b, ou("Mode : Produit").center(), Modifiers::NONE);
    assert_eq!(b.app.layers()[0].blend, crate::blend::BlendMode::Multiply);
    assert_eq!(etapes(&b), avant + 1, "une étape");
    assert!(!present("Mode : Produit"), "les pastilles se referment");
    annuler(&mut b);
    assert_eq!(b.app.layers()[0].blend, crate::blend::BlendMode::Normal, "Ctrl+Z rend le mode");
}

#[test]
fn in_a_narrow_track_column_the_thumbnail_and_the_pastille_stay_reachable() {
    let mut b = banc_165(true);
    // La colonne la plus étroite : une petite miniature à la place de l'ancienne barre, on y glisse de même ; et la pastille du mode.
    b.app.montage_head_w = crate::montagehead::HEAD_MIN;
    b.chauffer(4);
    let r = ou("Opacité du calque");
    assert!(r.width() >= 10.0, "{r:?}");
    let avant = etapes(&b);
    glisser(&mut b, r.center(), r.center() - vec2(60.0, 0.0));
    let o = b.app.layers()[0].opacity_at(b.app.cur_frame);
    assert!((o - 0.5).abs() < 0.02, "60 points à gauche : la moitié ({o})");
    assert_eq!(etapes(&b), avant + 1);
    annuler(&mut b);
    clic(&mut b, ou("Mode de fusion").center(), Modifiers::NONE);
    assert!(present("Mode : Écran") || present("Mode : Produit"), "les pastilles, même dans la colonne étroite : {:?}", vus::noms());
    // Et toujours le menu du calque (clic droit ou ⋮) : « Apparence » y garde l'opacité et le mode en chiffres (`menucalque_tests`).
    assert!(present("Commandes du calque"));
}

#[test]
fn the_framing_follows_the_drawing_and_the_size_the_panel() {
    // Un dessin dans un coin : le cadre l'entoure, de la forme de l'aperçu, dans la feuille.
    let c = a::cadrage(Some((0, 0, 0, 0)), (1920, 1080), 3.0).unwrap();
    assert!(c.left() >= 0.0 && c.top() >= 0.0 && c.right() <= 1920.0 && c.bottom() <= 1080.0, "{c:?}");
    assert!((c.width() / c.height() - 3.0).abs() < 0.01, "la forme de l'aperçu : {c:?}");
    assert!(c.width() >= 1920.0 * 0.35 - 0.5, "au moins un bon tiers de la feuille : {c:?}");
    // Toute la feuille dessinée, un aperçu plus haut que large : la feuille entière, pas plus.
    let c = a::cadrage(Some((-4, -4, 40, 40)), (1920, 1080), 0.5).unwrap();
    assert!(c.width() <= 1920.0 && c.height() <= 1080.0);
    assert!(a::cadrage(None, (1920, 1080), 3.0).is_none(), "rien de dessiné : pas de cadre");
    // La taille de calcul : à peu près la largeur du panneau, jamais plus petite.
    assert_eq!(a::niveau(1920, 340.0), 2, "1 920 px montrés sur 340 : un quart (480 px)");
    assert_eq!(a::niveau(1920, 2000.0), 0);
    assert_eq!(a::niveau(64, 340.0), 0);
}

#[test]
fn an_effect_that_cannot_be_shown_small_keeps_the_drawing() {
    // Un effet du trait (il lit les traits en pleine résolution) : le schéma, pas deux moitiés pareilles.
    let mut l = Layer::new("Dessin", LayerKind::Vector);
    l.fx.push(crate::fx::Effect::new(EffectKind::Sketch));
    assert!(a::piles(&l, 0, 0).is_none());
    // Une lueur, si : sans elle d'un côté, avec elle de l'autre.
    l.fx = vec![crate::fx::Effect::new(EffectKind::Glow)];
    let (sans, avec) = a::piles(&l, 0, 0).unwrap();
    assert!(sans.is_empty() && avec.len() == 1 && avec[0].kind == EffectKind::Glow);
}
