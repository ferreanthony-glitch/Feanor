//! Revue de l'étape 5 du rendu des brosses (branche `vecto`, 2026-10-18) : chaque constat confirmé
//! a ici son essai, qui échouait avant la correction et passe après (logique, pas visuel).
//! - gomme : les traits non touchés ne bougent pas, les morceaux gardent leurs appuis ;
//! - cache : oublié par toute transformation hors de l'aperçu d'un glissé ;
//! - glissé : translation arrondie au pixel, échelle et rotation posées depuis le cache ;
//! - boîte : un trait rejoué ne déborde pas de `bounds() + 1,5` ;
//! - cache en tuiles creuses : un trait fin en diagonale de la feuille est gardé ;
//! - budget global des caches, partagé par les copies de l'historique ;
//! - fichier : appuis et instants quantifiés ;
//! - papier des crayons à part, « Papier à dessin » inchangé.

use std::time::{Duration, Instant};

use eframe::egui::{pos2, vec2, Pos2, Rect};

use crate::brush::{BrushSettings, Connection, Curve, Source, StrokeMode, StrokeState, Toucher};
use crate::brushtex::{self, BrushLibrary};
use crate::document::{Cel, Document, LayerKind};
use crate::envelope::{self, Envelope};
use crate::project;
use crate::raster::Raster;
use crate::rejeu::Rejeu;
use crate::vector::{self, Affine, Element, Trace, VPoint, VectorDrawing};

/// Un geste en S avec l'appui en cloche, `n` échantillons, décalé de `dx`, `dy`.
fn geste(n: usize, dx: f32, dy: f32) -> Vec<(Pos2, f32, f64)> {
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            let p = pos2(30.0 + dx + t * 300.0, 90.0 + dy + (t * std::f32::consts::TAU).sin() * 40.0);
            (p, 0.1 + 0.85 * (t * std::f32::consts::PI).sin(), k as f64 / 200.0)
        })
        .collect()
}

/// Un geste quelconque à appui constant.
fn geste_plat(pts: &[(f32, f32)], pressure: f32) -> Vec<(Pos2, f32, f64)> {
    let mut out = Vec::new();
    for w in pts.windows(2) {
        let (a, b) = (pos2(w[0].0, w[0].1), pos2(w[1].0, w[1].1));
        let n = (a.distance(b) / 2.0).ceil().max(1.0) as usize;
        for k in 0..n {
            out.push((a + (b - a) * (k as f32 / n as f32), pressure, out.len() as f64 / 200.0));
        }
    }
    if let Some(&(x, y)) = pts.last() {
        out.push((pos2(x, y), pressure, out.len() as f64 / 200.0));
    }
    out
}

/// Le trait tracé par le moteur bitmap, et l'élément vectoriel qu'un lâcher garderait.
fn bitmap_and_element(s: &BrushSettings, lib: &BrushLibrary, w: usize, h: usize, pts: &[(Pos2, f32, f64)]) -> (Raster, Element) {
    let mut raster = Raster::new(w, h);
    let mut st = StrokeState::new(0, StrokeMode::Paint([20, 20, 20]), *s, None, true).with_textures(lib.textures(s)).with_feel(Toucher::default());
    let mut dirty = Vec::new();
    for &(p, pr, t) in pts {
        st.add_point_at(&mut raster, p, pr, t, &mut dirty);
    }
    st.finish(&mut raster, &mut dirty);
    let trace = Trace { pressure: st.path_pressure.clone(), time: st.path_time.clone(), feel: Some(Toucher::default()), secondary: None, cache: Default::default() };
    let e = Element::Stroke { points: st.path.clone(), color: [20, 20, 20], opacity: s.opacity, hardness: s.hardness, brush: lib.for_vector(s).map(Box::new), trace };
    (raster, e)
}

fn element(s: &BrushSettings, lib: &BrushLibrary, w: usize, h: usize, pts: &[(Pos2, f32, f64)]) -> Element {
    bitmap_and_element(s, lib, w, h, pts).1
}

fn near_pixels(a: &Raster, b: &Raster, tol: i32) -> Result<(), String> {
    let (mut n, mut worst) = (0usize, 0i32);
    for y in 0..a.height {
        for x in 0..a.width {
            let (p, q) = (a.get(x, y).to_array(), b.get(x, y).to_array());
            let d = (0..4).map(|k| (p[k] as i32 - q[k] as i32).abs()).max().unwrap_or(0);
            if d > tol {
                n += 1;
                worst = worst.max(d);
            }
        }
    }
    if n == 0 { Ok(()) } else { Err(format!("{n} pixels différents, écart jusqu'à {worst}")) }
}

