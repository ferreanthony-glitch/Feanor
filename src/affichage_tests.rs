//! Affichage pendant le déplacement d'une sélection vectorielle (feuille flottante) et coût d'un
//! trait vectoriel texturé pendant qu'on le trace (Anthony, 2026-10-18 : « de gros soucis
//! d'affichage lors du déplacement », « de la latence en dessinant »).

use std::time::Instant;

use eframe::egui::{pos2, vec2, Color32};

use crate::brush::{BrushSettings, StrokeMode, StrokeState, Toucher};
use crate::brushtex::BrushLibrary;
use crate::document::{CelId, Document, LayerKind};
use crate::flottante::Flottante;
use crate::raster::Raster;
use crate::render::{self, Plan, PlanItem};
use crate::vector::{Affine, Element, Trace};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Point `k` d'un long trait en lacets sur une feuille `w` × `h` : ~3 px entre deux points, en
/// aller-retour sur toute la largeur.
fn lacet(k: usize, w: f32, h: f32) -> (eframe::egui::Pos2, f32, f64) {
    let step = 3.0;
    let run = (w - 200.0) / step;
    let pass = (k as f32 / run).floor();
    let t = k as f32 % run;
    let x = if pass as usize % 2 == 0 { 100.0 + t * step } else { w - 100.0 - t * step };
    let y = 120.0 + (pass * 90.0) % (h - 240.0) + ((t * 0.05).sin() * 25.0);
    (pos2(x, y), 0.35 + 0.6 * ((k as f32 * 0.02).sin() * 0.5 + 0.5), k as f64 / 500.0)
}

fn line_item(cel: CelId) -> PlanItem {
    PlanItem { cel, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: Vec::new(), ctg: None, fond: None, pre: None }
}

/// Un hasard reproductible.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (1u64 << 31) as f32
    }
}

/// Un trait de `s` tracé par le moteur le long de `pts`, gardé comme l'élément qu'un lâcher poserait.
fn trait_rouge(s: &BrushSettings, lib: &BrushLibrary, w: usize, h: usize, pts: &[(eframe::egui::Pos2, f32, f64)]) -> Element {
    let mut raster = Raster::new(w, h);
    let mut st = StrokeState::new(0, StrokeMode::Paint([200, 30, 30]), *s, None, true).with_textures(lib.textures(s)).with_feel(Toucher::default());
    let mut dirty = Vec::new();
    for &(p, pr, t) in pts {
        st.add_point_at(&mut raster, p, pr, t, &mut dirty);
    }
    st.finish(&mut raster, &mut dirty);
    let trace = Trace { pressure: st.path_pressure.clone(), time: st.path_time.clone(), feel: Some(Toucher::default()), secondary: None, cache: Default::default() };
    Element::Stroke { points: st.path.clone(), color: [200, 30, 30], opacity: s.opacity, hardness: s.hardness, brush: lib.for_vector(s).map(Box::new), trace }
}

/// `n` traits larges, longs et qui se recouvrent, en gouache, feutre et fusain rouges, sur une
/// feuille `w` × `h` (le cas d'Anthony : « des traits rouges texturés, larges, qui se chevauchent »).
fn gros_traits(n: usize, w: usize, h: usize, seed: u64) -> Vec<Element> {
    let lib = BrushLibrary::builtin();
    lib.share();
    let mut rng = Lcg(seed);
    (0..n)
        .map(|k| {
            let name = ["Gouache", "Feutre", "Fusain"][k % 3];
            let mut s = lib.presets.iter().find(|p| p.name == name).expect("brosse").settings;
            s.size = 40.0 + rng.next() * 100.0;
            let (x0, y0) = (rng.next() * w as f32 * 0.8, rng.next() * h as f32 * 0.8);
            let (dx, dy) = ((rng.next() - 0.3) * 900.0, (rng.next() - 0.3) * 600.0);
            let steps = 120;
            let pts: Vec<_> = (0..=steps)
                .map(|i| {
                    let t = i as f32 / steps as f32;
                    (pos2(x0 + dx * t, y0 + dy * t + (t * 9.0).sin() * 40.0), 0.3 + 0.7 * (t * std::f32::consts::PI).sin(), i as f64 / 200.0)
                })
                .collect();
            trait_rouge(&s, &lib, w, h, &pts)
        })
        .collect()
}

fn doc_with(w: usize, h: usize, elements: Vec<Element>) -> (Document, CelId) {
    let mut doc = Document::new(w, h, 24);
    let id = doc.new_cel(LayerKind::Vector);
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.vector.as_mut().unwrap().elements = elements;
    cel.rerender();
    (doc, id)
}

