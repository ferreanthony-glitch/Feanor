//! Fusionner des calques (§82) : le résultat est ce que l'écran montrait, image par image ; une seule étape d'annulation ;
//! ce qui ne peut pas fusionner est refusé avec sa raison.

use eframe::egui::{self, Color32};

use crate::app::App;
use crate::blend::BlendMode;
use crate::document::{Document, Layer, LayerKind, LayerTransform};
use crate::fx::{Effect, EffectKind};
use crate::raster::outer_id;
use crate::render;
use crate::vector::{Element, VPoint};

const RED: Color32 = Color32::from_rgb(200, 0, 0);
const BLUE: Color32 = Color32::from_rgb(0, 0, 200);
const GREEN: Color32 = Color32::from_rgb(0, 160, 0);
const WHITE: [u8; 3] = [255, 255, 255];

fn paint(doc: &mut Document, cel: u64, x: usize, y: usize, c: Color32) {
    let r = &mut doc.cels.get_mut(&cel).unwrap().raster;
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % 64) * 64 + x % 64] = c;
}

/// Un bloc de pixels `c` (carré de 8) dont le coin haut gauche est en (`x`, `y`).
fn block(doc: &mut Document, cel: u64, x: usize, y: usize, c: Color32) {
    for dy in 0..8 {
        for dx in 0..8 {
            paint(doc, cel, x + dx, y + dy, c);
        }
    }
}