fn render(w: usize, h: usize, e: &Element) -> Raster {
    let mut r = Raster::new(w, h);
    vector::render_element(&mut r, e);
    r
}

fn ink(r: &Raster) -> f64 {
    (0..r.height).flat_map(|y| (0..r.width).map(move |x| (x, y))).map(|(x, y)| r.get(x, y).a() as f64).sum()
}

fn max_alpha(r: &Raster, x0: usize, x1: usize) -> u8 {
    (x0..x1).flat_map(|x| (0..r.height).map(move |y| (x, y))).map(|(x, y)| r.get(x, y).a()).max().unwrap_or(0)
}

fn stroke_parts(e: &Element) -> (&Vec<VPoint>, &Trace) {
    let Element::Stroke { points, trace, .. } = e else { panic!("un trait") };
    (points, trace)
}

// ------------------------------------------------------------------ gomme (constats 2 et 3)

#[test]
fn the_eraser_leaves_untouched_strokes_alone_and_keeps_pressure_on_the_pieces() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let s = BrushSettings::graphite();
    // A en S, B qui le croise : les deux boîtes se recouvrent.
    let a = element(&s, &lib, 400, 200, &geste(120, 0.0, 0.0));
    let b = element(&s, &lib, 400, 200, &geste_plat(&[(200.0, 20.0), (210.0, 180.0)], 0.6));
    let mut d = VectorDrawing { elements: vec![a.clone(), b.clone()] };
    let (pa, ta) = stroke_parts(&a);
    // La gomme passe sur A loin de B.
    assert!(vector::erase_circle(&mut d, pa[30].pos(), 8.0, false));
    let kept_b = d.elements.last().unwrap();
    let (pb, tb) = stroke_parts(kept_b);
    let (pb0, tb0) = stroke_parts(&b);
    assert_eq!(pb, pb0, "B n'a pas bougé d'un point");
    assert_eq!((&tb.pressure, &tb.time), (&tb0.pressure, &tb0.time), "ni d'un appui");
    // Les morceaux de A gardent un appui et un instant par point, dans la plage du trait, et qui
    // suivent le trait : le morceau qui commence au point k a l'appui du point k.
    let pieces: Vec<&Element> = d.elements.iter().take(d.elements.len() - 1).collect();
    assert!(pieces.len() >= 2, "{} morceaux", pieces.len());
    let (lo, hi) = ta.pressure.iter().fold((1.0f32, 0.0f32), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    for piece in &pieces {
        let (pp, tp) = stroke_parts(piece);
        assert_eq!(tp.pressure.len(), pp.len());
        assert_eq!(tp.time.len(), pp.len());
        assert!(tp.pressure.iter().all(|&p| p >= lo - 1e-4 && p <= hi + 1e-4));
        assert!(tp.time.windows(2).all(|w| w[0] <= w[1]), "les instants avancent");
        // Un morceau qui commence sur un point du trait (le premier) a l'appui de ce point.
        if let Some(k) = pa.iter().position(|q| q.pos().distance(pp[0].pos()) < 1e-3) {
            assert!((tp.pressure[0] - ta.pressure[k]).abs() < 1e-4);
        }
    }
    let dense_ok = pieces.iter().all(|piece| {
        let (pp, tp) = stroke_parts(piece);
        pp.iter().zip(&tp.pressure).all(|(q, &p)| {
            // Chaque point du morceau est sur un segment de A : l'appui est entre ceux des bouts.
            pa.windows(2).enumerate().any(|(i, w)| {
                let (x, y) = (w[0].pos(), w[1].pos());
                let d = x.distance(y).max(1e-6);
                let t = ((q.pos() - x).dot(y - x) / (d * d)).clamp(0.0, 1.0);
                (x + (y - x) * t).distance(q.pos()) < 1e-2 && {
                    let (u, v) = (ta.pressure[i], ta.pressure[i + 1]);
                    p >= u.min(v) - 1e-3 && p <= u.max(v) + 1e-3
                }
            })
        })
    });
    assert!(dense_ok, "les appuis des points ajoutés sont interpolés");
}

