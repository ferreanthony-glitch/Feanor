//! Mesures de vitesse des effets de calque (Anthony, 2026-10-18 : « trois clés d'animation sur un
//! calque vecto, un effet animé de flou, par-dessus un calque bitmap avec un bruit animé et une
//! option de fusion : ça rame à mort »). Les mesures sont des tests `#[ignore]` (à lancer à la
//! main : `cargo test --release --offline --target-dir target\msvc --lib fxperf -- --ignored
//! --nocapture --test-threads=1`).
//!
//! La scène reprend `TEST PELURE_effets` : feuille 3840×2160, cadre 1920×1080, 24 images/s ; le
//! calque « Dessin » (vecto, 47 images) passe par un flou gaussien animé (rayon 8 → 50 → 130 → 70 →
//! 120 aux images 1, 7, 22, 31, 36) puis luminosité / contraste (80, −45) ; par-dessus, « Calque 2 »
//! (bitmap, opacité 0,6, fusion Densité linéaire +) passe par un bruit animé (0 → 45 % de l'image 1
//! à l'image 45) puis teinte / saturation (10, −80, 0). Si le projet copié n'est pas là, des
//! dessins fabriqués de la même ampleur les remplacent.

use std::path::Path;
use std::time::Instant;

use eframe::egui::Color32;

use crate::blend::BlendMode;
use crate::document::{Cel, Document, LayerKind};
use crate::fx::{self, Effect, EffectKind};
use crate::raster::Raster;
use crate::render;
use crate::vector::{Element, VPoint};

/// Le projet de l'utilisateur, copié en lecture seule (absent : dessins fabriqués).
const COPY: &str = r"C:\Users\ferre\AppData\Local\Temp\claude\C--Users-ferre-Documents-Work-BRAWLSTAR-BRAWLIDAYS-3D\ee9aebda-4869-418a-b35f-eaef9d687780\scratchpad\test-effets\dessins";

fn real_vector(id: u64, w: usize, h: usize) -> Option<Cel> {
    let text = std::fs::read_to_string(Path::new(COPY).join(format!("{id}.json"))).ok()?;
    let drawing: crate::vector::VectorDrawing = serde_json::from_str(&text).ok()?;
    Some(Cel { raster: drawing.render(w, h), vector: Some(drawing), version: 0, fond: None, absent: None })
}

fn real_bitmap(id: u64, w: usize, h: usize) -> Option<Cel> {
    let img = image::open(Path::new(COPY).join(format!("{id}.png"))).ok()?.to_rgba8();
    (img.width() as usize == w && img.height() as usize == h).then(|| Cel::bitmap(Raster::from_rgba(w, h, img.as_raw())))
}

/// Un dessin vecto fabriqué : des traits épais sur une bonne partie de la feuille.
fn fake_vector(w: usize, h: usize, seed: f32) -> Cel {
    let mut c = Cel::new(w, h, LayerKind::Vector);
    let d = c.vector.as_mut().unwrap();
    for k in 0..40 {
        let x = w as f32 * (0.15 + 0.5 * (k as f32 * 0.37 + seed).fract());
        let y = h as f32 * (0.15 + 0.7 * (k as f32 * 0.61 + seed).fract());
        let pts = (0..12).map(|i| VPoint { x: x + i as f32 * 40.0, y: y + (i as f32 * 0.7 + k as f32).sin() * 120.0, r: 6.0 }).collect();
        d.elements.push(Element::Stroke { points: pts, color: [30 + k as u8 * 4, 40, 90], opacity: 1.0, hardness: 0.8, brush: None, trace: Default::default() });
    }
    c.rerender();
    c
}

/// Un dessin bitmap fabriqué : un grand aplat en dégradé qui couvre le quart de la feuille.
fn fake_bitmap(w: usize, h: usize) -> Cel {
    let mut rgba = vec![0u8; w * h * 4];
    for y in h / 4..h * 3 / 4 {
        for x in w / 4..w * 3 / 4 {
            let i = (y * w + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, 120, 255]);
        }
    }
    Cel::bitmap(Raster::from_rgba(w, h, &rgba))
}

