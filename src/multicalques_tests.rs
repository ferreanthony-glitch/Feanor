//! Partie 132 : la Sélection, la Gomme, Déformer et Transformer agissent sur tous les calques choisis (Anthony, 2026-11-10 :
//! « Les outils sélection, gomme, déformer et transformer s'appliquent à plusieurs calques quand plusieurs calques sont
//! sélectionnés »), et sur toutes les clés choisies dans la timeline (les dessins de plusieurs images).
//!
//! Au vrai geste sur le banc (`perf_tests::Banc`) : appui, glissé et lâcher du pointeur sur le dessin de l'espace Animation, les
//! calques choisis par le panneau Calques (`click_layer_head`, Ctrl + clic). Vérifié dans les données (traits, pixels, poses) :
//! calques vectoriels, bitmap et mélangés ; un calque verrouillé dans le lot ; un calque décalé ; annuler, rétablir ; enregistrer
//! et rouvrir ; plusieurs clés, un dessin tenu sur plusieurs images.

use eframe::egui::{pos2, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::app::{Mode, Tool};
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::envelopetools::DeformMode;
use crate::perf_tests::{banc_taille, Banc};
use crate::raster::{Raster, TILE};
use crate::vector::{Element, VPoint, VectorDrawing};

const W: usize = 400;
const H: usize = 300;

fn ecran(b: &Banc, p: Pos2) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), p)
}

fn bouton(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers }
}

