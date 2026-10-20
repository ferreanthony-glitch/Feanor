//! Calques d'effets (`calqueffet.rs`) : un calque d'ajustement n'agit que sur ce qui est dessous (dans
//! son plan, ou dans son dossier), aux images où il a une exposition, avec son opacité et son mode ;
//! l'écran (fil de fond, jamais bloquant) et l'export (exact) montrent le même dessin ; les calques
//! d'effets s'enregistrent et se relisent.

use std::borrow::Cow;

use eframe::egui::Color32;

use crate::blend::BlendMode;
use crate::calqueffet::{self, CalqueEffet, Contenu, Entree, Mode};
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::fx::{Effect, EffectKind};
use crate::incrustation::{Ancre, Incrustation, Portee, Quand, Source};
use crate::render::{self, Plan};

const W: usize = 192;
const H: usize = 128;
const RED: Color32 = Color32::from_rgb(255, 0, 0);
const BLUE: Color32 = Color32::from_rgb(0, 0, 255);

/// Un projet de 192 × 128 (cadre = feuille), 12 images, sans calque.
fn doc() -> Document {
    // (Les dessins dérivés sont dans le cache commun à tous les essais : un essai qui en calcule ne tourne pas
    // pendant qu'un autre le vide, `fxcache::monde`.)
    crate::fxcache::monde::lire();
    let mut d = Document::new(W, H, 24);
    d.panels[0].layers.clear();
    d.panels[0].length = 12;
    d
}

/// Un dessin bitmap : un rectangle (x, y, largeur, hauteur) de la couleur `c`.
fn painted(doc: &mut Document, (x0, y0, w, h): (usize, usize, usize, usize), c: Color32) -> CelId {
    let id = doc.new_cel(LayerKind::Bitmap);
    let r = &mut doc.cels.get_mut(&id).unwrap().raster;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let i = r.tile_index_of(x, y);
            r.tile_mut(i)[(y % 64) * 64 + x % 64] = c;
        }
    }
    id
}

fn layer(name: &str, cel: CelId, frames: usize) -> Layer {
    let mut l = Layer::new(name, LayerKind::Bitmap);
    l.frames = vec![Some(cel); frames];
    l
}

/// Un calque d'ajustement dont les images `from..to` ont une exposition, avec ces effets.
fn adjustment(doc: &mut Document, name: &str, fx: Vec<Effect>, from: usize, to: usize) -> Layer {
    let cel = doc.new_cel(LayerKind::Bitmap);
    let mut l = Layer::new_effect(name, CalqueEffet::ajustement(""));
    l.frames = [vec![None; from], vec![Some(cel); to - from]].concat();
    l.fx = fx;
    l
}

fn invert() -> Effect {
    Effect::new(EffectKind::Invert)
}

/// L'image de la feuille, comme l'export la compose (papier compris).
fn flat(doc: &Document, frame: usize) -> Vec<Color32> {
    let plan = render::panel_plan_at(doc, 0, frame, doc.render_paper());
    render::composite_full(doc, &plan)
}

fn at(buf: &[Color32], x: usize, y: usize) -> Color32 {
    buf[y * W + x]
}

#[test]
fn an_effect_layer_changes_only_what_is_below_it() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let b = painted(&mut d, (100, 10, 30, 30), BLUE);
    let e = adjustment(&mut d, "Négatif", vec![invert()], 0, 12);
    // De bas en haut : A, l'effet, B.
    d.panels[0].layers = vec![layer("A", a, 12), e, layer("B", b, 12)];
    let buf = flat(&d, 0);
    assert_eq!(at(&buf, 20, 20), Color32::from_rgb(0, 255, 255), "A, dessous : inversé");
    assert_eq!(at(&buf, 110, 20), BLUE, "B, dessus : intact");
    assert_eq!(at(&buf, 70, 90), Color32::BLACK, "le papier, dessous aussi : inversé");
}

#[test]
fn an_effect_layer_at_the_bottom_of_a_folder_covers_only_the_folder() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let c = painted(&mut d, (100, 10, 30, 30), RED);
    let e = adjustment(&mut d, "Négatif du dossier", vec![invert()], 0, 12);
    let mut folder = Layer::new_folder("Dossier");
    let (mut lc, mut le) = (layer("C", c, 12), e);
    lc.parent = Some(folder.id);
    le.parent = Some(folder.id);
    folder.collapsed = false;
    // De bas en haut : A (hors dossier), les calques du dossier (C, l'effet), puis le dossier.
    d.panels[0].layers = vec![layer("A", a, 12), lc, le, folder];
    crate::folders::normalize(&mut d.panels[0].layers);
    let buf = flat(&d, 0);
    assert_eq!(at(&buf, 20, 20), RED, "A est hors du dossier : intact");
    assert_eq!(at(&buf, 110, 20), Color32::from_rgb(0, 255, 255), "C est dans le dossier : inversé");
    assert_eq!(at(&buf, 70, 90), Color32::WHITE, "le papier n'est pas dans le dossier : intact");
}