fn keyed(kind: EffectKind, keys: &[(usize, f32)], at: usize) -> Effect {
    let mut e = Effect::new(kind);
    for &(f, v) in keys {
        let mut values = e.values.clone();
        values[at] = v;
        e.set_key(f, values);
    }
    e
}

/// La scène de l'utilisateur. Renvoie le document (panneau 0, 47 images) et si les dessins sont
/// les vrais.
pub(crate) fn scene() -> (Document, bool) {
    let (w, h) = (3840, 2160);
    let mut doc = Document::with_frame(1920, 1080, 24);
    assert_eq!((doc.width, doc.height), (w, h));
    let real_a = real_vector(8, w, h);
    let real_b = real_vector(18, w, h);
    let real_n = real_bitmap(20, w, h);
    let real = real_a.is_some() && real_b.is_some() && real_n.is_some();
    let a = doc.new_cel(LayerKind::Vector);
    doc.cels.insert(a, real_a.unwrap_or_else(|| fake_vector(w, h, 0.1)));
    let b = doc.new_cel(LayerKind::Vector);
    doc.cels.insert(b, real_b.unwrap_or_else(|| fake_vector(w, h, 0.6)));
    let n = doc.new_cel(LayerKind::Bitmap);
    doc.cels.insert(n, real_n.unwrap_or_else(|| fake_bitmap(w, h)));

    let mut draw = crate::document::Layer::new("Dessin", LayerKind::Vector);
    draw.frames = (0..47).map(|f| Some(if f < 15 { a } else { b })).collect();
    draw.fx = vec![
        keyed(EffectKind::GaussianBlur, &[(1, 8.0), (7, 50.0), (22, 130.0), (31, 70.0), (36, 120.0)], 0),
        {
            let mut e = Effect::new(EffectKind::BrightnessContrast);
            e.values = vec![80.0, -45.0];
            e
        },
    ];
    let mut noise = crate::document::Layer::new("Calque 2", LayerKind::Bitmap);
    noise.frames = vec![Some(n); 47];
    noise.opacity = 0.6;
    noise.blend = BlendMode::LinearBurn;
    noise.fx = vec![keyed(EffectKind::Noise, &[(1, 0.0), (45, 45.0)], 0), {
        let mut e = Effect::new(EffectKind::HueSaturation);
        e.values = vec![10.0, -80.0, 0.0];
        e
    }];
    doc.panels[0].layers = vec![draw, noise];
    (doc, real)
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn stats(label: &str, v: &mut [f64]) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    println!("{label:<48} moy {mean:8.1} ms  méd {:8.1}  pire {:8.1}  ({} mesures)", v[v.len() / 2], v[v.len() - 1], v.len());
}

/// Ce que le programme fait pour afficher une image : les dessins dérivés de chaque élément du
/// plan (la partie coûteuse, sur le fil de l'interface avant ce chantier).
fn display_cost(doc: &Document, frame: usize) -> f64 {
    let plan = render::panel_plan_at(doc, 0, frame, doc.render_paper());
    let t = Instant::now();
    for item in &plan.items {
        let _ = fx::shown(doc, item);
    }
    ms(t)
}

