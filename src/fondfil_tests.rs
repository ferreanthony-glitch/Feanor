//! Fond sous le trait calculé en arrière-plan (`fondfil.rs`, `fx::differe`) : demande caduque
//! jetée, adoption, résultat final identique au calcul direct, et durées sur le fil de
//! l'interface (essai `#[ignore]`).
//!
//! Les essais qui passent par le fond affiché (`fx::fond_poll`, le cache des dessins dérivés) touchent à
//! l'état commun : ils le prennent en lecture (`fxcache::monde::lire`), sinon un fond adopté ici change
//! le compte des fonds (`fx::fond_epoch`) sous un essai qui l'a pris pour lui seul.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{pos2, vec2, Color32, Pos2, Rect};

use crate::brush::{BrushSettings, StrokeMode, StrokeState, Toucher};
use crate::brushtex::BrushLibrary;
use crate::document::{Cel, CelId, Document, LayerKind};
use crate::fond::{compose, silhouette, FondReglages};
use crate::fondfil::{calculer, Demande, FondFil};
use crate::fx;
use crate::fxcache::monde;
use crate::raster::{Raster, TILE};
use crate::render::{self, FondPart, Plan, PlanItem};
use crate::vector::{Element, Trace};

fn put(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

/// Un anneau noir de 3 px de centre (`cx`, 100), rayon 40, dans un dessin 400×200.
fn ring(cx: f32) -> Raster {
    let mut r = Raster::new(400, 200);
    for y in 0..200 {
        for x in 0..400 {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - 100.0).powi(2)).sqrt();
            if (38.5..=41.5).contains(&d) {
                put(&mut r, x, y, Color32::BLACK);
            }
        }
    }
    r
}

fn demande(id: CelId, version: u64, line: Raster, r: FondReglages) -> Demande {
    Demande { id, version, src_version: version, reglages: r, width: line.width, height: line.height, forme: None, trait_: Some(line), peinture: None, coupe: None }
}

fn exact(line: &Raster, r: &FondReglages) -> Vec<Color32> {
    compose(&silhouette(line, r), line.width, line.height, r.couleur, None, None).to_flat()
}

#[test]
fn a_superseded_request_is_dropped_and_only_the_latest_arrives() {
    let r = FondReglages::default();
    let mut fil = FondFil::new();
    let (a, b) = (ring(100.0), ring(250.0));
    fil.submit(demande(7, 1, a, r));
    fil.submit(demande(7, 2, b.clone(), r));
    assert_eq!(fil.pending_version(7), Some(2));
    let done = fil.wait(Duration::from_secs(20));
    assert_eq!(done.len(), 1, "une seule demande valable");
    assert_eq!(done[0].version, 2, "la plus récente");
    assert_eq!(done[0].raster.to_flat(), exact(&b, &r), "pixels identiques au calcul direct");
    std::thread::sleep(Duration::from_millis(300));
    assert!(fil.poll().is_empty(), "la première n'arrive jamais");
    assert!(!fil.has_pending());
}

#[test]
fn a_result_arriving_after_a_newer_request_is_discarded() {
    let r = FondReglages::default();
    let mut fil = FondFil::new();
    fil.submit(demande(8, 1, ring(100.0), r));
    // Le fil commence à calculer la première ; la seconde la rend caduque avant qu'elle n'arrive.
    std::thread::sleep(Duration::from_millis(1));
    fil.submit(demande(8, 2, ring(250.0), r));
    let done = fil.wait(Duration::from_secs(20));
    assert!(done.iter().all(|d| d.version == 2), "aucun résultat de la version 1");
    assert_eq!(done.len(), 1);
}

#[test]
fn a_cancelled_request_never_arrives() {
    let r = FondReglages::default();
    let mut fil = FondFil::new();
    fil.submit(demande(9, 1, ring(100.0), r));
    fil.cancel(9);
    assert!(!fil.has_pending());
    std::thread::sleep(Duration::from_millis(300));
    assert!(fil.poll().is_empty());
}

#[test]
fn a_known_shape_is_composed_without_the_line() {
    // Seule la strate Fond a changé : la forme déjà connue suffit.
    let r = FondReglages::default();
    let line = ring(100.0);
    let forme = Arc::new(crate::fond::Couverture::depuis_feuille(400, 200, silhouette(&line, &r)));
    let d = Demande { id: 1, version: 1, src_version: 1, reglages: r, width: 400, height: 200, forme: Some(forme.clone()), trait_: None, peinture: None, coupe: None };
    let (f, raster) = calculer(&d);
    assert!(Arc::ptr_eq(&f, &forme));
    assert_eq!(raster.to_flat(), exact(&line, &r));
}