#[test]
fn it_acts_only_on_the_frames_where_it_has_an_exposure() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let e = adjustment(&mut d, "Négatif", vec![invert()], 3, 5);
    d.panels[0].layers = vec![layer("A", a, 12), e];
    for (frame, inverted) in [(0, false), (2, false), (3, true), (4, true), (5, false), (11, false)] {
        let px = at(&flat(&d, frame), 20, 20);
        assert_eq!(px, if inverted { Color32::from_rgb(0, 255, 255) } else { RED }, "image {frame}");
    }
}

#[test]
fn a_hidden_effect_layer_or_one_without_a_visible_effect_does_nothing() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let mut hidden = adjustment(&mut d, "Caché", vec![invert()], 0, 12);
    hidden.visible = false;
    let mut idle = invert();
    idle.values[0] = 0.0;
    let nothing = adjustment(&mut d, "Rien", vec![idle], 0, 12);
    let mut off = invert();
    off.enabled = false;
    let disabled = adjustment(&mut d, "Éteint", vec![off], 0, 12);
    d.panels[0].layers = vec![layer("A", a, 12), hidden, nothing, disabled];
    let plan = render::panel_plan_at(&d, 0, 0, d.render_paper());
    assert!(!calqueffet::has_markers(&plan), "aucun repère : rien à calculer");
    assert_eq!(at(&flat(&d, 0), 20, 20), RED);
}

#[test]
fn its_opacity_and_blend_mode_apply_to_the_effect() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let mut e = adjustment(&mut d, "Négatif", vec![invert()], 0, 12);
    e.opacity = 0.5;
    d.panels[0].layers = vec![layer("A", a, 12), e.clone()];
    // Le papier blanc, inversé en noir à moitié : un gris.
    let grey = at(&flat(&d, 0), 70, 90);
    assert!((grey.r() as i32 - 127).abs() <= 2 && grey.a() == 255, "{grey:?}");
    // Mode Écran : l'écran du négatif sur l'original ne peut qu'éclaircir.
    e.opacity = 1.0;
    e.blend = BlendMode::Screen;
    d.panels[0].layers = vec![layer("A", a, 12), e];
    let screen = at(&flat(&d, 0), 20, 20);
    assert!(screen.r() == 255 && screen.g() == 255 && screen.b() == 255, "rouge écran cyan = blanc : {screen:?}");
}

#[test]
fn effect_layers_chain_from_bottom_to_top() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let e1 = adjustment(&mut d, "Négatif 1", vec![invert()], 0, 12);
    let e2 = adjustment(&mut d, "Négatif 2", vec![invert()], 0, 12);
    // Deux négatifs de suite : l'image d'origine.
    d.panels[0].layers = vec![layer("A", a, 12), e1, e2];
    let buf = flat(&d, 0);
    assert_eq!(at(&buf, 20, 20), RED);
    assert_eq!(at(&buf, 70, 90), Color32::WHITE);
    // Un seul : inversé.
    let e1 = d.panels[0].layers[1].clone();
    d.panels[0].layers = vec![layer("A", a, 12), e1];
    assert_eq!(at(&flat(&d, 0), 20, 20), Color32::from_rgb(0, 255, 255));
}

#[test]
fn an_effect_layer_works_on_the_result_of_the_effects_of_the_layers_below() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let mut below = layer("A", a, 12);
    below.fx = vec![invert()];
    let e = adjustment(&mut d, "Négatif", vec![invert()], 0, 12);
    d.panels[0].layers = vec![below, e];
    assert_eq!(at(&flat(&d, 0), 20, 20), RED, "deux négatifs : l'original");
}

#[test]
fn an_unresolved_marker_shows_nothing_and_never_breaks_the_composition() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let e = adjustment(&mut d, "Négatif", vec![invert()], 0, 12);
    d.panels[0].layers = vec![layer("A", a, 12), e];
    let plan = render::panel_plan_at(&d, 0, 0, d.render_paper());
    assert!(calqueffet::has_markers(&plan));
    // `composite_tile` ne résout rien : le repère est sauté, ce qu'il couvre se montre tel quel.
    let mut buf = vec![Color32::TRANSPARENT; W * H];
    for idx in 0..(W.div_ceil(64) * H.div_ceil(64)) {
        render::composite_tile(&d, &plan, idx, &mut buf);
    }
    assert_eq!(at(&buf, 20, 20), RED);
}