#[test]
#[ignore]
fn mesure_scene_calcul_bloquant() {
    let (doc, real) = scene();
    println!("scène : dessins {}", if real { "réels" } else { "fabriqués" });
    let cam = doc.default_camera();
    // Chauffe (premières allocations, fils).
    let _ = display_cost(&doc, 0);
    // Une image, à froid, par élément puis composée (export).
    fx::clear_cache();
    let plan = render::panel_plan_at(&doc, 0, 22, doc.render_paper());
    for (i, item) in plan.items.iter().enumerate() {
        let t = Instant::now();
        let _ = fx::shown(&doc, item);
        println!("image 22, élément {i} ({} effets) à froid : {:.1} ms", item.fx.len(), ms(t));
    }
    let t = Instant::now();
    let _ = render::camera_image(&doc, &plan, &cam);
    println!("image 22 composée, caches chauds : {:.1} ms", ms(t));
    fx::clear_cache();
    let t = Instant::now();
    let _ = render::camera_image(&doc, &plan, &cam);
    println!("image 22 composée à froid (export) : {:.1} ms", ms(t));

    // Lecture : 47 images à la suite, deux tours (le second montre si le cache tient).
    fx::clear_cache();
    for lap in 0..2 {
        let mut v: Vec<f64> = (0..47).map(|f| display_cost(&doc, f)).collect();
        stats(&format!("lecture, tour {} (par image)", lap + 1), &mut v);
    }
    // Défilement : les images dans le désordre, à froid.
    fx::clear_cache();
    let order: Vec<usize> = (0..47).map(|k| (k * 17 + 5) % 47).collect();
    let mut v: Vec<f64> = order.iter().map(|&f| display_cost(&doc, f)).collect();
    stats("défilement dans le désordre, à froid", &mut v);
    // Curseur : le rayon du flou change d'un cran à chaque mouvement, image fixe.
    let mut doc2 = doc;
    let mut v = Vec::new();
    for r in 0..40 {
        doc2.panels[0].layers[0].fx[0].values = vec![20.0 + r as f32 * 2.0];
        doc2.panels[0].layers[0].fx[0].keys.clear();
        v.push(display_cost(&doc2, 22));
    }
    stats("curseur du flou (rayon change, image fixe)", &mut v);
    let mut v = Vec::new();
    for q in 0..40 {
        doc2.panels[0].layers[1].fx[0].values = vec![q as f32 + 1.0];
        doc2.panels[0].layers[1].fx[0].keys.clear();
        v.push(display_cost(&doc2, 22));
    }
    stats("curseur du bruit (quantité change, image fixe)", &mut v);
}

/// Ce que fait l'écran à chaque image d'écran avec le calcul de fond : demander l'image affichée
/// (`fx::display`) et les 24 qui suivent (`fx::prefetch`) ; on mesure le temps pris sur le fil de
/// l'interface et la part d'images affichées avec leurs effets prêts.
fn ui_frame(doc: &Document, frame: usize, level: u8, ahead: usize) -> (f64, bool) {
    let t = Instant::now();
    let plan = render::panel_plan_at(doc, 0, frame, doc.render_paper());
    let mut ready = true;
    for item in &plan.items {
        if let Some(d) = fx::display(doc, item, level) {
            ready &= d.ready;
        }
    }
    for k in 1..=ahead {
        let next = render::panel_plan_at(doc, 0, (frame + k) % 47, None);
        fx::prefetch(doc, &next.items, k as u32, level);
    }
    (ms(t), ready)
}