/// Ce que montrent la feuille flottante (`aff` posée sur la sélection `items`) et le dessin une
/// fois les éléments déplacés : nombre de pixels qui diffèrent, et le premier.
fn ecart_flottante_et_repos(doc: &mut Document, id: CelId, items: &[usize], aff: &Affine) -> (usize, Option<(usize, usize, Color32, Color32)>) {
    let mut f = Flottante::lift(doc, id, items).expect("feuille");
    f.aff = *aff;
    let plan = Plan { paper: None, items: f.items(&line_item(id)).to_vec() };
    let floating = render::composite_full(doc, &plan);
    Flottante::release(doc);
    let originals: Vec<Element> = doc.cels[&id].vector.as_ref().unwrap().elements.clone();
    let cel = doc.cels.get_mut(&id).unwrap();
    {
        let d = cel.vector.as_mut().unwrap();
        for &i in items {
            // (Comme l'ancien aperçu du glissé : les pixels en cache suivent la transformation ;
            // le rendu exact, grain collé à la feuille, ne serait pas le même.)
            d.elements[i] = originals[i].transformed_preview(aff);
        }
    }
    cel.rerender();
    let rest = render::composite_full(doc, &Plan { paper: None, items: vec![line_item(id)] });
    let w = doc.width;
    // (Deux feuilles composées l'une sur l'autre arrondissent autrement qu'une seule : ±2 niveaux.)
    let far = |a: Color32, b: Color32| a.to_array().iter().zip(b.to_array()).any(|(x, y)| (*x as i32 - y as i32).abs() > 2);
    let diff: Vec<usize> = (0..floating.len()).filter(|&i| far(floating[i], rest[i])).collect();
    if let (Some(x0), Some(x1), Some(y0), Some(y1)) = (diff.iter().map(|i| i % w).min(), diff.iter().map(|i| i % w).max(), diff.iter().map(|i| i / w).min(), diff.iter().map(|i| i / w).max()) {
        println!("écarts : {} pixels dans [{x0}..{x1}] × [{y0}..{y1}]", diff.len());
    }
    let first = diff.first().map(|&i| (i % w, i / w, floating[i], rest[i]));
    // Retour à l'état d'avant.
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.vector.as_mut().unwrap().elements = originals;
    cel.rerender();
    (diff.len(), first)
}

/// Le déplacement du cas d'Anthony : beaucoup de traits larges, texturés, qui se chevauchent.
#[test]
fn many_wide_textured_strokes_float_like_the_resting_drawing() {
    let (w, h) = (1920, 1080);
    let elements = gros_traits(30, w, h, 7);
    let n = elements.len();
    let (mut doc, id) = doc_with(w, h, elements);
    let items: Vec<usize> = (0..n).collect();
    for aff in [Affine::translate(vec2(120.0, -60.0)), Affine::translate(vec2(-300.0, 200.0))] {
        let (diff, first) = ecart_flottante_et_repos(&mut doc, id, &items, &aff);
        assert_eq!(diff, 0, "{diff} pixels diffèrent, le premier : {first:?}");
    }
}

/// Les états où peut se trouver le dessin quand on saisit la sélection : caches évincés par le
/// budget, trait trop grand pour le cache, après un lâcher (géométrie seule) ou une action qui
/// termine le travail (`apply_preview`).
#[test]
fn the_floating_sheet_matches_the_drawing_in_every_cache_state() {
    let (w, h) = (1920, 1080);
    let elements = gros_traits(24, w, h, 11);
    let n = elements.len();
    let (mut doc, id) = doc_with(w, h, elements);
    // Les derniers traits : au-dessus des autres, comme sur la feuille flottante.
    let items: Vec<usize> = (n / 2..n).collect();
    let moves = [Affine::translate(vec2(80.0, 40.0)), Affine::rotate_about(pos2(900.0, 500.0), 0.3), Affine::scale_about(pos2(300.0, 300.0), 1.3, 1.3)];
    let check = |doc: &mut Document, label: &str| {
        for (k, aff) in moves.iter().enumerate() {
            // Le rendu de référence d'une transformation non entière : pixels en cache rééchantillonnés.
            let (diff, first) = ecart_flottante_et_repos(doc, id, &items, aff);
            println!("{label} / {aff:?} : {diff} pixels diffèrent {first:?}");
            // Un déplacement entier depuis des pixels en cache : la même image que l'ancien aperçu.
            if k == 0 && !label.contains("évincés") {
                assert_eq!(diff, 0, "{label} : {diff} pixels diffèrent, le premier : {first:?}");
            }
        }
        let (diff, first) = ecart_flottante_et_repos(doc, id, &items, &Affine::IDENTITY);
        assert_eq!(diff, 0, "{label} : sans geste, {diff} pixels diffèrent, le premier : {first:?}");
    };
    check(&mut doc, "neuf");
    // Un budget dépassé évince des caches (rejoués à la demande) : un trait sur deux sans pixels.
    for e in doc.cels.get_mut(&id).unwrap().vector.as_mut().unwrap().elements.iter_mut().step_by(2) {
        e.invalidate_cache();
    }
    check(&mut doc, "caches évincés");
    // Géométrie seule (le lâcher) : caches vides.
    let originals = doc.cels[&id].vector.as_ref().unwrap().elements.clone();
    let picked: Vec<Element> = items.iter().map(|&i| originals[i].clone()).collect();
    Flottante::apply_geometry(&mut doc, id, &items, &picked, &moves[0]);
    doc.cels.get_mut(&id).unwrap().rerender();
    check(&mut doc, "après lâcher");
    // Terminé à l'ancienne : pixels en cache suivis (aperçu).
    let originals = doc.cels[&id].vector.as_ref().unwrap().elements.clone();
    let picked: Vec<Element> = items.iter().map(|&i| originals[i].clone()).collect();
    Flottante::apply_preview(&mut doc, id, &items, &picked, &moves[0]);
    check(&mut doc, "après apply_preview");
    // Un second geste sur la sélection déjà passée par un aperçu, tournée puis agrandie.
    for aff in &moves[1..] {
        let originals = doc.cels[&id].vector.as_ref().unwrap().elements.clone();
        let picked: Vec<Element> = items.iter().map(|&i| originals[i].clone()).collect();
        Flottante::apply_preview(&mut doc, id, &items, &picked, aff);
        check(&mut doc, "après deux aperçus");
    }
}

