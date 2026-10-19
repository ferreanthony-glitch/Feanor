//! Mesures de vitesse des effets sur la carte graphique (`gpufx.rs`), sur les deux scènes d'Anthony à
//! pleine résolution (feuille 3840 × 2160, cadre 1920 × 1080, 24 images/s) : celle de `fxperf_tests`
//! (flou + luminosité sur le dessin ; bruit + teinte / saturation + fusion sur le bitmap) et celle de
//! `effetsperf_tests` (image d'impact, lueur, secousse avec flou de bougé, timecode, en calques
//! d'ajustement). Des essais `#[ignore]` à lancer à la main :
//!
//! `cargo test --release --offline --target-dir target\msvc --lib gpufxperf -- --ignored --nocapture --test-threads=1`
//!
//! Ce qui est mesuré à chaque image : la préparation sur le fil de l'interface (`GpuCanvas::sync`,
//! envoi des tuiles, plan), les passes d'effets et la composition sur la carte, l'affichage d'un écran de
//! 1920 × 1080, et l'attente de la fin du travail de la carte (`Device::poll(Wait)`). La lecture réelle
//! est donc au moins aussi rapide que ce qu'on lit ici (l'attente se recouvre avec l'interface).

use std::time::Instant;

use eframe::egui::epaint::{ClippedPrimitive, Primitive};
use eframe::egui::{pos2, vec2, Rect};
use eframe::egui_wgpu::{wgpu, RenderState, ScreenDescriptor};

use crate::document::Document;
use crate::gpu::GpuCanvas;
use crate::gpu_tests::headless;
use crate::render;

const SCREEN: (u32, u32) = (1920, 1080);

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn summary(label: &str, v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    println!("{label:<52} moy {mean:7.2} ms ({:5.1} images/s)  méd {:7.2}  p95 {:7.2}  pire {:7.2}  ({} mesures)", 1000.0 / mean, v[v.len() / 2], v[(v.len() as f64 * 0.95) as usize], v[v.len() - 1], v.len());
    mean
}

/// Dessine l'image comme l'écran le fait, sans la relire, et attend la carte.
fn draw(rs: &RenderState, target: &wgpu::TextureView, rect: Rect, callback: eframe::egui::PaintCallback) {
    let jobs = [ClippedPrimitive { clip_rect: rect, primitive: Primitive::Callback(callback) }];
    let screen = ScreenDescriptor { size_in_pixels: [SCREEN.0, SCREEN.1], pixels_per_point: 1.0 };
    let mut encoder = rs.device.create_command_encoder(&Default::default());
    let extra = rs.renderer.write().update_buffers(&rs.device, &rs.queue, &mut encoder, &jobs, &screen);
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("écran"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        rs.renderer.read().render(&mut pass, &jobs, &screen);
    }
    rs.queue.submit(extra.into_iter().chain([encoder.finish()]));
    rs.device.poll(wgpu::Maintain::Wait);
}

struct Bench {
    rs: RenderState,
    gpu: GpuCanvas,
    target: wgpu::TextureView,
}

impl Bench {
    fn new() -> Option<Bench> {
        let rs = headless()?;
        println!("carte graphique : {}", rs.adapter.get_info().name);
        let gpu = GpuCanvas::new(&rs);
        let texture = rs.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("écran de mesure"),
            size: wgpu::Extent3d { width: SCREEN.0, height: SCREEN.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: rs.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target = texture.create_view(&Default::default());
        Some(Bench { rs, gpu, target })
    }

    /// Une image d'écran : `sync`, commande de dessin, exécution, attente. Rend les millisecondes.
    fn frame(&mut self, doc: &Document, frame: usize) -> f64 {
        let plan = render::panel_plan_at(doc, 0, frame, doc.render_paper());
        let t = Instant::now();
        let resolved = self.gpu.sync(doc, &plan, false, &[], &[], 0);
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(SCREEN.0 as f32, SCREEN.1 as f32));
        let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
        let callback = self.gpu.paint(&plan, &resolved, corners, None, rect, [doc.width, doc.height]);
        draw(&self.rs, &self.target, rect, callback);
        ms(t)
    }
}