#[test]
fn the_screen_shows_the_same_picture_as_the_export_and_never_waits_for_it() {
    crate::fxcache::monde::ecrire();
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let e = adjustment(&mut d, "Impact", vec![Effect::new(EffectKind::Invert), {
        let mut s = Effect::new(EffectKind::Shake);
        s.set_param(0, 7.0);
        s
    }], 0, 12);
    d.panels[0].layers = vec![layer("A", a, 12), e];
    let plan = render::panel_plan_at(&d, 0, 0, d.render_paper());
    // L'export : calcul tout de suite, exact.
    let exact = render::composite_full(&d, &plan);
    assert_eq!(at(&exact, 27, 20), Color32::from_rgb(0, 255, 255), "inversé et décalé de 7 à droite");
    // L'écran, cache vide : rien n'est calculé ici, ce que l'effet couvre se montre tel quel.
    crate::fx::forget_all();
    let first = calqueffet::resolve(&d, &plan, Mode::Display { level: 0, prio: 0 });
    assert!(matches!(first, Cow::Owned(_)));
    assert!(first.items.iter().all(|i| i.cel & (1 << 63) == 0), "pas encore de dessin dérivé");
    assert_eq!(first.items.len(), 1, "le dessin de A, sans le repère");
    // Le fil de fond finit, l'écran montre le dessin dérivé : le même que l'export, pixel pour pixel.
    crate::fxcache::settle();
    let second = calqueffet::resolve(&d, &plan, Mode::Display { level: 0, prio: 0 });
    assert_eq!(second.items.len(), 1);
    assert!(second.items[0].cel & (1 << 63) != 0, "le dessin dérivé");
    assert_eq!(render::composite_full(&d, &second), exact);
}

#[test]
fn the_preview_level_gives_a_smaller_picture_of_the_same_scene() {
    crate::fxcache::monde::ecrire();
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 40, 40), RED);
    let e = adjustment(&mut d, "Négatif", vec![invert()], 0, 12);
    d.panels[0].layers = vec![layer("A", a, 12), e];
    let plan = render::panel_plan_at(&d, 0, 0, d.render_paper());
    let mode = Mode::Display { level: 1, prio: 0 };
    let _ = calqueffet::resolve(&d, &plan, mode);
    crate::fxcache::settle();
    let shown = calqueffet::resolve(&d, &plan, mode);
    assert_eq!(shown.items.len(), 1);
    let cel = calqueffet::derived(shown.items[0].cel).expect("dessin dérivé");
    assert_eq!((cel.raster.width, cel.raster.height), (W / 2, H / 2));
    assert_eq!(cel.raster.get(15, 15), Color32::from_rgb(0, 255, 255), "le rouge, inversé, à moitié de la taille");
    assert_eq!(cel.raster.get(70, 50), Color32::BLACK);
}

#[test]
fn the_preview_of_an_effect_layer_is_cheap_enough_for_24_frames_a_second() {
    crate::fxcache::monde::ecrire();
    // Au repos (ni aperçu demandé, ni geste en cours) : la pleine résolution, quelle que soit la feuille.
    assert_eq!(calqueffet::proxy_level(0, 3840, 2160), 0);
    // Une feuille 4K en aperçu : le quart de la résolution (518 400 pixels) ; une feuille plus petite : la moitié.
    assert_eq!(calqueffet::proxy_level(1, 3840, 2160), 2);
    assert_eq!(calqueffet::proxy_level(2, 3840, 2160), 2);
    assert_eq!(calqueffet::proxy_level(3, 3840, 2160), 3);
    assert_eq!(calqueffet::proxy_level(1, 2560, 1440), 1);
    assert_eq!(calqueffet::proxy_level(1, 1920, 1080), 1);
    // Un geste ou la lecture à un zoom élevé : l'aperçu y est quand même.
    crate::fx::note_interaction(&eframe::egui::Context::default());
    assert_eq!(calqueffet::proxy_level(0, 3840, 2160), 2);
    assert_eq!(calqueffet::proxy_level(0, 1920, 1080), 1);
    crate::fx::reset_draft();
    assert_eq!(calqueffet::proxy_level(0, 3840, 2160), 0);
}