#[test]
fn pieces_of_an_opacity_only_brush_keep_their_light_touch() {
    let lib = BrushLibrary::builtin();
    lib.share();
    // Une brosse dont l'opacité suit l'appui, pas la taille : sans appuis, les morceaux
    // prenaient l'appui 1 (d'après le rayon) et devenaient pleins.
    let mut s = BrushSettings { size: 8.0, pressure_size: false, smoothing: 0.0, ..BrushSettings::brush() };
    s.dynamics.opacity = Connection { source: Source::Pressure, curve: Curve::linear(), ..Default::default() };
    let e = element(&s, &lib, 400, 100, &geste_plat(&[(20.0, 50.0), (380.0, 50.0)], 0.2));
    let whole = render(400, 100, &e);
    let mut d = VectorDrawing { elements: vec![e] };
    assert!(vector::erase_circle(&mut d, pos2(200.0, 50.0), 10.0, false));
    assert!(d.elements.len() >= 2);
    let mut pieces = Raster::new(400, 100);
    for p in &d.elements {
        vector::render_element(&mut pieces, p);
    }
    let (before, after) = (max_alpha(&whole, 40, 160), max_alpha(&pieces, 40, 160));
    assert!(after <= before + 2, "le morceau est devenu plus opaque : {after} contre {before}");
    assert!(before < 160, "un trait léger au départ ({before})");
}

// ------------------------------------------------------------------ cache (constats 1, 6, 7, 8)

#[test]
fn transforms_outside_the_drag_preview_forget_the_cache() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let s = BrushSettings::graphite();
    let e = element(&s, &lib, 400, 200, &geste(120, 0.0, 0.0));
    let _ = render(400, 200, &e);
    assert!(e.cache_bytes() > 0);
    // Décalé pour de bon (pousser au clavier, agrandir la feuille, retourner le dessin…) : le
    // cache est oublié et le trait est rejoué exactement à sa place.
    let moved = e.transformed(&Affine::translate(vec2(7.0, -3.0)));
    assert_eq!(moved.cache_bytes(), 0, "le cache n'est pas repris");
    let exact = render(400, 200, &moved);
    let (bitmap, _) = bitmap_and_element(&s, &lib, 400, 200, &geste(120, 7.0, -3.0));
    near_pixels(&bitmap, &exact, 2).unwrap();
    assert_eq!(e.transformed(&Affine::scale_about(pos2(0.0, 0.0), 2.0, 2.0)).cache_bytes(), 0);
    // Au niveau du document : agrandir la feuille et retourner un dessin donnent l'image d'un
    // rendu neuf, pas des pixels copiés.
    let mut doc = Document::new(400, 200, 24);
    let id = doc.new_cel(LayerKind::Vector);
    let c = doc.cels.get_mut(&id).unwrap();
    c.vector.as_mut().unwrap().elements.push(e.clone());
    c.rerender();
    doc.enlarge_stage(600, 300);
    let fresh = |c: &Cel| {
        let mut again = Cel::new(c.raster.width, c.raster.height, LayerKind::Vector);
        again.vector = c.vector.clone();
        for el in &mut again.vector.as_mut().unwrap().elements {
            el.invalidate_cache();
        }
        again.rerender();
        again.raster
    };
    let c = &doc.cels[&id];
    assert_eq!(c.raster.to_rgba(), fresh(c).to_rgba(), "feuille agrandie : rendu exact");
    let flipped = doc.flipped_cel(id, true, false);
    let c = &doc.cels[&flipped];
    assert_eq!(c.raster.to_rgba(), fresh(c).to_rgba(), "dessin retourné : rendu exact");
}

#[test]
fn the_drag_preview_uses_the_cache_for_fractional_moves_scales_and_rotations() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let e = element(&BrushSettings::graphite(), &lib, 400, 200, &geste(120, 0.0, 0.0));
    let first = render(400, 200, &e);
    let n0 = vector::replay_count();
    // Une translation quelconque (le cas courant du glissé) : arrondie au pixel entier, les pixels
    // sont copiés nets, sans rejeu ni flou.
    let moved = e.transformed_preview(&Affine::translate(vec2(3.4, 2.6)));
    let shifted = render(400, 200, &moved);
    assert_eq!(vector::replay_count(), n0, "pas de rejeu");
    for y in 10..190 {
        for x in 10..390 {
            assert_eq!(shifted.get(x, y), first.get(x - 3, y - 3), "({x}, {y})");
        }
    }
    // Les points, eux, ont été décalés d'un nombre entier aussi (le lâcher rejouera là).
    let (p0, _) = stroke_parts(&e);
    let (p1, _) = stroke_parts(&moved);
    assert!((p1[0].x - p0[0].x - 3.0).abs() < 1e-4 && (p1[0].y - p0[0].y - 3.0).abs() < 1e-4);
    // Échelle et rotation : posées depuis le cache (rééchantillonnées), sans rejeu.
    let scaled = e.transformed_preview(&Affine::scale_about(pos2(180.0, 90.0), 1.3, 1.3));
    let big = render(400, 200, &scaled);
    assert_eq!(vector::replay_count(), n0, "échelle : pas de rejeu");
    assert!((ink(&big) / ink(&first) - 1.3 * 1.3).abs() < 0.35, "{} contre {}", ink(&big), ink(&first));
    let turned = e.transformed_preview(&Affine::rotate_about(pos2(180.0, 90.0), 0.3));
    let rot = render(400, 200, &turned);
    assert_eq!(vector::replay_count(), n0, "rotation : pas de rejeu");
    assert!((ink(&rot) / ink(&first) - 1.0).abs() < 0.15);
    // Le lâcher : un rejeu.
    let released = e.transformed(&Affine::translate(vec2(3.0, 3.0)));
    let _ = render(400, 200, &released);
    assert_eq!(vector::replay_count(), n0 + 1);
}

