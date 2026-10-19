//! La carte graphique dessine les feuilles flottantes déformées par un maillage (`warp.rs`,
//! `gpu.rs`) comme le calcul de secours : essai sans fenêtre, sur le périphérique par défaut de la
//! machine (l'essai se passe s'il n'y en a aucun). Les mêmes pixels, au lissage de la carte près.

use std::future::Future;
use std::sync::Arc;

use eframe::egui::epaint::{ClippedPrimitive, Primitive};
use eframe::egui::mutex::RwLock;
use eframe::egui::{pos2, vec2, Color32, Rect};
use eframe::egui_wgpu::{wgpu, Renderer, RenderState, ScreenDescriptor};

use crate::affichage_tests::{doc_with, gros_traits, line_item};
use crate::blend::BlendMode;
use crate::document::Document;
use crate::envelope::Envelope;
use crate::flottante::Flottante;
use crate::gpu::GpuCanvas;
use crate::render::{self, Plan};
use crate::vector::Element;

fn block_on<F: Future>(f: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    let mut f = std::pin::pin!(f);
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// Un périphérique sans fenêtre, ou `None` (aucune carte graphique).
pub(crate) fn headless() -> Option<RenderState> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    let (device, queue) = block_on(adapter.request_device(
        &wgpu::DeviceDescriptor { label: Some("essai"), required_features: wgpu::Features::empty(), required_limits: adapter.limits(), memory_hints: wgpu::MemoryHints::Performance },
        None,
    ))
    .ok()?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let renderer = Renderer::new(&device, format, None, 1, false);
    Some(RenderState { adapter, available_adapters: Vec::new(), device, queue, target_format: format, renderer: Arc::new(RwLock::new(renderer)) })
}

/// Ce que la carte graphique affiche du plan, 1 pixel d'écran pour 1 pixel du dessin.
fn render_on_gpu(rs: &RenderState, doc: &Document, plan: &Plan) -> Vec<Color32> {
    let mut gpu = GpuCanvas::new(rs);
    render_with(&mut gpu, rs, doc, plan, true)
}

/// Comme `render_on_gpu`, avec une carte déjà en route (ses textures gardées d'une image à l'autre) ;
/// `all` : tout a pu changer.
pub(crate) fn render_with(gpu: &mut GpuCanvas, rs: &RenderState, doc: &Document, plan: &Plan, all: bool) -> Vec<Color32> {
    render_with_tiles(gpu, rs, doc, plan, all, &[])
}

/// Comme `render_with`, avec les tuiles touchées depuis l'image précédente (un trait en train de se
/// poser : `GpuCanvas::sync` n'envoie alors que celles-là).
pub(crate) fn render_with_tiles(gpu: &mut GpuCanvas, rs: &RenderState, doc: &Document, plan: &Plan, all: bool, tiles: &[usize]) -> Vec<Color32> {
    let (w, h) = (doc.width as u32, doc.height as u32);
    let resolved = gpu.sync(doc, plan, all, tiles, &[], 0);
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(w as f32, h as f32));
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    let callback = gpu.paint(plan, &resolved, corners, None, rect, [doc.width, doc.height]);
    read_callback(rs, w, h, rect, callback)
}

/// Ce que la carte graphique affiche pour une commande de dessin (`GpuCanvas::paint`, `paint_mix`) :
/// un écran de `w` × `h` pixels, lu en entier.
pub(crate) fn read_callback(rs: &RenderState, w: u32, h: u32, rect: Rect, callback: eframe::egui::PaintCallback) -> Vec<Color32> {
    let jobs = [ClippedPrimitive { clip_rect: rect, primitive: Primitive::Callback(callback) }];
    let screen = ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: 1.0 };
    let target = rs.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("écran"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: rs.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut encoder = rs.device.create_command_encoder(&Default::default());
    let extra = rs.renderer.write().update_buffers(&rs.device, &rs.queue, &mut encoder, &jobs, &screen);
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("écran"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        rs.renderer.read().render(&mut pass, &jobs, &screen);
    }
    let row = (w * 4).div_ceil(256) * 256;
    let readback = rs.device.create_buffer(&wgpu::BufferDescriptor { label: Some("lecture"), size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    rs.queue.submit(extra.into_iter().chain([encoder.finish()]));
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("lecture"));
    rs.device.poll(wgpu::Maintain::Wait);
    let data = slice.get_mapped_range();
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = y * row as usize + x * 4;
            out.push(Color32::from_rgba_premultiplied(data[i], data[i + 1], data[i + 2], data[i + 3]));
        }
    }
    out
}