/// Des traits larges qui dépassent de la feuille (en haut et à gauche), déplacés vers l'intérieur :
/// ce qui était hors de la feuille doit apparaître, comme sur le dessin refait à sa nouvelle place
/// (Anthony : « des morceaux du dessin manquent, coupés en ligne droite »).
fn bord_haut_gauche() -> Vec<Element> {
    let lib = BrushLibrary::builtin();
    lib.share();
    let mut out = Vec::new();
    for (k, name) in ["Gouache", "Feutre", "Fusain", "Gouache"].iter().enumerate() {
        let mut s = lib.presets.iter().find(|p| p.name == *name).expect("brosse").settings;
        s.size = 70.0 + 10.0 * k as f32;
        // De hors de la feuille (y < 0, ou x < 0) vers l'intérieur.
        let (from, to) = if k % 2 == 0 { (pos2(150.0 + 120.0 * k as f32, -90.0), pos2(260.0 + 120.0 * k as f32, 420.0)) } else { (pos2(-110.0, 180.0 + 90.0 * k as f32), pos2(480.0, 260.0 + 90.0 * k as f32)) };
        let pts: Vec<_> = (0..=80)
            .map(|i| {
                let t = i as f32 / 80.0;
                (from + (to - from) * t, 0.8, i as f64 / 200.0)
            })
            .collect();
        out.push(trait_rouge(&s, &lib, 1920, 1080, &pts));
    }
    out
}

/// Au lever du stylet, le trait vectoriel texturé n'est plus rejoué sur le fil de l'interface :
/// l'aperçu du geste reste, le fil de travail rend le dessin exact (le seul trait nouveau est
/// rejoué, les autres gardent leurs pixels) et le trait en trouve ses pixels en cache.
#[test]
fn pen_up_hands_the_replay_to_the_worker_and_only_the_new_stroke_is_replayed() {
    use crate::document::Cel;
    use crate::rejeu::Rejeu;
    use std::time::Duration;
    let (w, h) = (1920, 1080);
    let mut cel = Cel::new(w, h, LayerKind::Vector);
    cel.vector.as_mut().unwrap().elements = gros_traits(12, w, h, 21);
    cel.rerender();
    let before: Vec<usize> = cel.vector.as_ref().unwrap().elements.iter().map(Element::cache_bytes).collect();
    assert!(before.iter().all(|&b| b > 0), "les traits sont en cache");
    // Le trait qu'on vient de tracer : aperçu posé par le moteur, élément sans pixels en cache.
    let lib = BrushLibrary::builtin();
    lib.share();
    let mut s = lib.presets.iter().find(|p| p.name == "Gouache").unwrap().settings;
    s.size = 45.0;
    let pts: Vec<_> = (0..=300).map(|i| (pos2(200.0 + i as f32 * 4.0, 500.0 + (i as f32 * 0.05).sin() * 120.0), 0.6, i as f64 / 200.0)).collect();
    let element = trait_rouge(&s, &lib, w, h, &pts);
    assert_eq!(element.cache_bytes(), 0);
    cel.vector.as_mut().unwrap().elements.push(element);
    cel.version += 1;
    let mut rejeu = Rejeu::new();
    let t0 = Instant::now();
    rejeu.submit_with(1, cel.vector.as_ref().unwrap(), w, h, cel.version, true);
    let ui = ms(t0);
    let done = rejeu.wait(Duration::from_secs(30));
    assert_eq!(done.len(), 1, "le rendu exact arrive");
    assert!(crate::rejeu::adoptable(&cel, &done[0]));
    // Les traits d'avant ont gardé leurs pixels ; le nouveau a les siens (dans le cache partagé).
    let after: Vec<usize> = cel.vector.as_ref().unwrap().elements.iter().map(Element::cache_bytes).collect();
    assert_eq!(&after[..12], &before[..], "les traits déjà rendus gardent leurs pixels");
    assert!(after[12] > 0, "le nouveau trait trouve ses pixels en cache");
    // Même image qu'un rendu complet du dessin.
    let mut fresh = cel.clone();
    fresh.rerender();
    assert_eq!(done[0].raster.to_flat(), fresh.raster.to_flat(), "l'image rendue en arrière-plan est le dessin exact");
    println!("copie du dessin au lever du stylet : {ui:.2} ms ; rendu en arrière-plan : {:.1} ms", done[0].took.as_secs_f64() * 1e3);
}