#[test]
fn a_replayed_stroke_stays_inside_its_bounds() {
    let lib = BrushLibrary::builtin();
    lib.share();
    // Une grosse pointe en image tournée au hasard : ses coins dépassent du cercle.
    let s = BrushSettings { size: 40.0, pressure_size: false, ..BrushSettings::graphite() };
    let e = element(&s, &lib, 400, 200, &geste_plat(&[(60.0, 100.0), (340.0, 100.0)], 1.0));
    let r = render(400, 200, &e);
    let allowed = e.bounds().expand(1.5);
    let outside = (0..200).flat_map(|y| (0..400).map(move |x| (x, y))).filter(|&(x, y)| r.get(x, y).a() > 0 && !allowed.contains(pos2(x as f32 + 0.5, y as f32 + 0.5))).count();
    assert_eq!(outside, 0, "{outside} pixels hors de la boîte {allowed:?}");
    // Et refaire une zone donne la même image qu'un rendu complet, avec des traits texturés.
    let mut cel = Cel::new(400, 200, LayerKind::Vector);
    let mut d = cel.vector.take().unwrap();
    for k in 0..6 {
        d.elements.push(element(&BrushSettings::graphite(), &lib, 400, 200, &geste(80, k as f32 * 9.0, k as f32 * 12.0 - 30.0)));
    }
    cel.vector = Some(d);
    cel.rerender();
    let before = cel.vector.as_ref().unwrap().elements[2].bounds();
    let moved = cel.vector.as_ref().unwrap().elements[2].transformed(&Affine::translate(vec2(25.0, -8.0)));
    let after = moved.bounds();
    cel.vector.as_mut().unwrap().elements[2] = moved;
    cel.rerender_region(before.union(after).expand(3.0));
    let mut full = cel.clone();
    full.rerender();
    assert_eq!(cel.raster.to_rgba(), full.raster.to_rgba(), "zone refaite = rendu complet");
}

#[test]
fn thin_strokes_across_the_whole_sheet_are_cached_in_sparse_tiles() {
    let lib = BrushLibrary::builtin();
    lib.share();
    // Une diagonale de la feuille 1920×1080 : une boîte de 2 millions de pixels (8 Mo pleins),
    // mais une centaine de tuiles touchées.
    let s = BrushSettings { size: 4.0, ..BrushSettings::brush() };
    let e = element(&s, &lib, 1920, 1080, &geste_plat(&[(10.0, 10.0), (1910.0, 1070.0)], 0.8));
    let _ = render(1920, 1080, &e);
    let bytes = e.cache_bytes();
    assert!(bytes > 0, "gardé en cache");
    assert!(bytes < 1_500_000, "{bytes} octets : des tuiles creuses, pas la boîte");
    // Les pixels du cache sont ceux du rendu, et une zone refaite les retrouve.
    let n0 = vector::replay_count();
    let mut again = Raster::new(1920, 1080);
    vector::render_element_clipped(&mut again, &e, Rect::from_min_max(pos2(900.0, 500.0), pos2(1000.0, 600.0)));
    assert_eq!(vector::replay_count(), n0);
    let full = render(1920, 1080, &e);
    for y in 500..600 {
        for x in 900..1000 {
            assert_eq!(again.get(x, y), full.get(x, y));
        }
    }
}

// ------------------------------------------------------------------ mémoire (constat 4)

