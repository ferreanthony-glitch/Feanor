//! Vue d'édition de la scène 3D, dessinée par la carte graphique (wgpu).
//!
//! Chaque objet est un appel de dessin sur un maillage gardé dans la carte (la forme ne change pas
//! quand on le déplace : seule sa matrice, un petit bloc de 176 octets, est renvoyée), avec un tampon
//! de profondeur et un lissage 4×. Rien n'est recalculé sur le processeur pendant qu'on déplace un
//! objet ou qu'on tourne autour. Les passes, dans l'ordre : le contour (coque retournée, élargie de
//! quelques pixels), les aplats ombrés, les arêtes vives, le sol quadrillé. L'image est ensuite
//! posée sur la zone du dessin (une seule texture à afficher). Les manipulateurs et les légendes
//! sont tracés par egui par-dessus.
//!
//! Les formules d'ombrage sont celles du rendu vers un calque (`scene3drender::shade`).

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use eframe::egui_wgpu::{self, wgpu, CallbackResources, CallbackTrait, RenderState};
use wgpu::util::DeviceExt;

use crate::scene3d::{Light, RenderStyle};
use crate::scene3dmath::{M4, V3, View};
use crate::scene3dmesh::{MeshKey, SolidMesh};

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;
/// Octets d'un objet dans le tampon : deux matrices, la couleur, des indicateurs, la couleur des traits.
const OBJECT_BYTES: u64 = 176;
const OBJECT_STRIDE: u64 = 256;
const GLOBAL_BYTES: u64 = 112;
/// Plan proche et lointain de la vue d'édition, en mètres.
pub const NEAR: f32 = 0.05;
pub const FAR: f32 = 2000.0;
/// Fond de la vue.
pub const BACKGROUND: [f32; 3] = [0.16, 0.165, 0.18];

const SHADER: &str = r#"
struct Globals {
    vp: mat4x4<f32>,
    eye: vec4<f32>,
    light: vec4<f32>,
    params: vec4<f32>,
};
struct Obj {
    model: mat4x4<f32>,
    nmat: mat4x4<f32>,
    color: vec4<f32>,
    flags: vec4<f32>,
    edge: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var<uniform> o: Obj;

struct FillOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) wpos: vec3<f32>,
    @location(1) n: vec3<f32>,
};

@vertex
fn vs_fill(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) on: vec3<f32>) -> FillOut {
    var out: FillOut;
    let w = o.model * vec4<f32>(p, 1.0);
    out.wpos = w.xyz;
    out.pos = g.vp * w;
    out.n = (o.nmat * vec4<f32>(n, 0.0)).xyz;
    return out;
}

@fragment
fn fs_fill(in: FillOut) -> @location(0) vec4<f32> {
    var n = normalize(in.n);
    let v = normalize(g.eye.xyz - in.wpos);
    if (dot(n, v) < 0.0) { n = -n; }
    let lit = g.params.x + g.light.w * max(dot(n, g.light.xyz), 0.0);
    let base = o.color.rgb;
    var c: vec3<f32>;
    if (g.params.y > 1.5) {
        let k = min(0.9 + 0.07 * clamp(lit - 0.35, 0.0, 1.0), 1.0);
        c = vec3<f32>(k, k, k);
    } else if (g.params.y > 0.5 || o.flags.x > 0.5) {
        let y = dot(base, vec3<f32>(0.299, 0.587, 0.114));
        let k = clamp(y * lit, 0.0, 1.0);
        c = vec3<f32>(k, k, k);
    } else {
        c = clamp(base * lit, vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0));
    }
    c = c * (1.0 - 0.3 * o.flags.z);
    c = mix(c, vec3<f32>(0.27, 0.65, 1.0), 0.22 * o.flags.y);
    return vec4<f32>(c, 1.0);
}

@vertex
fn vs_hull(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) on: vec3<f32>) -> @builtin(position) vec4<f32> {
    let w = o.model * vec4<f32>(p, 1.0);
    var clip = g.vp * w;
    let nw = (o.nmat * vec4<f32>(on, 0.0)).xyz;
    let nc = (g.vp * vec4<f32>(nw, 0.0)).xy;
    let len = length(nc);
    if (len > 0.000001) {
        let off = nc / len * o.flags.w * 2.0 / g.params.zw;
        clip = vec4<f32>(clip.xy + off * clip.w, clip.z, clip.w);
    }
    return clip;
}

@fragment
fn fs_hull() -> @location(0) vec4<f32> {
    return vec4<f32>(o.edge.rgb, 1.0);
}