#[test]
fn changing_what_is_below_makes_a_new_picture() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let e = adjustment(&mut d, "Négatif", vec![invert()], 0, 12);
    d.panels[0].layers = vec![layer("A", a, 12), e];
    assert_eq!(at(&flat(&d, 0), 60, 20), Color32::BLACK);
    // Un trait de plus sur A (la version du dessin change) : le dessin dérivé suit.
    let cel = d.cels.get_mut(&a).unwrap();
    let i = cel.raster.tile_index_of(60, 20);
    cel.raster.tile_mut(i)[(20 % 64) * 64 + 60 % 64] = RED;
    cel.version += 1;
    assert_eq!(at(&flat(&d, 0), 60, 20), Color32::from_rgb(0, 255, 255));
}

#[test]
fn what_is_ready_is_sent_ahead_to_the_graphics_card() {
    crate::fxcache::monde::ecrire();
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let e = adjustment(&mut d, "Négatif", vec![invert()], 0, 12);
    d.panels[0].layers = vec![layer("A", a, 12), e];
    let plan = render::panel_plan_at(&d, 0, 1, d.render_paper());
    assert!(calqueffet::ahead(&d, &plan, 1, 0).is_empty(), "pas encore calculé : demandé au fil de fond");
    crate::fxcache::settle();
    let ready = calqueffet::ahead(&d, &plan, 1, 0);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].1.raster.get(20, 20), Color32::from_rgb(0, 255, 255));
}

#[test]
fn a_floating_selection_below_bypasses_the_effect_during_the_gesture() {
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let e = adjustment(&mut d, "Négatif", vec![invert()], 0, 12);
    d.panels[0].layers = vec![layer("A", a, 12), e];
    let mut plan = render::panel_plan_at(&d, 0, 0, d.render_paper());
    plan.items[0].pre = Some(render::Pre { aff: crate::vector::Affine::IDENTITY, dims: None, warp: None });
    let resolved = calqueffet::resolve(&d, &plan, Mode::Blocking);
    assert_eq!(resolved.items.len(), 1, "le repère est retiré, A reste");
    assert!(resolved.items[0].cel & (1 << 63) == 0);
}

// ---------------------------------------------------------------------- enregistrement

#[test]
fn effect_layers_survive_the_manifest_and_a_damaged_one_is_brought_back_to_sane_values() {
    let incrustation = Incrustation {
        source: Source::Libre,
        texte: "{plan} {timecode}".into(),
        portee: Portee::Plan,
        ancre: Ancre::Centre,
        taille: 7.5,
        couleur: [1, 2, 3, 200],
        fond: Some([9, 8, 7, 100]),
        marge: 4.0,
        quand: Quand::EcranSeulement,
        police: "Arial".into(),
    };
    for effet in [CalqueEffet::ajustement("Lueur (glow)"), CalqueEffet::incrustation(incrustation.clone(), "Timecode"), CalqueEffet::incrustation(Incrustation::default(), "")] {
        let entry = Entree::from(&effet);
        let text = serde_json::to_string(&entry).unwrap();
        let back: Entree = serde_json::from_str(&text).unwrap();
        assert_eq!(back.into_model(), effet, "{text}");
    }
    // Un manifeste abîmé ou d'une autre version : des valeurs sûres, jamais d'erreur.
    let damaged: Entree = serde_json::from_str(
        r#"{"sorte":"incrustation","incrustation":{"contenu":"inconnu","compte":"?","place":"?","taille":1e30,"couleur":[0,0,0,0],"marge":-5,"visible":"?"}}"#,
    )
    .unwrap();
    let Contenu::Incrustation(i) = damaged.into_model().contenu else { panic!("une incrustation") };
    let d = Incrustation::default();
    assert_eq!((i.source, i.portee, i.ancre, i.quand), (d.source, d.portee, d.ancre, d.quand));
    assert!((0.5..=60.0).contains(&i.taille) && (0.0..=40.0).contains(&i.marge));
    let unknown: Entree = serde_json::from_str(r#"{"sorte":"autre chose"}"#).unwrap();
    assert_eq!(unknown.into_model().contenu, Contenu::Ajustement);
}

#[test]
fn the_effect_layers_are_saved_with_the_project_and_read_back() {
    let dir = crate::fiabilite_fixture::dossier("calques-effets");
    let mut d = doc();
    let a = painted(&mut d, (10, 10, 30, 30), RED);
    let mut e = adjustment(&mut d, "Impact", vec![invert()], 2, 5);
    e.opacity = 0.7;
    e.blend = BlendMode::Screen;
    let cel = d.new_cel(LayerKind::Bitmap);
    let mut tc = Layer::new_effect("Timecode", CalqueEffet::incrustation(Incrustation::default(), "Timecode"));
    tc.frames = vec![Some(cel); 12];
    d.panels[0].layers = vec![layer("A", a, 12), e, tc];
    let manifest = dir.join("Film.pelure");
    crate::project::save(&d, &manifest, &mut Default::default()).unwrap();
    let json = crate::fiabilite_fixture::lire_json(&manifest);
    assert_eq!(json["version"], 31);
    let loaded = crate::project::load(&manifest).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let layers = &loaded.doc.panels[0].layers;
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[1].effet, Some(CalqueEffet::ajustement("")));
    assert_eq!((layers[1].opacity, layers[1].blend), (0.7, BlendMode::Screen));
    assert_eq!(layers[1].frames.iter().filter(|f| f.is_some()).count(), 3);
    assert_eq!(layers[1].fx, vec![invert()]);
    assert_eq!(layers[2].effet, Some(CalqueEffet::incrustation(Incrustation::default(), "Timecode")));
    // Et le film est le même : l'effet se voit après la relecture (image 3), pas avant (image 0).
    for frame in [0, 1, 2, 3, 4, 5, 11] {
        assert_eq!(flat(&loaded.doc, frame), flat(&d, frame), "image {frame}");
    }
    assert_ne!(flat(&loaded.doc, 3), flat(&loaded.doc, 0));
    assert_eq!(at(&flat(&loaded.doc, 0), 20, 20), RED);
    // Un projet sans calque d'effet ne porte pas le champ (les anciens projets restent lisibles).
    let plain = dir.join("Sans").join("Sans.pelure");
    let mut d2 = doc();
    let b = painted(&mut d2, (1, 1, 5, 5), RED);
    d2.panels[0].layers = vec![layer("A", b, 12)];
    crate::project::save(&d2, &plain, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&plain).unwrap();
    assert!(!text.contains("calque_effet"));
}