/// Pixels qui diffèrent de plus de `tol` niveaux, et pixels nettement peints d'un côté seulement.
fn ecarts(a: &[Color32], b: &[Color32], tol: i32) -> (usize, usize, usize) {
    let far = a.iter().zip(b).filter(|(x, y)| x.to_array().iter().zip(y.to_array()).any(|(p, q)| (*p as i32 - q as i32).abs() > tol)).count();
    // (Sur le papier blanc : « peint » veut dire autre chose que du blanc, des deux côtés.)
    let ink = |v: &[Color32]| v.iter().filter(|c| c.g() < 200).count();
    let (ia, ib) = (ink(a), ink(b));
    assert!(ia > 3_000 && ib > 3_000, "la carte graphique a peint {ia} pixels, le calcul de secours {ib} : un des deux est vide");
    assert!((ia as f32 / ib as f32 - 1.0).abs() < 0.03, "encre : {ia} contre {ib}");
    (far, ia.abs_diff(ib), ib)
}

pub(crate) fn scene(w: usize, h: usize) -> (Document, crate::document::CelId, Vec<Element>, Rect) {
    let elements = gros_traits(14, w, h, 9);
    let src = elements.iter().fold(Rect::NOTHING, |r, e| r.union(e.bounds()));
    let (doc, id) = doc_with(w, h, elements.clone());
    (doc, id, elements, src)
}

#[test]
fn the_gpu_draws_a_meshed_sheet_like_the_fallback_does() {
    let Some(rs) = headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    println!("carte graphique de l'essai : {:?}", rs.adapter.get_info().name);
    let (w, h) = (960, 540);
    let (mut doc, id, elements, src) = scene(w, h);
    let items: Vec<usize> = (0..elements.len()).collect();
    let mut f = Flottante::lift(&mut doc, id, &items).expect("feuille");
    // Le contrôle : la feuille posée par la transformation du geste (le chemin d'avant),
    f.aff = crate::vector::Affine::translate(vec2(30.0, 12.0));
    let plan = Plan { paper: Some([255, 255, 255]), items: f.items(&line_item(id)).to_vec() };
    let (far, cover, painted) = ecarts(&render_on_gpu(&rs, &doc, &plan), &render::composite_full(&doc, &plan), 3);
    println!("translation (contrôle) : {far} pixels diffèrent de plus de 3 niveaux, {cover} de couverture, sur {painted} peints");
    assert!(far * 200 < painted.max(1) && cover * 200 < painted.max(1), "le banc d'essai lui-même est faux");
    // L'enveloppe : le maillage.
    f.aff = crate::vector::Affine::IDENTITY;
    let mut env = Envelope::identity(src);
    env.move_corner(2, vec2(-70.0, 40.0));
    env.move_corner(0, vec2(30.0, 20.0));
    env.handles[1] += vec2(0.0, 50.0);
    assert!(f.set_warp(src, 8, |p| env.map(src, p)));
    let plan = Plan { paper: Some([255, 255, 255]), items: f.items(&line_item(id)).to_vec() };
    let gpu = render_on_gpu(&rs, &doc, &plan);
    let cpu = render::composite_full(&doc, &plan);
    let (far, cover, painted) = ecarts(&gpu, &cpu, 3);
    println!("enveloppe : {far} pixels diffèrent de plus de 3 niveaux, {cover} de couverture, sur {painted} peints");
    assert!(painted > 5_000);
    assert!(far * 100 < painted, "{far} pixels diffèrent de plus de 3 niveaux");
    assert!(cover * 200 < painted, "{cover} pixels peints d'un côté seulement");
    // L'enveloppe a bien déformé la feuille : l'image n'est plus celle de la translation.
    let moved = {
        let mut g = f.warp.take();
        let aff = f.aff;
        f.aff = crate::vector::Affine::translate(vec2(30.0, 12.0));
        let img = render_on_gpu(&rs, &doc, &Plan { paper: Some([255, 255, 255]), items: f.items(&line_item(id)).to_vec() });
        f.aff = aff;
        f.warp = g.take();
        img
    };
    let different = gpu.iter().zip(&moved).filter(|(a, b)| a != b).count();
    assert!(different > 3_000, "{different} pixels changés par l'enveloppe");
    // Un mode de fusion et une opacité sur la feuille déformée (le chemin qui lit le fond).
    let mut plan = Plan { paper: Some([255, 255, 255]), items: f.items(&line_item(id)).to_vec() };
    for it in &mut plan.items {
        it.blend = BlendMode::Multiply;
        it.opacity = 200;
    }
    let (far, cover, painted) = ecarts(&render_on_gpu(&rs, &doc, &plan), &render::composite_full(&doc, &plan), 4);
    println!("enveloppe en Produit à 78 % : {far} pixels diffèrent de plus de 4 niveaux, {cover} de couverture, sur {painted} peints");
    assert!(far * 100 < painted.max(1) && cover * 200 < painted.max(1));
    // La transformation du calque s'applique après le maillage.
    let mut plan = Plan { paper: Some([255, 255, 255]), items: f.items(&line_item(id)).to_vec() };
    let t = crate::document::LayerTransform { x: 40.0, y: -25.0, scale: 0.8, ..Default::default() };
    for it in &mut plan.items {
        it.transform = Some(t);
    }
    let (far, cover, painted) = ecarts(&render_on_gpu(&rs, &doc, &plan), &render::composite_full(&doc, &plan), 4);
    println!("enveloppe sous une transformation de calque : {far} pixels diffèrent de plus de 4 niveaux, {cover} de couverture, sur {painted} peints");
    assert!(far * 100 < painted.max(1) && cover * 200 < painted.max(1));
}