/// Savoir quels traits seraient rejoués sur le fil de l'interface (pour les réchauffer en arrière-
/// plan avant qu'on saisisse la sélection) : ni les formes, ni les traits lisses, ni un trait trop
/// grand pour le cache (déjà rendu) n'en font partie.
#[test]
fn missing_caches_counts_only_the_strokes_a_view_would_replay() {
    let (w, h) = (1920, 1080);
    let mut elements = gros_traits(6, w, h, 9);
    elements.push(Element::Stroke { points: vec![crate::vector::VPoint { x: 10.0, y: 10.0, r: 3.0 }, crate::vector::VPoint { x: 90.0, y: 40.0, r: 3.0 }], color: [0, 0, 0], opacity: 1.0, hardness: 0.8, brush: None, trace: Trace::default() });
    let (mut doc, id) = doc_with(w, h, elements);
    let d = |doc: &Document| doc.cels[&id].vector.as_ref().unwrap().missing_caches();
    assert_eq!(d(&doc), 0, "tout est rendu");
    for e in doc.cels.get_mut(&id).unwrap().vector.as_mut().unwrap().elements.iter_mut().take(4) {
        e.invalidate_cache();
    }
    assert_eq!(d(&doc), 4, "quatre traits évincés");
    doc.cels.get_mut(&id).unwrap().rerender();
    assert_eq!(d(&doc), 0, "rendus de nouveau");
}

/// Le lâcher ne pose que la géométrie (l'image du dessin est encore celle d'avant) ; si
/// l'utilisateur agit avant l'arrivée du rendu exact, `apply_preview` termine le travail : la
/// sélection ne doit pas rester en double à son ancienne place.
#[test]
fn finishing_a_released_gesture_early_leaves_no_ghost_at_the_old_place() {
    let (w, h) = (1920, 1080);
    let elements = gros_traits(20, w, h, 3);
    let n = elements.len();
    let (mut doc, id) = doc_with(w, h, elements);
    let items: Vec<usize> = (n / 2..n).collect();
    let originals: Vec<Element> = items.iter().map(|&i| doc.cels[&id].vector.as_ref().unwrap().elements[i].clone()).collect();
    let aff = Affine::translate(vec2(400.0, 150.0));
    Flottante::apply_geometry(&mut doc, id, &items, &originals, &aff);
    Flottante::apply_preview(&mut doc, id, &items, &originals, &aff);
    let shown = doc.cels[&id].raster.to_flat();
    let mut fresh = doc.cels[&id].clone();
    fresh.rerender();
    let ghosts = shown.iter().zip(fresh.raster.to_flat()).filter(|(a, b)| a != &b).count();
    assert_eq!(ghosts, 0, "{ghosts} pixels de l'image du dessin ne sont pas ceux du dessin refait");
}

