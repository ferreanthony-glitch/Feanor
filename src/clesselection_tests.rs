//! L'outil Sélection sur plusieurs clés (Anthony, 2026-10-22 : « trois images clés sont choisies, pour
//! déplacer ou modifier les dessins de ces trois clés en même temps ; je n'ai pas d'outil pour ça ») :
//! quand deux clés ou plus sont choisies, le lasso, « tout sélectionner », la boîte (déplacer, échelle,
//! rotation, pivot) et les flèches prennent le dessin de chacune ; une seule étape d'annulation ; une
//! clé tenue sur plusieurs images est un seul dessin ; une seule clé, ou Échap, et l'on revient au
//! dessin courant seul.

use eframe::egui::{self, pos2, vec2, Color32, Pos2};

use crate::app::{App, Tool};
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::montagekeys::KeyRef;
use crate::multisel::{self, PartKind};
use crate::vector::{Affine, Element, VPoint};

fn stroke(points: &[(f32, f32)]) -> Element {
    Element::Stroke {
        points: points.iter().map(|&(x, y)| VPoint { x, y, r: 2.0 }).collect(),
        color: [20, 20, 20],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

/// Un projet de 400 × 300 avec un calque « A » (vectoriel ou bitmap) dont les dessins sont aux
/// images 0 à 4, 5 à 9 et 10 à 19 (trois clés). Chaque dessin vectoriel a deux traits décalés selon
/// sa clé ; chaque dessin bitmap, un rectangle rouge décalé de même.
struct Fixture {
    app: App,
    panel: u64,
    layer: u64,
    cels: Vec<CelId>,
}

fn fixture(name: &str, kind: LayerKind) -> Fixture {
    let dir = std::env::temp_dir().join(format!("shot-clesselection-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(200, 150, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(20);
    let mut layer = Layer::new("A", kind);
    let mut cels = Vec::new();
    for (k, (from, to)) in [(0usize, 5usize), (5, 10), (10, 20)].into_iter().enumerate() {
        let cel = doc.new_cel(kind);
        let dx = k as f32 * 6.0;
        let c = doc.cels.get_mut(&cel).unwrap();
        match kind {
            LayerKind::Vector => {
                c.vector.as_mut().unwrap().elements = vec![stroke(&[(20.0 + dx, 20.0), (80.0 + dx, 30.0)]), stroke(&[(30.0 + dx, 60.0), (90.0 + dx, 70.0)])];
                c.rerender();
            }
            LayerKind::Bitmap => {
                for y in 20..50 {
                    for x in (20 + k * 6)..(60 + k * 6) {
                        let i = c.raster.tile_index_of(x, y);
                        c.raster.tile_mut(i)[(y % 64) * 64 + x % 64] = Color32::from_rgb(200, 0, 0);
                    }
                }
            }
        }
        for f in from..to {
            layer.set(f, Some(cel));
        }
        cels.push(cel);
    }
    let id = layer.id;
    doc.panels[0].layers.push(layer);
    let panel = doc.panels[0].id;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app.tool = Tool::Select;
    Fixture { app, panel, layer: id, cels }
}

impl Fixture {
    fn key(&self, start: usize) -> KeyRef {
        KeyRef { panel: self.panel, layer: self.layer, start }
    }

    fn choose(&mut self, starts: &[usize]) {
        self.app.key_sel = starts.iter().map(|&s| self.key(s)).collect();
    }

    fn points(&self, k: usize) -> Vec<Pos2> {
        let d = self.app.doc.cels[&self.cels[k]].vector.as_ref().unwrap();
        d.elements
            .iter()
            .flat_map(|e| match e {
                Element::Stroke { points, .. } => points.iter().map(|p| pos2(p.x, p.y)).collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect()
    }

    fn lasso_all(&mut self) {
        self.app.multi_select_polygon(&[pos2(0.0, 0.0), pos2(300.0, 0.0), pos2(300.0, 200.0), pos2(0.0, 200.0)], false);
    }
}

fn near(a: Pos2, b: Pos2) -> bool {
    a.distance(b) < 0.05
}

#[test]
fn two_or_more_chosen_keys_switch_the_selection_to_every_chosen_drawing() {
    let mut f = fixture("mode", LayerKind::Vector);
    assert!(!f.app.keys_mode() && !f.app.multi_wanted(), "pas de clé choisie : le dessin courant seul");
    f.choose(&[0]);
    assert!(!f.app.keys_mode(), "une seule clé : comme avant");
    f.choose(&[0, 5, 10]);
    assert!(f.app.keys_mode() && f.app.multi_wanted());
    // Sans l'outil Sélection, rien ne change.
    f.app.tool = Tool::Brush;
    assert!(!f.app.multi_wanted());
}

#[test]
fn the_lasso_takes_the_strokes_of_every_chosen_key() {
    let mut f = fixture("lasso", LayerKind::Vector);
    f.choose(&[0, 5, 10]);
    f.lasso_all();
    let sel = f.app.multi.sel.as_ref().expect("une sélection");
    assert!(sel.by_keys);
    assert_eq!(sel.parts.len(), 3, "un morceau par dessin");
    assert_eq!(sel.summary(), "3 dessins, 6 éléments");
    let live: Vec<bool> = sel.parts.iter().map(|p| p.live).collect();
    assert_eq!(live, [true, false, false], "seul le dessin de l'image courante est vu");
    // Un lasso étroit ne prend que le trait du haut de chaque dessin.
    f.app.multi.sel = None;
    f.app.multi_select_polygon(&[pos2(0.0, 0.0), pos2(300.0, 0.0), pos2(300.0, 45.0), pos2(0.0, 45.0)], false);
    let sel = f.app.multi.sel.as_ref().unwrap();
    assert_eq!(sel.summary(), "3 dessins, 3 éléments");
}

#[test]
fn without_several_keys_only_the_current_drawing_is_taken() {
    let mut f = fixture("seul", LayerKind::Vector);
    f.choose(&[5]);
    f.app.tool = Tool::Select;
    // Avec une seule clé choisie (ou aucune), c'est la sélection d'un seul dessin : `multi_wanted` est faux, le geste
    // passe par `select_press` (vsel), qui ne connaît que la clé courante.
    assert!(!f.app.multi_wanted());
    let targets = f.app.multi_targets();
    assert_eq!(targets.list.len(), 1, "le calque courant seul");
    assert_eq!(targets.list[0].cel, f.cels[0], "le dessin de l'image courante, pas celui de la clé choisie");
}

#[test]
fn a_move_moves_every_chosen_drawing_by_the_same_amount_and_one_undo_restores_all() {
    let mut f = fixture("move", LayerKind::Vector);
    f.choose(&[0, 5, 10]);
    f.lasso_all();
    let before: Vec<Vec<Pos2>> = (0..3).map(|k| f.points(k)).collect();
    f.app.multi_nudge(vec2(10.0, 5.0));
    for k in 0..3 {
        let after = f.points(k);
        assert_eq!(after.len(), before[k].len());
        for (a, b) in after.iter().zip(&before[k]) {
            assert!(near(*a, *b + vec2(10.0, 5.0)), "dessin {k} : {a:?} au lieu de {:?}", *b + vec2(10.0, 5.0));
        }
    }
    // Une seule annulation les rend tous les trois.
    f.app.run_action("edition.annuler");
    for k in 0..3 {
        let back = f.points(k);
        assert!(back.iter().zip(&before[k]).all(|(a, b)| near(*a, *b)), "dessin {k} pas revenu");
    }
    // Et rétablir les remet.
    f.app.run_action("edition.retablir");
    assert!(near(f.points(2)[0], before[2][0] + vec2(10.0, 5.0)));
}

#[test]
fn a_box_gesture_rotates_and_scales_every_drawing_around_the_shared_pivot() {
    let mut f = fixture("pivot", LayerKind::Vector);
    f.choose(&[0, 5, 10]);
    f.lasso_all();
    let before: Vec<Vec<Pos2>> = (0..3).map(|k| f.points(k)).collect();
    let pivot = pos2(100.0, 50.0);
    // Une rotation de 30° puis une échelle de 1,5 autour du même point, d'un seul geste.
    let a = Affine::rotate_about(pivot, 30f32.to_radians()).then(&Affine::scale_about(pivot, 1.5, 1.5));
    let sel = f.app.multi.sel.as_mut().unwrap();
    let mut floats = Vec::new();
    let (edits, _) = multisel::run_gesture(&mut f.app.doc, sel, &mut floats, &a, false);
    assert_eq!(edits.len(), 3, "une étape par dessin, réunies en une seule par l'application");
    for k in 0..3 {
        for (got, was) in f.points(k).iter().zip(&before[k]) {
            assert!(near(*got, a.apply(*was)), "dessin {k} : {got:?} au lieu de {:?}", a.apply(*was));
        }
    }
}

#[test]
fn a_bitmap_layer_moves_the_pixels_of_every_chosen_key() {
    let mut f = fixture("bitmap", LayerKind::Bitmap);
    f.choose(&[0, 5, 10]);
    f.app.multi_select_all();
    let sel = f.app.multi.sel.as_ref().expect("une sélection");
    assert_eq!(sel.parts.len(), 3);
    assert!(sel.parts.iter().all(|p| matches!(p.kind, PartKind::Bitmap { .. })));
    let red = |app: &App, k: usize, x: usize, y: usize| app.doc.cels[&f_cel(app, k)].raster.get(x, y).r() > 150;
    fn f_cel(app: &App, k: usize) -> CelId {
        let l = &app.doc.panels[0].layers[0];
        l.cel_at([0, 5, 10][k]).unwrap()
    }
    for k in 0..3 {
        assert!(red(&f.app, k, 30 + k * 6, 30), "le rectangle de la clé {k} est là au départ");
    }
    f.app.multi_nudge(vec2(20.0, 10.0));
    for k in 0..3 {
        assert!(red(&f.app, k, 30 + k * 6 + 20, 30 + 10), "dessin {k} : les pixels ont bougé de (20, 10)");
        assert!(!red(&f.app, k, 30 + k * 6, 22), "dessin {k} : plus rien à l'ancienne place");
    }
    f.app.run_action("edition.annuler");
    for k in 0..3 {
        assert!(red(&f.app, k, 30 + k * 6, 30), "dessin {k} : l'annulation rend les pixels");
    }
}

#[test]
fn a_key_held_over_several_images_is_one_drawing_moved_once() {
    let mut f = fixture("tenue", LayerKind::Vector);
    // La clé 0 est aussi posée aux images 12 à 14 : le même dessin à deux endroits de la ligne du temps.
    let first = f.cels[0];
    for fr in 12..15 {
        f.app.doc.panels[0].layers[0].set(fr, Some(first));
    }
    // Trois « clés » (0, 12, 5) : deux d'entre elles montrent le même dessin.
    f.choose(&[0, 12, 5]);
    let t = f.app.multi_targets();
    assert_eq!(t.list.len(), 2, "le dessin tenu ne compte qu'une fois : {:?}", t.list.iter().map(|t| t.cel).collect::<Vec<_>>());
    f.lasso_all();
    assert_eq!(f.app.multi.sel.as_ref().unwrap().parts.len(), 2);
    let before = f.points(0);
    f.app.multi_nudge(vec2(7.0, 0.0));
    assert!(f.points(0).iter().zip(&before).all(|(a, b)| near(*a, *b + vec2(7.0, 0.0))), "déplacé une seule fois, pas deux");
}

#[test]
fn choosing_a_single_key_or_pressing_escape_goes_back_to_the_current_drawing() {
    let mut f = fixture("sortie", LayerKind::Vector);
    f.choose(&[0, 5, 10]);
    f.lasso_all();
    assert!(f.app.multi.sel.is_some());
    // Un clic sur une seule clé : le choix passe à une clé, la sélection sur plusieurs dessins s'en va.
    f.choose(&[5]);
    f.app.multi_sync();
    assert!(f.app.multi.sel.is_none(), "une seule clé : retour au dessin courant");
    assert!(!f.app.keys_mode());
    // Échap : les clés sont lâchées.
    f.choose(&[0, 5, 10]);
    f.lasso_all();
    assert!(f.app.multi.sel.is_some());
    assert!(f.app.keys_escape(egui::Key::Escape));
    assert!(f.app.key_sel.is_empty() && f.app.multi.sel.is_none());
    assert!(!f.app.multi_wanted());
    assert!(!f.app.keys_escape(egui::Key::Escape), "rien à faire la fois d'après");
}

#[test]
fn keys_on_several_layers_move_together_with_each_layers_own_transform() {
    let mut f = fixture("calques", LayerKind::Vector);
    // Un deuxième calque, décalé de 50 px sur la feuille : la boîte est sur la feuille, chaque dessin la suit là où on le voit.
    let cel = f.app.doc.new_cel(LayerKind::Vector);
    f.app.doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements = vec![stroke(&[(10.0, 10.0), (50.0, 20.0)])];
    f.app.doc.cels.get_mut(&cel).unwrap().rerender();
    let mut b = Layer::new("B", LayerKind::Vector);
    for fr in 0..20 {
        b.set(fr, Some(cel));
    }
    let b_id = b.id;
    f.app.doc.panels[0].layers.push(b);
    f.app.key_sel = vec![f.key(0), f.key(5), KeyRef { panel: f.panel, layer: b_id, start: 0 }];
    f.lasso_all();
    let sel = f.app.multi.sel.as_ref().unwrap();
    assert_eq!(sel.parts.len(), 3, "trois dessins sur deux calques");
    let (a0, b0) = (f.points(0), {
        let d = f.app.doc.cels[&cel].vector.as_ref().unwrap();
        d.elements.iter().flat_map(|e| e.outline().into_iter().flatten()).collect::<Vec<_>>()
    });
    f.app.multi_nudge(vec2(4.0, 3.0));
    assert!(f.points(0).iter().zip(&a0).all(|(a, b)| near(*a, *b + vec2(4.0, 3.0))));
    let b1: Vec<Pos2> = f.app.doc.cels[&cel].vector.as_ref().unwrap().elements.iter().flat_map(|e| e.outline().into_iter().flatten()).collect();
    assert!(b1.iter().zip(&b0).all(|(a, b)| near(*a, *b + vec2(4.0, 3.0))), "le calque B suit aussi");
}
