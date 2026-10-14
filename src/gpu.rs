//! Superposition des calques par la carte graphique (wgpu).
//!
//! Deux temps, comme le calcul sur processeur qu'il remplace à l'écran :
//! 1. **composer** : le papier puis les calques (opacité, teinte de la pelure d'oignon) sont
//!    empilés dans une image à la taille du projet, pixel pour pixel ;
//! 2. **afficher** : cette image est posée à l'écran avec la vue (zoom, rotation, miroir), lissée
//!    comme egui lisse ses images.
//!
//! Chaque dessin affiché est une texture, envoyée en entier quand il a changé, ou seulement dans les
//! tuiles touchées pendant un trait. Le calcul sur processeur (`render.rs`) reste pour l'export,
//! les vignettes, la pipette, le pot de peinture, et en secours. L'étape « composer » est aussi
//! celle où viendront la caméra et le multiplan.

use std::collections::HashMap;
use std::num::NonZeroU64;

use eframe::egui::{self, Pos2, Rect};
use eframe::egui_wgpu::{self, wgpu, CallbackResources, CallbackTrait, RenderState};

use crate::document::{CelId, Document};
use crate::raster::{Raster, TILE};
use crate::render::Plan;

/// Réglages d'un dessin dans le tampon : 4 coins (position, coordonnées de texture), une couleur,
/// des paramètres = 6 vec4 = 96 octets.
const ITEM_BYTES: u64 = 96;
/// Au-delà, les textures les moins récemment affichées sont libérées.
const CACHE_BYTES: u64 = 1 << 30;
/// Format des images composées : valeurs gamma, comme le calcul sur processeur.
const FRAME_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

const SHADER: &str = r#"
struct Item {
    corners: array<vec4<f32>, 4>,
    color: vec4<f32>,
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> item: Item;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    var order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let c = item.corners[order[i]];
    var out: VOut;
    out.pos = vec4<f32>(c.xy, 0.0, 1.0);
    out.uv = c.zw;
    return out;
}

// Composer. Couleurs prémultipliées, en valeurs gamma, comme les dessins. params.x : opacité ;
// params.y : 1 = dessin, 0 = papier ; color.w : 1 = teinte de pelure d'oignon (color.rgb).
@fragment
fn fs_compose(v: VOut) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, v.uv);
    let op = item.params.x;
    if (item.params.y < 0.5) {
        return vec4<f32>(item.color.rgb, 1.0);
    }
    if (item.color.w > 0.5) {
        let a = s.a * op;
        return vec4<f32>(item.color.rgb * a, a);
    }
    return s * op;
}