/// Lit la lecture : trois tours de toutes les images du plan, le premier (envoi des dessins) à part.
fn playback(bench: &mut Bench, doc: &Document, name: &str) -> f64 {
    let images = doc.panels[0].frame_count();
    let first: Vec<f64> = (0..images).map(|f| bench.frame(doc, f)).collect();
    println!("{name} : premier tour (envoi des dessins, compilation des shaders) : {:.0} ms en tout", first.iter().sum::<f64>());
    let before = bench.gpu.effects_passes();
    let mut laps: Vec<f64> = Vec::new();
    for _ in 0..3 {
        laps.extend((0..images).map(|f| bench.frame(doc, f)));
    }
    let passes = (bench.gpu.effects_passes() - before) as f64 / (3 * images) as f64;
    let mean = summary(&format!("{name} : lecture, pleine résolution"), &mut laps);
    println!("  {passes:.1} passes d'effets par image ; mémoire vidéo : dessins {:.0} Mo, effets {:.0} Mo", bench.gpu.stats().bytes as f64 / 1e6, bench.gpu.effects_bytes() as f64 / 1e6);
    mean
}

#[test]
#[ignore]
fn mesure_lecture_gpu_scene_flou_bruit() {
    let Some(mut bench) = Bench::new() else { return };
    let (doc, real) = crate::fxperf_tests::scene();
    println!("scène de flou + bruit + teinte / saturation + fusion, dessins {}", if real { "réels" } else { "fabriqués" });
    let mean = playback(&mut bench, &doc, "flou + luminosité / bruit + teinte + fusion");
    assert!(mean < 1000.0 / 24.0, "{mean:.1} ms par image, il en faut moins de 41,7 pour 24 images/s");
}

#[test]
#[ignore]
fn mesure_lecture_gpu_scene_impact_lueur_secousse() {
    let Some(mut bench) = Bench::new() else { return };
    let doc = crate::effetsperf_tests::scene_with_effects();
    let mean = playback(&mut bench, &doc, "impact + lueur + secousse + timecode");
    // Les images chargées : tremblement + lueur + timecode (20 à 43).
    let mut busy: Vec<f64> = Vec::new();
    for _ in 0..3 {
        busy.extend((20..44).map(|f| bench.frame(&doc, f)));
    }
    let busy_mean = summary("  images chargées (secousse avec flou + lueur)", &mut busy);
    assert!(mean < 1000.0 / 24.0 && busy_mean < 1000.0 / 24.0);
}

#[test]
#[ignore]
fn mesure_curseur_gpu() {
    let Some(mut bench) = Bench::new() else { return };
    let (mut doc, _) = crate::fxperf_tests::scene();
    for f in 0..3 {
        bench.frame(&doc, 22 + f);
    }
    // Le rayon du flou change à chaque cran, image fixe : le délai entre le mouvement et l'image finie.
    let mut v = Vec::new();
    for r in 0..40 {
        doc.panels[0].layers[0].fx[0].values = vec![20.0 + r as f32 * 2.0];
        doc.panels[0].layers[0].fx[0].keys.clear();
        v.push(bench.frame(&doc, 22));
    }
    summary("curseur du flou (rayon change, image fixe)", &mut v);
    let mut v = Vec::new();
    for q in 0..40 {
        doc.panels[0].layers[1].fx[0].values = vec![q as f32 + 1.0];
        doc.panels[0].layers[1].fx[0].keys.clear();
        v.push(bench.frame(&doc, 22));
    }
    summary("curseur du bruit (quantité change, image fixe)", &mut v);
}