@vertex
fn vs_edge(@location(0) p: vec3<f32>) -> @builtin(position) vec4<f32> {
    return g.vp * (o.model * vec4<f32>(p, 1.0));
}

@fragment
fn fs_edge() -> @location(0) vec4<f32> {
    return vec4<f32>(o.edge.rgb, 1.0);
}

struct GridOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) col: vec4<f32>,
};

@vertex
fn vs_grid(@location(0) p: vec3<f32>, @location(1) c: vec4<f32>) -> GridOut {
    var out: GridOut;
    out.pos = g.vp * vec4<f32>(p, 1.0);
    out.col = c;
    return out;
}

@fragment
fn fs_grid(in: GridOut) -> @location(0) vec4<f32> {
    return in.col;
}
"#;

const BLIT_SHADER: &str = r#"
struct Blit { srgb: vec4<f32> };
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var<uniform> b: Blit;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    var out: Out;
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    out.uv = vec2<f32>(x, 1.0 - y);
    out.pos = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    let c = textureSample(tex, smp, in.uv);
    if (b.srgb.x > 0.5) {
        return vec4<f32>(pow(c.rgb, vec3<f32>(2.2, 2.2, 2.2)), 1.0);
    }
    return vec4<f32>(c.rgb, 1.0);
}
"#;

/// Un maillage dans la carte graphique.
struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    lines: Option<(wgpu::Buffer, u32)>,
    used: u64,
}

struct Targets {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolve: wgpu::TextureView,
    blit_group: wgpu::BindGroup,
}

struct SceneResources {
    hull: wgpu::RenderPipeline,
    fill: wgpu::RenderPipeline,
    edge: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
    global_buffer: wgpu::Buffer,
    global_group: wgpu::BindGroup,
    object_layout: wgpu::BindGroupLayout,
    object_buffer: wgpu::Buffer,
    object_group: wgpu::BindGroup,
    object_capacity: u64,
    blit_layout: wgpu::BindGroupLayout,
    blit_uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    grid_buffer: Option<(wgpu::Buffer, u64)>,
    meshes: HashMap<MeshKey, GpuMesh>,
    targets: Option<Targets>,
    tick: u64,
    /// Objets dessinés par la dernière image (essais, mesures).
    drawn: usize,
}

/// Un objet à dessiner.
#[derive(Clone)]
pub struct DrawObject {
    pub key: MeshKey,
    pub mesh: Arc<SolidMesh>,
    pub matrix: M4,
    pub normal_matrix: M4,
    pub color: [f32; 3],
    pub gray: bool,
    pub selected: bool,
    /// Le décor, quand on modifie le plan (ou l'inverse) : estompé.
    pub ghost: bool,
}

/// Une ligne du quadrillage (couleur avec transparence).
#[derive(Clone, Copy)]
pub struct GridLine {
    pub a: V3,
    pub b: V3,
    pub color: [f32; 4],
}

/// Tout ce qu'une image de la vue demande.
pub struct Frame {
    pub view: View,
    /// Taille de la zone, en points d'écran.
    pub size_pts: [f32; 2],
    pub light: Light,
    pub style: RenderStyle,
    /// Contours autour des objets et arêtes vives.
    pub lines: bool,
    pub line_px: f32,
    pub objects: Vec<DrawObject>,
    pub grid: Vec<GridLine>,
}

/// Côté application : installe les ressources dans la carte graphique et commande le dessin.
pub struct Gpu3d {
    render_state: RenderState,
}

impl Drop for Gpu3d {
    /// Rend à la carte graphique les maillages et les cibles de la scène 3D.
    fn drop(&mut self) {
        self.render_state.renderer.write().callback_resources.remove::<SceneResources>();
    }
}