// Afficher : comme egui, l'image est lue en sRGB (lissée en lumière linéaire) puis ramenée en gamma.
fn gamma_from_linear_rgb(rgb: vec3<f32>) -> vec3<f32> {
    let cutoff = rgb < vec3<f32>(0.0031308);
    let lower = rgb * vec3<f32>(12.92);
    let higher = vec3<f32>(1.055) * pow(rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(higher, lower, cutoff);
}

@fragment
fn fs_display(v: VOut) -> @location(0) vec4<f32> {
    let lin = textureSample(tex, samp, v.uv);
    return vec4<f32>(gamma_from_linear_rgb(lin.rgb), lin.a);
}
"#;

struct CelTexture {
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    size: (u32, u32),
    /// Version du dessin envoyée (`u64::MAX` : rien d'envoyé).
    version: u64,
    last_used: u64,
}

/// Image composée, à la taille du projet.
struct Frame {
    size: (u32, u32),
    target: wgpu::TextureView,
    /// Lecture en sRGB pour l'affichage.
    display_group: wgpu::BindGroup,
}

/// Ressources gardées par egui-wgpu d'une image à l'autre.
struct CanvasResources {
    compose: wgpu::RenderPipeline,
    display: wgpu::RenderPipeline,
    uniform_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    /// Composer : un pixel du dessin = un pixel de l'image.
    nearest: wgpu::Sampler,
    /// Afficher : pixels nets en grossissant, lissés en réduisant (comme l'ancien affichage).
    smooth: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    uniform_group: wgpu::BindGroup,
    uniform_capacity: u64,
    stride: u64,
    /// Texture d'un pixel blanc, pour le papier.
    white: wgpu::BindGroup,
    textures: HashMap<CelId, CelTexture>,
    frame: Option<Frame>,
    tick: u64,
}

/// Côté application : de quoi mettre à jour les textures et commander le dessin.
pub struct GpuCanvas {
    render_state: RenderState,
}

impl GpuCanvas {
    pub fn new(rs: &RenderState) -> Self {
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("calques"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("calques réglages"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(ITEM_BYTES),
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("calques texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("calques"),
            bind_group_layouts: &[&uniform_layout, &texture_layout],
            push_constant_ranges: &[],
        });
        let pipeline = |entry: &str, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let compose = pipeline("fs_compose", FRAME_FORMAT);
        let display = pipeline("fs_display", rs.target_format);
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("composer"),
            ..Default::default()
        });
        let smooth = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("afficher"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let align = device.limits().min_uniform_buffer_offset_alignment as u64;
        let stride = ITEM_BYTES.div_ceil(align) * align;
        let (uniforms, uniform_group) = uniform_buffer(device, &uniform_layout, stride, 16);
        let white_tex = cel_texture(device, 1, 1);
        write_region(&rs.queue, &white_tex, 0, 0, 1, 1, &[255, 255, 255, 255]);
        let white = texture_group(device, &texture_layout, &white_tex.create_view(&Default::default()), &nearest);
        rs.renderer.write().callback_resources.insert(CanvasResources {
            compose,
            display,
            uniform_layout,
            texture_layout,
            nearest,
            smooth,
            uniforms,
            uniform_group,
            uniform_capacity: 16,
            stride,
            white,
            textures: HashMap::new(),
            frame: None,
            tick: 0,
        });
        Self {
            render_state: rs.clone(),
        }
    }

    /// Met à jour les textures des dessins du plan. `all` : tout a pu changer ; `tiles` : tuiles
    /// touchées depuis l'image précédente (dans le dessin en cours de modification).
    pub fn sync(&mut self, doc: &Document, plan: &Plan, all: bool, tiles: &[usize]) {
        let rs = &self.render_state;
        let mut renderer = rs.renderer.write();
        let Some(res) = renderer.callback_resources.get_mut::<CanvasResources>() else { return };
        res.tick += 1;
        let size = (doc.width as u32, doc.height as u32);
        for item in &plan.items {
            let Some(cel) = doc.cels.get(&item.cel) else { continue };
            let entry = match res.textures.get_mut(&item.cel) {
                Some(e) if e.size == size => e,
                _ => {
                    let texture = cel_texture(&rs.device, size.0, size.1);
                    let view = texture.create_view(&Default::default());
                    let bind_group = texture_group(&rs.device, &res.texture_layout, &view, &res.nearest);
                    res.textures.insert(
                        item.cel,
                        CelTexture {
                            bind_group,
                            texture,
                            size,
                            version: u64::MAX,
                            last_used: 0,
                        },
                    );
                    res.textures.get_mut(&item.cel).expect("texture ajoutée juste avant")
                }
            };
            if all || entry.version != cel.version {
                upload_full(&rs.queue, &entry.texture, &cel.raster);
                entry.version = cel.version;
            } else if !tiles.is_empty() {
                upload_tiles(&rs.queue, &entry.texture, &cel.raster, tiles);
            }
            entry.last_used = res.tick;
        }
        // Mémoire de la carte graphique : on libère les textures les plus anciennes au-delà d'un Go.
        let per = size.0 as u64 * size.1 as u64 * 4;
        let max = (CACHE_BYTES / per.max(1)).max(plan.items.len() as u64 + 8) as usize;
        if res.textures.len() > max {
            let mut ages: Vec<(u64, CelId)> = res.textures.iter().map(|(&id, t)| (t.last_used, id)).collect();
            ages.sort_unstable();
            for (_, id) in ages.into_iter().take(res.textures.len() - max) {
                res.textures.remove(&id);
            }
        }
        // Image composée à la taille du projet.
        if res.frame.as_ref().is_none_or(|f| f.size != size) {
            let texture = rs.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("image composée"),
                size: extent(size.0, size.1),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FRAME_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
            });
            let target = texture.create_view(&Default::default());
            let srgb = texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                ..Default::default()
            });
            let display_group = texture_group(&rs.device, &res.texture_layout, &srgb, &res.smooth);
            res.frame = Some(Frame {
                size,
                target,
                display_group,
            });
        }
    }

    /// Commande de dessin : composer le papier et les calques du plan, puis afficher l'image.
    /// `corners` : les coins du dessin à l'écran, en points (haut-gauche, haut-droite, bas-droite,
    /// bas-gauche) ; `rect` : la zone d'affichage (ce qui dépasse est coupé).
    pub fn paint(&self, plan: &Plan, corners: [Pos2; 4], rect: Rect) -> egui::PaintCallback {
        // Composer : l'image entière, en coordonnées normalisées.
        let full = [[-1.0, 1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 0.0], [1.0, -1.0, 1.0, 1.0], [-1.0, -1.0, 0.0, 1.0]];
        let mut items = Vec::with_capacity(plan.items.len() + 1);
        if let Some(p) = plan.paper {
            items.push(GpuItem {
                cel: None,
                corners: full,
                color: [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, 0.0],
                params: [1.0, 0.0, 0.0, 0.0],
            });
        }
        for item in &plan.items {
            let (color, tinted) = match item.tint {
                Some(t) => ([t[0] as f32 / 255.0, t[1] as f32 / 255.0, t[2] as f32 / 255.0], 1.0),
                None => ([0.0; 3], 0.0),
            };
            items.push(GpuItem {
                cel: Some(item.cel),
                corners: full,
                color: [color[0], color[1], color[2], tinted],
                params: [item.opacity as f32 / 255.0, 1.0, 0.0, 0.0],
            });
        }
        // Afficher : positions en points ; elles passent en coordonnées de l'écran entier au moment
        // du dessin (taille exacte en pixels), pour tomber sur les mêmes pixels que l'ancien affichage.
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let screen = std::array::from_fn(|i| [corners[i].x, corners[i].y, uv[i][0], uv[i][1]]);
        egui_wgpu::Callback::new_paint_callback(rect, CanvasPaint { items, screen })
    }
}