#[test]
fn caches_are_shared_by_history_copies_and_stay_under_the_budget() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let mut d = VectorDrawing { elements: (0..4).map(|k| element(&BrushSettings::graphite(), &lib, 400, 200, &geste(100, 0.0, k as f32 * 10.0))).collect() };
    let mut r = Raster::new(400, 200);
    for e in &d.elements {
        vector::render_element(&mut r, e);
    }
    let total = vector::cache_total_bytes();
    let mine: usize = d.elements.iter().map(Element::cache_bytes).sum();
    assert!(mine > 0 && total >= mine);
    // La copie que garde l'historique partage les pixels : rien de plus dans le budget.
    let snapshot = d.clone();
    assert_eq!(vector::cache_total_bytes(), total);
    assert_eq!(snapshot.elements.iter().map(Element::cache_bytes).sum::<usize>(), mine);
    // Un trait retouché oublie ses pixels, la copie de l'historique garde les siens.
    d.elements[0].set_color([200, 0, 0]);
    assert_eq!(d.elements[0].cache_bytes(), 0);
    assert!(snapshot.elements[0].cache_bytes() > 0);
    assert!(vector::cache_total_bytes() <= vector::CACHE_BUDGET_BYTES);
}

// ------------------------------------------------------------------ fichier (constat 9)

/// La trace telle qu'elle s'enregistrait avant : des flottants en clair.
#[derive(serde::Serialize)]
struct LegacyTrace<'a> {
    appui: &'a [f32],
    instants: &'a [f64],
}

#[test]
fn saved_traces_are_quantized_and_much_smaller() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let d = VectorDrawing { elements: (0..50).map(|k| element(&BrushSettings::graphite(), &lib, 1920, 1080, &geste(200, (k % 5) as f32 * 300.0, (k / 5) as f32 * 95.0))).collect() };
    let json = serde_json::to_vec(&d).unwrap();
    let (mut legacy, mut now, mut brush_bytes, mut points_bytes) = (0usize, 0usize, 0usize, 0usize);
    for e in &d.elements {
        let Element::Stroke { points, brush, trace, .. } = e else { panic!() };
        legacy += serde_json::to_vec(&LegacyTrace { appui: &trace.pressure, instants: &trace.time }).unwrap().len();
        now += serde_json::to_vec(trace).unwrap().len();
        brush_bytes += serde_json::to_vec(brush).unwrap().len();
        points_bytes += serde_json::to_vec(points).unwrap().len();
    }
    println!(
        "50 traits de 200 points : fichier {} Ko ; par trait : points {} o, trace {} o (avant : {} o), réglage {} o",
        json.len() / 1024,
        points_bytes / 50,
        now / 50,
        legacy / 50,
        brush_bytes / 50
    );
    // (Dans ce geste de synthèse les instants sont courts, k / 200 : l'écart réel, avec des instants
    // de session à dix chiffres, est plus grand.)
    assert!(now * 100 / legacy < 80, "trace : {now} contre {legacy} octets avant");
    // Relu : les appuis au 1/65535, les instants au dixième de milliseconde depuis le premier.
    let back: VectorDrawing = serde_json::from_slice(&json).unwrap();
    for (a, b) in d.elements.iter().zip(&back.elements) {
        let ((pa, ta), (pb, tb)) = (stroke_parts(a), stroke_parts(b));
        assert_eq!(pa, pb);
        assert!(ta.pressure.iter().zip(&tb.pressure).all(|(x, y)| (x - y).abs() <= 1.0 / 65535.0));
        assert!(ta.time.iter().zip(&tb.time).all(|(x, y)| ((x - ta.time[0]) - y).abs() <= 1e-4));
    }
    // Un trait sans trace (forme, vieux trait) n'écrit pas de champ « trace ».
    let plain = Element::Stroke { points: vec![VPoint { x: 1.0, y: 2.0, r: 3.0 }], color: [0, 0, 0], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() };
    assert!(!serde_json::to_string(&plain).unwrap().contains("trace"));
}

// ------------------------------------------------------------------ papier (constat 10)

