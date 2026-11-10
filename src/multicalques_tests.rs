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