#[test]
fn an_effect_layer_cannot_be_drawn_on_and_has_its_own_kind() {
    let mut d = doc();
    let e = adjustment(&mut d, "Négatif", vec![invert()], 0, 12);
    assert!(!e.drawable());
    assert!(e.is_adjustment() && e.overlay().is_none());
    assert!(e.effect_active(0) && !e.effect_active(12));
    let tc = Layer::new_effect("Timecode", CalqueEffet::incrustation(Incrustation::default(), ""));
    assert!(!tc.is_adjustment() && tc.overlay().is_some());
    assert!(!tc.effect_active(0), "sans exposition, il n'agit nulle part");
    assert!(Layer::new("Dessin", LayerKind::Bitmap).drawable());
}

#[test]
fn the_scope_of_an_effect_layer_is_its_folder_or_the_whole_stack() {
    let mut d = doc();
    let mut folder = Layer::new_folder("D");
    let c1 = layer("C1", painted(&mut d, (0, 0, 2, 2), RED), 3);
    let mut c1 = c1;
    c1.parent = Some(folder.id);
    let mut e = adjustment(&mut d, "E", vec![invert()], 0, 3);
    e.parent = Some(folder.id);
    folder.collapsed = false;
    let top = adjustment(&mut d, "Haut", vec![invert()], 0, 3);
    let mut layers = vec![layer("A", painted(&mut d, (0, 0, 2, 2), RED), 3), c1, e, folder, top];
    crate::folders::normalize(&mut layers);
    // A (0), C1 (1), E (2), dossier (3), Haut (4).
    assert_eq!(calqueffet::scope_low(&layers, 2), 1, "dans le dossier : depuis son calque le plus bas");
    assert_eq!(calqueffet::scope_low(&layers, 4), 0, "hors dossier : tout ce qui est dessous");
    // Les rangs de départ vus par le repère : A est à 0, le dossier ne pose rien.
    d.panels[0].layers = layers;
    let plan: Plan = render::panel_plan_at(&d, 0, 0, None);
    let markers: Vec<usize> = plan.items.iter().enumerate().filter(|(_, i)| matches!(i.fond, Some(render::FondPart::Reglage(_)))).map(|(k, _)| k).collect();
    assert_eq!(markers.len(), 2);
    let froms: Vec<usize> = markers
        .iter()
        .map(|&k| match &plan.items[k].fond {
            Some(render::FondPart::Reglage(r)) => r.from,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(froms, vec![1, 0], "celui du dossier part du dessin de C1 (après A), l'autre du début");
}