#[test]
fn the_pencils_have_their_own_paper_and_the_drawing_paper_is_unchanged() {
    let lib = BrushLibrary::builtin();
    let paper = |id| lib.textures(&BrushSettings { paper: Some(id), ..BrushSettings::brush() }).paper.unwrap();
    // Le « Papier à dessin » des étapes 1 à 3, à l'octave près.
    let expected = brushtex::multiscale_paper(256, 0x0A11, &[(0, 0.25), (1, 0.45), (2, 0.55), (4, 0.35)]);
    assert_eq!(paper(brushtex::PAPER_DRAWING).as_ref(), &expected);
    assert_ne!(paper(brushtex::PAPER_GRAPHITE).as_ref(), &expected);
    assert_eq!(BrushSettings::graphite().paper, Some(brushtex::PAPER_GRAPHITE));
    assert_eq!(BrushSettings::colour_pencil().paper, Some(brushtex::PAPER_GRAPHITE));
    assert_eq!(BrushSettings::charcoal().paper, Some(brushtex::PAPER_DRAWING));
    let grain_eraser = lib.presets.iter().find(|p| p.name == "Gomme à grain").unwrap();
    assert_eq!(grain_eraser.settings.paper, Some(brushtex::PAPER_DRAWING));
    assert!(lib.papers().iter().any(|(id, n)| *id == brushtex::PAPER_GRAPHITE && n == "Papier à dessin fin"));
}

// ------------------------------------------------------------------ rejeu en arrière-plan

#[test]
fn the_background_replay_keeps_only_the_latest_request_and_gives_exact_pixels() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let mut d = VectorDrawing { elements: (0..3).map(|k| element(&BrushSettings::graphite(), &lib, 400, 200, &geste(100, 0.0, k as f32 * 20.0))).collect() };
    let mut rejeu = Rejeu::new();
    let cel = 7;
    // Première demande, puis le dessin bouge (aperçu) et une seconde demande la remplace.
    let g1 = rejeu.submit(cel, &d, 400, 200);
    for e in &mut d.elements {
        *e = e.transformed_preview(&Affine::translate(vec2(10.0, 4.0)));
    }
    let g2 = rejeu.submit(cel, &d, 400, 200);
    assert!(rejeu.is_pending(cel));
    let done = rejeu.wait(Duration::from_secs(20));
    assert_eq!(done.len(), 1, "une seule réponse valable");
    let done = done.into_iter().next().unwrap();
    assert_eq!(done.gen, g2);
    assert_ne!(done.gen, g1);
    assert!(!rejeu.is_pending(cel));
    // Le résultat est bien le dessin du moment, rendu exactement : les mêmes pixels qu'un rendu
    // direct, et chaque trait a ses pixels exacts en cache (identité, plus un aperçu).
    assert!(d.elements.iter().zip(&done.drawing.elements).all(|(a, b)| a.same_as(b)));
    let mut exact = Raster::new(400, 200);
    for e in &d.elements {
        let mut fresh = e.clone();
        fresh.invalidate_cache();
        vector::render_element(&mut exact, &fresh);
    }
    assert_eq!(done.raster.to_rgba(), exact.to_rgba());
    for (e, x) in d.elements.iter_mut().zip(&done.drawing.elements) {
        assert!(x.cache_bytes() > 0);
        e.adopt_cache(x);
    }
    let n0 = vector::replay_count();
    let mut again = Raster::new(400, 200);
    for e in &d.elements {
        vector::render_element(&mut again, e);
    }
    assert_eq!(vector::replay_count(), n0, "les caches adoptés servent, sans rejeu");
    assert_eq!(again.to_rgba(), exact.to_rgba());
    // Une demande annulée (rendu direct, annulation) ne revient jamais.
    rejeu.submit(cel, &d, 400, 200);
    rejeu.cancel(cel);
    assert!(!rejeu.has_pending());
    assert!(rejeu.wait(Duration::from_millis(300)).is_empty());
    // Après une annulation (Ctrl+Z) : un résultat pour un dessin qui n'est plus celui-là ne serait
    // pas adopté (`App::poll_rejeu` compare le dessin), et `cancel_all` l'écarte d'avance.
    rejeu.submit(cel, &d, 400, 200);
    rejeu.submit(9, &d, 400, 200);
    rejeu.cancel_all();
    assert!(!rejeu.has_pending());
    assert!(rejeu.poll().is_empty());
}

// ------------------------------------------------------------------ durées (scénario d'Anthony)