struct GpuItem {
    cel: Option<CelId>,
    corners: [[f32; 4]; 4],
    color: [f32; 4],
    params: [f32; 4],
}

struct CanvasPaint {
    /// Ce qu'on compose, du fond vers le dessus.
    items: Vec<GpuItem>,
    /// Coins de l'image à l'écran, en points.
    screen: [[f32; 4]; 4],
}

fn put_item(bytes: &mut [u8], corners: &[[f32; 4]; 4], color: &[f32; 4], params: &[f32; 4]) {
    for (k, v) in corners.iter().flatten().chain(color).chain(params).enumerate() {
        bytes[k * 4..k * 4 + 4].copy_from_slice(&v.to_ne_bytes());
    }
}

impl CallbackTrait for CanvasPaint {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(res) = resources.get_mut::<CanvasResources>() else { return Vec::new() };
        // Les réglages des dessins à composer, puis ceux de l'affichage (en dernier).
        let n = self.items.len() as u64 + 1;
        if n > res.uniform_capacity {
            let capacity = n.next_power_of_two();
            let (buffer, group) = uniform_buffer(device, &res.uniform_layout, res.stride, capacity);
            res.uniforms = buffer;
            res.uniform_group = group;
            res.uniform_capacity = capacity;
        }
        let stride = res.stride as usize;
        let mut bytes = vec![0u8; n as usize * stride];
        for (i, item) in self.items.iter().enumerate() {
            put_item(&mut bytes[i * stride..], &item.corners, &item.color, &item.params);
        }
        let [w, h] = screen.size_in_pixels.map(|v| v.max(1) as f32);
        let ppp = screen.pixels_per_point;
        let on_screen = self.screen.map(|c| [c[0] * ppp / w * 2.0 - 1.0, 1.0 - c[1] * ppp / h * 2.0, c[2], c[3]]);
        put_item(&mut bytes[self.items.len() * stride..], &on_screen, &[0.0; 4], &[1.0, 1.0, 0.0, 0.0]);
        queue.write_buffer(&res.uniforms, 0, &bytes);