impl Gpu3d {
    pub fn new(rs: &RenderState) -> Gpu3d {
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("scène 3D"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("scène 3D : afficher"), source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()) });
        let uniform = |size: u64, dynamic: bool| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: dynamic, min_binding_size: NonZeroU64::new(size) },
            count: None,
        };
        let global_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("scène 3D globale"), entries: &[uniform(GLOBAL_BYTES, false)] });
        let object_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("scène 3D objet"), entries: &[uniform(OBJECT_BYTES, true)] });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("scène 3D"), bind_group_layouts: &[&global_layout, &object_layout], push_constant_ranges: &[] });
        let grid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("scène 3D quadrillage"), bind_group_layouts: &[&global_layout], push_constant_ranges: &[] });

        const SOLID: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
        const LINE: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
        const GRID: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let solid_buf = [wgpu::VertexBufferLayout { array_stride: 36, step_mode: wgpu::VertexStepMode::Vertex, attributes: &SOLID }];
        let line_buf = [wgpu::VertexBufferLayout { array_stride: 12, step_mode: wgpu::VertexStepMode::Vertex, attributes: &LINE }];
        let grid_buf = [wgpu::VertexBufferLayout { array_stride: 28, step_mode: wgpu::VertexStepMode::Vertex, attributes: &GRID }];

        #[allow(clippy::too_many_arguments)]
        let pipeline = |label: &str,
                        layout: &wgpu::PipelineLayout,
                        vs: &str,
                        fs: &str,
                        buffers: &[wgpu::VertexBufferLayout],
                        topology: wgpu::PrimitiveTopology,
                        cull: Option<wgpu::Face>,
                        depth_write: bool,
                        compare: wgpu::CompareFunction,
                        bias: wgpu::DepthBiasState,
                        blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), buffers, compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState { topology, cull_mode: cull, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: depth_write, depth_compare: compare, stencil: Default::default(), bias }),
                multisample: wgpu::MultisampleState { count: SAMPLES, mask: !0, alpha_to_coverage_enabled: false },
                multiview: None,
                cache: None,
            })
        };
        let flat = wgpu::DepthBiasState::default();
        let tris = wgpu::PrimitiveTopology::TriangleList;
        let lines = wgpu::PrimitiveTopology::LineList;
        // La coque du contour est repoussée vers le fond, les arêtes ramenées vers l'œil.
        let hull = pipeline("scène 3D contour", &layout, "vs_hull", "fs_hull", &solid_buf, tris, Some(wgpu::Face::Front), true, wgpu::CompareFunction::Less, wgpu::DepthBiasState { constant: 4, slope_scale: 2.0, clamp: 0.0 }, None);
        let fill = pipeline("scène 3D aplats", &layout, "vs_fill", "fs_fill", &solid_buf, tris, None, true, wgpu::CompareFunction::LessEqual, flat, None);
        let edge = pipeline("scène 3D arêtes", &layout, "vs_edge", "fs_edge", &line_buf, lines, None, false, wgpu::CompareFunction::LessEqual, wgpu::DepthBiasState { constant: -8, slope_scale: -1.0, clamp: 0.0 }, None);
        let grid = pipeline("scène 3D quadrillage", &grid_layout, "vs_grid", "fs_grid", &grid_buf, lines, None, false, wgpu::CompareFunction::LessEqual, wgpu::DepthBiasState { constant: -8, slope_scale: -1.0, clamp: 0.0 }, Some(wgpu::BlendState::ALPHA_BLENDING));

        let global_buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("scène 3D globale"), size: GLOBAL_BYTES, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let global_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scène 3D globale"),
            layout: &global_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: global_buffer.as_entire_binding() }],
        });
        let capacity = 64;
        let (object_buffer, object_group) = object_buffer(device, &object_layout, capacity);

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scène 3D afficher"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: NonZeroU64::new(16) },
                    count: None,
                },
            ],
        });
        let blit_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("scène 3D afficher"), bind_group_layouts: &[&blit_layout], push_constant_ranges: &[] });
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scène 3D afficher"),
            layout: Some(&blit_pl),
            vertex: wgpu::VertexState { module: &blit_shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState { format: rs.target_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let srgb = if rs.target_format.is_srgb() { 1.0f32 } else { 0.0 };
        let blit_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scène 3D afficher"),
            contents: bytemuck::cast_slice(&[srgb, 0.0f32, 0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("scène 3D"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        rs.renderer.write().callback_resources.insert(SceneResources {
            hull,
            fill,
            edge,
            grid,
            blit,
            global_buffer,
            global_group,
            object_layout,
            object_buffer,
            object_group,
            object_capacity: capacity,
            blit_layout,
            blit_uniform,
            sampler,
            grid_buffer: None,
            meshes: HashMap::new(),
            targets: None,
            tick: 0,
            drawn: 0,
        });
        Gpu3d { render_state: rs.clone() }
    }

    /// La commande de dessin de l'image `frame` dans `rect`.
    pub fn paint(&self, frame: Frame, rect: eframe::egui::Rect) -> eframe::egui::PaintCallback {
        egui_wgpu::Callback::new_paint_callback(rect, ScenePaint { frame })
    }

    /// Objets dessinés par la dernière image (essais).
    #[cfg(test)]
    pub fn drawn(&self) -> usize {
        self.render_state.renderer.read().callback_resources.get::<SceneResources>().map_or(0, |r| r.drawn)
    }

    /// Octets des maillages gardés dans la carte (essais, mesures).
    #[cfg(test)]
    pub fn mesh_count(&self) -> usize {
        self.render_state.renderer.read().callback_resources.get::<SceneResources>().map_or(0, |r| r.meshes.len())
    }
}

fn object_buffer(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, capacity: u64) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scène 3D objets"),
        size: OBJECT_STRIDE * capacity,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scène 3D objets"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buffer, offset: 0, size: NonZeroU64::new(OBJECT_BYTES) }),
        }],
    });
    (buffer, group)
}