/// Un glissé du pointeur sur les points de la feuille `pts`, une image d'écran par point.
fn glisser(b: &mut Banc, pts: &[Pos2]) {
    let p0 = ecran(b, pts[0]);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, Modifiers::NONE)], 1.0 / 60.0);
    for &p in &pts[1..] {
        let q = ecran(b, p);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    let fin = ecran(b, *pts.last().unwrap());
    b.image(vec![bouton(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Un glissé en ligne droite de `a` à `z`, en `n` pas.
fn ligne(a: Pos2, z: Pos2, n: usize) -> Vec<Pos2> {
    (0..=n).map(|k| a + (z - a) * (k as f32 / n as f32)).collect()
}

/// Un clic sur le point de la feuille `p`.
fn clic(b: &mut Banc, p: Pos2) {
    glisser(b, &[p, p]);
}

/// Un trait horizontal dense (un point tous les 5 px) de `x0` à `x1` en `y`.
fn trait_h(x0: f32, x1: f32, y: f32) -> Element {
    let n = ((x1 - x0) / 5.0) as usize;
    Element::Stroke {
        points: (0..=n).map(|k| VPoint { x: x0 + (x1 - x0) * k as f32 / n as f32, y, r: 2.0 }).collect(),
        color: [20, 20, 20],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

/// Une bande pleine de `x0..x1` × `y0..y1`.
fn bande(x0: usize, x1: usize, y0: usize, y1: usize) -> Raster {
    let mut r = Raster::new(W, H);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = r.tile_index_of(x, y);
            r.tile_mut(i)[(y % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(30, 30, 30, 255);
        }
    }
    r
}

/// Ce que contient un calque du banc : des traits, ou des pixels.
enum Contenu {
    Traits(Vec<Element>),
    Pixels(Raster),
}

/// Un dessin neuf de ce contenu.
fn dessin(doc: &mut Document, c: Contenu) -> CelId {
    match c {
        Contenu::Traits(els) => {
            let id = doc.new_cel(LayerKind::Vector);
            let cel = doc.cels.get_mut(&id).unwrap();
            cel.vector.as_mut().unwrap().elements = els;
            cel.rerender();
            id
        }
        Contenu::Pixels(r) => {
            let id = doc.new_cel(LayerKind::Bitmap);
            doc.cels.get_mut(&id).unwrap().raster = r;
            id
        }
    }
}

/// La hauteur de la ligne du calque `i` (les contenus des calques ne se touchent pas).
fn y_de(i: usize) -> f32 {
    60.0 + 45.0 * i as f32
}

/// Le contenu du calque `i` de cette sorte : un trait, ou une bande de 6 px, de x 110 à 290.
fn contenu(kind: LayerKind, i: usize) -> Contenu {
    let y = y_de(i);
    match kind {
        LayerKind::Vector => Contenu::Traits(vec![trait_h(110.0, 290.0, y)]),
        LayerKind::Bitmap => Contenu::Pixels(bande(110, 290, y as usize - 3, y as usize + 3)),
    }
}

/// Un plan de quatre images, un calque par sorte donnée (une seule clé, tenue sur tout le plan), dans l'espace Animation ; le
/// calque 0 est le courant, tous les autres sont choisis (Ctrl + clic dans le panneau Calques). L'outil donné.
fn banc(kinds: &[LayerKind], tool: Tool) -> Banc {
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 4;
    for (i, &k) in kinds.iter().enumerate() {
        let cel = dessin(&mut doc, contenu(k, i));
        let mut l = Layer::new(&format!("Calque {i}"), k);
        l.set(0, Some(cel));
        doc.panels[0].layers.push(l);
    }
    ouvrir(doc, tool, kinds.len())
}

fn ouvrir(doc: Document, tool: Tool, choisis: usize) -> Banc {
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    for li in 1..choisis {
        b.app.click_layer_head(li, true, false);
    }
    b.app.cur_layer = 0;
    b.app.set_tool(tool);
    b.app.eraser.size = 30.0;
    b.app.eraser.pressure_size = false;
    b.app.vector_erase_whole = false;
    b.chauffer(10);
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    assert_eq!(b.app.chosen_layers().len(), choisis.max(1), "les calques choisis");
    b
}

fn cel_de(b: &Banc, li: usize, f: usize) -> CelId {
    b.app.doc.panels[0].layers[li].cel_at(f).expect("un dessin")
}

fn vect(b: &Banc, cel: CelId) -> VectorDrawing {
    b.app.doc.cels[&cel].vector.clone().expect("vectoriel")
}

fn pointes(d: &VectorDrawing) -> Vec<Pos2> {
    d.elements
        .iter()
        .flat_map(|e| match e {
            Element::Stroke { points, .. } => points.iter().map(|p| pos2(p.x, p.y)).collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

/// La distance de `p` au segment `a`–`z`.
fn au_segment(p: Pos2, a: Pos2, z: Pos2) -> f32 {
    let d = z - a;
    let t = if d.length_sq() < 1e-6 { 0.0 } else { ((p - a).dot(d) / d.length_sq()).clamp(0.0, 1.0) };
    p.distance(a + d * t)
}

/// Le dessin `cel` a-t-il de l'encre près du point `p` (de son repère) ? Un trait : il passe à moins de 6 px (un trait coupé
/// par la gomme est simplifié : ses points ne sont plus réguliers) ; des pixels : le pixel est couvert.
fn encre(b: &Banc, cel: CelId, p: Pos2) -> bool {
    let c = &b.app.doc.cels[&cel];
    match &c.vector {
        Some(d) => d.elements.iter().any(|e| match e {
            Element::Stroke { points, .. } => points.windows(2).any(|w| au_segment(p, pos2(w[0].x, w[0].y), pos2(w[1].x, w[1].y)) < 6.0) || points.iter().any(|q| pos2(q.x, q.y).distance(p) < 6.0),
            _ => false,
        }),
        None => c.raster.get(p.x as usize, p.y as usize).a() > 0,
    }
}

fn etapes(b: &Banc) -> usize {
    b.app.history.names().0.len()
}

/// Ce que contient un dessin, pour comparer avant et après.
fn empreinte(b: &Banc, cel: CelId) -> (Vec<Pos2>, Vec<Option<crate::raster::Tile>>) {
    let c = &b.app.doc.cels[&cel];
    match &c.vector {
        Some(d) => (pointes(d), Vec::new()),
        None => (Vec::new(), c.raster.tiles.clone()),
    }
}

/// Les images jusqu'au repos (rendus exacts en arrière-plan compris).
fn repos(b: &mut Banc) {
    crate::perf_tests::jusqu_au_repos(b, 200);
}

// ---------------------------------------------------------------------------------------------- la gomme

/// La gomme en ligne verticale à x = 200, de haut en bas de la feuille.
fn gommer_en_200(b: &mut Banc) {
    glisser(b, &ligne(pos2(200.0, 20.0), pos2(200.0, 280.0), 26));
}

#[test]
fn the_eraser_erases_on_every_chosen_layer_vector_and_bitmap_in_one_undo_step() {
    let kinds = [LayerKind::Vector, LayerKind::Vector, LayerKind::Bitmap, LayerKind::Bitmap];
    let mut b = banc(&kinds, Tool::Eraser);
    let cels: Vec<CelId> = (0..4).map(|li| cel_de(&b, li, 0)).collect();
    let avant: Vec<_> = cels.iter().map(|&c| empreinte(&b, c)).collect();
    let n0 = etapes(&b);
    gommer_en_200(&mut b);
    repos(&mut b);
    for (li, &cel) in cels.iter().enumerate() {
        let y = y_de(li);
        assert!(!encre(&b, cel, pos2(200.0, y)), "calque {li} ({:?}) : gommé là où la gomme est passée", kinds[li]);
        assert!(encre(&b, cel, pos2(130.0, y)) && encre(&b, cel, pos2(270.0, y)), "calque {li} : le reste du dessin est intact");
    }
    assert_eq!(etapes(&b), n0 + 1, "une seule étape d'annulation pour les quatre calques");
    assert!(b.app.status.contains("4 calques"), "la barre d'état dit combien de calques : {}", b.app.status);
    b.app.undo();
    repos(&mut b);
    for (li, &cel) in cels.iter().enumerate() {
        assert!(empreinte(&b, cel) == avant[li], "calque {li} : Ctrl+Z rend le dessin d'avant, exactement");
    }
    b.app.redo();
    repos(&mut b);
    for (li, &cel) in cels.iter().enumerate() {
        assert!(!encre(&b, cel, pos2(200.0, y_de(li))), "calque {li} : rétabli");
    }
}

#[test]
fn the_eraser_leaves_a_locked_layer_alone_and_says_so() {
    let kinds = [LayerKind::Bitmap, LayerKind::Vector, LayerKind::Vector];
    let mut b = banc(&kinds, Tool::Eraser);
    b.app.doc.panels[0].layers[1].locked = true;
    let cels: Vec<CelId> = (0..3).map(|li| cel_de(&b, li, 0)).collect();
    let verrouille = empreinte(&b, cels[1]);
    gommer_en_200(&mut b);
    repos(&mut b);
    assert!(empreinte(&b, cels[1]) == verrouille, "le calque verrouillé n'a pas changé");
    assert!(!encre(&b, cels[0], pos2(200.0, y_de(0))) && !encre(&b, cels[2], pos2(200.0, y_de(2))), "les autres sont gommés");
    assert!(b.app.status.contains("verrouillé"), "la barre d'état nomme le calque laissé de côté : {}", b.app.status);
}

#[test]
fn the_eraser_erases_each_layer_where_it_is_seen_even_when_it_is_moved() {
    let kinds = [LayerKind::Vector, LayerKind::Bitmap];
    let mut b = banc(&kinds, Tool::Eraser);
    // Le calque bitmap est décalé de 40 px vers la droite : on le voit en x = 150 à 330.
    b.app.doc.panels[0].layers[1].transform.x = 40.0;
    b.chauffer(2);
    let cel = cel_de(&b, 1, 0);
    gommer_en_200(&mut b);
    repos(&mut b);
    // La gomme passée en x = 200 sur la feuille : en x = 160 dans son dessin.
    assert!(!encre(&b, cel, pos2(160.0, y_de(1))), "gommé là où on le voit");
    assert!(encre(&b, cel, pos2(200.0, y_de(1))), "pas à la place de son dessin d'origine");
}

#[test]
fn an_erased_set_of_layers_is_the_same_once_saved_and_reopened() {
    let kinds = [LayerKind::Vector, LayerKind::Bitmap];
    let mut b = banc(&kinds, Tool::Eraser);
    gommer_en_200(&mut b);
    repos(&mut b);
    let cels: Vec<CelId> = (0..2).map(|li| cel_de(&b, li, 0)).collect();
    let dir = std::env::temp_dir().join(format!("shot-multicalques-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join("P.pelure");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("ouverture").doc;
    for (li, &cel) in cels.iter().enumerate() {
        let c2 = relu.panels[0].layers[li].cel_at(0).expect("un dessin");
        let (a, z) = (&b.app.doc.cels[&cel], &relu.cels[&c2]);
        match (&a.vector, &z.vector) {
            (Some(d1), Some(d2)) => assert!(pointes(d1) == pointes(d2), "calque {li} : les mêmes traits"),
            _ => assert!(a.raster.get(200, y_de(li) as usize).a() == 0 && z.raster.get(200, y_de(li) as usize).a() == 0 && z.raster.get(130, y_de(li) as usize).a() > 0, "calque {li} : les mêmes pixels"),
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------------------------- déformer

/// Un calque bitmap plein (x 100..300, y 100..200) et un calque vectoriel (un trait en y = 150 de 120 à 280) : leur boîte
/// commune est celle du bitmap, coins (100, 100) et (300, 200).
fn banc_deformer(mode: DeformMode, courant: usize) -> Banc {
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 4;
    let a = dessin(&mut doc, Contenu::Pixels(bande(100, 300, 100, 200)));
    let z = dessin(&mut doc, Contenu::Traits(vec![trait_h(120.0, 280.0, 150.0)]));
    for (nom, kind, cel) in [("Fond", LayerKind::Bitmap, a), ("Encre", LayerKind::Vector, z)] {
        let mut l = Layer::new(nom, kind);
        l.set(0, Some(cel));
        doc.panels[0].layers.push(l);
    }
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, courant, 0);
    b.app.click_layer_head(1 - courant, true, false);
    b.app.cur_layer = courant;
    b.app.set_tool(Tool::Perspective);
    b.app.deform_mode = mode;
    b.chauffer(10);
    assert_eq!(b.app.chosen_layers().len(), 2, "les deux calques sont choisis");
    b
}

/// Ouvre la déformation (un clic au milieu), puis tire le coin haut-gauche de (100, 100) à (140, 130).
fn tirer_le_coin(b: &mut Banc) {
    clic(b, pos2(200.0, 150.0));
    glisser(b, &ligne(pos2(100.0, 100.0), pos2(140.0, 130.0), 8));
}

fn deformer_les_deux(mode: DeformMode, courant: usize) {
    let mut b = banc_deformer(mode, courant);
    let (fond, encre_cel) = (cel_de(&b, 0, 0), cel_de(&b, 1, 0));
    let (avant_fond, avant_encre) = (empreinte(&b, fond), empreinte(&b, encre_cel));
    let n0 = etapes(&b);
    tirer_le_coin(&mut b);
    repos(&mut b);
    // Le bitmap : son coin haut-gauche est parti vers l'intérieur, son centre est toujours couvert.
    assert!(!encre(&b, fond, pos2(103.0, 103.0)), "{mode:?} : le coin du dessin bitmap a suivi le coin du cadre");
    assert!(encre(&b, fond, pos2(200.0, 160.0)), "{mode:?} : le reste du bitmap est là");
    // Le vectoriel : le même cadre le déforme (son premier point, près du coin tiré, a bougé).
    let p0 = pointes(&vect(&b, encre_cel))[0];
    assert!(p0.distance(pos2(120.0, 150.0)) > 3.0, "{mode:?} : le trait est déformé par le même cadre ({p0:?})");
    assert_eq!(etapes(&b), n0 + 1, "{mode:?} : une seule étape d'annulation pour les deux calques");
    b.app.undo();
    repos(&mut b);
    assert!(empreinte(&b, fond) == avant_fond && empreinte(&b, encre_cel) == avant_encre, "{mode:?} : Ctrl+Z rend les deux dessins d'avant");
}

#[test]
fn the_perspective_deforms_every_chosen_layer_as_one_object() {
    deformer_les_deux(DeformMode::Perspective, 1);
}

#[test]
fn the_envelope_deforms_every_chosen_layer_as_one_object() {
    deformer_les_deux(DeformMode::Envelope, 0);
}

// ---------------------------------------------------------------------------------------------- transformer

/// Les calques bitmap et vectoriel de `banc_deformer`, outil Transformer : la boîte commune est celle de leurs contenus
/// (le bitmap à la tuile près).
fn boite_commune() -> eframe::egui::Rect {
    let t = TILE as f32;
    eframe::egui::Rect::from_min_max(pos2((100.0 / t).floor() * t, (100.0 / t).floor() * t), pos2((300.0 / t).ceil() * t, (200.0 / t).ceil() * t))
}

#[test]
fn transform_stretches_all_chosen_layers_by_the_middle_of_a_side() {
    let mut b = banc_deformer(DeformMode::Perspective, 1);
    b.app.set_tool(Tool::Transform);
    b.chauffer(3);
    let r = boite_commune();
    let n0 = etapes(&b);
    // Le milieu du côté droit, tiré de 50 px vers la droite : le côté gauche reste en place.
    glisser(&mut b, &ligne(r.right_center(), r.right_center() + eframe::egui::vec2(50.0, 0.0), 10));
    let k = (r.width() + 50.0) / r.width();
    let pivot = b.app.doc.stage_center();
    for li in 0..2 {
        let t = b.app.doc.panels[0].layers[li].transform;
        assert!((t.stretch_x - k).abs() < 0.02 && (t.stretch_y - 1.0).abs() < 1e-3, "calque {li} : étiré en largeur seulement ({t:?})");
        let gauche = t.affine(pivot).apply(r.left_center());
        assert!(gauche.distance(r.left_center()) < 1.0, "calque {li} : le côté gauche reste où il était ({gauche:?})");
    }
    assert_eq!(etapes(&b), n0 + 1, "une seule étape");
}

#[test]
fn transform_moves_the_position_keys_of_each_chosen_layer_at_the_current_frame() {
    let mut b = banc_deformer(DeformMode::Perspective, 1);
    // Les deux calques sont animés : une clé de position à l'image 0 et une à l'image 3.
    for li in 0..2 {
        let l = &mut b.app.doc.panels[0].layers[li];
        crate::poskeys::set_key(&mut l.pos_keys, 0, crate::document::LayerTransform::default());
        crate::poskeys::set_key(&mut l.pos_keys, 3, crate::document::LayerTransform { x: 60.0, ..Default::default() });
        assert!(l.is_animated());
    }
    b.app.set_tool(Tool::Transform);
    b.chauffer(3);
    let r = boite_commune();
    glisser(&mut b, &ligne(r.center(), r.center() + eframe::egui::vec2(20.0, 10.0), 8));
    for li in 0..2 {
        let l = &b.app.doc.panels[0].layers[li];
        let (p0, p3) = (l.pose(0), l.pose(3));
        assert!((p0.x - 20.0).abs() < 0.5 && (p0.y - 10.0).abs() < 0.5, "calque {li} : la clé de l'image courante a bougé ({p0:?})");
        assert!((p3.x - 60.0).abs() < 1e-3 && p3.y.abs() < 1e-3, "calque {li} : la clé de l'image 3 n'a pas bougé ({p3:?})");
    }
}

// ---------------------------------------------------------------------------------------------- la sélection

#[test]
fn the_selection_lasso_and_box_move_every_chosen_layer_and_undo_in_one_step() {
    let kinds = [LayerKind::Vector, LayerKind::Bitmap, LayerKind::Vector, LayerKind::Bitmap];
    let mut b = banc(&kinds, Tool::Select);
    b.app.select_rect = true;
    let cels: Vec<CelId> = (0..4).map(|li| cel_de(&b, li, 0)).collect();
    let avant: Vec<_> = cels.iter().map(|&c| empreinte(&b, c)).collect();
    // Un rectangle autour de tout, puis la boîte commune glissée de 30 px vers le bas.
    glisser(&mut b, &ligne(pos2(90.0, 30.0), pos2(310.0, 230.0), 6));
    assert!(b.app.multi.sel.as_ref().is_some_and(|s| s.parts.len() == 4), "une part par calque : {}", b.app.status);
    let n0 = etapes(&b);
    glisser(&mut b, &ligne(pos2(200.0, y_de(0)), pos2(200.0, y_de(0) + 30.0), 6));
    b.app.drop_multi();
    b.app.multi_flush();
    repos(&mut b);
    for (li, &cel) in cels.iter().enumerate() {
        let y = y_de(li);
        assert!(encre(&b, cel, pos2(150.0, y + 30.0)) && !encre(&b, cel, pos2(150.0, y)), "calque {li} ({:?}) : déplacé de 30 px", kinds[li]);
    }
    assert_eq!(etapes(&b), n0 + 1, "un seul geste, une seule étape");
    b.app.undo();
    repos(&mut b);
    for (li, &cel) in cels.iter().enumerate() {
        assert!(empreinte(&b, cel) == avant[li], "calque {li} : Ctrl+Z rend le dessin d'avant");
    }
}

// ---------------------------------------------------------------------------------------------- plusieurs clés

/// Deux calques (vectoriel, bitmap), trois clés chacun aux images 0, 2 et 4 (la clé de l'image 0 est tenue sur les images 0 et
/// 1) ; toutes les clés sont choisies dans la timeline ; image courante 0.
fn banc_cles(tool: Tool) -> Banc {
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 6;
    for (i, k) in [LayerKind::Vector, LayerKind::Bitmap].into_iter().enumerate() {
        let mut l = Layer::new(&format!("Calque {i}"), k);
        for f in [0, 2, 4] {
            let cel = dessin(&mut doc, contenu(k, i));
            l.set(f, Some(cel));
        }
        doc.panels[0].layers.push(l);
    }
    let mut b = ouvrir(doc, tool, 1);
    let keys: Vec<_> = (0..2).flat_map(|li| [0, 2, 4].map(|f| (li, f))).filter_map(|(li, f)| b.app.key_ref(0, li, f)).collect();
    assert_eq!(keys.len(), 6, "six clés");
    b.app.key_anchor = keys.first().copied();
    b.app.key_sel = keys;
    b.chauffer(2);
    b
}

fn dessins_des_cles(b: &Banc) -> Vec<(usize, CelId)> {
    (0..2).flat_map(|li| [0, 2, 4].map(|f| (li, cel_de(b, li, f)))).collect()
}

#[test]
fn the_eraser_erases_the_same_place_on_every_chosen_key_of_every_layer() {
    let mut b = banc_cles(Tool::Eraser);
    let dessins = dessins_des_cles(&b);
    let n0 = etapes(&b);
    gommer_en_200(&mut b);
    repos(&mut b);
    for &(li, cel) in &dessins {
        assert!(!encre(&b, cel, pos2(200.0, y_de(li))), "calque {li}, dessin {cel} : gommé");
        assert!(encre(&b, cel, pos2(130.0, y_de(li))), "calque {li}, dessin {cel} : le reste est intact");
    }
    assert_eq!(etapes(&b), n0 + 1, "six dessins, une seule étape");
    assert!(b.app.status.contains("6 dessins"), "la barre d'état dit combien de dessins : {}", b.app.status);
    b.app.undo();
    repos(&mut b);
    for &(li, cel) in &dessins {
        assert!(encre(&b, cel, pos2(200.0, y_de(li))), "calque {li}, dessin {cel} : rendu par Ctrl+Z");
    }
}

#[test]
fn the_deformation_is_the_same_on_every_chosen_key() {
    let mut b = banc_cles(Tool::Perspective);
    b.app.deform_mode = DeformMode::Perspective;
    let dessins = dessins_des_cles(&b);
    let avant: Vec<_> = dessins.iter().map(|&(_, c)| empreinte(&b, c)).collect();
    let n0 = etapes(&b);
    // Le cadre commun : x 110..290 (le trait déborde de son rayon), y de la ligne 0 à la ligne 1.
    clic(&mut b, pos2(200.0, y_de(0)));
    glisser(&mut b, &ligne(pos2(200.0, (y_de(0) + y_de(1)) / 2.0), pos2(200.0, (y_de(0) + y_de(1)) / 2.0 + 20.0), 6));
    repos(&mut b);
    for (k, &(li, cel)) in dessins.iter().enumerate() {
        assert!(empreinte(&b, cel) != avant[k], "calque {li}, dessin {cel} : déformé (déplacé de 20 px)");
        assert!(encre(&b, cel, pos2(200.0, y_de(li) + 20.0)), "calque {li}, dessin {cel} : à sa nouvelle place");
    }
    assert_eq!(etapes(&b), n0 + 1, "une seule étape");
}

// ---------------------------------------------------------------------------------------------- relecture de 853bdbf

#[test]
fn a_gesture_back_to_its_start_and_a_double_reset_leave_no_floating_sheet_and_no_preview() {
    let mut b = banc_deformer(DeformMode::Envelope, 1);
    let (fond, encre_cel) = (cel_de(&b, 0, 0), cel_de(&b, 1, 0));
    let (avant_fond, avant_encre) = (empreinte(&b, fond), empreinte(&b, encre_cel));
    let n0 = etapes(&b);
    clic(&mut b, pos2(200.0, 150.0));
    // L'intérieur tiré de 30 px, puis ramené à son point de départ, et lâché.
    let c = pos2(200.0, 150.0);
    let mut aller_retour = ligne(c, c + eframe::egui::vec2(30.0, 20.0), 5);
    aller_retour.extend(ligne(c + eframe::egui::vec2(30.0, 20.0), c, 5));
    glisser(&mut b, &aller_retour);
    // « Remettre droit » deux fois, puis terminer.
    b.app.deform_multi_reset();
    b.app.deform_multi_reset();
    b.app.deform_multi_fermer();
    repos(&mut b);
    assert!(b.app.multi.floats.iter().all(|f| f.pending.is_some()), "aucune feuille flottante sans fin");
    assert!(b.app.multi.floats.is_empty(), "les feuilles s'en vont avec le rendu exact ({})", b.app.multi.floats.len());
    assert!(empreinte(&b, fond) == avant_fond, "les pixels du dessin bitmap sont ceux d'avant, sans aperçu resté");
    assert!(empreinte(&b, encre_cel) == avant_encre, "les traits sont ceux d'avant");
    assert_eq!(etapes(&b), n0, "rien n'a été posé : aucune étape");
}

#[test]
fn a_colour_layer_with_several_keys_is_erased_once_on_each_key() {
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 6;
    let mut l = Layer::new("Couleur", LayerKind::Bitmap);
    l.couleur = Some(crate::calquecouleur::CalqueCouleur::new(0));
    for f in [0, 2, 4] {
        let cel = dessin(&mut doc, contenu(LayerKind::Bitmap, 0));
        l.set(f, Some(cel));
    }
    doc.panels[0].layers.push(l);
    let mut b = ouvrir(doc, Tool::Eraser, 1);
    b.app.eraser.opacity = 0.5;
    let keys: Vec<_> = [0, 2, 4].iter().filter_map(|&f| b.app.key_ref(0, 0, f)).collect();
    b.app.key_anchor = keys.first().copied();
    b.app.key_sel = keys;
    b.chauffer(2);
    gommer_en_200(&mut b);
    repos(&mut b);
    let alpha = |b: &Banc, f: usize| b.app.doc.cels[&cel_de(b, 0, f)].raster.get(200, y_de(0) as usize).a();
    let courant = alpha(&b, 0);
    assert!(courant > 0 && courant < 255, "la gomme à 50 % laisse de l'encre sur la clé qu'on voit ({courant})");
    for f in [2, 4] {
        assert_eq!(alpha(&b, f), courant, "clé de l'image {f} : gommée une seule fois, comme celle qu'on voit");
    }
}

#[test]
fn the_shift_straight_eraser_segment_reaches_every_chosen_layer() {
    let kinds = [LayerKind::Bitmap, LayerKind::Bitmap];
    let mut b = banc(&kinds, Tool::Eraser);
    let cels: Vec<CelId> = (0..2).map(|li| cel_de(&b, li, 0)).collect();
    let (a, z) = (ecran(&b, pos2(200.0, 20.0)), ecran(&b, pos2(200.0, 280.0)));
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![bouton(a, true, Modifiers::NONE)], 1.0 / 60.0);
    // Maj tenue : un segment droit jusqu'au bas de la feuille, posé au lâcher.
    b.touches = Modifiers::SHIFT;
    b.image(vec![Event::PointerMoved(z)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![bouton(z, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    repos(&mut b);
    for (li, &cel) in cels.iter().enumerate() {
        assert!(!encre(&b, cel, pos2(200.0, y_de(li))), "calque {li} : le segment droit l'a gommé");
        assert!(encre(&b, cel, pos2(130.0, y_de(li))), "calque {li} : le reste est intact");
    }
}

#[test]
fn a_drawing_changed_while_the_frame_is_open_is_not_overwritten_by_the_old_original() {
    let mut b = banc_deformer(DeformMode::Perspective, 1);
    let encre_cel = cel_de(&b, 1, 0);
    clic(&mut b, pos2(200.0, 150.0));
    // Pendant que le cadre est ouvert, un trait de plus sur le calque vectoriel (comme un trait d'un autre outil).
    b.app.doc.cels.get_mut(&encre_cel).unwrap().vector.as_mut().unwrap().elements.push(trait_h(130.0, 270.0, 180.0));
    b.chauffer(2);
    // Puis le coin du cadre d'avant est tiré.
    glisser(&mut b, &ligne(pos2(100.0, 100.0), pos2(140.0, 130.0), 8));
    repos(&mut b);
    assert!(encre(&b, encre_cel, pos2(200.0, 180.0)) || vect(&b, encre_cel).elements.len() == 2, "le trait ajouté n'est pas perdu");
    assert_eq!(vect(&b, encre_cel).elements.len(), 2, "le dessin a toujours ses deux traits");
}

#[test]
fn two_chosen_keys_with_only_one_usable_drawing_keep_the_single_layer_envelope() {
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 4;
    for (i, nom) in ["Encre", "Verrouillé"].iter().enumerate() {
        let cel = dessin(&mut doc, contenu(LayerKind::Vector, i));
        let mut l = Layer::new(*nom, LayerKind::Vector);
        l.set(0, Some(cel));
        doc.panels[0].layers.push(l);
    }
    doc.panels[0].layers[1].locked = true;
    let mut b = ouvrir(doc, Tool::Perspective, 1);
    b.app.deform_mode = DeformMode::Envelope;
    let keys: Vec<_> = (0..2).filter_map(|li| b.app.key_ref(0, li, 0)).collect();
    assert_eq!(keys.len(), 2);
    b.app.key_sel = keys;
    b.chauffer(2);
    clic(&mut b, pos2(200.0, y_de(0)));
    assert!(b.app.envelope.is_some(), "un seul dessin visé : l'enveloppe d'un seul calque ({})", b.app.status);
}

// ---------------------------------------------------------------------------------------------- les clés qu'on ne voit pas, sur un fil de travail

/// La gomme sur les six clés de `banc_cles`, lâchée ; rien n'est encore attendu (aucune image d'écran après le lâcher).
fn gommer_les_cles_sans_attendre(b: &mut Banc) {
    let pts = ligne(pos2(200.0, 20.0), pos2(200.0, 280.0), 26);
    let p0 = ecran(b, pts[0]);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, Modifiers::NONE)], 1.0 / 60.0);
    for &p in &pts[1..] {
        let q = ecran(b, p);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    let fin = ecran(b, *pts.last().unwrap());
    b.image(vec![bouton(fin, false, Modifiers::NONE)], 1.0 / 60.0);
}

/// Les calculs en route sont attendus et posés (comme avant un enregistrement), puis le repos.
fn attendre_les_calculs(b: &mut Banc) {
    b.app.attendre_aplat_multi();
    repos(b);
}

#[test]
fn the_release_says_what_is_being_computed_and_everything_lands_in_the_same_step() {
    let mut b = banc_cles(Tool::Eraser);
    let dessins = dessins_des_cles(&b);
    let n0 = etapes(&b);
    gommer_les_cles_sans_attendre(&mut b);
    assert!(b.app.status.contains("Gomme :") && b.app.status.contains("/4"), "la barre d'état dit le travail en cours : {}", b.app.status);
    attendre_les_calculs(&mut b);
    for &(li, cel) in &dessins {
        assert!(!encre(&b, cel, pos2(200.0, y_de(li))), "calque {li}, dessin {cel} : gommé");
    }
    assert_eq!(etapes(&b), n0 + 1, "les résultats arrivés rejoignent l'étape du geste");
    b.app.undo();
    attendre_les_calculs(&mut b);
    for &(li, cel) in &dessins {
        assert!(encre(&b, cel, pos2(200.0, y_de(li))), "calque {li}, dessin {cel} : un seul Ctrl+Z rend tout");
    }
}

#[test]
fn an_undo_before_the_results_arrive_undoes_the_whole_gesture_and_drops_the_work() {
    let mut b = banc_cles(Tool::Eraser);
    let dessins = dessins_des_cles(&b);
    let avant: Vec<_> = dessins.iter().map(|&(_, c)| empreinte(&b, c)).collect();
    let n0 = etapes(&b);
    gommer_les_cles_sans_attendre(&mut b);
    b.app.undo();
    attendre_les_calculs(&mut b);
    for (k, &(li, cel)) in dessins.iter().enumerate() {
        assert!(empreinte(&b, cel) == avant[k], "calque {li}, dessin {cel} : rien n'est resté du geste annulé");
    }
    assert_eq!(etapes(&b), n0, "aucune étape n'est revenue");
}

#[test]
fn a_hidden_drawing_retouched_before_its_result_arrives_is_left_alone_with_a_message() {
    let mut b = banc_cles(Tool::Eraser);
    // Le dessin bitmap de la clé de l'image 4 (on ne le voit pas à l'image 0).
    let cel = cel_de(&b, 1, 4);
    gommer_les_cles_sans_attendre(&mut b);
    // Retouché avant l'arrivée de son résultat : un point posé ailleurs.
    {
        let c = b.app.doc.cels.get_mut(&cel).unwrap();
        let i = c.raster.tile_index_of(20, 20);
        c.raster.tile_mut(i)[(20 % TILE) * TILE + 20 % TILE] = Color32::from_rgba_premultiplied(200, 0, 0, 255);
        c.version += 1;
    }
    attendre_les_calculs(&mut b);
    let c = &b.app.doc.cels[&cel];
    assert!(c.raster.get(20, 20).r() == 200, "la retouche est gardée");
    assert!(c.raster.get(200, y_de(1) as usize).a() > 0, "le résultat périmé n'a pas écrasé la retouche");
    assert!(b.app.status.contains("retouch"), "la barre d'état le dit : {}", b.app.status);
}

#[test]
fn saving_while_the_hidden_keys_are_computed_writes_the_whole_gesture() {
    let mut b = banc_cles(Tool::Eraser);
    gommer_les_cles_sans_attendre(&mut b);
    let dir = std::env::temp_dir().join(format!("shot-multicalques-cles-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join("P.pelure");
    assert!(b.app.save_to_blocking(manifeste.clone()), "enregistré");
    let relu = crate::project::load(&manifeste).expect("ouverture").doc;
    for li in 0..2 {
        for f in [0, 2, 4] {
            let cel = relu.panels[0].layers[li].cel_at(f).unwrap();
            let c = &relu.cels[&cel];
            let gomme = match &c.vector {
                Some(d) => d.elements.iter().all(|e| match e {
                    Element::Stroke { points, .. } => !points.windows(2).any(|w| au_segment(pos2(200.0, y_de(li)), pos2(w[0].x, w[0].y), pos2(w[1].x, w[1].y)) < 6.0),
                    _ => true,
                }),
                None => c.raster.get(200, y_de(li) as usize).a() == 0,
            };
            assert!(gomme, "calque {li}, clé {f} : le fichier a le geste entier");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_release_of_a_long_eraser_stroke_on_twenty_hidden_keys_is_quick() {
    let (w, h) = (1920usize, 1080usize);
    let mut doc = Document::new(w, h, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 40;
    let mut l = Layer::new("Dessin", LayerKind::Bitmap);
    for f in 0..20 {
        let mut r = Raster::new(w, h);
        for y in (100..980).step_by(3) {
            for x in 100..1820 {
                let k = r.tile_index_of(x, y);
                r.tile_mut(k)[(y % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(30, 30, 30, 255);
            }
        }
        let cel = dessin(&mut doc, Contenu::Pixels(r));
        l.set(f * 2, Some(cel));
    }
    doc.panels[0].layers.push(l);
    let mut b = ouvrir(doc, Tool::Eraser, 1);
    let keys: Vec<_> = (0..20).filter_map(|f| b.app.key_ref(0, 0, f * 2)).collect();
    b.app.key_sel = keys;
    b.chauffer(2);
    let mut pts = Vec::new();
    for k in 0..12 {
        let y = 150.0 + 70.0 * k as f32;
        let (a, z) = if k % 2 == 0 { (150.0, 1770.0) } else { (1770.0, 150.0) };
        pts.extend(ligne(pos2(a, y), pos2(z, y + 35.0), 40));
    }
    let p0 = ecran(&b, pts[0]);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, Modifiers::NONE)], 1.0 / 60.0);
    for &p in &pts[1..] {
        let q = ecran(&b, p);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    let fin = ecran(&b, *pts.last().unwrap());
    let t = std::time::Instant::now();
    b.image(vec![bouton(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    let lacher = t.elapsed().as_secs_f64() * 1e3;
    println!("LACHER long trait sur 20 cles : {lacher:.1} ms");
    // (Le but est 50 ms ; la marge laisse passer une machine chargée par d'autres essais.)
    assert!(lacher < 200.0, "le lâcher rend la main tout de suite ({lacher:.0} ms)");
    attendre_les_calculs(&mut b);
    let cel = b.app.doc.panels[0].layers[0].cel_at(38).unwrap();
    assert_eq!(b.app.doc.cels[&cel].raster.get(960, 166).a(), 0, "la dernière clé est gommée, une fois le calcul arrivé");
    assert!(b.app.doc.cels[&cel].raster.get(960, 100).a() > 0, "et pas ailleurs");
}

/// Un calque bitmap de `n` clés (images 0, 1, 2…), toutes choisies ; l'outil Gomme.
fn banc_n_cles(n: usize) -> Banc {
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = n;
    let mut l = Layer::new("Dessin", LayerKind::Bitmap);
    for f in 0..n {
        let cel = dessin(&mut doc, contenu(LayerKind::Bitmap, 0));
        l.set(f, Some(cel));
    }
    doc.panels[0].layers.push(l);
    let mut b = ouvrir(doc, Tool::Eraser, 1);
    let keys: Vec<_> = (0..n).filter_map(|f| b.app.key_ref(0, 0, f)).collect();
    assert_eq!(keys.len(), n);
    b.app.key_sel = keys;
    b.chauffer(2);
    b
}

#[test]
fn undo_then_redo_during_the_computation_gives_back_the_whole_gesture() {
    let mut b = banc_n_cles(20);
    let cels: Vec<CelId> = (0..20).map(|f| cel_de(&b, 0, f)).collect();
    gommer_les_cles_sans_attendre(&mut b);
    // Ctrl+Z tout de suite ; quelques images passent (les calculs arrivent) ; puis Ctrl+Maj+Z : le geste entier revient, sur les
    // vingt clés.
    b.app.undo();
    attendre_les_calculs(&mut b);
    for f in 1..20 {
        assert!(encre(&b, cels[f], pos2(200.0, y_de(0))), "clé de l'image {f} : rien ne reste du geste annulé");
    }
    b.app.redo();
    attendre_les_calculs(&mut b);
    for (f, &cel) in cels.iter().enumerate() {
        assert!(!encre(&b, cel, pos2(200.0, y_de(0))), "clé de l'image {f} : gommée, comme sans annulation");
        assert!(encre(&b, cel, pos2(130.0, y_de(0))), "clé de l'image {f} : le reste est intact");
    }
    // Et un second Ctrl+Z rend les vingt dessins d'avant.
    b.app.undo();
    attendre_les_calculs(&mut b);
    for (f, &cel) in cels.iter().enumerate() {
        assert!(encre(&b, cel, pos2(200.0, y_de(0))), "clé de l'image {f} : rendue par Ctrl+Z");
    }
}

#[test]
fn the_eraser_warns_on_the_drawing_above_twenty_four_drawings_and_only_during_the_gesture() {
    assert_eq!(crate::gommemulti::SEUIL_ALERTE, 24);
    for (n, alerte) in [(24, false), (25, true)] {
        let mut b = banc_n_cles(n);
        b.garder_textes = true;
        let (a, z) = (ecran(&b, pos2(200.0, 20.0)), ecran(&b, pos2(200.0, 120.0)));
        b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
        b.image(vec![bouton(a, true, Modifiers::NONE)], 1.0 / 60.0);
        b.image(vec![Event::PointerMoved(z)], 1.0 / 60.0);
        let texte = format!("Gomme sur {n} dessins (Ctrl+Z annule tout)");
        assert_eq!(b.app.gomme_alerte().is_some(), alerte, "{n} dessins : alerte {alerte}");
        assert_eq!(b.textes.iter().any(|t| *t == texte), alerte, "{n} dessins : le bandeau est peint sur le dessin ({:?})", b.app.gomme_alerte());
        b.image(vec![bouton(z, false, Modifiers::NONE)], 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
        assert!(b.app.gomme_alerte().is_none(), "{n} dessins : plus d'alerte après le lâcher");
        assert!(!b.textes.iter().any(|t| t.starts_with("Gomme sur") && t.contains("Ctrl+Z annule tout")), "le bandeau s'en va");
        attendre_les_calculs(&mut b);
    }
}

/// Mesure : la gomme sur 20 clés d'un calque bitmap 1920 × 1080, un long trait en zigzag sur toute la feuille.
/// `cargo test --release --offline --locked --target-dir target\msvc --lib cout_long_trait -- --ignored --nocapture`
#[test]
#[ignore]
fn cout_long_trait_vingt_cles() {
    use std::time::Instant;
    let (w, h) = (1920usize, 1080usize);
    let mut doc = Document::new(w, h, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 40;
    let mut l = Layer::new("Dessin", LayerKind::Bitmap);
    for f in 0..20 {
        let mut r = Raster::new(w, h);
        for y in (100..980).step_by(3) {
            for x in 100..1820 {
                let k = r.tile_index_of(x, y);
                r.tile_mut(k)[(y % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(30, 30, 30, 255);
            }
        }
        let cel = dessin(&mut doc, Contenu::Pixels(r));
        l.set(f * 2, Some(cel));
    }
    doc.panels[0].layers.push(l);
    let mut b = ouvrir(doc, Tool::Eraser, 1);
    let keys: Vec<_> = (0..20).filter_map(|f| b.app.key_ref(0, 0, f * 2)).collect();
    b.app.key_sel = keys;
    b.chauffer(2);
    // Un zigzag de 12 allers-retours sur toute la largeur : environ 22 000 px de trait.
    let mut pts = Vec::new();
    for k in 0..12 {
        let y = 150.0 + 70.0 * k as f32;
        let (a, z) = if k % 2 == 0 { (150.0, 1770.0) } else { (1770.0, 150.0) };
        pts.extend(ligne(pos2(a, y), pos2(z, y + 35.0), 40));
    }
    let p0 = ecran(&b, pts[0]);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, Modifiers::NONE)], 1.0 / 60.0);
    assert!(b.app.status.contains("Gomme"), "{}", b.app.status);
    let mut pire: f64 = 0.0;
    for &p in &pts[1..] {
        let q = ecran(&b, p);
        let t = Instant::now();
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
        pire = pire.max(t.elapsed().as_secs_f64() * 1e3);
    }
    let fin = ecran(&b, *pts.last().unwrap());
    let t = Instant::now();
    b.image(vec![bouton(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    let lacher = t.elapsed().as_secs_f64() * 1e3;
    println!("COUT long trait, gomme sur 20 cles 1920x1080 : {} points, pire image du glisse {pire:.1} ms, lacher {lacher:.1} ms", pts.len());
}

/// Mesure : 10 calques × 20 clés (la moitié vectoriels), la gomme puis la perspective, sur une feuille de 960 × 540.
/// `cargo test --release --offline --locked --target-dir target\msvc --lib cout_dix_calques -- --ignored --nocapture`
#[test]
#[ignore]
fn cout_dix_calques_vingt_cles() {
    use std::time::Instant;
    let (w, h) = (960usize, 540usize);
    let mut doc = Document::new(w, h, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 40;
    for i in 0..10 {
        let kind = if i % 2 == 0 { LayerKind::Vector } else { LayerKind::Bitmap };
        let mut l = Layer::new(&format!("Calque {i}"), kind);
        for f in 0..20 {
            let y = 30.0 + 48.0 * i as f32;
            let c = match kind {
                LayerKind::Vector => Contenu::Traits((0..6).map(|j| trait_h(100.0, 860.0, y + j as f32 * 6.0)).collect()),
                LayerKind::Bitmap => {
                    let mut r = Raster::new(w, h);
                    for yy in y as usize..y as usize + 36 {
                        for x in 100..860 {
                            let k = r.tile_index_of(x, yy);
                            r.tile_mut(k)[(yy % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(30, 30, 30, 255);
                        }
                    }
                    Contenu::Pixels(r)
                }
            };
            let cel = dessin(&mut doc, c);
            l.set(f * 2, Some(cel));
        }
        doc.panels[0].layers.push(l);
    }
    let mut b = ouvrir(doc, Tool::Eraser, 1);
    let keys: Vec<_> = (0..10).flat_map(|li| (0..20).map(move |f| (li, f * 2))).filter_map(|(li, f)| b.app.key_ref(0, li, f)).collect();
    assert_eq!(keys.len(), 200);
    b.app.key_sel = keys;
    b.chauffer(2);
    // La gomme : chaque image du glissé, puis le lâcher.
    let pts = ligne(pos2(480.0, 40.0), pos2(480.0, 520.0), 30);
    let p0 = ecran(&b, pts[0]);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    let t = Instant::now();
    b.image(vec![bouton(p0, true, Modifiers::NONE)], 1.0 / 60.0);
    let appui = t.elapsed().as_secs_f64() * 1e3;
    let etat = b.app.status.clone();
    assert!(etat.contains("Gomme"), "la gomme a commencé : {etat} (pointeur {p0:?}, dessin {:?}, {} clés choisies)", b.app.canvas_rect, b.app.key_sel.len());
    let mut pire: f64 = 0.0;
    for &p in &pts[1..] {
        let q = ecran(&b, p);
        let t = Instant::now();
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
        pire = pire.max(t.elapsed().as_secs_f64() * 1e3);
    }
    let fin = ecran(&b, *pts.last().unwrap());
    let t = Instant::now();
    b.image(vec![bouton(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    let lacher = t.elapsed().as_secs_f64() * 1e3;
    println!("COUT gomme 10 calques x 20 cles : appui {appui:.1} ms, pire image du glisse {pire:.1} ms, lacher {lacher:.1} ms ({etat})");
    repos(&mut b);
    // Le dessin de la dernière clé du dernier calque (qu'on ne voit pas) est gommé lui aussi.
    let cel = b.app.doc.panels[0].layers[9].cel_at(38).unwrap();
    assert!(b.app.doc.cels[&cel].raster.get(480, 30 + 48 * 9 + 10).a() == 0, "gommé sur une clé qu'on ne voit pas");
    assert!(b.app.doc.cels[&cel].raster.get(300, 30 + 48 * 9 + 10).a() > 0);
    // La perspective : ouvrir, glisser l'intérieur, lâcher.
    b.app.set_tool(Tool::Perspective);
    b.app.deform_mode = DeformMode::Perspective;
    b.chauffer(2);
    let cel = b.app.doc.panels[0].layers[9].cel_at(38).unwrap();
    let avant = empreinte(&b, cel);
    let t = Instant::now();
    clic(&mut b, pos2(480.0, 200.0));
    let ouvrir_ms = t.elapsed().as_secs_f64() * 1e3;
    let etat = b.app.status.clone();
    let pts = ligne(pos2(480.0, 200.0), pos2(500.0, 220.0), 10);
    let p0 = ecran(&b, pts[0]);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, Modifiers::NONE)], 1.0 / 60.0);
    let mut pire: f64 = 0.0;
    for &p in &pts[1..] {
        let q = ecran(&b, p);
        let t = Instant::now();
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
        pire = pire.max(t.elapsed().as_secs_f64() * 1e3);
    }
    let fin = ecran(&b, *pts.last().unwrap());
    let t = Instant::now();
    b.image(vec![bouton(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    let lacher = t.elapsed().as_secs_f64() * 1e3;
    println!("COUT perspective 10 calques x 20 cles : ouverture {ouvrir_ms:.1} ms, pire image du glisse {pire:.1} ms, lacher {lacher:.1} ms ({etat})");
    repos(&mut b);
    assert!(empreinte(&b, cel) != avant, "déformé sur une clé qu'on ne voit pas");
}

// ---------------------------------------------------------------------------------------------- la sélection agit sur tout

/// Quatre images ; un calque vectoriel (courant), un bitmap avec un aplat, un calque de couleur (non lié), et un quatrième
/// vectoriel : chacun une clé à l'image 0, tenue jusqu'au bout. Les quatre calques sont choisis.
fn banc_quatre_sortes() -> Banc {
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 4;
    let sortes = [LayerKind::Vector, LayerKind::Bitmap, LayerKind::Bitmap, LayerKind::Vector];
    for (i, &k) in sortes.iter().enumerate() {
        let cel = dessin(&mut doc, contenu(k, i));
        let mut l = Layer::new(&format!("Calque {i}"), k);
        // (Une seule clé, tenue sur les quatre images.)
        l.frames = vec![Some(cel); 4];
        if i == 1 {
            l.fond = Some(Default::default());
        }
        if i == 2 {
            l.couleur = Some(crate::calquecouleur::CalqueCouleur::new(0));
        }
        doc.panels[0].layers.push(l);
    }
    let mut b = ouvrir(doc, Tool::Brush, 4);
    b.app.cur_frame = 2;
    b.chauffer(2);
    b
}

/// Le calque `li` commence-t-il une clé à l'image `f` ?
fn cle_ici(b: &Banc, li: usize, f: usize) -> bool {
    crate::handles::key_start_taken(&b.app.doc.panels[0].layers[li], f)
}

#[test]
fn a_new_key_goes_on_every_chosen_layer_in_one_step_and_survives_undo_redo_and_save() {
    let mut b = banc_quatre_sortes();
    // Le quatrième a déjà une clé à l'image 2 ; le troisième (calque de couleur) est verrouillé.
    let deja = dessin(&mut b.app.doc, contenu(LayerKind::Vector, 3));
    b.app.doc.panels[0].layers[3].set(2, Some(deja));
    b.app.doc.panels[0].layers[2].locked = true;
    b.chauffer(1);
    let n0 = etapes(&b);
    b.app.run_action("dessin.nouvelle_cle");
    for li in [0, 1] {
        assert!(cle_ici(&b, li, 2), "calque {li} : une nouvelle clé à l'image 2");
        let (a, z) = (cel_de(&b, li, 0), cel_de(&b, li, 2));
        assert_ne!(a, z, "calque {li} : un dessin à lui");
        assert!(encre(&b, z, pos2(200.0, y_de(li))), "calque {li} : une copie du dessin montré");
    }
    assert_eq!(cel_de(&b, 1, 2) != cel_de(&b, 1, 0) && b.app.doc.panels[0].layers[1].kind == LayerKind::Bitmap, true, "le bitmap reste bitmap");
    assert!(!cle_ici(&b, 2, 2), "le calque verrouillé est laissé de côté");
    assert_eq!(cel_de(&b, 3, 2), deja, "la clé qui était déjà là reste");
    assert_eq!(etapes(&b), n0 + 1, "une seule étape");
    assert!(b.app.status.contains("2 calques") && b.app.status.contains("verrouillé") && b.app.status.contains("une clé commence déjà ici"), "{}", b.app.status);
    b.app.undo();
    assert!(!cle_ici(&b, 0, 2) && !cle_ici(&b, 1, 2), "Ctrl+Z retire les deux clés");
    b.app.redo();
    assert!(cle_ici(&b, 0, 2) && cle_ici(&b, 1, 2), "Ctrl+Maj+Z les rend");
    // La clé vide : sur les calques choisis, à l'image 3.
    b.app.cur_frame = 3;
    b.app.run_action("dessin.nouvelle_cle_vide");
    for li in [0, 1] {
        assert!(cle_ici(&b, li, 3), "calque {li} : une clé vide à l'image 3");
        assert!(!encre(&b, cel_de(&b, li, 3), pos2(200.0, y_de(li))), "calque {li} : elle est vide");
    }
    let dir = std::env::temp_dir().join(format!("shot-multicalques-nouvelles-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifeste = dir.join("P.pelure");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("ouverture").doc;
    for li in [0, 1] {
        let l = &relu.panels[0].layers[li];
        assert!(crate::handles::key_start_taken(l, 2) && crate::handles::key_start_taken(l, 3), "calque {li} : les clés sont dans le fichier");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_new_key_with_a_single_chosen_layer_is_unchanged() {
    let mut b = banc_quatre_sortes();
    b.app.run_action("calque.choisir_aucun");
    b.app.cur_layer = 1;
    b.chauffer(1);
    b.app.run_action("dessin.nouvelle_cle");
    assert!(cle_ici(&b, 1, 2) && !cle_ici(&b, 0, 2) && !cle_ici(&b, 3, 2), "le calque courant seul");
    assert!(b.app.status.contains("Nouvelle clé sur « Calque 1 »"), "{}", b.app.status);
}

/// Le champ d'opacité du panneau Calques : le calque courant est mis à 37 % (une valeur que rien d'autre n'affiche), et le texte
/// « 37 % » peint une seule fois est celui du champ (la ligne du calque, dans la liste, le peint aussi : deux fois, au même endroit).
fn champ_opacite(b: &mut Banc) -> Pos2 {
    b.app.doc.panels[0].layers[0].opacity = 0.37;
    b.garder_formes = true;
    b.chauffer(2);
    let mut trouves: Vec<Pos2> = Vec::new();
    for s in &b.formes {
        if let eframe::egui::epaint::Shape::Text(t) = &s.shape {
            if t.galley.job.text == "37 %" {
                trouves.push(t.pos + t.galley.size() * 0.5);
            }
        }
    }
    b.garder_formes = false;
    let seuls: Vec<Pos2> = trouves.iter().copied().filter(|p| trouves.iter().filter(|q| q.distance(*p) < 1.0).count() == 1).collect();
    assert_eq!(seuls.len(), 1, "un seul champ « 37 % » à l'écran : {trouves:?}");
    seuls[0]
}

#[test]
fn the_opacity_field_sets_every_chosen_layer_in_one_step() {
    let mut b = banc_quatre_sortes();
    b.app.doc.panels[0].layers[2].locked = true;
    b.chauffer(1);
    let c = champ_opacite(&mut b);
    let n0 = etapes(&b);
    // Glisser le champ de 60 px vers la droite (0,5 % par pixel).
    b.image(vec![Event::PointerMoved(c)], 1.0 / 60.0);
    b.image(vec![bouton(c, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=12 {
        b.image(vec![Event::PointerMoved(c + eframe::egui::vec2(5.0 * k as f32, 0.0))], 1.0 / 60.0);
    }
    b.image(vec![bouton(c + eframe::egui::vec2(60.0, 0.0), false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(2);
    let v0 = b.app.doc.panels[0].layers[0].opacity_at(2);
    assert!(v0 > 0.45, "le calque courant a changé ({v0})");
    for li in 0..4 {
        let v = b.app.doc.panels[0].layers[li].opacity_at(2);
        assert!((v - v0).abs() < 1e-4, "calque {li} : la même opacité que le courant ({v} et {v0}), verrouillé compris");
    }
    assert_eq!(etapes(&b), n0 + 1, "un glissé, une seule étape");
    b.app.undo();
    assert!((b.app.doc.panels[0].layers[0].opacity_at(2) - 0.37).abs() < 1e-4, "le courant : Ctrl+Z rend 37 %");
    for li in 1..4 {
        assert!((b.app.doc.panels[0].layers[li].opacity_at(2) - 1.0).abs() < 1e-4, "calque {li} : Ctrl+Z rend 100 %");
    }
}

#[test]
fn delete_on_chosen_keys_empties_their_drawings_once_and_keeps_the_keys() {
    let mut b = banc_cles(Tool::Brush);
    // Le dessin de la clé de l'image 2 du calque 0 est aussi montré à l'image 3 par une seconde clé (la même image tenue deux fois).
    let partage = cel_de(&b, 0, 2);
    b.app.doc.panels[0].layers[0].set(3, Some(partage));
    let mut keys = b.app.key_sel.clone();
    keys.extend(b.app.key_ref(0, 0, 3));
    b.app.key_sel = keys;
    b.app.key_focus = true;
    let dessins = dessins_des_cles(&b);
    let n0 = etapes(&b);
    b.app.run_action("edition.supprimer");
    for &(li, cel) in &dessins {
        assert!(!encre(&b, cel, pos2(200.0, y_de(li))) && !encre(&b, cel, pos2(130.0, y_de(li))), "calque {li}, dessin {cel} : vidé");
    }
    for li in 0..2 {
        for f in [0, 2, 4] {
            assert!(cle_ici(&b, li, f), "calque {li}, clé de l'image {f} : toujours là");
        }
    }
    assert_eq!(etapes(&b), n0 + 1, "une seule étape");
    assert!(b.app.status.contains("7 clés vidées sur 2 calques"), "{}", b.app.status);
    b.app.undo();
    for &(li, cel) in &dessins {
        assert!(encre(&b, cel, pos2(200.0, y_de(li))), "calque {li}, dessin {cel} : rendu par Ctrl+Z");
    }
}

#[test]
fn delete_leaves_a_locked_layer_and_keeps_deleting_a_canvas_selection_first() {
    let mut b = banc_cles(Tool::Brush);
    b.app.doc.panels[0].layers[1].locked = true;
    b.app.key_focus = true;
    b.app.run_action("edition.supprimer");
    assert!(encre(&b, cel_de(&b, 1, 2), pos2(200.0, y_de(1))), "le calque verrouillé n'est pas vidé");
    assert!(!encre(&b, cel_de(&b, 0, 2), pos2(200.0, y_de(0))), "l'autre l'est");
    assert!(b.app.status.contains("verrouillé"), "{}", b.app.status);
    // Une sélection sur le dessin : Suppr l'efface, elle, et pas les clés.
    let mut b = banc_cles(Tool::Select);
    let cel = cel_de(&b, 0, 0);
    b.app.vsel = Some(crate::vtools::VSelection { cel, items: vec![0] });
    b.app.key_focus = true;
    b.app.run_action("edition.supprimer");
    assert!(vect(&b, cel).elements.is_empty(), "la sélection du dessin est effacée");
    assert!(encre(&b, cel_de(&b, 0, 2), pos2(200.0, y_de(0))), "les clés ne sont pas vidées");
}

#[test]
fn clear_flip_and_opacity_key_act_on_every_chosen_layer() {
    // Vider le dessin.
    let mut b = banc_quatre_sortes();
    b.app.doc.panels[0].layers[3].locked = true;
    let n0 = etapes(&b);
    b.app.run_action("dessin.vider");
    for li in 0..3 {
        assert!(!encre(&b, cel_de(&b, li, 2), pos2(200.0, y_de(li))), "calque {li} : vidé");
    }
    assert!(encre(&b, cel_de(&b, 3, 2), pos2(200.0, y_de(3))), "le verrouillé ne l'est pas");
    assert_eq!(etapes(&b), n0 + 1);
    // Retourner : ensemble, autour du milieu de leur boîte commune (les traits de x 108 à 292, les bitmap à la tuile près).
    let mut b = banc_quatre_sortes();
    b.app.run_action("calque.miroir_h");
    let pivot = b.app.doc.stage_center();
    let t_ = TILE as f32;
    let axe = ((110.0 / t_).floor() * t_).min(108.0) * 0.5 + ((290.0 / t_).ceil() * t_).max(292.0) * 0.5;
    for li in 0..4 {
        let t = b.app.doc.panels[0].layers[li].transform;
        assert!(t.stretch_x < 0.0, "calque {li} : retourné");
        let p = t.affine(pivot).apply(pos2(120.0, y_de(li)));
        assert!((p.x - (2.0 * axe - 120.0)).abs() < 1.5, "calque {li} : autour du milieu commun x = {axe} ({p:?})");
    }
    // Clé d'opacité.
    let mut b = banc_quatre_sortes();
    b.app.run_action("calque.cle_opacite");
    for li in 0..4 {
        assert!(!b.app.doc.panels[0].layers[li].opacity_keys.is_empty(), "calque {li} : une clé d'opacité");
    }
}

// ---------------------------------------------------------------------------------------------- copier, coller une sélection
//
// Au clavier, comme dans la vraie fenêtre : egui transforme Ctrl+C en `Event::Copy` et Ctrl+V en `Event::Paste` (avec le texte du
// presse-papiers : celui qu'une copie de FEANOR y écrit). Le presse-papiers de Windows n'est jamais lu dans les essais
// (`pasteimage::clipboard_image`).

fn ctrl_c(b: &mut Banc) {
    b.image(vec![Event::Copy], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
}

fn ctrl_v(b: &mut Banc) {
    b.image(vec![Event::Paste(format!("{}copie", crate::pasteimage::MARK))], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Un rectangle de l'outil Sélection autour de tout le dessin (le pointeur part du vide).
fn rectangle_autour(b: &mut Banc) {
    // (Un tour fermé : un lasso pour un seul calque vectoriel, le même tour pour les autres.)
    b.app.select_rect = false;
    let (a, z) = (pos2(60.0, 20.0), pos2(340.0, 280.0));
    let mut tour = ligne(a, pos2(z.x, a.y), 8);
    tour.extend(ligne(pos2(z.x, a.y), z, 8));
    tour.extend(ligne(z, pos2(a.x, z.y), 8));
    tour.extend(ligne(pos2(a.x, z.y), a, 8));
    glisser(b, &tour);
}

#[test]
fn copy_then_paste_a_vector_selection_puts_editable_strokes_at_the_same_place_and_selects_them() {
    for mode in [Mode::Animation, Mode::Storyboard] {
        let mut b = banc(&[LayerKind::Vector], Tool::Select);
        b.app.set_mode(mode);
        b.app.board_view = crate::app::BoardView::Scene;
        b.chauffer(4);
        let cel = cel_de(&b, 0, 0);
        rectangle_autour(&mut b);
        assert!(b.app.vsel.as_ref().is_some_and(|s| !s.items.is_empty()), "{mode:?} : le trait est choisi ({})", b.app.status);
        let n0 = etapes(&b);
        ctrl_c(&mut b);
        ctrl_v(&mut b);
        let d = vect(&b, cel);
        assert_eq!(d.elements.len(), 2, "{mode:?} : la copie est collée, en trait vectoriel ({})", b.app.status);
        assert!(d.elements[0].bounds() == d.elements[1].bounds(), "{mode:?} : à la même place");
        assert_eq!(b.app.vsel.as_ref().map(|s| s.items.clone()), Some(vec![1]), "{mode:?} : la copie est choisie, prête à bouger");
        assert_eq!(etapes(&b), n0 + 1, "{mode:?} : une étape");
        // À une autre image : la copie va dans le dessin de cette image.
        b.app.cur_frame = 2;
        b.chauffer(2);
        ctrl_v(&mut b);
        let ici = b.app.doc.panels[0].layers[0].cel_at(2).expect("un dessin à l'image 2");
        assert!(vect(&b, ici).elements.iter().any(|e| e.bounds() == d.elements[0].bounds()), "{mode:?} : collée à l'image 2 aussi");
    }
}

#[test]
fn copy_then_paste_a_bitmap_selection_pastes_pixels() {
    let mut b = banc(&[LayerKind::Bitmap], Tool::Select);
    let cel = cel_de(&b, 0, 0);
    rectangle_autour(&mut b);
    assert!(b.app.bsel.is_some(), "une zone est choisie ({})", b.app.status);
    ctrl_c(&mut b);
    ctrl_v(&mut b);
    assert!(b.app.bsel.as_ref().is_some_and(|s| s.lifted.is_some()), "les pixels collés sont soulevés, prêts à bouger ({})", b.app.status);
    b.app.drop_bitmap_selection();
    repos(&mut b);
    assert!(encre(&b, cel, pos2(200.0, y_de(0))), "les pixels sont là");
}

#[test]
fn copy_then_paste_on_several_chosen_layers_gives_each_its_own_strokes() {
    let mut b = banc(&[LayerKind::Vector, LayerKind::Vector], Tool::Select);
    rectangle_autour(&mut b);
    assert!(b.app.multi.sel.is_some(), "{}", b.app.status);
    ctrl_c(&mut b);
    ctrl_v(&mut b);
    for li in 0..2 {
        let d = vect(&b, cel_de(&b, li, 0));
        assert_eq!(d.elements.len(), 2, "calque {li} : sa copie, sur lui ({})", b.app.status);
    }
}

#[test]
fn copy_then_paste_strokes_on_the_board_puts_a_whole_copy_and_selects_it() {
    use crate::vignettes_selection_tests::{choisies, lasso, parts};
    let Some(mut b) = crate::traits_continus_tests::planche(crate::traits_continus_tests::des_plans(12, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let t2 = xf.rect(l.cards[2].thumb);
    // Un trait dans la case 2, qui en déborde à droite.
    let (de, a) = (t2.center() - eframe::egui::vec2(20.0, 0.0), pos2(t2.right() + 25.0, t2.center().y + 5.0));
    crate::traits_continus_tests::tracer(&mut b, de, a);
    crate::traits_continus_tests::repos(&mut b);
    let (el0, pl0) = parts(&b);
    lasso(&mut b, t2.center(), 14.0);
    assert_eq!(choisies(&b), 1, "le trait est choisi ({})", b.app.status);
    let n0 = b.app.history.undo_len();
    ctrl_c(&mut b);
    ctrl_v(&mut b);
    let (el1, pl1) = parts(&b);
    assert_eq!(el1 + pl1, el0 + pl0 + 1, "un trait de plus, entier ({}) ; avant {:?}, après {:?}", b.app.status, (el0, pl0), (el1, pl1));
    assert_eq!(choisies(&b), 1, "la copie est choisie, prête à bouger");
    assert_eq!(b.app.history.undo_len(), n0 + 1, "une étape");
}

// ---------------------------------------------------------------------------------------------- seconde relecture (après c3ec7c9)

#[test]
fn a_hidden_vector_drawing_whose_exact_render_arrives_meanwhile_is_still_erased() {
    let mut b = banc_cles(Tool::Eraser);
    // Le dessin vectoriel de la clé de l'image 2 : on ne le voit pas.
    let cel = cel_de(&b, 0, 2);
    gommer_les_cles_sans_attendre(&mut b);
    // Son rendu exact arrive pendant le calcul : la version monte, les traits ne changent pas (`poll_rejeu`).
    b.app.doc.cels.get_mut(&cel).unwrap().version += 1;
    attendre_les_calculs(&mut b);
    assert!(!encre(&b, cel, pos2(200.0, y_de(0))), "gommé quand même : ce n'est pas une retouche ({})", b.app.status);
}

#[test]
fn a_result_waits_while_its_drawing_is_being_drawn_on() {
    let mut b = banc_cles(Tool::Eraser);
    let cel = cel_de(&b, 1, 2);
    // (Le calcul attend que le trait ait commencé : il arrive pendant le trait, à coup sûr. Un projet à lui : les autres essais ne
    // sont pas retenus.)
    b.app.doc_epoch = crate::onglets::nouvelle_epoque();
    *crate::gommemulti::PAUSE_ESSAI.lock().unwrap() = Some(b.app.doc_epoch);
    gommer_les_cles_sans_attendre(&mut b);
    // On va à l'image 2, sur le calque bitmap, et on y trace un trait au pinceau (là où il n'y a rien), lentement : le calcul arrive
    // pendant le trait.
    b.app.key_sel.clear();
    (b.app.cur_layer, b.app.cur_frame) = (1, 2);
    b.app.set_tool(Tool::Brush);
    b.app.brush.size = 8.0;
    b.app.brush.opacity = 1.0;
    b.chauffer(1);
    let (a, z) = (ecran(&b, pos2(120.0, 250.0)), ecran(&b, pos2(280.0, 250.0)));
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![bouton(a, true, Modifiers::NONE)], 1.0 / 60.0);
    *crate::gommemulti::PAUSE_ESSAI.lock().unwrap() = None;
    for k in 1..=40 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 40.0))], 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(encre(&b, cel, pos2(200.0, y_de(1))), "pendant le trait, le résultat de la gomme n'est pas posé");
    }
    b.image(vec![bouton(z, false, Modifiers::NONE)], 1.0 / 60.0);
    attendre_les_calculs(&mut b);
    for x in [125usize, 200, 275] {
        assert!(b.app.doc.cels[&cel].raster.get(x, 250).a() > 0, "le trait est entier, rien n'a été écrasé (x = {x})");
    }
    assert!(b.app.status.contains("retouch"), "le dessin retouché pendant le calcul est laissé tel quel, et c'est dit : {}", b.app.status);
}

#[test]
fn a_failed_computation_undoes_the_whole_gesture_and_never_blocks() {
    let mut b = banc_cles(Tool::Eraser);
    let dessins = dessins_des_cles(&b);
    let avant: Vec<_> = dessins.iter().map(|&(_, c)| empreinte(&b, c)).collect();
    let n0 = etapes(&b);
    // (Un projet à lui : les autres essais, qui tournent en même temps, ne sont pas touchés par ce crochet.)
    b.app.doc_epoch = crate::onglets::nouvelle_epoque();
    *crate::gommemulti::PANIQUE_ESSAI.lock().unwrap() = Some((b.app.doc_epoch, cel_de(&b, 1, 4)));
    gommer_les_cles_sans_attendre(&mut b);
    let t = std::time::Instant::now();
    attendre_les_calculs(&mut b);
    *crate::gommemulti::PANIQUE_ESSAI.lock().unwrap() = None;
    assert!(t.elapsed() < std::time::Duration::from_secs(20), "l'attente finit ({:?})", t.elapsed());
    for (k, &(li, cel)) in dessins.iter().enumerate() {
        assert!(empreinte(&b, cel) == avant[k], "calque {li}, dessin {cel} : tout ou rien, rien n'a changé");
    }
    assert_eq!(etapes(&b), n0, "le geste est retiré de l'historique");
    assert!(b.app.status.contains("annulée en entier") && b.app.status.contains("1 dessin"), "{}", b.app.status);
}

#[test]
fn the_eraser_step_exists_at_release_even_when_nothing_seen_changed() {
    let mut b = banc_cles(Tool::Eraser);
    // Les dessins qu'on voit, à l'image 0, sont vides : rien n'y change.
    b.app.doc.cels.get_mut(&cel_de(&b, 0, 0)).unwrap().vector.as_mut().unwrap().elements.clear();
    b.app.doc.cels.get_mut(&cel_de(&b, 1, 0)).unwrap().raster = Raster::new(W, H);
    let n0 = etapes(&b);
    gommer_les_cles_sans_attendre(&mut b);
    assert_eq!(etapes(&b), n0 + 1, "l'étape « Gomme » est là dès le lâcher");
    attendre_les_calculs(&mut b);
    assert_eq!(etapes(&b), n0 + 1, "les résultats l'ont rejointe");
    assert!(!encre(&b, cel_de(&b, 0, 2), pos2(200.0, y_de(0))), "les autres clés sont gommées");
    b.app.undo();
    assert!(encre(&b, cel_de(&b, 0, 2), pos2(200.0, y_de(0))), "un Ctrl+Z annule tout le geste");
}

/// Vingt clés d'un calque bitmap 1920 × 1080 couvert de lignes (le banc du long trait).
fn banc_vingt_cles_hd() -> Banc {
    let (w, h) = (1920usize, 1080usize);
    let mut doc = Document::new(w, h, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 40;
    let mut l = Layer::new("Dessin", LayerKind::Bitmap);
    for f in 0..20 {
        let mut r = Raster::new(w, h);
        for y in (100..980).step_by(3) {
            for x in 100..1820 {
                let k = r.tile_index_of(x, y);
                r.tile_mut(k)[(y % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(30, 30, 30, 255);
            }
        }
        let cel = dessin(&mut doc, Contenu::Pixels(r));
        l.set(f * 2, Some(cel));
    }
    doc.panels[0].layers.push(l);
    let mut b = ouvrir(doc, Tool::Eraser, 1);
    let keys: Vec<_> = (0..20).filter_map(|f| b.app.key_ref(0, 0, f * 2)).collect();
    b.app.key_sel = keys;
    b.chauffer(2);
    b
}

/// Un long zigzag de gomme sur toute la feuille, lâché, sans attendre ; `dy` le décale vers le bas.
fn zigzag(b: &mut Banc, dy: f32) {
    let mut pts = Vec::new();
    for k in 0..12 {
        let y = 150.0 + 70.0 * k as f32 + dy;
        let (a, z) = if k % 2 == 0 { (150.0, 1770.0) } else { (1770.0, 150.0) };
        pts.extend(ligne(pos2(a, y), pos2(z, y + 35.0), 40));
    }
    let p0 = ecran(b, pts[0]);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true, Modifiers::NONE)], 1.0 / 60.0);
    for &p in &pts[1..] {
        let q = ecran(b, p);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    let fin = ecran(b, *pts.last().unwrap());
    b.image(vec![bouton(fin, false, Modifiers::NONE)], 1.0 / 60.0);
}

#[test]
fn a_second_eraser_gesture_starts_at_once_and_its_hidden_part_follows_in_order() {
    let mut b = banc_vingt_cles_hd();
    zigzag(&mut b, 0.0);
    // Le second geste, tout de suite : l'appui ne bloque pas.
    let p = ecran(&b, pos2(960.0, 120.0));
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    let t = std::time::Instant::now();
    b.image(vec![bouton(p, true, Modifiers::NONE)], 1.0 / 60.0);
    let appui = t.elapsed().as_secs_f64() * 1e3;
    let q = ecran(&b, pos2(960.0, 990.0));
    for k in 1..=20 {
        b.image(vec![Event::PointerMoved(p + (q - p) * (k as f32 / 20.0))], 1.0 / 60.0);
    }
    b.image(vec![bouton(q, false, Modifiers::NONE)], 1.0 / 60.0);
    println!("APPUI du second geste pendant le calcul du premier : {appui:.1} ms");
    assert!(appui < 150.0, "le second geste démarre tout de suite ({appui:.0} ms)");
    attendre_les_calculs(&mut b);
    let cel = b.app.doc.panels[0].layers[0].cel_at(38).unwrap();
    let r = &b.app.doc.cels[&cel].raster;
    assert_eq!(r.get(960, 166).a(), 0, "le premier geste est posé sur la dernière clé");
    assert_eq!(r.get(961, 400).a() | r.get(960, 400).a(), 0, "le second aussi");
    assert!(!b.app.status.contains("retouch"), "rien n'est pris pour une retouche : {}", b.app.status);
}

#[test]
fn undo_during_a_long_computation_does_not_freeze_and_shows_the_progress() {
    let mut b = banc_vingt_cles_hd();
    let cel = b.app.doc.panels[0].layers[0].cel_at(38).unwrap();
    zigzag(&mut b, 0.0);
    let t = std::time::Instant::now();
    b.app.undo();
    let ms = t.elapsed().as_secs_f64() * 1e3;
    println!("CTRL+Z pendant le calcul : rendu la main en {ms:.1} ms");
    if b.app.gomme_travaux.en_route > 0 || ms > 150.0 {
        assert!(ms < 400.0, "Ctrl+Z rend la main ({ms:.0} ms)");
        assert!(b.app.gomme_alerte().is_some_and(|a| a.contains("Annuler attend")), "l'avancée se voit : {:?}", b.app.gomme_alerte());
    }
    attendre_les_calculs(&mut b);
    b.chauffer(3);
    assert!(b.app.doc.cels[&cel].raster.get(960, 166).a() > 0, "Ctrl+Z a fini par annuler le geste entier");
}

#[test]
fn delete_empties_the_chosen_keys_even_with_a_selection_left_on_another_frame() {
    let mut b = banc_cles(Tool::Brush);
    let ailleurs = cel_de(&b, 0, 4);
    b.app.vsel = Some(crate::vtools::VSelection { cel: ailleurs, items: vec![0] });
    b.app.key_focus = true;
    b.app.run_action("edition.supprimer");
    assert!(!encre(&b, cel_de(&b, 0, 2), pos2(200.0, y_de(0))), "les clés choisies sont vidées ({})", b.app.status);
}

#[test]
fn delete_on_the_board_never_empties_keys_and_without_chosen_keys_does_nothing() {
    let mut b = banc_cles(Tool::Brush);
    b.app.set_mode(Mode::Storyboard);
    b.app.board_view = crate::app::BoardView::Panels;
    b.app.key_focus = true;
    b.app.run_action("edition.supprimer");
    assert!(encre(&b, cel_de(&b, 0, 2), pos2(200.0, y_de(0))), "dans les Vignettes, Suppr ne vide pas les clés");
    let mut b = banc_cles(Tool::Brush);
    b.app.key_sel.clear();
    b.app.key_focus = true;
    b.app.run_action("edition.supprimer");
    assert!(encre(&b, cel_de(&b, 0, 0), pos2(200.0, y_de(0))), "sans clé choisie, Suppr n'efface rien");
}

#[test]
fn a_computation_for_a_project_left_behind_ends_silently() {
    let mut b = banc_cles(Tool::Eraser);
    gommer_les_cles_sans_attendre(&mut b);
    // Un autre projet est ouvert dans ce même onglet avant la fin du calcul.
    b.app.doc = Document::new(W, H, 24);
    b.app.doc_epoch = crate::onglets::nouvelle_epoque();
    b.app.status = "autre projet".into();
    b.app.attendre_gomme_multi();
    assert_eq!(b.app.status, "autre projet", "aucun message de la gomme d'un autre projet");
}

// ---------------------------------------------------------------------------------------------- Ctrl+A

/// Trois calques (vectoriel, bitmap, vectoriel), le premier courant ; `choisis` : combien sont choisis (en partant du bas).
fn banc_trois(choisis: usize) -> Banc {
    let kinds = [LayerKind::Vector, LayerKind::Bitmap, LayerKind::Vector];
    let mut doc = Document::new(W, H, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 4;
    for (i, &k) in kinds.iter().enumerate() {
        let cel = dessin(&mut doc, contenu(k, i));
        let mut l = Layer::new(&format!("Calque {i}"), k);
        l.set(0, Some(cel));
        doc.panels[0].layers.push(l);
    }
    ouvrir(doc, Tool::Brush, choisis)
}

#[test]
fn select_all_with_one_chosen_layer_takes_only_its_strokes() {
    let mut b = banc_trois(1);
    b.app.run_action("edition.tout_selectionner");
    assert!(b.app.multi.sel.is_none(), "un seul calque : pas de sélection sur plusieurs calques");
    assert_eq!(b.app.vsel.as_ref().map(|s| s.cel), Some(cel_de(&b, 0, 0)), "les traits du calque courant seul");
    assert!(b.app.status.contains("1 trait sur 1 calque"), "{}", b.app.status);
}

#[test]
fn select_all_with_three_chosen_layers_takes_all_and_only_theirs_and_a_move_leaves_the_others() {
    let mut doc_banc = banc_trois(3);
    let b = &mut doc_banc;
    // Un quatrième calque, pas choisi.
    let cel4 = dessin(&mut b.app.doc, contenu(LayerKind::Vector, 4));
    let mut l = Layer::new("Pas choisi", LayerKind::Vector);
    l.set(0, Some(cel4));
    b.app.doc.panels[0].layers.push(l);
    b.chauffer(1);
    let avant4 = empreinte(b, cel4);
    b.app.run_action("edition.tout_selectionner");
    let sel = b.app.multi.sel.as_ref().expect("une sélection sur plusieurs calques");
    assert_eq!(sel.parts.len(), 3, "les trois calques choisis, pas le quatrième");
    assert!(b.app.status.contains("2 traits et 1 zone sur 3 calques"), "{}", b.app.status);
    // Déplacer la sélection (flèches) : le calque pas choisi ne bouge pas.
    b.app.multi_nudge(eframe::egui::vec2(10.0, 0.0));
    b.app.drop_multi();
    b.app.multi_flush();
    repos(b);
    assert!(empreinte(b, cel4) == avant4, "le calque pas choisi n'a pas bougé");
    assert!(encre(b, cel_de(b, 0, 0), pos2(295.0, y_de(0))), "le calque choisi a bougé");
}

#[test]
fn select_all_leaves_a_locked_layer_aside_and_says_so() {
    let mut b = banc_trois(3);
    b.app.doc.panels[0].layers[2].locked = true;
    b.app.run_action("edition.tout_selectionner");
    let sel = b.app.multi.sel.as_ref().expect("une sélection");
    assert_eq!(sel.parts.len(), 2, "le verrouillé est laissé de côté");
    assert!(b.app.status.contains("1 trait et 1 zone sur 2 calques") && b.app.status.contains("verrouillé"), "{}", b.app.status);
}