// ------------------------------------------------------------------ le fond affiché (fx)

fn fond_item(cel: CelId, r: FondReglages) -> PlanItem {
    PlanItem { cel, opacity: 255, tint: None, transform: None, blend: Default::default(), fx: Vec::new(), ctg: None, fond: Some(FondPart::Forme(r)), pre: None }
}

fn arc_of(doc: &Document, item: &PlanItem) -> Arc<Cel> {
    match fx::shown(doc, item).expect("le fond") {
        fx::Shown::Fx(c) => c,
        fx::Shown::Plain(_) => panic!("le fond est un dessin dérivé"),
    }
}

/// Un document 400×200 dont le dessin est `line`, et ses réglages de fond (propres à l'essai : le
/// cache est commun à tous les essais).
fn doc_with(line: Raster, couleur: [u8; 3]) -> (Document, CelId, FondReglages) {
    let mut doc = Document::new(400, 200, 24);
    let id = doc.new_cel(LayerKind::Bitmap);
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.raster = line;
    cel.version += 1;
    (doc, id, FondReglages { couleur, ..Default::default() })
}

fn change(doc: &mut Document, id: CelId, line: Raster) {
    let cel = doc.cels.get_mut(&id).unwrap();
    cel.raster = line;
    cel.version += 1;
}

/// Attend que le fond affiché de `item` ne soit plus `old`.
fn wait_new(doc: &Document, item: &PlanItem, old: &Arc<Cel>) -> Arc<Cel> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        fx::fond_poll();
        let now = fx::differe(|| arc_of(doc, item));
        if !Arc::ptr_eq(&now, old) {
            return now;
        }
        assert!(Instant::now() < deadline, "le fond n'est pas arrivé");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn on_screen_the_previous_background_stays_until_the_new_one_is_ready() {
    // (L'identité des dessins (`Arc::ptr_eq`) tient à la réserve commune : un autre essai qui ouvre un projet (`forget_all`) ou la
    // remplit (éviction) la change sous ses pieds. État commun pour lui seul.)
    monde::ecrire();
    let (mut doc, id, r) = doc_with(ring(100.0), [11, 22, 33]);
    let item = fond_item(id, r);
    // Premier affichage : rien d'avant, calculé tout de suite.
    let first = fx::differe(|| arc_of(&doc, &item));
    assert_eq!(first.raster.to_flat(), exact(&doc.cels[&id].raster, &r));
    // Le dessin change : l'écran garde le fond d'avant (le même, sans calcul).
    let new_line = ring(250.0);
    change(&mut doc, id, new_line.clone());
    let shown = fx::differe(|| arc_of(&doc, &item));
    assert!(Arc::ptr_eq(&shown, &first), "le fond d'avant en attendant");
    // Il arrive, identique au calcul direct.
    let arrived = wait_new(&doc, &item, &first);
    assert_eq!(arrived.raster.to_flat(), exact(&new_line, &r), "résultat final identique");
    // Adopté : plus rien d'ancien, et plus de calcul.
    let again = fx::differe(|| arc_of(&doc, &item));
    assert!(Arc::ptr_eq(&again, &arrived));
}

#[test]
fn the_exact_background_is_computed_at_once_for_export_and_save() {
    monde::lire();
    let (mut doc, id, r) = doc_with(ring(100.0), [12, 23, 34]);
    let item = fond_item(id, r);
    let first = arc_of(&doc, &item);
    let new_line = ring(250.0);
    change(&mut doc, id, new_line.clone());
    // Hors de `differe` : pas de fond d'avant, le bon tout de suite.
    let now = arc_of(&doc, &item);
    assert!(!Arc::ptr_eq(&now, &first));
    assert_eq!(now.raster.to_flat(), exact(&new_line, &r));
}