struct ScenePaint {
    frame: Frame,
}

fn upload_mesh(device: &wgpu::Device, mesh: &SolidMesh, tick: u64) -> GpuMesh {
    let mut data: Vec<f32> = Vec::with_capacity(mesh.positions.len() * 9);
    for i in 0..mesh.positions.len() {
        data.extend_from_slice(&mesh.positions[i]);
        data.extend_from_slice(&mesh.normals[i]);
        data.extend_from_slice(mesh.outline.get(i).unwrap_or(&mesh.normals[i]));
    }
    let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("maillage"), contents: bytemuck::cast_slice(&data), usage: wgpu::BufferUsages::VERTEX });
    let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("indices"), contents: bytemuck::cast_slice(&mesh.indices), usage: wgpu::BufferUsages::INDEX });
    let lines = (!mesh.edges.is_empty()).then(|| {
        let flat: Vec<f32> = mesh.edges.iter().flat_map(|e| e[0].into_iter().chain(e[1])).collect();
        (device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("arêtes"), contents: bytemuck::cast_slice(&flat), usage: wgpu::BufferUsages::VERTEX }), (mesh.edges.len() * 2) as u32)
    });
    GpuMesh { vertices, indices, index_count: mesh.indices.len() as u32, lines, used: tick }
}

fn put(bytes: &mut [u8], at: usize, values: &[f32]) {
    for (k, v) in values.iter().enumerate() {
        bytes[at + k * 4..at + k * 4 + 4].copy_from_slice(&v.to_ne_bytes());
    }
}

impl CallbackTrait for ScenePaint {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(res) = resources.get_mut::<SceneResources>() else { return Vec::new() };
        let f = &self.frame;
        res.tick += 1;
        let ppp = screen.pixels_per_point;
        let size = [((f.size_pts[0] * ppp).round() as u32).max(1), ((f.size_pts[1] * ppp).round() as u32).max(1)];