/// 50 traits de graphite (un dessin 1920×1080) et un grand trait en travers de la feuille.
fn scene(lib: &BrushLibrary) -> Cel {
    let s = BrushSettings::graphite();
    let mut cel = Cel::new(1920, 1080, LayerKind::Vector);
    let d = cel.vector.as_mut().unwrap();
    for k in 0..50 {
        d.elements.push(element(&s, lib, 1920, 1080, &geste(200, 100.0 + (k % 5) as f32 * 300.0, 100.0 + (k / 5) as f32 * 80.0)));
    }
    d.elements.push(element(&s, lib, 1920, 1080, &geste_plat(&[(150.0, 120.0), (1700.0, 980.0)], 0.7)));
    cel
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Le scénario d'Anthony (2026-10-18 : « un vrai souci de ralentissement avec les brosses vecto
/// quand elles sont enveloppées et qu'on veut les déplacer ») : enveloppe sur la sélection, lâcher,
/// puis déplacement de la sélection enveloppée, lâcher, une flèche. Chaque étape : ms sur le fil de
/// l'interface, et la durée du rejeu exact en arrière-plan.
/// `cargo test --release --offline --target-dir target\audit envelope_then_drag -- --ignored --nocapture`
#[test]
#[ignore]
fn envelope_then_drag_is_fast_on_the_ui_thread() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let mut cel = scene(&lib);
    let n = cel.vector.as_ref().unwrap().elements.len();
    let items: Vec<usize> = (0..n).collect();
    let mut rejeu = Rejeu::new();
    let t = Instant::now();
    cel.rerender();
    println!("premier rendu ({n} traits) : {:.1} ms", ms(t));
    let src = cel.vector.as_ref().unwrap().elements.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
    // L'enveloppe : à l'ouverture, les deux feuilles (reste / choisis) depuis les caches.
    let t = Instant::now();
    let (base, sheet) = cel.vector.as_ref().unwrap().render_split(Some(&items), 1920, 1080);
    println!("ouverture de l'enveloppe (deux feuilles depuis les caches) : {:.1} ms", ms(t));
    // Pendant le geste : la feuille des choisis déformée par le maillage, posée sur le reste.
    let mut env = Envelope::identity(src);
    let frames = 20;
    let mut total = 0.0;
    for f in 1..=frames {
        env.move_corner(2, vec2(-6.0 * f as f32, 3.0 * f as f32));
        env.handles[1] = Envelope::identity(src).handles[1] + vec2(0.0, 4.0 * f as f32);
        let t = Instant::now();
        let mut img = base.clone();
        img.composite_over(&envelope::warp_raster_fast(&sheet, src, |p| env.map(src, p)));
        cel.raster = img;
        total += ms(t);
    }
    let env_frame = total / frames as f64;
    println!("enveloppe en cours (maillage sur {:.0}×{:.0} px) : {env_frame:.1} ms par image", src.width(), src.height());
    let t = Instant::now();
    let exact = envelope::warp_raster(&sheet, src, &env);
    println!("  (la déformation exacte, bilinéaire, aurait pris {:.1} ms)", ms(t));
    let fast = envelope::warp_raster_fast(&sheet, src, |p| env.map(src, p));
    let (ie, iff) = (ink(&exact), ink(&fast));
    println!("  encre de l'aperçu rapide : {:.0} % de l'exacte", iff / ie * 100.0);
    assert!((iff / ie - 1.0).abs() < 0.3, "l'aperçu couvre à peu près la même chose");
    // Le lâcher : la géométrie, et la demande du rejeu exact ; l'aperçu reste.
    let original = cel.vector.clone().unwrap();
    let t = Instant::now();
    let warped = envelope::warp_drawing(&original, Some(&items), src, &env);
    cel.vector = Some(warped);
    rejeu.submit(1, cel.vector.as_ref().unwrap(), 1920, 1080);
    println!("lâcher de l'enveloppe : {:.1} ms (fil de l'interface)", ms(t));
    let done = rejeu.wait(Duration::from_secs(30)).into_iter().next().expect("le rejeu");
    println!("  rejeu exact en arrière-plan : {:.1} ms", done.took.as_secs_f64() * 1e3);
    for (e, x) in cel.vector.as_mut().unwrap().elements.iter_mut().zip(&done.drawing.elements) {
        e.adopt_cache(x);
    }
    cel.raster = done.raster;
    // Le déplacement de la sélection enveloppée : 30 images, pas de 3,4 × 2,6 px (non entier).
    let originals = cel.vector.as_ref().unwrap().elements.clone();
    let frames = 30;
    let mut total = 0.0;
    let n0 = vector::replay_count();
    for f in 1..=frames {
        let aff = Affine::translate(vec2(3.4 * f as f32, 2.6 * f as f32));
        let t = Instant::now();
        let d = cel.vector.as_mut().unwrap();
        let mut region = Rect::NOTHING;
        for (i, src) in originals.iter().enumerate() {
            region = region.union(d.elements[i].bounds().expand(1.5));
            d.elements[i] = src.transformed_preview(&aff);
            region = region.union(d.elements[i].bounds().expand(1.5));
        }
        cel.rerender_region(region.expand(2.0));
        total += ms(t);
    }
    assert_eq!(vector::replay_count(), n0, "aucun rejeu pendant le glissé");
    println!("glissé après l'enveloppe : {:.1} ms par image", total / frames as f64);
    // Le lâcher du glissé : la demande seule ; puis le rejeu arrive.
    let t = Instant::now();
    rejeu.submit(1, cel.vector.as_ref().unwrap(), 1920, 1080);
    println!("lâcher du glissé : {:.1} ms (fil de l'interface)", ms(t));
    let done = rejeu.wait(Duration::from_secs(30)).into_iter().next().expect("le rejeu");
    println!("  rejeu exact en arrière-plan : {:.1} ms", done.took.as_secs_f64() * 1e3);
    for (e, x) in cel.vector.as_mut().unwrap().elements.iter_mut().zip(&done.drawing.elements) {
        e.adopt_cache(x);
    }
    cel.raster = done.raster;
    // Une flèche : copie des caches dans la zone, demande du rejeu.
    let t = Instant::now();
    let d = cel.vector.as_mut().unwrap();
    let mut region = Rect::NOTHING;
    for e in &mut d.elements {
        region = region.union(e.bounds().expand(1.5));
        *e = e.transformed_preview(&Affine::translate(vec2(1.0, 0.0)));
        region = region.union(e.bounds().expand(1.5));
    }
    cel.rerender_region(region.expand(2.0));
    rejeu.submit(1, cel.vector.as_ref().unwrap(), 1920, 1080);
    println!("une flèche (1 px) : {:.1} ms (fil de l'interface)", ms(t));
    let per_frame = total / frames as f64;
    assert!(per_frame < 16.0, "{per_frame:.1} ms par image");
    assert!(env_frame < 16.0, "enveloppe : {env_frame:.1} ms par image");
}