/// Où va le temps : chaque effet seul sur le grand bitmap (3840 × 2160, 859 tuiles) puis sur toute la feuille.
#[test]
#[ignore]
fn mesure_un_effet_a_la_fois_gpu() {
    use crate::fx::EffectKind;
    let Some(mut bench) = Bench::new() else { return };
    let (mut doc, _) = crate::fxperf_tests::scene();
    // Un seul calque : le bitmap.
    doc.panels[0].layers.remove(0);
    let list: [(&str, EffectKind, Vec<f32>); 14] = [
        ("flou gaussien 40 px", EffectKind::GaussianBlur, vec![40.0]),
        ("flou gaussien 130 px", EffectKind::GaussianBlur, vec![130.0]),
        ("luminosité / contraste", EffectKind::BrightnessContrast, vec![30.0, 20.0]),
        ("niveaux", EffectKind::Levels, vec![20.0, 230.0, 1.4]),
        ("teinte / saturation", EffectKind::HueSaturation, vec![20.0, -30.0, 0.0]),
        ("bruit", EffectKind::Noise, vec![40.0]),
        ("netteté", EffectKind::Sharpen, vec![100.0, 2.0]),
        ("ombre portée", EffectKind::DropShadow, vec![20.0, 20.0, 12.0, 60.0]),
        ("lueur", EffectKind::Glow, vec![20.0, 100.0]),
        ("flou de mouvement 120 px", EffectKind::MotionBlur, vec![120.0, 30.0]),
        ("secousse avec flou de bougé", EffectKind::Shake, vec![40.0, 20.0, 1.0, 102.0, 60.0]),
        ("lueur diffuse 60 px", EffectKind::Bloom, vec![60.0, 120.0, 140.0]),
        ("image d'impact", EffectKind::Impact, vec![100.0, 0.0, 128.0, 230.0, 30.0, 30.0, 40.0, 60.0, 50.0, 50.0, 50.0, 3.0]),
        ("aberration chromatique", EffectKind::Aberration, vec![8.0, 50.0, 50.0]),
    ];
    for (name, kind, values) in list {
        doc.panels[0].layers[0].fx = vec![crate::fx::Effect { kind, enabled: true, values: values.clone(), keys: Vec::new() }];
        for _ in 0..2 {
            bench.frame(&doc, 22);
        }
        let mut v: Vec<f64> = (0..10).map(|_| bench.frame(&doc, 22)).collect();
        // (Les réglages ne changent pas : la pile n'est dessinée qu'une fois. On force le redessin.)
        let mut w = Vec::new();
        let sign = if values[0] + 0.5 > kind.params()[0].max { -1.0 } else { 1.0 };
        for k in 0..10 {
            let e = &mut doc.panels[0].layers[0].fx[0];
            e.values[0] = values[0] + sign * 0.01 * (k + 1) as f32;
            w.push(bench.frame(&doc, 22));
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        w.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("{name:<32} image fixe (composition gardée) {:6.2} ms ; effet redessiné {:6.2} ms", v[v.len() / 2], w[w.len() / 2]);
    }
}

/// Le pire cas : un dessin qui couvre toute la feuille de 3840 × 2160, six effets lourds à la suite.
#[test]
#[ignore]
fn mesure_pire_cas_gpu() {
    use crate::document::{Cel, LayerKind};
    use crate::fx::{Effect, EffectKind};
    use crate::raster::Raster;
    let Some(mut bench) = Bench::new() else { return };
    let (w, h) = (3840usize, 2160usize);
    let mut doc = Document::with_frame(1920, 1080, 24);
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, ((x ^ y) & 255) as u8, 255]);
        }
    }
    let id = doc.new_cel(LayerKind::Bitmap);
    doc.cels.insert(id, Cel::bitmap(Raster::from_rgba(w, h, &rgba)));
    let mut layer = crate::document::Layer::new("Plein cadre", LayerKind::Bitmap);
    layer.frames = vec![Some(id); 24];
    let stacks: [(&str, Vec<(EffectKind, Vec<f32>)>); 3] = [
        ("flou 190 px", vec![(EffectKind::GaussianBlur, vec![190.0])]),
        ("flou 100 + lueur 60 + lueur diffuse 150 + secousse avec flou + teinte + bruit", vec![(EffectKind::GaussianBlur, vec![100.0]), (EffectKind::Glow, vec![60.0, 100.0]), (EffectKind::Bloom, vec![150.0, 120.0, 140.0]), (EffectKind::Shake, vec![30.0, 10.0, 1.0, 102.0, 60.0]), (EffectKind::HueSaturation, vec![20.0, -10.0, 0.0]), (EffectKind::Noise, vec![30.0])]),
        ("flou de mouvement 300 + ombre portée + netteté", vec![(EffectKind::MotionBlur, vec![300.0, 30.0]), (EffectKind::DropShadow, vec![30.0, 30.0, 40.0, 60.0]), (EffectKind::Sharpen, vec![100.0, 4.0])],),
    ];
    doc.panels[0].layers = vec![layer];
    for (name, stack) in stacks {
        doc.panels[0].layers[0].fx = stack.into_iter().map(|(kind, values)| Effect { kind, enabled: true, values, keys: Vec::new() }).collect();
        for _ in 0..2 {
            bench.frame(&doc, 3);
        }
        let mut v = Vec::new();
        for k in 0..12 {
            let e = &mut doc.panels[0].layers[0].fx[0];
            e.values[0] += 0.01 * (k + 1) as f32;
            v.push(bench.frame(&doc, 3));
        }
        summary(&format!("pire cas, {name}"), &mut v);
        println!("  mémoire vidéo : dessins {:.0} Mo, effets {:.0} Mo", bench.gpu.stats().bytes as f64 / 1e6, bench.gpu.effects_bytes() as f64 / 1e6);
    }
}
