//! Réglages de l'en-tête des calques (13b) : préserver la transparence, pochoirs, position
//! verrouillée, groupes de couleur, et leur enregistrement (format 13).

use eframe::egui::{pos2, vec2, Color32};

use crate::brush::{BrushSettings, StrokeMode, StrokeState};
use crate::document::{Document, Layer, LayerKind};
use crate::groups;
use crate::project;
use crate::raster::{premultiply, Raster};
use crate::stencil::Stencil;
use crate::vector::Affine;

/// Remplit le rectangle [x0, x1[ × [y0, y1[ de `color` (prémultipliée).
fn put(r: &mut Raster, (x0, x1): (usize, usize), (y0, y1): (usize, usize), color: Color32) {
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = r.tile_index_of(x, y);
            let t = crate::raster::TILE;
            r.tile_mut(idx)[(y % t) * t + x % t] = color;
        }
    }
}

/// Trait dur de 10 px, opaque, de `from` à `to`.
fn stroke(r: &mut Raster, mode: StrokeMode, keep_alpha: bool, stencil: Option<Stencil>, from: (f32, f32), to: (f32, f32)) {
    let mut s = BrushSettings::brush();
    s.size = 10.0;
    s.hardness = 1.0;
    s.opacity = 1.0;
    s.smoothing = 0.0;
    s.pressure_size = false;
    s.pressure_opacity = false;
    let mut st = StrokeState::new(1, mode, s, None, false).on_layer(keep_alpha, stencil);
    let mut dirty = Vec::new();
    for i in 0..=40 {
        let t = i as f32 / 40.0;
        st.add_point(r, pos2(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t), 1.0, &mut dirty);
    }
    st.finish(r, &mut dirty);
}

#[test]
fn keeping_alpha_paints_only_what_is_drawn() {
    let mut r = Raster::new(200, 100);
    put(&mut r, (0, 100), (0, 100), premultiply(255, 0, 0, 255));
    put(&mut r, (100, 140), (0, 100), premultiply(255, 0, 0, 128));
    stroke(&mut r, StrokeMode::Paint([0, 0, 255]), true, None, (10.0, 50.0), (190.0, 50.0));
    let opaque = r.get(50, 50);
    assert_eq!(opaque.a(), 255);
    assert!(opaque.b() > 240 && opaque.r() < 15, "le dessin opaque prend la couleur : {opaque:?}");
    let half = r.get(120, 50);
    assert_eq!(half.a(), 128, "l'opacité d'un pixel ne change pas");
    assert!(half.b() > 110 && half.r() < 15, "mais sa couleur, oui : {half:?}");
    assert_eq!(r.get(170, 50).a(), 0, "le vide reste vide");
    assert_eq!(r.get(50, 10), premultiply(255, 0, 0, 255), "hors du trait, rien ne change");
}

#[test]
fn the_eraser_ignores_kept_alpha() {
    let mut r = Raster::new(200, 100);
    put(&mut r, (0, 200), (0, 100), premultiply(255, 0, 0, 255));
    stroke(&mut r, StrokeMode::Erase, true, None, (10.0, 50.0), (190.0, 50.0));
    assert_eq!(r.get(100, 50).a(), 0, "la gomme gomme normalement");
}

#[test]
fn a_positive_stencil_lets_paint_through_its_shape_only() {
    let mut shape = Raster::new(200, 100);
    put(&mut shape, (0, 100), (0, 100), premultiply(0, 0, 0, 255));
    let mut stencil = Stencil::new(200, 100);
    stencil.add(&shape, false, None);
    let mut r = Raster::new(200, 100);
    stroke(&mut r, StrokeMode::Paint([0, 0, 0]), false, Some(stencil), (10.0, 50.0), (190.0, 50.0));
    assert_eq!(r.get(50, 50).a(), 255);
    assert_eq!(r.get(150, 50).a(), 0);
}

#[test]
fn a_negative_stencil_lets_paint_outside_its_shape_only() {
    let mut shape = Raster::new(200, 100);
    put(&mut shape, (0, 100), (0, 100), premultiply(0, 0, 0, 255));
    let mut stencil = Stencil::new(200, 100);
    stencil.add(&shape, true, None);
    let mut r = Raster::new(200, 100);
    stroke(&mut r, StrokeMode::Paint([0, 0, 0]), false, Some(stencil), (10.0, 50.0), (190.0, 50.0));
    assert_eq!(r.get(50, 50).a(), 0);
    assert_eq!(r.get(150, 50).a(), 255);
}