#[test]
fn only_the_latest_of_several_quick_changes_is_adopted() {
    monde::lire();
    let (mut doc, id, r) = doc_with(ring(60.0), [13, 24, 35]);
    let item = fond_item(id, r);
    let first = fx::differe(|| arc_of(&doc, &item));
    // Un glissé : une version par image, le fil n'en finit qu'une.
    let mut last = ring(60.0);
    for k in 1..=8 {
        last = ring(60.0 + 30.0 * k as f32);
        change(&mut doc, id, last.clone());
        let shown = fx::differe(|| arc_of(&doc, &item));
        assert!(Arc::ptr_eq(&shown, &first) || shown.version != first.version, "toujours un fond montré");
    }
    // Quoi qu'il arrive au passage, ce qui reste est le fond de la dernière version.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        fx::fond_poll();
        let now = fx::differe(|| arc_of(&doc, &item));
        if now.raster.to_flat() == exact(&last, &r) {
            break;
        }
        assert!(Instant::now() < deadline, "le fond final n'est pas arrivé");
        std::thread::sleep(Duration::from_millis(2));
    }
}

// ------------------------------------------------------------------ durées (scénario d'Anthony)

pub(crate) fn geste(n: usize, dx: f32, dy: f32) -> Vec<(Pos2, f32, f64)> {
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            let p = pos2(30.0 + dx + t * 300.0, 90.0 + dy + (t * std::f32::consts::TAU).sin() * 40.0);
            (p, 0.1 + 0.85 * (t * std::f32::consts::PI).sin(), k as f64 / 200.0)
        })
        .collect()
}

pub(crate) fn trait_de(s: &BrushSettings, lib: &BrushLibrary, pts: &[(Pos2, f32, f64)]) -> Element {
    let mut raster = Raster::new(1920, 1080);
    let mut st = StrokeState::new(0, StrokeMode::Paint([20, 20, 20]), *s, None, true).with_textures(lib.textures(s)).with_feel(Toucher::default());
    let mut dirty = Vec::new();
    for &(p, pr, t) in pts {
        st.add_point_at(&mut raster, p, pr, t, &mut dirty);
    }
    st.finish(&mut raster, &mut dirty);
    let trace = Trace { pressure: st.path_pressure.clone(), time: st.path_time.clone(), feel: Some(Toucher::default()), secondary: None, cache: Default::default() };
    Element::Stroke { points: st.path.clone(), color: [20, 20, 20], opacity: s.opacity, hardness: s.hardness, brush: lib.for_vector(s).map(Box::new), trace }
}

pub(crate) fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn plan_of(id: CelId, r: FondReglages) -> Plan {
    let line = PlanItem { fond: None, ..fond_item(id, r) };
    Plan { paper: Some([255, 255, 255]), items: vec![fond_item(id, r), line] }
}

/// Un nouveau trait après l'autre sur un calque à fond (dessin 1920×1080) : durée du rendu du plan
/// sur le fil de l'interface, au trait 1, 10, 30 et 60 ; `differe` : comme à l'écran (fond calculé
/// en arrière-plan), sinon comme avant (calcul direct).
fn serie(elements: &[Element], differe: bool, couleur: [u8; 3]) -> Vec<(usize, f64, f64, f64)> {
    let mut doc = Document::new(1920, 1080, 24);
    let id = doc.new_cel(LayerKind::Vector);
    doc.cels.insert(id, Cel::new(1920, 1080, LayerKind::Vector));
    let r = FondReglages { couleur, ..Default::default() };
    let plan = plan_of(id, r);
    let mut out = Vec::new();
    for (k, e) in elements.iter().enumerate() {
        let cel = doc.cels.get_mut(&id).unwrap();
        cel.vector.as_mut().unwrap().elements.push(e.clone());
        cel.rerender_region(e.bounds().expand(2.0));
        let t = Instant::now();
        let plan_ms = if differe { fx::differe(|| render::composite_full(&doc, &plan)) } else { render::composite_full(&doc, &plan) };
        let first_ms = ms(t);
        let _ = plan_ms;
        let t = Instant::now();
        let _ = fx::differe(|| render::composite_full(&doc, &plan));
        let again_ms = ms(t);
        let t = Instant::now();
        while fx::fond_pending() {
            fx::fond_poll();
            std::thread::sleep(Duration::from_millis(1));
        }
        fx::fond_poll();
        let wait_ms = ms(t);
        if [1, 10, 30, 60].contains(&(k + 1)) {
            out.push((k + 1, first_ms, again_ms, wait_ms));
        }
    }
    out
}