fn stroke(color: [u8; 3], y: f32) -> Element {
    Element::Stroke {
        points: vec![VPoint { x: 4.0, y, r: 1.5 }, VPoint { x: 30.0, y, r: 1.5 }],
        color,
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

/// Un plan de six images, deux calques vides (le plus bas d'abord) de sorte `below` et `above`.
fn two_layers(below: LayerKind, above: LayerKind) -> Document {
    let mut doc = Document::new(48, 48, 24);
    doc.panels[0].set_length(6);
    doc.panels[0].layers = vec![Layer::new("Bas", below), Layer::new("Haut", above)];
    doc
}

/// Un dessin de la sorte donnée, avec un bloc de couleur (bitmap) ou un trait (vectoriel) à la hauteur `y`.
fn drawing(doc: &mut Document, kind: LayerKind, color: Color32, x: usize, y: usize) -> u64 {
    let cel = doc.new_cel(kind);
    match kind {
        LayerKind::Bitmap => block(doc, cel, x, y, color),
        LayerKind::Vector => {
            let c = doc.cels.get_mut(&cel).unwrap();
            c.vector.as_mut().unwrap().elements.push(stroke([color.r(), color.g(), color.b()], y as f32 + 4.0));
            c.rerender();
        }
    }
    cel
}

/// Toutes les images du plan, composées avec ce papier.
fn images(doc: &Document, paper: Option<[u8; 3]>) -> Vec<Vec<Color32>> {
    (0..doc.panels[0].frame_count())
        .map(|f| render::composite_full(doc, &render::panel_plan_at(doc, 0, f, paper)))
        .collect()
}

/// La fusion du calque 1 dans le calque 0 rend les mêmes images, pixel pour pixel, sur un papier transparent ; sur du
/// papier blanc quand les deux calques sont opaques.
fn fusion_identique(mut doc: Document) -> Document {
    let (avant_nu, avant_blanc) = (images(&doc, None), images(&doc, Some(WHITE)));
    doc.merge_layer_down(0, 1).unwrap();
    assert_eq!(doc.panels[0].layers.len(), 1);
    assert_eq!(images(&doc, None), avant_nu, "papier transparent : les mêmes pixels");
    assert_eq!(images(&doc, Some(WHITE)), avant_blanc, "papier blanc : les mêmes pixels");
    doc
}

#[test]
fn every_pair_of_kinds_merges_into_what_the_screen_showed() {
    for (below, above) in [
        (LayerKind::Bitmap, LayerKind::Bitmap),
        (LayerKind::Vector, LayerKind::Vector),
        (LayerKind::Bitmap, LayerKind::Vector),
        (LayerKind::Vector, LayerKind::Bitmap),
    ] {
        let mut doc = two_layers(below, above);
        let (b, a) = (drawing(&mut doc, below, RED, 4, 4), drawing(&mut doc, above, BLUE, 4, 20));
        doc.panels[0].layers[0].frames = vec![Some(b); 6];
        doc.panels[0].layers[1].frames = vec![Some(a); 6];
        let doc = fusion_identique(doc);
        let l = &doc.panels[0].layers[0];
        let attendu = if below == LayerKind::Vector && above == LayerKind::Vector { LayerKind::Vector } else { LayerKind::Bitmap };
        assert_eq!(l.kind, attendu, "{below:?} sous {above:?}");
        assert_eq!((l.opacity, l.blend), (1.0, BlendMode::Normal));
    }
}

#[test]
fn overlapping_opaque_layers_keep_their_stacking() {
    // Le dessus recouvre une partie du dessous : l'ordre est gardé.
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    let (b, a) = (drawing(&mut doc, LayerKind::Bitmap, RED, 8, 8), drawing(&mut doc, LayerKind::Bitmap, BLUE, 12, 12));
    doc.panels[0].layers[0].frames = vec![Some(b); 6];
    doc.panels[0].layers[1].frames = vec![Some(a); 6];
    let doc = fusion_identique(doc);
    let cel = &doc.cels[&doc.panels[0].layers[0].frames[0].unwrap()];
    assert_eq!(cel.raster.get(9, 9), RED);
    assert_eq!(cel.raster.get(13, 13), BLUE, "le dessus recouvre");
}

#[test]
fn opacity_blend_mode_and_effects_are_baked_exactly() {
    // Opacité du dessus et du dessous, mode de fusion du dessus, flou du dessous : le papier transparent rend les mêmes
    // pixels (l'écran fait la même suite de compositions).
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    let (b, a) = (drawing(&mut doc, LayerKind::Bitmap, RED, 8, 8), drawing(&mut doc, LayerKind::Bitmap, GREEN, 12, 12));
    doc.panels[0].layers[0].frames = vec![Some(b); 6];
    doc.panels[0].layers[1].frames = vec![Some(a); 6];
    doc.panels[0].layers[0].opacity = 0.8;
    doc.panels[0].layers[1].opacity = 0.5;
    doc.panels[0].layers[1].blend = BlendMode::Multiply;
    doc.panels[0].layers[0].fx.push(Effect::new(EffectKind::GaussianBlur));
    let avant = images(&doc, None);
    doc.merge_layer_down(0, 1).unwrap();
    let l = &doc.panels[0].layers[0];
    assert!(l.fx.is_empty() && l.opacity == 1.0 && l.opacity_keys.is_empty(), "tout est cuit dans les dessins");
    assert_eq!(images(&doc, None), avant);
    // Les effets des deux calques sont cuits, pas seulement ceux du dessous.
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    let (b, a) = (drawing(&mut doc, LayerKind::Bitmap, RED, 8, 8), drawing(&mut doc, LayerKind::Bitmap, BLUE, 28, 28));
    doc.panels[0].layers[0].frames = vec![Some(b); 6];
    doc.panels[0].layers[1].frames = vec![Some(a); 6];
    doc.panels[0].layers[1].fx.push(Effect::new(EffectKind::GaussianBlur));
    let avant = images(&doc, None);
    assert!(avant[0].iter().any(|c| c.a() > 0 && c.a() < 255), "le flou fait des bords doux");
    doc.merge_layer_down(0, 1).unwrap();
    assert_eq!(images(&doc, None), avant, "le flou du calque du dessus n'est pas perdu");
}

#[test]
fn a_semi_transparent_stack_on_paper_is_within_one_step() {
    // Sur du papier blanc, deux calques semi-transparents qui se recouvrent : l'arrondi peut différer d'un cran.
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    let (b, a) = (drawing(&mut doc, LayerKind::Bitmap, RED, 8, 8), drawing(&mut doc, LayerKind::Bitmap, BLUE, 12, 12));
    doc.panels[0].layers[0].frames = vec![Some(b); 6];
    doc.panels[0].layers[1].frames = vec![Some(a); 6];
    doc.panels[0].layers[0].opacity = 0.6;
    doc.panels[0].layers[1].opacity = 0.5;
    let avant = images(&doc, Some(WHITE));
    doc.merge_layer_down(0, 1).unwrap();
    let apres = images(&doc, Some(WHITE));
    for (x, y) in avant.iter().flatten().zip(apres.iter().flatten()) {
        for k in 0..4 {
            assert!((x.to_array()[k] as i32 - y.to_array()[k] as i32).abs() <= 2, "{x:?} {y:?}");
        }
    }
}

#[test]
fn animated_opacity_and_position_are_baked_frame_by_frame() {
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    let (b, a) = (drawing(&mut doc, LayerKind::Bitmap, RED, 8, 8), drawing(&mut doc, LayerKind::Bitmap, BLUE, 20, 20));
    doc.panels[0].layers[0].frames = vec![Some(b); 6];
    doc.panels[0].layers[1].frames = vec![Some(a); 6];
    // Le dessus s'estompe sur le plan et glisse vers la droite en tournant.
    doc.panels[0].layers[1].put_opacity_key(0, 1.0);
    doc.panels[0].layers[1].put_opacity_key(5, 0.2);
    crate::poskeys::set_key(&mut doc.panels[0].layers[1].pos_keys, 0, LayerTransform::default());
    crate::poskeys::set_key(&mut doc.panels[0].layers[1].pos_keys, 5, LayerTransform { x: 6.0, y: -3.0, angle: 20.0, ..Default::default() });
    let avant = images(&doc, None);
    assert_ne!(avant[0], avant[5], "l'animation se voit");
    doc.merge_layer_down(0, 1).unwrap();
    let l = &doc.panels[0].layers[0];
    assert!(l.opacity_keys.is_empty() && l.pos_keys.is_empty() && l.transform.is_identity(), "plus de clés : tout est cuit");
    assert_eq!(images(&doc, None), avant, "chaque image garde son rendu");
}

#[test]
fn a_transformed_layer_merges_into_the_pixels_it_showed() {
    let mut doc = two_layers(LayerKind::Vector, LayerKind::Bitmap);
    let (b, a) = (drawing(&mut doc, LayerKind::Vector, RED, 4, 4), drawing(&mut doc, LayerKind::Bitmap, BLUE, 20, 20));
    doc.panels[0].layers[0].frames = vec![Some(b); 6];
    doc.panels[0].layers[1].frames = vec![Some(a); 6];
    doc.panels[0].layers[1].transform = LayerTransform { x: 5.0, y: 2.0, scale: 1.25, angle: 12.0, ..Default::default() };
    let avant = images(&doc, None);
    doc.merge_layer_down(0, 1).unwrap();
    assert_eq!(images(&doc, None), avant);
    assert!(doc.panels[0].layers[0].transform.is_identity());
}

#[test]
fn what_is_drawn_beyond_the_sheet_is_not_lost() {
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    let (b, a) = (drawing(&mut doc, LayerKind::Bitmap, RED, 8, 8), drawing(&mut doc, LayerKind::Bitmap, BLUE, 20, 20));
    doc.cels.get_mut(&b).unwrap().raster.tile_mut(outer_id(-2, 1))[0] = RED;
    doc.cels.get_mut(&a).unwrap().raster.tile_mut(outer_id(3, -1))[5] = BLUE;
    doc.panels[0].layers[0].frames = vec![Some(b); 6];
    doc.panels[0].layers[1].frames = vec![Some(a); 6];
    doc.merge_layer_down(0, 1).unwrap();
    let cel = &doc.cels[&doc.panels[0].layers[0].frames[0].unwrap()];
    assert_eq!(cel.raster.get_i(-128, 64), RED, "ce que le dessous avait hors de la feuille");
    assert_eq!(cel.raster.get_i(3 * 64 + 5, -64), BLUE, "ce que le dessus avait hors de la feuille");
}

#[test]
fn different_exposures_are_united_and_each_image_keeps_its_timing() {
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    let (a, b) = (drawing(&mut doc, LayerKind::Bitmap, RED, 2, 2), drawing(&mut doc, LayerKind::Bitmap, RED, 2, 30));
    let (c, d) = (drawing(&mut doc, LayerKind::Bitmap, BLUE, 30, 2), drawing(&mut doc, LayerKind::Bitmap, BLUE, 30, 30));
    doc.panels[0].layers[0].frames = vec![Some(a), Some(a), Some(a), Some(b), Some(b), Some(b)];
    doc.panels[0].layers[1].frames = vec![Some(c), Some(c), None, None, Some(d), Some(d)];
    doc.panels[0].layers[0].names.insert(a, "Bras levé".into());
    let avant = images(&doc, None);
    doc.merge_layer_down(0, 1).unwrap();
    let l = &doc.panels[0].layers[0];
    let f: Vec<_> = l.frames.iter().map(|c| c.expect("une image à chaque pose")).collect();
    assert_eq!(f[0], f[1], "l'exposition commune est gardée");
    assert_eq!(f[4], f[5]);
    assert!(f[1] != f[2] && f[2] != f[3] && f[3] != f[4], "quatre combinaisons, quatre dessins : {f:?}");
    assert_eq!(images(&doc, None), avant);
    assert_eq!(l.names.get(&f[0]).map(String::as_str), Some("Bras levé"), "le nom d'instance suit le dessin fusionné");
    // Les anciens dessins sont intacts : Ctrl+Z y revient.
    assert!(doc.cels.contains_key(&a) && doc.cels.contains_key(&c));
}

#[test]
fn looping_layers_are_baked_over_the_whole_plan() {
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    let (a, b) = (drawing(&mut doc, LayerKind::Bitmap, RED, 2, 2), drawing(&mut doc, LayerKind::Bitmap, RED, 2, 30));
    let c = drawing(&mut doc, LayerKind::Bitmap, BLUE, 30, 30);
    doc.panels[0].layers[0].frames = vec![Some(a), Some(b)];
    doc.panels[0].layers[0].post = crate::document::Behaviour::Loop;
    doc.panels[0].layers[1].frames = vec![Some(c); 6];
    let avant = images(&doc, None);
    doc.merge_layer_down(0, 1).unwrap();
    let l = &doc.panels[0].layers[0];
    assert_eq!(l.frames.len(), 6, "la boucle est cuite sur tout le plan");
    assert_eq!(l.post, crate::document::Behaviour::Nothing);
    assert_eq!(images(&doc, None), avant);
}

#[test]
fn several_chosen_layers_merge_in_one_go_like_the_stack() {
    let mut doc = Document::new(48, 48, 24);
    doc.panels[0].set_length(2);
    let mut layers = Vec::new();
    for (k, (c, kind)) in [(RED, LayerKind::Bitmap), (BLUE, LayerKind::Vector), (GREEN, LayerKind::Bitmap)].into_iter().enumerate() {
        let cel = drawing(&mut doc, kind, c, 4 + k * 12, 4 + k * 12);
        let mut l = Layer::new(format!("C{k}"), kind);
        l.frames = vec![Some(cel); 2];
        layers.push(l);
    }
    layers[2].opacity = 0.5;
    doc.panels[0].layers = layers;
    let avant = images(&doc, None);
    let at = crate::layerops::merge_layers(&mut doc, 0, &[0, 1, 2]).unwrap();
    assert_eq!((at, doc.panels[0].layers.len()), (0, 1));
    assert_eq!(images(&doc, None), avant);
}

#[test]
fn what_cannot_merge_says_why_and_changes_nothing() {
    use crate::document::SceneWide;
    let casse: Vec<(&str, Box<dyn Fn(&mut Document)>)> = vec![
        ("caché", Box::new(|d| d.panels[0].layers[1].visible = false)),
        ("verrouillé", Box::new(|d| d.panels[0].layers[0].locked = true)),
        ("vidéo", Box::new(|d| d.panels[0].layers[1].video = Some(crate::video::VideoClip { id: 1, name: "v.mp4".into(), frames: 10, fps: 24.0, start: 0, ext: "jpg".into() }))),
        ("effet", Box::new(|d| d.panels[0].layers[1] = Layer::new_effect("Lueur", crate::calqueffet::CalqueEffet::ajustement("")))),
        ("Couleur unie", Box::new(|d| d.panels[0].layers[0].unie = Some([10, 20, 30]))),
        ("Décor", Box::new(|d| d.panels[0].layers[0].decor = true)),
        ("toute la scène", Box::new(|d| d.panels[0].layers[1].scene_wide = Some(SceneWide { key: 1, scene: 1 }))),
        ("profondeur", Box::new(|d| d.panels[0].layers[1].depth = 1.0)),
        ("inclin", Box::new(|d| d.panels[0].layers[1].transform = LayerTransform { tilt_x: 20.0, ..Default::default() })),
        ("annotation", Box::new(|d| d.panels[0].layers[1].annotation = true)),
        ("dossier", Box::new(|d| d.panels[0].layers[1] = Layer::new_folder("Rangement"))),
    ];
    for (mot, casse) in casse {
        let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
        let cel = doc.new_cel(LayerKind::Bitmap);
        doc.panels[0].layers[0].frames = vec![Some(cel); 6];
        casse(&mut doc);
        let avant = doc.panels[0].layers.clone();
        let raison = crate::fusioncalques::blocker_down(&doc, 0, 1).unwrap_or_else(|| panic!("{mot} : la fusion devrait être refusée"));
        assert!(raison.to_lowercase().contains(&mot.to_lowercase()), "{mot} : la raison dit pourquoi : {raison}");
        assert_eq!(doc.merge_layer_down(0, 1), Err(raison.clone()), "{mot}");
        assert_eq!(doc.panels[0].layers, avant, "{mot} : rien n'a changé");
        // La liste dit la même chose.
        assert!(crate::fusioncalques::blocker_list(&doc, 0, &[0, 1]).is_some(), "{mot}");
    }
    // Rien en dessous du premier calque ; un seul calque choisi n'est pas une fusion.
    let doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    assert!(crate::fusioncalques::blocker_down(&doc, 0, 0).unwrap().contains("en dessous"));
    assert!(crate::fusioncalques::blocker_list(&doc, 0, &[1]).unwrap().contains("Ctrl"));
    assert!(crate::fusioncalques::blocker_down(&doc, 0, 1).is_none());
}

#[test]
fn the_tooltip_tells_what_kind_of_layer_comes_out() {
    let mut doc = two_layers(LayerKind::Vector, LayerKind::Vector);
    assert!(crate::fusioncalques::resultat_de(&doc, 0, &[0, 1]).contains("vectoriel"));
    doc.panels[0].layers[1].kind = LayerKind::Bitmap;
    assert!(crate::fusioncalques::resultat_de(&doc, 0, &[0, 1]).contains("bitmap") && crate::fusioncalques::resultat_de(&doc, 0, &[0, 1]).contains("vectoriels"));
    // Deux vectoriels avec un effet : bitmap.
    let mut doc = two_layers(LayerKind::Vector, LayerKind::Vector);
    doc.panels[0].layers[1].fx.push(Effect::new(EffectKind::GaussianBlur));
    assert!(crate::fusioncalques::resultat_de(&doc, 0, &[0, 1]).contains("bitmap"));
}

// ---------------------------------------------------------------------------------------------- l'application

fn app_with(name: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("shot-fusion-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 1;
    app.cur_frame = 0;
    app
}

#[test]
fn one_undo_step_gives_the_layers_back_exactly() {
    let mut doc = two_layers(LayerKind::Vector, LayerKind::Bitmap);
    let (b, a) = (drawing(&mut doc, LayerKind::Vector, RED, 4, 4), drawing(&mut doc, LayerKind::Bitmap, BLUE, 20, 20));
    doc.panels[0].layers[0].frames = vec![Some(b); 6];
    doc.panels[0].layers[1].frames = vec![Some(a); 6];
    doc.panels[0].layers[1].opacity = 0.7;
    doc.panels[0].layers[1].blend = BlendMode::Multiply;
    let mut app = app_with("annuler", doc);
    let layers = app.layers().to_vec();
    let images_avant = images(&app.doc, None);
    app.run_action("calque.fusionner_dessous");
    assert_eq!(app.layers().len(), 1, "{}", app.status);
    assert!(app.status.contains("bitmap") && app.status.contains("Ctrl+Z"), "la barre d'état dit ce qui est sorti : {}", app.status);
    assert_eq!(images(&app.doc, None), images_avant, "même rendu");
    app.undo();
    assert_eq!(app.layers(), &layers[..], "une seule annulation rend les calques exactement comme avant");
    assert_eq!(images(&app.doc, None), images_avant);
    app.redo();
    assert_eq!(app.layers().len(), 1, "et le rétablissement refait la fusion");
}

#[test]
fn merging_the_chosen_layers_from_the_app_is_one_undo_step_too() {
    let mut doc = Document::new(48, 48, 24);
    doc.panels[0].set_length(3);
    let mut layers = Vec::new();
    for (k, c) in [RED, BLUE, GREEN, RED].into_iter().enumerate() {
        let cel = drawing(&mut doc, LayerKind::Bitmap, c, 2 + k * 10, 2 + k * 10);
        let mut l = Layer::new(format!("C{k}"), LayerKind::Bitmap);
        l.frames = vec![Some(cel); 3];
        layers.push(l);
    }
    doc.panels[0].layers = layers;
    let mut app = app_with("choisis", doc);
    let ids: Vec<u64> = app.layers().iter().map(|l| l.id).collect();
    let avant = app.layers().to_vec();
    app.cur_layer = 2;
    app.layer_sel = vec![ids[0], ids[1]];
    let images_avant = images(&app.doc, None);
    app.run_action("calque.fusionner_choisis");
    assert_eq!(app.layers().len(), 2, "{}", app.status);
    assert_eq!(images(&app.doc, None), images_avant);
    app.undo();
    assert_eq!(app.layers(), &avant[..]);
}

#[test]
fn merging_a_line_with_its_hidden_neighbour_is_refused_with_a_message() {
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    doc.panels[0].layers[0].visible = false;
    let mut app = app_with("refus", doc);
    let avant = app.layers().to_vec();
    app.run_action("calque.fusionner_dessous");
    assert!(app.status.contains("caché"), "{}", app.status);
    assert_eq!(app.layers(), &avant[..]);
}

#[test]
fn a_layer_with_its_colour_layer_still_merges_into_an_ordinary_layer() {
    // (Le cas du trait et de sa couleur : `calquecouleur_tests::merging_a_line_with_its_colour_gives_one_ordinary_layer`.)
    // Une couleur fondue avec un autre calque n'a plus de trait à suivre : le lien est oublié, rien ne se perd.
    let mut doc = two_layers(LayerKind::Bitmap, LayerKind::Bitmap);
    let (b, a) = (drawing(&mut doc, LayerKind::Bitmap, RED, 8, 8), drawing(&mut doc, LayerKind::Bitmap, BLUE, 20, 20));
    doc.panels[0].layers[0].frames = vec![Some(b); 6];
    doc.panels[0].layers[1].frames = vec![Some(a); 6];
    doc.panels[0].layers[0].couleur = Some(crate::calquecouleur::CalqueCouleur::new(0));
    let avant = images(&doc, None);
    doc.merge_layer_down(0, 1).unwrap();
    assert!(doc.panels[0].layers[0].couleur.is_none());
    assert_eq!(images(&doc, None), avant);
}