/// Lecture de 47 images à 24 images/s sur l'horloge, l'écran redessiné toutes les 8 ms : deux tours
/// (à froid, puis cache chaud), pour la pleine résolution (`level` 0) et l'aperçu (`level` 1).
fn play_simulation(doc: &Document, level: u8) {
    let ctx = eframe::egui::Context::default();
    fx::clear_cache();
    for lap in 0..2 {
        let start = Instant::now();
        let (mut cost, mut ready_frames, mut shown) = (Vec::new(), 0usize, 0usize);
        let mut distinct_ready = std::collections::BTreeSet::new();
        while start.elapsed().as_secs_f64() < 47.0 / 24.0 {
            crate::fxcache::begin_frame(&ctx);
            let frame = ((start.elapsed().as_secs_f64() * 24.0) as usize).min(46);
            let (c, ready) = ui_frame(doc, frame, level, 24);
            cost.push(c);
            shown += 1;
            if ready {
                ready_frames += 1;
                distinct_ready.insert(frame);
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        let total = cost.iter().sum::<f64>();
        let n = cost.len();
        cost.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "lecture {} tour {} : fil de l'interface moy {:.2} ms / image d'écran, p95 {:.2}, pire {:.2} ; images d'écran prêtes {}/{} ; images du plan montrées avec leurs effets {}/47 ({:.1} sur 24 par s.) ; total {:.0} ms",
            if level == 0 { "pleine résolution" } else { "aperçu 1/2" },
            lap + 1,
            total / n as f64,
            cost[(n as f64 * 0.95) as usize],
            cost[n - 1],
            ready_frames,
            shown,
            distinct_ready.len(),
            distinct_ready.len() as f64 / (47.0 / 24.0),
            total
        );
        crate::fxcache::settle();
    }
}

#[test]
#[ignore]
fn mesure_scene_calcul_de_fond() {
    let (doc, real) = scene();
    println!("scène : dessins {}", if real { "réels" } else { "fabriqués" });
    // Chauffe (premières allocations, fils).
    let _ = display_cost(&doc, 0);
    for level in [0, 1] {
        play_simulation(&doc, level);
    }

    // Défilement : on saute d'une image à l'autre toutes les 40 ms, dans le désordre, à froid.
    let ctx = eframe::egui::Context::default();
    for level in [0u8, 1] {
        fx::clear_cache();
        let mut cost = Vec::new();
        let mut ready = 0;
        let order: Vec<usize> = (0..40).map(|k| (k * 17 + 5) % 47).collect();
        for &f in &order {
            crate::fxcache::begin_frame(&ctx);
            let (c, r) = ui_frame(&doc, f, level, 2);
            cost.push(c);
            ready += r as usize;
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        stats(&format!("défilement, niveau {level} : fil de l'interface par image d'écran"), &mut cost);
        println!("  images prêtes tout de suite : {ready}/40");
        crate::fxcache::settle();
    }

    // Curseur : le rayon du flou change toutes les 16 ms ; image fixe 22. On note le temps du fil de
    // l'interface, et le délai entre un mouvement du curseur et l'arrivée de son résultat.
    let mut doc2 = doc;
    for level in [0u8, 1] {
        fx::clear_cache();
        let (mut cost, mut latency) = (Vec::new(), Vec::new());
        for r in 0..40 {
            crate::fxcache::begin_frame(&ctx);
            doc2.panels[0].layers[0].fx[0].values = vec![20.0 + r as f32 * 2.0];
            doc2.panels[0].layers[0].fx[0].keys.clear();
            let t = Instant::now();
            let (c, _) = ui_frame(&doc2, 22, level, 0);
            cost.push(c);
            // Attente du résultat de ce cran (le curseur réel continuerait d'avancer).
            let plan = render::panel_plan_at(&doc2, 0, 22, None);
            while !plan.items.iter().all(|i| fx::display(&doc2, i, level).is_none_or(|d| d.ready)) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            latency.push(ms(t));
        }
        stats(&format!("curseur du flou, niveau {level} : fil de l'interface"), &mut cost);
        stats(&format!("curseur du flou, niveau {level} : délai jusqu'au résultat"), &mut latency);
        let (mut cost, mut latency) = (Vec::new(), Vec::new());
        for q in 0..40 {
            crate::fxcache::begin_frame(&ctx);
            doc2.panels[0].layers[1].fx[0].values = vec![q as f32 + 1.0];
            doc2.panels[0].layers[1].fx[0].keys.clear();
            let t = Instant::now();
            let (c, _) = ui_frame(&doc2, 22, level, 0);
            cost.push(c);
            let plan = render::panel_plan_at(&doc2, 0, 22, None);
            while !plan.items.iter().all(|i| fx::display(&doc2, i, level).is_none_or(|d| d.ready)) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            latency.push(ms(t));
        }
        stats(&format!("curseur du bruit, niveau {level} : fil de l'interface"), &mut cost);
        stats(&format!("curseur du bruit, niveau {level} : délai jusqu'au résultat"), &mut latency);
    }
}

#[test]
#[ignore]
fn debug_curseur() {
    let (mut doc, _) = scene();
    let ctx = eframe::egui::Context::default();
    for q in 0..8 {
        crate::fxcache::begin_frame(&ctx);
        doc.panels[0].layers[1].fx[0].values = vec![q as f32 + 1.0];
        doc.panels[0].layers[1].fx[0].keys.clear();
        let t = Instant::now();
        let (c, _) = ui_frame(&doc, 22, 0, 0);
        let plan = render::panel_plan_at(&doc, 0, 22, None);
        let mut spins = 0;
        while !plan.items.iter().all(|i| fx::display(&doc, i, 0).is_none_or(|d| d.ready)) {
            std::thread::sleep(std::time::Duration::from_millis(1));
            spins += 1;
        }
        let ready_at = ms(t);
        let states: Vec<bool> = plan.items.iter().map(|i| fx::display(&doc, i, 0).is_some_and(|d| d.ready)).collect();
        println!("tick {q}: ui {c:.2} ms, prêt après {ready_at:.1} ms ({spins} tours), éléments prêts {states:?}");
    }
}

/// Une vignette de la bande de montage (192 × 108) de la scène, effets déjà calculés : les dessins
/// sont cherchés une fois (maintenant) ou une fois par pixel lu (avant, `composite_pixel` seul).
#[test]
#[ignore]
fn mesure_vignette() {
    let (doc, _) = scene();
    let plan = render::panel_plan_at(&doc, 0, 22, Some(doc.paper));
    let cam = doc.default_camera();
    for item in &plan.items {
        let _ = fx::shown(&doc, item);
    }
    let (tw, th) = (192usize, 108usize);
    let mut now = Vec::new();
    for _ in 0..10 {
        let t = Instant::now();
        let _ = render::thumbnail(&doc, &plan, &cam, tw, th);
        now.push(ms(t));
    }
    stats("vignette 192×108, dessins cherchés une fois", &mut now);
    // Avant : une recherche par pixel lu (4 par pixel de vignette).
    let mut before = Vec::new();
    for _ in 0..3 {
        let t = Instant::now();
        let mut acc = 0u32;
        for y in 0..th * 2 {
            for x in 0..tw * 2 {
                acc += render::composite_pixel(&doc, &plan, 960 + x * 4, 540 + y * 4).a() as u32;
            }
        }
        std::hint::black_box(acc);
        before.push(ms(t));
    }
    stats("même nombre de pixels, une recherche par pixel (avant)", &mut before);
}

/// Le même calcul fait en bloquant, puis par le fil de fond : où part la différence ?
#[test]
#[ignore]
fn mesure_fil_de_fond_contre_bloquant() {
    let (doc, _) = scene();
    let plan = render::panel_plan_at(&doc, 0, 22, None);
    let item = plan.items[1].clone();
    let mut doc = doc;
    let (mut blocking, mut background) = (Vec::new(), Vec::new());
    for q in 1..13 {
        let mut it = item.clone();
        it.fx[0].values = vec![q as f32 * 3.0 + 0.1];
        let t = Instant::now();
        let _ = fx::shown(&doc, &it);
        blocking.push(ms(t));
        it.fx[0].values = vec![q as f32 * 3.0 + 0.2];
        let t = Instant::now();
        let _ = fx::display(&doc, &it, 0);
        let t_call = ms(t);
        crate::fxcache::settle();
        background.push(ms(t));
        if q == 1 {
            println!("premier appel de display : {t_call:.1} ms (copie du dessin d'origine)");
        }
    }
    doc.panels[0].layers.clear();
    stats("bloquant (fil appelant, tous les cœurs)", &mut blocking);
    stats("fil de fond (14 fils), de la demande au résultat", &mut background);
}

/// Rectangle (en tuiles) qu'occupent les tuiles non vides d'un dessin.
fn tile_box(r: &Raster) -> String {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for (i, t) in r.tiles.iter().enumerate() {
        if t.is_some() {
            let (x, y) = (i % r.tiles_x, i / r.tiles_x);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    format!("tuiles x {x0}..={x1}, y {y0}..={y1} sur {} × {}", r.tiles_x, r.tiles.len() / r.tiles_x)
}

/// Où va le temps : chaque effet de la scène, l'un après l'autre, sur le dessin de son calque.
#[test]
#[ignore]
fn mesure_effets_un_par_un() {
    let (doc, _) = scene();
    for (id, cel) in &doc.cels {
        println!("dessin {id} : {} tuiles non vides, {}", cel.raster.tiles.iter().flatten().count(), tile_box(&cel.raster));
    }
    let plan = render::panel_plan_at(&doc, 0, 22, doc.render_paper());
    let _: Color32 = Color32::TRANSPARENT;
    for item in &plan.items {
        let cel = &doc.cels[&item.cel];
        let mut cur = cel.raster.clone();
        let t = Instant::now();
        println!("dessin {} :", item.cel);
        for s in &item.fx {
            let t1 = Instant::now();
            cur = fx::apply(&cur, std::slice::from_ref(s));
            println!("  {:?} {:?} : {:.1} ms", s.kind, s.values, ms(t1));
        }
        println!("  total {:.1} ms", ms(t));
    }
}