/// Un cache de textures plein recycle la plus ancienne texture pour le dessin suivant (au lieu d'en
/// créer une neuve) : ce que la carte montre reste exactement le dessin, sans reste du précédent
/// (tuiles que le nouveau dessin n'a pas, effacées), dans un sens comme dans l'autre.
#[test]
fn recycled_cel_textures_show_exactly_their_own_drawing() {
    let Some(rs) = headless() else {
        println!("pas de carte graphique : essai passé");
        return;
    };
    let (w, h) = (256, 192);
    let mut doc = Document::new(w, h, 24);
    // Quinze dessins qui n'ont pas les mêmes tuiles : une tuile dessinée dans l'un est vide dans
    // l'autre, et leurs motifs diffèrent.
    let cels: Vec<_> = (0..15usize)
        .map(|k| {
            let mut cel = crate::document::Cel::new(w, h, crate::document::LayerKind::Bitmap);
            for t in 0..cel.raster.tiles.len() {
                if (t + k) % 3 != 0 {
                    continue;
                }
                let tile = cel.raster.tile_mut(t);
                for (i, p) in tile.iter_mut().enumerate() {
                    if (i + k * 7) % 4 < 2 {
                        *p = Color32::from_rgba_premultiplied(20 + k as u8 * 10, 60, 200 - k as u8 * 8, 220);
                    }
                }
            }
            doc.add_cel(cel)
        })
        .collect();
    let mut gpu = GpuCanvas::new(&rs);
    // Neuf dessins au plus tiennent (huit de marge, un affiché) : le dixième recycle une texture.
    gpu.set_cache_bytes(1);
    let mut checked = 0;
    for round in 0..3 {
        for step in 0..cels.len() {
            // Dans un ordre qui revient sur des dessins déjà vus, chassés puis recréés.
            let k = if round == 1 { cels.len() - 1 - step } else { (step * 4) % cels.len() };
            let plan = Plan { paper: Some([255, 255, 255]), items: vec![line_item(cels[k])] };
            let on_gpu = render_with(&mut gpu, &rs, &doc, &plan, false);
            let cpu = render::composite_full(&doc, &plan);
            let far = on_gpu.iter().zip(&cpu).filter(|(a, b)| a.to_array().iter().zip(b.to_array()).any(|(x, y)| (*x as i32 - y as i32).abs() > 1)).count();
            assert_eq!(far, 0, "round {round}, dessin {k} : {far} pixels diffèrent de la composition sur processeur");
            checked += 1;
        }
    }
    assert_eq!(checked, 45);
}