#[test]
fn stencils_add_up_and_also_hold_back_the_eraser() {
    let mut left = Raster::new(200, 100);
    put(&mut left, (0, 100), (0, 100), premultiply(0, 0, 0, 255));
    let mut top = Raster::new(200, 100);
    put(&mut top, (0, 200), (0, 50), premultiply(0, 0, 0, 255));
    let make = || {
        let mut s = Stencil::new(200, 100);
        s.add(&left, false, None);
        s.add(&top, true, None);
        s
    };
    let mut r = Raster::new(200, 100);
    stroke(&mut r, StrokeMode::Paint([0, 0, 0]), false, Some(make()), (10.0, 75.0), (190.0, 75.0));
    stroke(&mut r, StrokeMode::Paint([0, 0, 0]), false, Some(make()), (10.0, 25.0), (190.0, 25.0));
    assert_eq!(r.get(50, 75).a(), 255, "à gauche et en bas : on peint");
    assert_eq!(r.get(150, 75).a(), 0, "hors du pochoir positif");
    assert_eq!(r.get(50, 25).a(), 0, "dans le pochoir négatif");

    let mut full = Raster::new(200, 100);
    put(&mut full, (0, 200), (0, 100), premultiply(0, 0, 0, 255));
    stroke(&mut full, StrokeMode::Erase, false, Some(make()), (10.0, 75.0), (190.0, 75.0));
    assert_eq!(full.get(50, 75).a(), 0, "la gomme passe là où le pochoir laisse passer");
    assert_eq!(full.get(150, 75).a(), 255, "et pas ailleurs");
}

#[test]
fn a_moved_stencil_masks_where_it_is_seen() {
    // Le calque-pochoir est décalé de 100 px vers la droite : sa forme (x < 100 dans son dessin)
    // est vue à droite de la feuille.
    let mut shape = Raster::new(200, 100);
    put(&mut shape, (0, 100), (0, 100), premultiply(0, 0, 0, 255));
    let mut stencil = Stencil::new(200, 100);
    stencil.add(&shape, false, Some(Affine::translate(vec2(-100.0, 0.0))));
    let mut r = Raster::new(200, 100);
    stroke(&mut r, StrokeMode::Paint([0, 0, 0]), false, Some(stencil), (10.0, 50.0), (190.0, 50.0));
    assert_eq!(r.get(50, 50).a(), 0);
    assert_eq!(r.get(150, 50).a(), 255);
}

#[test]
fn a_position_locked_layer_cannot_be_drawn_on() {
    let mut l = Layer::new("Décor", LayerKind::Bitmap);
    assert!(l.drawable());
    l.position_locked = true;
    assert!(!l.drawable());
    l.position_locked = false;
    l.locked = true;
    assert!(!l.drawable());
}

#[test]
fn header_rows_follow_collapsed_layers() {
    use crate::layerheads::{row_at, rows_height, rows_of, ROW_FULL, ROW_SMALL};
    let mut layers: Vec<Layer> = (0..3).map(|i| Layer::new(format!("C{i}"), LayerKind::Bitmap)).collect();
    layers[1].collapsed = true;
    let rows = rows_of(&layers, &groups::defaults());
    // Le calque le plus haut (le dernier) en premier.
    assert_eq!(rows.iter().map(|r| r.li).collect::<Vec<_>>(), [2, 1, 0]);
    assert_eq!(rows[1].top, ROW_FULL);
    assert_eq!(rows[1].h, ROW_SMALL);
    assert_eq!(rows[2].top, ROW_FULL + ROW_SMALL);
    assert_eq!(rows_height(&rows), ROW_FULL * 2.0 + ROW_SMALL);
    assert_eq!(row_at(&rows, ROW_FULL + 1.0).map(|r| r.li), Some(1));
    assert_eq!(row_at(&rows, rows_height(&rows) + 1.0), None);
}