/// Un trait qui dépasse de la feuille est rendu sur une image plus grande (pour garder ce qui
/// dépasse) : sur la feuille, il doit rester pixel pour pixel celui du moteur, grain du papier
/// compris (le papier se lit comme sur la feuille, pas sur l'image décalée).
#[test]
fn a_stroke_crossing_the_sheet_edge_keeps_the_engine_pixels_on_the_sheet() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let (w, h) = (700usize, 500usize);
    for (name, inside) in [("Fusain", true), ("Fusain", false), ("Craie", false), ("Crayon graphite", false), ("Feutre", false), ("Encre", false)] {
        let mut s = lib.presets.iter().find(|p| p.name == name).expect("brosse").settings;
        s.size = 50.0;
        // (`inside` : le même trait décalé dans la feuille, pour voir ce que l'écart doit au bord.)
        let (ox, oy) = if inside { (200.0, 120.0) } else { (0.0, 0.0) };
        let pts: Vec<_> = (0..=100)
            .map(|i| {
                let t = i as f32 / 100.0;
                (pos2(ox - 120.0 + 600.0 * t, oy - 60.0 + 380.0 * t + (t * 7.0).sin() * 40.0), 0.3 + 0.7 * (t * std::f32::consts::PI).sin(), i as f64 / 200.0)
            })
            .collect();
        // Le moteur sur la feuille (comme pendant le geste).
        let mut live = Raster::new(w, h);
        let mut st = StrokeState::new(0, StrokeMode::Paint([200, 30, 30]), s, None, true).with_textures(lib.textures(&s)).with_feel(Toucher::default());
        let mut dirty = Vec::new();
        for &(p, pr, t) in &pts {
            st.add_point_at(&mut live, p, pr, t, &mut dirty);
        }
        st.finish(&mut live, &mut dirty);
        let e = trait_rouge(&s, &lib, w, h, &pts);
        assert_eq!(e.bounds().min.x < 0.0 && e.bounds().min.y < 0.0, !inside, "{name} : le trait dépasse de la feuille");
        let mut replay = Raster::new(w, h);
        crate::vector::render_element(&mut replay, &e);
        // (Décalée de la marge, la position des empreintes s'arrondit autrement dans les flottants :
        // un niveau d'écart sur quelques pixels au bord, pas plus.)
        let tol = if inside { 0 } else { 1 };
        let far = |a: Color32, b: Color32| a.to_array().iter().zip(b.to_array()).any(|(x, y)| (*x as i32 - y as i32).abs() > tol);
        let bad = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| far(live.get(x, y), replay.get(x, y))).count();
        assert_eq!(bad, 0, "{name} {} : {bad} pixels de la feuille diffèrent du moteur de plus de {tol} niveau", if inside { "dans la feuille" } else { "qui dépasse" });
    }
}

/// Pixels nettement peints dans une image et nettement vides dans l'autre.
fn couverture_differente(a: &[Color32], b: &[Color32]) -> usize {
    a.iter().zip(b).filter(|(x, y)| (x.a() >= 128 && y.a() <= 8) || (y.a() >= 128 && x.a() <= 8)).count()
}

#[test]
fn strokes_crossing_the_sheet_edge_show_their_outside_part_when_moved_inside() {
    // Translation (entière), rotation autour du centre de la sélection, agrandissement : dans
    // chaque cas la couverture est celle du dessin refait à sa nouvelle place.
    let moves: [(&str, Affine, usize); 3] = [
        ("translation", Affine::translate(vec2(160.0, 140.0)), 1),
        ("rotation", Affine::rotate_about(pos2(300.0, 250.0), 0.35).then(&Affine::translate(vec2(140.0, 120.0))), 6),
        ("échelle", Affine::scale_about(pos2(480.0, 420.0), 1.4, 1.4), 6),
    ];
    for (label, aff, percent) in moves {
        let (mut doc, id) = doc_with(1920, 1080, bord_haut_gauche());
        let items: Vec<usize> = (0..4).collect();
        let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
        assert!(f.dims.is_some() && f.margin.0 > 0 && f.margin.1 > 0, "la feuille garde ce qui dépasse en haut et à gauche");
        f.aff = aff;
        let plan = Plan { paper: None, items: f.items(&line_item(id)).to_vec() };
        let floating = render::composite_full(&doc, &plan);
        Flottante::release(&mut doc);
        // Le dessin refait à sa nouvelle place (rendu exact, rien d'en cache).
        let cel = doc.cels.get_mut(&id).unwrap();
        for e in cel.vector.as_mut().unwrap().elements.iter_mut() {
            *e = e.transformed(&aff);
        }
        cel.rerender();
        let exact = render::composite_full(&doc, &Plan { paper: None, items: vec![line_item(id)] });
        let n = couverture_differente(&floating, &exact);
        let painted = exact.iter().filter(|c| c.a() >= 128).count();
        // (Rééchantillonné, le bord d'un trait tourné ou agrandi diffère un peu.)
        assert!(n * 100 < painted * percent, "{label} : {n} pixels sur {painted} peints diffèrent (morceaux manquants)");
    }
}

#[test]
fn a_selection_inside_the_sheet_keeps_a_sheet_of_the_drawing_size() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let s = lib.presets.iter().find(|p| p.name == "Gouache").unwrap().settings;
    let elements: Vec<Element> = (0..3)
        .map(|k| {
            let pts: Vec<_> = (0..=40).map(|i| (pos2(600.0 + i as f32 * 8.0, 400.0 + k as f32 * 120.0), 0.7, i as f64 / 200.0)).collect();
            trait_rouge(&s, &lib, 1920, 1080, &pts)
        })
        .collect();
    let (mut doc, id) = doc_with(1920, 1080, elements);
    let b = doc.cels[&id].vector.as_ref().unwrap().elements.iter().fold(eframe::egui::Rect::NOTHING, |r, e| r.union(e.bounds()));
    assert!(b.min.x > 0.0 && b.min.y > 0.0 && b.max.x < 1920.0 && b.max.y < 1080.0, "les traits sont dans la feuille");
    let f = Flottante::lift(&mut doc, id, &[0, 1, 2]).expect("feuille");
    assert_eq!((f.margin, f.dims), ((0, 0), None));
    let sheet = &doc.cels[&crate::flottante::SHEET_CEL].raster;
    assert_eq!((sheet.width, sheet.height), (1920, 1080));
}