/// `cargo test --release --offline --target-dir target\audit fond_background -- --ignored --nocapture`
#[test]
#[ignore]
fn fond_background_keeps_the_ui_thread_free() {
    monde::lire();
    let lib = BrushLibrary::builtin();
    lib.share();
    let s = BrushSettings::graphite();
    let elements: Vec<Element> = (0..60).map(|k| trait_de(&s, &lib, &geste(200, 100.0 + (k % 5) as f32 * 300.0, 60.0 + (k / 5) as f32 * 75.0))).collect();
    println!("rendu du plan (composition 1920×1080 + fond) après le trait n, ms sur le fil de l'interface :");
    println!("  trait | avant (direct) | après (arrière-plan) | image suivante | arrivée du fond (fil de travail)");
    let before = serie(&elements, false, [250, 250, 240]);
    let after = serie(&elements, true, [250, 251, 240]);
    for (b, a) in before.iter().zip(&after) {
        println!("  {:>5} | {:>14.1} | {:>20.1} | {:>14.1} | {:>8.1}", b.0, b.1, a.1, a.2, a.3);
    }
    let worst = after.iter().map(|a| a.1).fold(0.0, f64::max);
    // Le plan composé coûte la composition seule (le fond est montré tel quel) ; le fond lui-même
    // sur le fil de l'interface se mesure plus bas.
    println!("  (composition seule, fond en cache : {:.1} ms)", after.last().map_or(0.0, |a| a.2));
    assert!(worst < 8.0, "rendu du plan : {worst:.1} ms sur le fil de l'interface");

    // Un glissé de 50 traits : une version par image.
    let mut doc = Document::new(1920, 1080, 24);
    let id = doc.new_cel(LayerKind::Vector);
    doc.cels.insert(id, Cel::new(1920, 1080, LayerKind::Vector));
    let r = FondReglages { couleur: [250, 252, 240], ..Default::default() };
    let item = fond_item(id, r);
    {
        let cel = doc.cels.get_mut(&id).unwrap();
        cel.vector.as_mut().unwrap().elements.extend(elements.iter().take(50).cloned());
        cel.rerender();
    }
    let _ = fx::differe(|| arc_of(&doc, &item));
    let originals = doc.cels[&id].vector.as_ref().unwrap().elements.clone();
    let mut shown_total = 0.0;
    let frames = 30;
    for f in 1..=frames {
        let aff = crate::vector::Affine::translate(vec2(3.4 * f as f32, 2.6 * f as f32));
        let cel = doc.cels.get_mut(&id).unwrap();
        let d = cel.vector.as_mut().unwrap();
        let mut region = Rect::NOTHING;
        for (i, src) in originals.iter().enumerate() {
            region = region.union(d.elements[i].bounds().expand(1.5));
            d.elements[i] = src.transformed_preview(&aff);
            region = region.union(d.elements[i].bounds().expand(1.5));
        }
        cel.rerender_region(region.expand(2.0));
        let t = Instant::now();
        let _ = fx::differe(|| arc_of(&doc, &item));
        shown_total += ms(t);
    }
    println!("glissé de 50 traits, fond (différé) : {:.2} ms par image", shown_total / frames as f64);
    assert!(shown_total / (frames as f64) < 8.0, "glissé : le fond coûte trop sur le fil de l'interface");
    // Le lâcher : la dernière version ; le fond exact arrive ensuite.
    let t = Instant::now();
    doc.cels.get_mut(&id).unwrap().version += 1;
    let _ = fx::differe(|| arc_of(&doc, &item));
    println!("lâcher, fond (différé) : {:.2} ms", ms(t));
    let t = Instant::now();
    while fx::fond_pending() {
        fx::fond_poll();
        std::thread::sleep(Duration::from_millis(1));
    }
    println!("  fond exact arrivé après {:.1} ms (fil de travail)", ms(t));

    // Avant : le même glissé, fond calculé tout de suite.
    let (mut direct_total, mut n) = (0.0, 0);
    let r2 = FondReglages { couleur: [250, 253, 240], ..Default::default() };
    let item2 = fond_item(id, r2);
    let _ = arc_of(&doc, &item2);
    for f in 1..=10 {
        let aff = crate::vector::Affine::translate(vec2(1.7 * f as f32, 1.3 * f as f32));
        let cel = doc.cels.get_mut(&id).unwrap();
        let d = cel.vector.as_mut().unwrap();
        for (i, src) in originals.iter().enumerate() {
            d.elements[i] = src.transformed_preview(&aff);
        }
        cel.rerender();
        let t = Instant::now();
        let _ = arc_of(&doc, &item2);
        direct_total += ms(t);
        n += 1;
    }
    println!("glissé de 50 traits, fond direct (avant) : {:.1} ms par image", direct_total / n as f64);
}