#[test]
fn a_dragged_header_row_lands_between_rows() {
    use crate::layerheads::{cible_du_glisse, rows_of, Row, ROW_FULL};
    let mut layers: Vec<Layer> = (0..3).map(|i| Layer::new(format!("C{i}"), LayerKind::Bitmap)).collect();
    let rows = rows_of(&layers, &groups::defaults());
    // La pile (les noms, du bas vers le haut) une fois le calque `from` lâché à `y` ; rien : elle ne change pas.
    let lache = |layers: &[Layer], rows: &[Row], from: usize, y: f32| -> Option<Vec<String>> {
        let cible = cible_du_glisse(rows, layers, &[from], y, true)?;
        crate::folders::poser(layers, &[layers[from].id], cible.pose()).map(|pile| pile.iter().map(|l| l.name.clone()).collect())
    };
    assert_eq!(lache(&layers, &rows, 0, 1.0).unwrap(), ["C1", "C2", "C0"], "le plus bas lâché tout en haut devient le plus haut");
    assert_eq!(lache(&layers, &rows, 2, ROW_FULL * 3.0).unwrap(), ["C2", "C0", "C1"], "le plus haut lâché tout en bas devient le plus bas");
    assert_eq!(lache(&layers, &rows, 2, ROW_FULL * 0.9), None, "sur sa propre ligne : il ne bouge pas");
    assert_eq!(lache(&layers, &rows, 2, ROW_FULL * 1.2), None, "juste sous lui-même : il ne bouge pas");
    assert_eq!(lache(&layers, &rows, 0, ROW_FULL * 1.2).unwrap(), ["C1", "C0", "C2"], "entre le premier et le deuxième");

    // C1 dans un groupe caché de la timeline : seules C2 et C0 ont une ligne.
    let mut hidden = groups::defaults();
    hidden[4].shown = false;
    layers.push(Layer::new("C3", LayerKind::Bitmap));
    layers[1].group = 4;
    let rows = rows_of(&layers, &hidden);
    assert_eq!(rows.iter().map(|r| r.li).collect::<Vec<_>>(), [3, 2, 0]);
    // C0 lâché entre C3 et C2 : juste au-dessus de C2 (C1, sans ligne, ne bouge pas).
    assert_eq!(lache(&layers, &rows, 0, ROW_FULL * 1.2).unwrap(), ["C1", "C2", "C0", "C3"]);
    // C3 lâché tout en bas : sous C0.
    assert_eq!(lache(&layers, &rows, 3, ROW_FULL * 3.0).unwrap(), ["C3", "C0", "C1", "C2"]);
}

#[test]
fn hiding_a_group_hides_its_rows_only() {
    use crate::layerheads::rows_of;
    let mut layers: Vec<Layer> = (0..4).map(|i| Layer::new(format!("C{i}"), LayerKind::Bitmap)).collect();
    layers[0].group = 2;
    layers[2].group = 2;
    let mut g = groups::defaults();
    g[2].shown = false;
    let shown: Vec<usize> = rows_of(&layers, &g).iter().map(|r| r.li).collect();
    assert_eq!(shown, [3, 1]);
    assert!(layers.iter().all(|l| l.visible), "le filtre ne cache rien à l'image");
}

#[test]
fn groups_are_always_thirteen() {
    assert_eq!(groups::defaults().len(), groups::COUNT);
    let mut few = groups::defaults();
    few.truncate(3);
    few[1].name = "Persos".into();
    let back = groups::completed(few);
    assert_eq!(back.len(), groups::COUNT);
    assert_eq!(back[1].name, "Persos");
    assert_eq!(back[12], groups::defaults()[12]);
    let mut many = groups::defaults();
    many.extend(groups::defaults());
    assert_eq!(groups::completed(many).len(), groups::COUNT);
}

#[test]
fn header_settings_and_groups_are_saved() {
    let mut doc = Document::new(64, 64, 24);
    {
        let l = &mut doc.panels[0].layers[0];
        l.onion = true;
        l.keep_alpha = true;
        l.stencil = true;
        l.stencil_negative = true;
        l.position_locked = true;
        l.group = 7;
        l.collapsed = true;
    }
    doc.groups[7].name = "Personnages".into();
    doc.groups[7].color = [1, 2, 3];
    doc.groups[2].shown = false;
    let dir = std::env::temp_dir().join(format!("pelure-test-entetes-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("E.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    let l = &back.panels[0].layers[0];
    assert!(l.onion && l.keep_alpha && l.stencil && l.stencil_negative && l.position_locked && l.collapsed);
    assert_eq!(l.group, 7);
    assert_eq!(back.groups, doc.groups);
    assert!(text.contains(&format!("\"version\": {}", project::VERSION)), "format courant");

    // Un projet sans ces réglages les relit éteints, avec les groupes d'un projet neuf.
    let plain = Document::new(64, 64, 24);
    let dir = std::env::temp_dir().join(format!("pelure-test-entetes2-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("F.pelure");
    project::save(&plain, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    let back = project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert!(!text.contains("groupes") && !text.contains("pochoir"), "rien d'écrit quand tout est par défaut");
    let l = &back.panels[0].layers[0];
    assert!(!l.onion && !l.keep_alpha && !l.stencil && !l.position_locked && !l.collapsed);
    assert_eq!(l.group, 0);
    assert_eq!(back.groups, groups::defaults());
}