#[test]
fn the_overhang_is_bounded_by_the_margin_and_the_texture_size() {
    use crate::vector::{overhang, MAX_OVERHANG, MAX_SIDE};
    use eframe::egui::Rect;
    // Dans la feuille ou sans boîte : rien.
    assert_eq!(overhang(Rect::from_min_max(pos2(10.0, 10.0), pos2(90.0, 90.0)), 100, 100), (0, 0, 0, 0));
    assert_eq!(overhang(Rect::NOTHING, 100, 100), (0, 0, 0, 0));
    // Ce qui dépasse, en pixels entiers.
    assert_eq!(overhang(Rect::from_min_max(pos2(-20.5, -3.0), pos2(130.2, 100.0)), 100, 100), (21, 3, 31, 0));
    // Bornée par la marge.
    let (l, t, r, d) = overhang(Rect::from_min_max(pos2(-5000.0, -5000.0), pos2(9000.0, 9000.0)), 100, 100);
    assert!([l, t, r, d].iter().all(|&v| v == MAX_OVERHANG));
    // Et par la taille des textures de la carte graphique.
    let (l, _, r, _) = overhang(Rect::from_min_max(pos2(-900.0, 0.0), pos2(8000.0, 10.0)), 7500, 100);
    assert!(7500 + l + r <= MAX_SIDE, "{l} + {r}");
}