// ------------------------------------------------------------------ chargement (constat 5)

/// Le temps d'ouverture d'un gros projet : 24 dessins vectoriels de 30 traits de graphite en
/// 1920×1080, enregistrés puis relus (les traits sont rejoués par le moteur à la lecture).
/// `cargo test --release --offline --target-dir target\audit loading_a_big_project -- --ignored --nocapture`
#[test]
#[ignore]
fn loading_a_big_project_is_measured() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let mut doc = Document::new(1920, 1080, 24);
    let s = BrushSettings::graphite();
    for i in 0..24 {
        let id = doc.new_cel(LayerKind::Vector);
        let c = doc.cels.get_mut(&id).unwrap();
        for k in 0..30 {
            c.vector.as_mut().unwrap().elements.push(element(&s, &lib, 1920, 1080, &geste(200, (k % 5) as f32 * 300.0, (k / 5) as f32 * 150.0 + i as f32)));
        }
        c.rerender();
        doc.panels[0].layers[0].set(i, Some(id));
    }
    let dir = std::env::temp_dir().join(format!("pelure-test-chargement-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("Gros.pelure");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let size: u64 = std::fs::read_dir(dir.join("dessins")).unwrap().filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum();
    let t0 = Instant::now();
    let back = project::load(&manifest).unwrap();
    let dt = t0.elapsed().as_secs_f64();
    println!("24 dessins × 30 traits de graphite (dessins : {} Ko) : chargement exact en {:.2} s ({:.0} ms par dessin)", size / 1024, dt, dt * 1000.0 / 24.0);
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    assert_eq!(back.doc.cels.len(), 24);
    // L'ouverture de l'application : rendu rapide tout de suite, rejeu exact en arrière-plan.
    let t0 = Instant::now();
    let quick = project::load_quick(&manifest).unwrap();
    let dt = t0.elapsed().as_secs_f64();
    println!("chargement rapide (affiché tout de suite) : {:.2} s ; {} dessins à rejouer", dt, quick.approx.len());
    assert_eq!(quick.approx.len(), 24);
    let mut rejeu = Rejeu::new();
    let t0 = Instant::now();
    for &id in &quick.approx {
        rejeu.submit(id, quick.doc.cels[&id].vector.as_ref().unwrap(), 1920, 1080);
    }
    let mut got = 0;
    while got < 24 {
        let done = rejeu.wait(Duration::from_secs(60));
        assert!(!done.is_empty(), "le rejeu n'arrive pas");
        for d in done {
            assert_eq!(d.raster.to_rgba(), back.doc.cels[&d.cel].raster.to_rgba(), "le rejeu est le rendu exact");
            got += 1;
        }
    }
    println!("rejeu exact des 24 dessins en arrière-plan : {:.2} s", t0.elapsed().as_secs_f64());
    std::fs::remove_dir_all(&dir).ok();
}