        // Cibles à la taille de la zone (en pixels de l'écran).
        if res.targets.as_ref().is_none_or(|t| t.size != size) {
            let make = |format, samples, usage| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("scène 3D"),
                        size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: samples,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            let msaa = make(COLOR_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
            let depth = make(DEPTH_FORMAT, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
            let resolve = make(COLOR_FORMAT, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
            let blit_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scène 3D afficher"),
                layout: &res.blit_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&resolve) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&res.sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: res.blit_uniform.as_entire_binding() },
                ],
            });
            res.targets = Some(Targets { size, msaa, depth, resolve, blit_group });
        }

        // Maillages manquants.
        for o in &f.objects {
            if !res.meshes.contains_key(&o.key) {
                res.meshes.insert(o.key, upload_mesh(device, &o.mesh, res.tick));
            }
            if let Some(m) = res.meshes.get_mut(&o.key) {
                m.used = res.tick;
            }
        }
        if res.meshes.len() > 300 {
            let tick = res.tick;
            res.meshes.retain(|_, m| m.used == tick);
        }

        // Réglages communs.
        let vp = f.view.clip_matrix([size[0] as f32, size[1] as f32], NEAR, FAR);
        let dir = f.light.direction();
        let style = match f.style {
            RenderStyle::Color => 0.0,
            RenderStyle::Gray => 1.0,
            RenderStyle::Outline => 2.0,
        };
        let mut g = [0u8; GLOBAL_BYTES as usize];
        put(&mut g, 0, &vp);
        put(&mut g, 64, &[f.view.eye[0], f.view.eye[1], f.view.eye[2], 1.0]);
        put(&mut g, 80, &[dir[0], dir[1], dir[2], f.light.intensity]);
        put(&mut g, 96, &[f.light.ambient, style, size[0] as f32, size[1] as f32]);
        queue.write_buffer(&res.global_buffer, 0, &g);

        // Un bloc par objet.
        let n = f.objects.len() as u64;
        if n > res.object_capacity {
            let capacity = n.next_power_of_two();
            let (buffer, group) = object_buffer(device, &res.object_layout, capacity);
            res.object_buffer = buffer;
            res.object_group = group;
            res.object_capacity = capacity;
        }
        let mut bytes = vec![0u8; (n.max(1) * OBJECT_STRIDE) as usize];
        for (i, o) in f.objects.iter().enumerate() {
            let at = i * OBJECT_STRIDE as usize;
            put(&mut bytes, at, &o.matrix);
            put(&mut bytes, at + 64, &o.normal_matrix);
            put(&mut bytes, at + 128, &[o.color[0], o.color[1], o.color[2], 1.0]);
            let outline = if o.selected { 3.0 } else if f.lines { f.line_px * ppp } else { 0.0 };
            put(&mut bytes, at + 144, &[o.gray as u8 as f32, o.selected as u8 as f32, o.ghost as u8 as f32, outline]);
            let ink = if o.selected { [0.27, 0.65, 1.0] } else { [0.05, 0.05, 0.06] };
            put(&mut bytes, at + 160, &[ink[0], ink[1], ink[2], 1.0]);
        }
        queue.write_buffer(&res.object_buffer, 0, &bytes);

        // Quadrillage.
        if !f.grid.is_empty() {
            let mut data: Vec<f32> = Vec::with_capacity(f.grid.len() * 14);
            for l in &f.grid {
                data.extend_from_slice(&l.a);
                data.extend_from_slice(&l.color);
                data.extend_from_slice(&l.b);
                data.extend_from_slice(&l.color);
            }
            let need = (data.len() * 4) as u64;
            if res.grid_buffer.as_ref().is_none_or(|(_, cap)| *cap < need) {
                let cap = need.next_power_of_two().max(4096);
                let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("quadrillage"), size: cap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
                res.grid_buffer = Some((buffer, cap));
            }
            if let Some((buffer, _)) = &res.grid_buffer {
                queue.write_buffer(buffer, 0, bytemuck::cast_slice(&data));
            }
        }

        // Les passes, dans la cible.
        let Some(targets) = &res.targets else { return Vec::new() };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scène 3D"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.msaa,
                resolve_target: Some(&targets.resolve),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: BACKGROUND[0] as f64, g: BACKGROUND[1] as f64, b: BACKGROUND[2] as f64, a: 1.0 }), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let mut drawn = 0;
        // Contours : seulement des objets choisis, ou de tous si le style en veut.
        pass.set_bind_group(0, &res.global_group, &[]);
        pass.set_pipeline(&res.hull);
        for (i, o) in f.objects.iter().enumerate() {
            if !(f.lines || o.selected) {
                continue;
            }
            let Some(m) = res.meshes.get(&o.key) else { continue };
            pass.set_bind_group(1, &res.object_group, &[(i as u64 * OBJECT_STRIDE) as u32]);
            pass.set_vertex_buffer(0, m.vertices.slice(..));
            pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..m.index_count, 0, 0..1);
        }
        pass.set_pipeline(&res.fill);
        for (i, o) in f.objects.iter().enumerate() {
            let Some(m) = res.meshes.get(&o.key) else { continue };
            pass.set_bind_group(1, &res.object_group, &[(i as u64 * OBJECT_STRIDE) as u32]);
            pass.set_vertex_buffer(0, m.vertices.slice(..));
            pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..m.index_count, 0, 0..1);
            drawn += 1;
        }
        if f.lines {
            pass.set_pipeline(&res.edge);
            for (i, o) in f.objects.iter().enumerate() {
                let Some((buffer, count)) = res.meshes.get(&o.key).and_then(|m| m.lines.as_ref()) else { continue };
                pass.set_bind_group(1, &res.object_group, &[(i as u64 * OBJECT_STRIDE) as u32]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, 0..1);
            }
        }
        if let (false, Some((buffer, _))) = (f.grid.is_empty(), &res.grid_buffer) {
            pass.set_pipeline(&res.grid);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..(f.grid.len() * 2) as u32, 0..1);
        }
        drop(pass);
        res.drawn = drawn;
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        let Some(res) = resources.get::<SceneResources>() else { return };
        let Some(targets) = &res.targets else { return };
        pass.set_pipeline(&res.blit);
        pass.set_bind_group(0, &targets.blit_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

use eframe::egui;