/// Chaque pinceau de la bibliothèque : tout ce que le moteur peint tient dans la boîte de
/// l'élément (+ 1,5), sinon le rendu par zone (`render_element_clipped`) coupe le trait en
/// ligne droite.
#[test]
fn every_builtin_brush_paints_inside_the_element_bounds() {
    let lib = BrushLibrary::builtin();
    lib.share();
    let (w, h) = (1000usize, 700usize);
    let mut bad = Vec::new();
    for p in &lib.presets {
        for size in [8.0f32, 40.0, 120.0] {
            let mut s = p.settings;
            s.size = size;
            let pts: Vec<_> = (0..=60)
                .map(|i| {
                    let t = i as f32 / 60.0;
                    (pos2(250.0 + 500.0 * t, 350.0 + (t * 6.0).sin() * 120.0), 0.2 + 0.8 * (t * std::f32::consts::PI).sin(), i as f64 / 200.0)
                })
                .collect();
            let e = trait_rouge(&s, &lib, w, h, &pts);
            // Le trait rendu seul, sans le découpage par la boîte : le moteur sur une feuille vide.
            let mut raster = Raster::new(w, h);
            let mut st = StrokeState::new(0, StrokeMode::Paint([200, 30, 30]), s, None, true).with_textures(lib.textures(&s)).with_feel(Toucher::default());
            let mut dirty = Vec::new();
            for &(q, pr, t) in &pts {
                st.add_point_at(&mut raster, q, pr, t, &mut dirty);
            }
            st.finish(&mut raster, &mut dirty);
            let b = e.bounds().expand(1.5);
            let mut outside = 0usize;
            for y in 0..h {
                for x in 0..w {
                    if raster.get(x, y).a() > 0 && !b.contains(pos2(x as f32 + 0.5, y as f32 + 0.5)) {
                        outside += 1;
                    }
                }
            }
            if outside > 0 {
                bad.push(format!("{} {size} px : {outside} pixels hors de la boîte", p.name));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Le cas d'Anthony, en durées : 40 gros traits texturés qui dépassent de la feuille, saisis puis
/// déplacés. Coût de la feuille flottante élargie (saisie) contre la feuille à la taille du
/// dessin, mémoire des caches, et coût d'une image du geste.
/// `cargo test --release --offline --target-dir target\msvc floating_sheet_with_overhang -- --ignored --nocapture`
#[test]
#[ignore]
fn floating_sheet_with_overhang_cost() {
    let (w, h) = (1920, 1080);
    for (label, clamp) in [("dans la feuille", true), ("dépassent de la feuille", false)] {
        let t0 = Instant::now();
        let elements: Vec<Element> = gros_traits(40, w, h, 5)
            .into_iter()
            .map(|e| {
                if !clamp {
                    return e;
                }
                // Le même trait, ramené dans la feuille.
                let b = e.bounds();
                let (dx, dy) = ((100.0 - b.min.x).max(0.0) - (b.max.x - (w as f32 - 100.0)).max(0.0), (100.0 - b.min.y).max(0.0) - (b.max.y - (h as f32 - 100.0)).max(0.0));
                e.transformed(&Affine::translate(vec2(dx, dy)))
            })
            .collect();
        let n = elements.len();
        let build = ms(t0);
        let (mut doc, id) = doc_with(w, h, elements);
        let items: Vec<usize> = (0..n).collect();
        let outside = doc.cels[&id].vector.as_ref().unwrap().elements.iter().filter(|e| {
            let b = e.bounds();
            b.min.x < 0.0 || b.min.y < 0.0 || b.max.x > w as f32 || b.max.y > h as f32
        });
        let outside = outside.count();
        let cached: usize = doc.cels[&id].vector.as_ref().unwrap().elements.iter().map(Element::cache_bytes).sum();
        // Premier rendu complet (remplit les caches), puis la saisie depuis les caches.
        let t0 = Instant::now();
        doc.cels.get_mut(&id).unwrap().rerender();
        let rerender = ms(t0);
        let replays = crate::vector::replay_count();
        // (La saisie d'avant : les deux feuilles à la taille du dessin, sans ce qui dépasse.)
        let t0 = Instant::now();
        let _ = doc.cels[&id].vector.as_ref().unwrap().render_split(Some(&items), w, h);
        let old_lift = ms(t0);
        let t0 = Instant::now();
        let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
        let lift = ms(t0);
        let replayed = crate::vector::replay_count() - replays;
        let sheet = &doc.cels[&crate::flottante::SHEET_CEL].raster;
        let tiles = sheet.tiles.iter().flatten().count();
        let t0 = Instant::now();
        let frames = 50;
        for k in 0..frames {
            f.aff = Affine::translate(vec2(3.0 * k as f32, 2.0 * k as f32));
            let plan = Plan { paper: None, items: f.items(&line_item(id)).to_vec() };
            for it in &plan.items {
                let _ = crate::fx::shown(&doc, it);
            }
        }
        let frame = ms(t0) / frames as f64;
        println!(
            "{label} : {outside}/{n} traits dépassent | caches {:.1} Mo, premier rendu {rerender:.0} ms | saisie {lift:.1} ms (avant {old_lift:.1} ms ; {replayed} rejeux), feuille {}×{} ({} tuiles, {:.1} Mo) | {frame:.3} ms par image",
            cached as f64 / 1e6,
            sheet.width,
            sheet.height,
            tiles,
            tiles as f64 * 16384.0 / 1e6
        );
        let _ = build;
        // Caches évincés (budget dépassé, ou dessin pas encore rejoué à l'ouverture) : la saisie
        // rejoue les traits sur le fil de l'interface — correct, mais pas instantané.
        Flottante::release(&mut doc);
        for e in doc.cels.get_mut(&id).unwrap().vector.as_mut().unwrap().elements.iter_mut() {
            e.invalidate_cache();
        }
        let replays = crate::vector::replay_count();
        let t0 = Instant::now();
        let _ = Flottante::lift(&mut doc, id, &items).expect("feuille");
        println!("   {label}, sans aucun cache : saisie {:.0} ms ({} rejeux sur le fil de l'interface)", ms(t0), crate::vector::replay_count() - replays);
    }
}

/// Le coût par image d'écran du trait vectoriel en direct (le moteur peint sur l'image du
/// dessin, 4 points par image d'écran) selon sa longueur, par brosse.
/// `cargo test --release --offline --target-dir target\msvc live_textured_stroke -- --ignored --nocapture`
#[test]
#[ignore]
fn live_textured_stroke_cost_per_frame() {
    let lib = BrushLibrary::builtin();
    lib.share();
    for (name, size) in [("Gouache", 40.0), ("Gouache", 90.0), ("Fusain", 60.0), ("Feutre", 60.0), ("Craie", 60.0)] {
        let mut s = lib.presets.iter().find(|p| p.name == name).expect("brosse").settings;
        s.size = size;
        let (w, h) = (1920.0, 1080.0);
        let mut raster = Raster::new(w as usize, h as usize);
        let mut st = StrokeState::new(0, StrokeMode::Paint([200, 30, 30]), s, None, true).with_textures(lib.textures(&s)).with_feel(Toucher::default());
        let mut dirty = Vec::new();
        let mut buckets: Vec<(usize, f64, usize)> = Vec::new();
        let (mut acc, mut frames, mut tiles) = (0.0, 0usize, 0usize);
        let total = 2400;
        for k in 0..total {
            let (p, pr, t) = lacet(k, w, h);
            let t0 = Instant::now();
            st.add_point_at(&mut raster, p, pr, t, &mut dirty);
            acc += ms(t0);
            if k % 4 == 3 {
                tiles += {
                    dirty.sort_unstable();
                    dirty.dedup();
                    dirty.len()
                };
                dirty.clear();
                frames += 1;
            }
            if (k + 1) % 400 == 0 {
                buckets.push((k + 1, acc / frames.max(1) as f64, tiles / frames.max(1)));
                (acc, frames, tiles) = (0.0, 0, 0);
            }
        }
        let line: Vec<String> = buckets.iter().map(|(k, m, t)| format!("{k}: {m:.2} ms ({t} tuiles)")).collect();
        println!("{name} {size} px : par image d'écran (4 points), par tranche de 400 points | {}", line.join(" | "));
        let t0 = Instant::now();
        let _ = st.finish(&mut raster, &mut dirty);
        println!("   finish : {:.1} ms", ms(t0));
    }
}

/// Un dessin vectoriel de `n` traits texturés déjà rendus (leurs pixels sont en cache), puis un
/// trait de plus tracé en direct et posé au lâcher comme `commit_vector_stroke` : durées sur le
/// fil de l'interface au début du trait (copie du dessin pour l'annulation), pendant (par image
/// d'écran) et au lâcher.
/// `cargo test --release --offline --target-dir target\msvc vector_stroke_begin_and_commit -- --ignored --nocapture`
#[test]
#[ignore]
fn vector_stroke_begin_and_commit_cost() {
    use crate::document::{Cel, LayerKind};
    use crate::fondfil_tests::{geste, trait_de};
    let lib = BrushLibrary::builtin();
    lib.share();
    let s = lib.presets.iter().find(|p| p.name == "Gouache").expect("brosse").settings;
    for (n, points) in [(0usize, 600usize), (60, 600), (60, 2400)] {
        let mut cel = Cel::new(1920, 1080, LayerKind::Vector);
        let elements: Vec<_> = (0..n).map(|k| trait_de(&s, &lib, &geste(160, 20.0 + (k % 8) as f32 * 225.0, 60.0 + (k / 8) as f32 * 120.0))).collect();
        cel.vector.as_mut().unwrap().elements = elements;
        cel.rerender();
        let t0 = Instant::now();
        let before = cel.vector.clone();
        let begin = ms(t0);
        drop(before);
        let mut st = StrokeState::new(0, StrokeMode::Paint([200, 30, 30]), s, None, true).with_textures(lib.textures(&s)).with_feel(Toucher::default());
        let mut dirty = Vec::new();
        let (mut worst, mut sum) = (0.0f64, 0.0);
        for k in 0..points {
            let (p, pr, t) = lacet(k, 1920.0, 1080.0);
            let t0 = Instant::now();
            st.add_point_at(&mut cel.raster, p, pr, t, &mut dirty);
            if k % 4 == 3 {
                dirty.clear();
            }
            let d = ms(t0);
            sum += d;
            worst = worst.max(d);
        }
        let t0 = Instant::now();
        let edit = st.finish(&mut cel.raster, &mut dirty);
        let finish = ms(t0);
        let (path, pressure, time) = (st.path.clone(), st.path_pressure.clone(), st.path_time.clone());
        let t0 = Instant::now();
        let element = crate::vector::Element::Stroke {
            points: path,
            color: [200, 30, 30],
            opacity: s.opacity,
            hardness: s.hardness,
            brush: lib.for_vector(&s).map(Box::new),
            trace: crate::vector::Trace { pressure, time, feel: Some(Toucher::default()), secondary: Some([255, 255, 255]), cache: Default::default() },
        };
        cel.vector.as_mut().unwrap().elements.push(element.clone());
        let Some(crate::history::Edit::Tiles { tiles, .. }) = edit else { panic!("tuiles") };
        let changed = cel.render_on_top(tiles, &element);
        let commit = ms(t0);
        // Maintenant : le même trait, sans pixels en cache, confié au fil de travail.
        let mut cel2 = cel.clone();
        cel2.vector.as_mut().unwrap().elements.last_mut().unwrap().invalidate_cache();
        let mut rejeu = crate::rejeu::Rejeu::new();
        let t0 = Instant::now();
        rejeu.submit_with(1, cel2.vector.as_ref().unwrap(), 1920, 1080, cel2.version, true);
        let hand_over = ms(t0);
        let done = rejeu.wait(std::time::Duration::from_secs(60));
        let worker = done.first().map_or(0.0, |d| d.took.as_secs_f64() * 1e3);
        println!(
            "{n} traits déjà là, trait de {points} points : début (copie du dessin) {begin:.2} ms | en direct {:.3} ms par point (pire {worst:.2}) | finish {finish:.1} ms | pose au lâcher AVANT {commit:.1} ms ({} tuiles) ; APRÈS {hand_over:.2} ms sur le fil de l'interface, {worker:.0} ms en arrière-plan",
            sum / points as f64,
            changed.len()
        );
    }
}