        // Composer dans l'image à la taille du projet.
        let Some(frame) = &res.frame else { return Vec::new() };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composer les calques"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &frame.target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&res.compose);
        for (i, item) in self.items.iter().enumerate() {
            let group = match item.cel {
                None => &res.white,
                Some(id) => match res.textures.get(&id) {
                    Some(t) => &t.bind_group,
                    None => continue,
                },
            };
            pass.set_bind_group(0, &res.uniform_group, &[(i * stride) as u32]);
            pass.set_bind_group(1, group, &[]);
            pass.draw(0..6, 0..1);
        }
        Vec::new()
    }

    fn paint(&self, info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        let Some(res) = resources.get::<CanvasResources>() else { return };
        let Some(frame) = &res.frame else { return };
        // Tout l'écran comme repère (la zone d'affichage reste la limite, par le découpage d'egui).
        let [w, h] = info.screen_size_px;
        pass.set_viewport(0.0, 0.0, w as f32, h as f32, 0.0, 1.0);
        pass.set_pipeline(&res.display);
        pass.set_bind_group(0, &res.uniform_group, &[(self.items.len() as u64 * res.stride) as u32]);
        pass.set_bind_group(1, &frame.display_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: width.max(1),
        height: height.max(1),
        depth_or_array_layers: 1,
    }
}

fn uniform_buffer(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, stride: u64, capacity: u64) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("calques réglages"),
        size: stride * capacity,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("calques réglages"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: NonZeroU64::new(ITEM_BYTES),
            }),
        }],
    });
    (buffer, group)
}

/// Texture d'un dessin : valeurs gamma prémultipliées, telles qu'elles sont en mémoire.
fn cel_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("dessin"),
        size: extent(width, height),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn texture_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, view: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("dessin"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn write_region(queue: &wgpu::Queue, texture: &wgpu::Texture, x: u32, y: u32, w: u32, h: u32, data: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        extent(w, h),
    );
}

fn upload_full(queue: &wgpu::Queue, texture: &wgpu::Texture, raster: &Raster) {
    let bytes: Vec<u8> = raster.to_flat().iter().flat_map(|c| c.to_array()).collect();
    write_region(queue, texture, 0, 0, raster.width as u32, raster.height as u32, &bytes);
}

fn upload_tiles(queue: &wgpu::Queue, texture: &wgpu::Texture, raster: &Raster, tiles: &[usize]) {
    let mut bytes = Vec::with_capacity(TILE * TILE * 4);
    for &idx in tiles {
        if idx >= raster.tiles.len() {
            continue;
        }
        let (ox, oy, tw, th) = raster.tile_bounds(idx);
        bytes.clear();
        match &raster.tiles[idx] {
            Some(tile) => {
                for y in 0..th {
                    bytes.extend(tile[y * TILE..y * TILE + tw].iter().flat_map(|c| c.to_array()));
                }
            }
            None => bytes.resize(tw * th * 4, 0),
        }
        write_region(queue, texture, ox as u32, oy as u32, tw as u32, th as u32, &bytes);
    }
}
