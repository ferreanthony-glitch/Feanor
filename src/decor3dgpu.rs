//! Le décor 3D dessiné par la carte graphique (partie 180) : dans **l'image du plan** à l'écran (une passe de la composition
//! de `GpuCanvas`, l'élément le plus bas, `gpu.rs`) et dans la **vue Espace 3D** (une commande de dessin d'egui, `VueDecor`).
//!
//! Règle d'Anthony : « pas de compromis avec les ralentissements ». Donc :
//! - les maillages et les textures partent à la carte **une fois** (première image où le décor se montre), jamais à chaque
//!   image ; la caméra qui bouge ne fait que changer une matrice ;
//! - à l'écran, l'image du décor n'est jamais refaite par le processeur : c'est la carte qui la dessine, de l'œil de la
//!   caméra, alignée sur la feuille (la même prise que `decor3drendu::Prise::de_la_feuille`), et l'homographie de la caméra 2D
//!   s'applique ensuite comme à toute carte ;
//! - les tampons respectent `device.limits()` : une primitive trop grosse est coupée en morceaux, une texture trop grande
//!   réduite au plus grand côté que la carte accepte.
//!
//! **L'aspect** (Anthony, « Dans l'étape 1 », low poly doux) : facettes (normales par face, d'origine) ou lissé ; lumière
//! « hémisphère » (ambiance froide du ciel, chaude du sol) et soleil chaud ; ciel en dégradé pastel derrière le décor (dans
//! l'image du plan) ; brume vers la couleur de l'horizon avec la distance ; ombres douces du soleil (une carte d'ombres filtrée
//! PCF), dont la qualité suit le budget de cet ordinateur (`decor3dbudget::Qualite`). Les formules sont celles du rendu par le
//! processeur (`decor3drendu.rs`), sauf les ombres que le rendu de secours n'a pas.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use eframe::egui;
use eframe::egui_wgpu::{self, wgpu, CallbackResources, CallbackTrait};
use wgpu::util::DeviceExt;

use crate::decor3dgltf::{reduire, Scene3d};
use crate::decor3drendu::{Aspect, Prise};
use crate::espace3dmath::{self as m, M4, V3};

const COULEUR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const PROFONDEUR: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Lissage des bords de la vue Espace 3D (l'image du plan, à la taille de la feuille, n'en a pas besoin : elle est réduite à
/// l'écran).
const ECHANTILLONS_VUE: u32 = 4;
const GLOBAL: u64 = 512;
const OBJET: u64 = 160;
const PAS_OBJET: u64 = 256;
/// Triangles au plus par morceau envoyé à la carte (3 millions de sommets × 32 octets : sous les 256 Mo de tampon que toute
/// carte accepte).
const TRIANGLES_PAR_MORCEAU: usize = 1_000_000;

const SHADER: &str = r#"
struct Globals {
    vp: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    eye: vec4<f32>,        // xyz ; w : facettes (1) ou lissé (0)
    key_dir: vec4<f32>,    // vers le soleil ; w : ombres (taps, 0 = sans)
    key_col: vec4<f32>,    // w : texel de la carte d'ombres
    sky_amb: vec4<f32>,
    ground_amb: vec4<f32>,
    fog_col: vec4<f32>,    // w : brume la plus forte
    fog: vec4<f32>,        // début, fin, ciel (1/0), sortie sRGB (1) ou linéaire (0)
    zenith: vec4<f32>,
    cam_r: vec4<f32>,      // droite ; w : px par unité
    cam_u: vec4<f32>,      // haut ; w : centre x
    cam_f: vec4<f32>,      // avant ; w : centre y
    size: vec4<f32>,       // taille de la cible (pixels), ortho (1/0)
};
struct Obj {
    model: mat4x4<f32>,
    nmat: mat4x4<f32>,
    color: vec4<f32>,
    flags: vec4<f32>,      // texture (1/0), seuil d'alpha (0 : opaque)
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var t_shadow: texture_depth_2d;
@group(0) @binding(2) var s_shadow: sampler_comparison;
@group(1) @binding(0) var<uniform> o: Obj;
@group(2) @binding(0) var t_base: texture_2d<f32>;
@group(2) @binding(1) var s_base: sampler;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) wpos: vec3<f32>,
    @location(1) n: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) uv: vec2<f32>) -> VOut {
    var out: VOut;
    let w = o.model * vec4<f32>(p, 1.0);
    out.wpos = w.xyz;
    out.pos = g.vp * w;
    out.n = (o.nmat * vec4<f32>(n, 0.0)).xyz;
    out.uv = uv;
    return out;
}

@vertex
fn vs_shadow(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) uv: vec2<f32>) -> @builtin(position) vec4<f32> {
    return g.light_vp * (o.model * vec4<f32>(p, 1.0));
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn shadow(wpos: vec3<f32>, n: vec3<f32>) -> f32 {
    let taps = i32(g.key_dir.w);
    if (taps <= 0) { return 1.0; }
    let lp = g.light_vp * vec4<f32>(wpos + n * 0.02, 1.0);
    let q = lp.xyz / lp.w;
    let uv = vec2<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || q.z > 1.0) { return 1.0; }
    let texel = g.key_col.w;
    var sum = 0.0;
    var count = 0.0;
    let r = select(1, 2, taps > 4);
    for (var y = -r; y <= r; y = y + 1) {
        for (var x = -r; x <= r; x = x + 1) {
            sum = sum + textureSampleCompareLevel(t_shadow, s_shadow, uv + vec2<f32>(f32(x), f32(y)) * texel * 1.5, q.z - 0.0015);
            count = count + 1.0;
        }
    }
    return sum / count;
}

fn finish(c: vec3<f32>) -> vec3<f32> {
    if (g.fog.w > 0.5) { return to_srgb(c); }
    return c;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // (Calculés avant tout `discard` : les dérivées veulent un flot uniforme.)
    let flat_n = normalize(cross(dpdx(in.wpos), dpdy(in.wpos)));
    let tex = textureSampleLevel(t_base, s_base, in.uv, 0.0);
    var base = o.color;
    if (o.flags.x > 0.5) { base = base * tex; }
    if (o.flags.y > 0.0 && base.a < o.flags.y) { discard; }
    var n = normalize(in.n);
    if (g.eye.w > 0.5) { n = flat_n; }
    let v = g.eye.xyz - in.wpos;
    if (dot(n, v) < 0.0) { n = -n; }
    let hemi = mix(g.ground_amb.rgb, g.sky_amb.rgb, n.y * 0.5 + 0.5);
    let sun = max(dot(n, g.key_dir.xyz), 0.0) * shadow(in.wpos, n);
    var c = base.rgb * (hemi + g.key_col.rgb * sun);
    let d = length(v);
    let f = clamp((d - g.fog.x) / max(g.fog.y - g.fog.x, 0.0001), 0.0, 1.0) * g.fog_col.w;
    c = mix(c, g.fog_col.rgb, f * f * (3.0 - 2.0 * f));
    return vec4<f32>(finish(c), 1.0);
}

struct SkyOut { @builtin(position) pos: vec4<f32> };

@vertex
fn vs_sky(@builtin(vertex_index) i: u32) -> SkyOut {
    var out: SkyOut;
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    out.pos = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 1.0, 1.0);
    return out;
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    if (g.fog.z < 0.5) { discard; }
    let s = g.cam_r.w;
    var dir = g.cam_f.xyz;
    if (g.size.z < 0.5) {
        let x = (in.pos.x - g.cam_u.w) / s;
        let y = -(in.pos.y - g.cam_f.w) / s;
        dir = normalize(g.cam_r.xyz * x + g.cam_u.xyz * y + g.cam_f.xyz);
    }
    let e = clamp(dir.y, 0.0, 1.0);
    let k = pow(e, 0.6);
    let c = mix(g.fog_col.rgb, g.zenith.rgb, k);
    return vec4<f32>(finish(c), 1.0);
}
"#;

const BLIT: &str = r#"
struct Blit { srgb: vec4<f32> };
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var<uniform> b: Blit;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
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
    if (b.srgb.x > 0.5 && c.a > 0.0) {
        // (La cible d'egui est en sRGB : la carte y remet du linéaire.)
        let rgb = pow(c.rgb / c.a, vec3<f32>(2.2)) * c.a;
        return vec4<f32>(rgb, c.a);
    }
    return c;
}
"#;

/// Une texture de couleur de base dans la carte.
struct TextureGpu {
    groupe: wgpu::BindGroup,
}

/// Un morceau de primitive dans la carte.
struct Morceau {
    sommets: wgpu::Buffer,
    indices: wgpu::Buffer,
    nombre: u32,
    /// Rang de la primitive (sa matière) dans la scène.
    prim: usize,
}

/// Un décor dans la carte : envoyé une fois.
pub struct ModeleGpu {
    morceaux: Vec<Morceau>,
    textures: Vec<Option<TextureGpu>>,
    /// Les matières : couleur, texture, seuil.
    matieres: Vec<([f32; 4], Option<usize>, f32)>,
    pub octets: u64,
    pub triangles: usize,
    utilise: u64,
}

/// Les cibles et les tampons d'une prise (la vue, chaque vue multiple, l'image du plan) : deux prises de la même image d'écran
/// ne s'écrasent pas leurs réglages (les écritures de la file passent toutes avant les commandes).
struct Prise3 {
    globals: wgpu::Buffer,
    groupe_global: wgpu::BindGroup,
    objets: wgpu::Buffer,
    groupe_objets: wgpu::BindGroup,
    capacite: u64,
    cibles: Option<Cibles>,
    ombre: Option<(u32, wgpu::TextureView)>,
    utilise: u64,
}

struct Cibles {
    taille: [u32; 2],
    echantillons: u32,
    couleur: wgpu::TextureView,
    profondeur: wgpu::TextureView,
    resolue: Option<(wgpu::TextureView, wgpu::BindGroup)>,
}

/// Le moteur du décor : pipelines, décors envoyés, prises.
pub struct Moteur {
    main1: wgpu::RenderPipeline,
    main4: wgpu::RenderPipeline,
    ciel1: wgpu::RenderPipeline,
    ciel4: wgpu::RenderPipeline,
    ombre_pipe: wgpu::RenderPipeline,
    /// Le poser de la vue sur l'écran d'egui, fait à la première vue (il dépend du format de l'écran).
    blit: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    blit_shader: wgpu::ShaderModule,
    blit_layout: wgpu::BindGroupLayout,
    blit_uniform: wgpu::Buffer,
    layout_global: wgpu::BindGroupLayout,
    layout_objet: wgpu::BindGroupLayout,
    layout_texture: wgpu::BindGroupLayout,
    echant_base: wgpu::Sampler,
    echant_ombre: wgpu::Sampler,
    echant_blit: wgpu::Sampler,
    blanc: wgpu::BindGroup,
    ombre_vide: wgpu::TextureView,
    modeles: HashMap<u64, ModeleGpu>,
    prises: HashMap<u64, Prise3>,
    tic: u64,
    max_texture: u32,
    /// Triangles dessinés par la dernière passe (essais, mesures).
    pub dessines: usize,
}

/// Une chose à dessiner : un décor (sa scène, pour l'envoyer s'il n'est pas encore dans la carte) et sa matrice.
#[derive(Clone)]
pub struct Dessin {
    pub id: u64,
    pub scene: Arc<Scene3d>,
    pub matrice: M4,
    pub aspect: Aspect,
}

/// Le point de vue d'une passe.
#[derive(Clone, Copy, Debug)]
pub enum Vue {
    /// La perspective d'une `Prise` (image du plan, vue en perspective, caméra du plan).
    Prise(Prise),
    /// Une vue orthographique de l'Espace 3D (`View::ortho`) : œil, axes, pixels par unité du monde, centre.
    Ortho { oeil: V3, droite: V3, haut: V3, avant: V3, px_par_unite: f32, centre: [f32; 2] },
}

impl Vue {
    pub fn de_view(v: &m::View, centre: [f32; 2]) -> Vue {
        match v.ortho {
            Some(_) => Vue::Ortho { oeil: v.eye, droite: v.right, haut: v.up, avant: v.forward, px_par_unite: v.px_per_unit(), centre },
            None => Vue::Prise(Prise { oeil: v.eye, droite: v.right, haut: v.up, avant: v.forward, px_par_unite: v.px_per_unit(), centre, proche: 0.01 }),
        }
    }

    fn axes(&self) -> (V3, V3, V3, V3, f32, [f32; 2], bool) {
        match *self {
            Vue::Prise(p) => (p.oeil, p.droite, p.haut, p.avant, p.px_par_unite, p.centre, false),
            Vue::Ortho { oeil, droite, haut, avant, px_par_unite, centre } => (oeil, droite, haut, avant, px_par_unite, centre, true),
        }
    }

    /// Matrice monde → découpe pour une image de `taille` (mêmes unités que `centre` et `px_par_unite`), profondeur de 0
    /// (`proche`) à 1 (`loin`).
    pub fn decoupe(&self, taille: [f32; 2], proche: f32, loin: f32) -> M4 {
        let (oeil, r, u, f, s, c, ortho) = self.axes();
        let (w, h) = (taille[0].max(1.0), taille[1].max(1.0));
        let (kx, ky) = (2.0 * s / w, 2.0 * s / h);
        let (ox, oy) = (2.0 * c[0] / w - 1.0, 1.0 - 2.0 * c[1] / h);
        let ligne = |a: V3| [a[0], a[1], a[2], -m::dot(a, oeil)];
        let (lr, lu, lf) = (ligne(r), ligne(u), ligne(f));
        let lignes: [[f32; 4]; 4] = if ortho {
            let k = 1.0 / (loin - proche);
            [
                [lr[0] * kx, lr[1] * kx, lr[2] * kx, lr[3] * kx + ox],
                [lu[0] * ky, lu[1] * ky, lu[2] * ky, lu[3] * ky + oy],
                [lf[0] * k, lf[1] * k, lf[2] * k, (lf[3] - proche) * k],
                [0.0, 0.0, 0.0, 1.0],
            ]
        } else {
            let k = loin / (loin - proche);
            let comb = |a: [f32; 4], kk: f32, o: f32| [0, 1, 2, 3].map(|i| a[i] * kk + lf[i] * o);
            [comb(lr, kx, ox), comb(lu, ky, oy), [lf[0] * k, lf[1] * k, lf[2] * k, (lf[3] - proche) * k], lf]
        };
        let mut mx = [0.0; 16];
        for (i, l) in lignes.iter().enumerate() {
            for (col, v) in l.iter().enumerate() {
                mx[col * 4 + i] = *v;
            }
        }
        mx
    }
}

fn put(bytes: &mut [u8], at: usize, values: &[f32]) {
    for (k, v) in values.iter().enumerate() {
        bytes[at + k * 4..at + k * 4 + 4].copy_from_slice(&v.to_ne_bytes());
    }
}

impl Moteur {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Moteur {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D : poser"), source: wgpu::ShaderSource::Wgsl(BLIT.into()) });
        let uniforme = |binding: u32, size: u64, dynamic: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: dynamic, min_binding_size: NonZeroU64::new(size) },
            count: None,
        };
        let layout_global = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D globale"),
            entries: &[
                uniforme(0, GLOBAL, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
            ],
        });
        let layout_objet = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("décor 3D objet"), entries: &[uniforme(0, OBJET, true)] });
        let layout_texture = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("décor 3D"), bind_group_layouts: &[&layout_global, &layout_objet, &layout_texture], push_constant_ranges: &[] });
        const ATTR: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let tampons = [wgpu::VertexBufferLayout { array_stride: 32, step_mode: wgpu::VertexStepMode::Vertex, attributes: &ATTR }];
        let principal = |samples: u32| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("décor 3D"),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &tampons, compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState { format: COULEUR, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: PROFONDEUR, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: Default::default() }),
                multisample: wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false },
                multiview: None,
                cache: None,
            })
        };
        let ciel = |samples: u32| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("décor 3D : ciel"),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_sky"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_sky"),
                    targets: &[Some(wgpu::ColorTargetState { format: COULEUR, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState { format: PROFONDEUR, depth_write_enabled: false, depth_compare: wgpu::CompareFunction::Always, stencil: Default::default(), bias: Default::default() }),
                multisample: wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false },
                multiview: None,
                cache: None,
            })
        };
        let ombre_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("décor 3D : ombres"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_shadow"), buffers: &tampons, compilation_options: Default::default() },
            fragment: None,
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: PROFONDEUR, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 } }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D poser"),
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
        let blit_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("décor 3D poser"), contents: bytemuck::cast_slice(&[0.0f32; 4]), usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST });
        let echant_base = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("décor 3D texture"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let echant_ombre = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("décor 3D ombres"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let echant_blit = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("décor 3D poser"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let blanc_tex = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("décor 3D blanc"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255, 255, 255, 255],
        );
        let blanc = groupe_texture(device, &layout_texture, &blanc_tex.create_view(&Default::default()), &echant_base);
        let ombre_vide = texture_profondeur(device, [1, 1], 1, true);
        Moteur {
            main1: principal(1),
            main4: principal(ECHANTILLONS_VUE),
            ciel1: ciel(1),
            ciel4: ciel(ECHANTILLONS_VUE),
            ombre_pipe,
            blit: None,
            blit_shader,
            blit_layout,
            blit_uniform,
            layout_global,
            layout_objet,
            layout_texture,
            echant_base,
            echant_ombre,
            echant_blit,
            blanc,
            ombre_vide,
            modeles: HashMap::new(),
            prises: HashMap::new(),
            tic: 0,
            max_texture: device.limits().max_texture_dimension_2d,
            dessines: 0,
        }
    }

    /// Le poser de la vue sur l'écran d'egui, au format `format` (la première fois, ou si le format change).
    pub fn assurer_blit(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) {
        if self.blit.as_ref().is_some_and(|b| b.0 == format) {
            return;
        }
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("décor 3D poser"), bind_group_layouts: &[&self.blit_layout], push_constant_ranges: &[] });
        // L'image du décor (prémultipliée, transparente hors du modèle) posée sur ce qu'egui a déjà dessiné : le fond et le
        // quadrillage de la vue restent visibles autour.
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
        };
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("décor 3D poser"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &self.blit_shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &self.blit_shader, entry_point: Some("fs"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        queue.write_buffer(&self.blit_uniform, 0, bytemuck::cast_slice(&[if format.is_srgb() { 1.0f32 } else { 0.0 }, 0.0, 0.0, 0.0]));
        self.blit = Some((format, pipe));
    }

    /// Envoie le décor `id` à la carte s'il n'y est pas (une fois). Les textures plus grandes que ce que la carte accepte y
    /// sont réduites.
    pub fn assurer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, id: u64, scene: &Scene3d) {
        self.tic += 1;
        let tic = self.tic;
        if let Some(mg) = self.modeles.get_mut(&id) {
            mg.utilise = tic;
            return;
        }
        let mut octets = 0u64;
        let textures: Vec<Option<TextureGpu>> = scene
            .textures
            .iter()
            .map(|t| {
                let t = if t.largeur.max(t.hauteur) > self.max_texture { reduire(t, self.max_texture) } else { t.clone() };
                let tex = device.create_texture_with_data(
                    queue,
                    &wgpu::TextureDescriptor {
                        label: Some("décor 3D texture"),
                        size: wgpu::Extent3d { width: t.largeur, height: t.hauteur, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    &t.rgba,
                );
                let o = t.largeur as u64 * t.hauteur as u64 * 4;
                octets += o;
                Some(TextureGpu { groupe: groupe_texture(device, &self.layout_texture, &tex.create_view(&Default::default()), &self.echant_base) })
            })
            .collect();
        let mut morceaux = Vec::new();
        for (pi, p) in scene.primitives.iter().enumerate() {
            for tris in p.indices.chunks(TRIANGLES_PAR_MORCEAU * 3) {
                // Un morceau a ses propres sommets (renumérotés) : aucun tampon ne dépasse ce que la carte accepte.
                let mut remap: HashMap<u32, u32> = HashMap::new();
                let mut data: Vec<f32> = Vec::new();
                let mut idx: Vec<u32> = Vec::with_capacity(tris.len());
                for &i in tris {
                    let j = *remap.entry(i).or_insert_with(|| {
                        let k = i as usize;
                        data.extend_from_slice(&p.positions[k]);
                        data.extend_from_slice(&p.normales[k]);
                        data.extend_from_slice(&p.uvs.get(k).copied().unwrap_or([0.0; 2]));
                        (data.len() / 8 - 1) as u32
                    });
                    idx.push(j);
                }
                let sommets = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("décor 3D sommets"), contents: bytemuck::cast_slice(&data), usage: wgpu::BufferUsages::VERTEX });
                let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("décor 3D indices"), contents: bytemuck::cast_slice(&idx), usage: wgpu::BufferUsages::INDEX });
                octets += data.len() as u64 * 4 + idx.len() as u64 * 4;
                morceaux.push(Morceau { sommets, indices, nombre: idx.len() as u32, prim: pi });
            }
        }
        let matieres = scene.primitives.iter().map(|p| (p.couleur, p.texture, p.seuil_alpha.unwrap_or(0.0))).collect();
        self.modeles.insert(id, ModeleGpu { morceaux, textures, matieres, octets, triangles: scene.triangles(), utilise: tic });
        // Les décors qui ne se montrent plus depuis longtemps partent (un autre projet, un décor enlevé).
        if self.modeles.len() > 8 {
            let plus_vieux = self.modeles.iter().filter(|(k, _)| **k != id).min_by_key(|(_, mg)| mg.utilise).map(|(k, _)| *k);
            if let Some(k) = plus_vieux {
                self.modeles.remove(&k);
            }
        }
    }

    /// Octets que les décors occupent dans la carte.
    pub fn octets(&self) -> u64 {
        self.modeles.values().map(|mg| mg.octets).sum()
    }

    fn prise(&mut self, device: &wgpu::Device, cle: u64, n: u64) -> &mut Prise3 {
        let tic = self.tic;
        let entry = self.prises.entry(cle).or_insert_with(|| {
            let globals = device.create_buffer(&wgpu::BufferDescriptor { label: Some("décor 3D globale"), size: GLOBAL, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let groupe_global = groupe_global(device, &self.layout_global, &globals, &self.ombre_vide, &self.echant_ombre);
            let (objets, groupe_objets) = tampon_objets(device, &self.layout_objet, 16);
            Prise3 { globals, groupe_global, objets, groupe_objets, capacite: 16, cibles: None, ombre: None, utilise: tic }
        });
        entry.utilise = tic;
        if n > entry.capacite {
            let cap = n.next_power_of_two();
            let (b, g) = tampon_objets(device, &self.layout_objet, cap);
            entry.objets = b;
            entry.groupe_objets = g;
            entry.capacite = cap;
        }
        entry
    }

    /// Dessine `dessins` vus par `vue` dans `couleur` (et sa profondeur `profondeur`, `echantillons` échantillons ; `resolue` :
    /// la cible où l'image lissée se pose). `cle` : la prise (ses réglages ne se mêlent à aucune autre). `qualite` : celle des
    /// ombres.
    #[allow(clippy::too_many_arguments)]
    fn dessiner_dans(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        cle: u64,
        dessins: &[Dessin],
        vue: &Vue,
        taille_unites: [f32; 2],
        taille_px: [u32; 2],
        couleur: &wgpu::TextureView,
        resolue: Option<&wgpu::TextureView>,
        profondeur: &wgpu::TextureView,
        echantillons: u32,
        ciel: bool,
        qualite: crate::decor3dbudget::Qualite,
    ) {
        for d in dessins {
            self.assurer(device, queue, d.id, &d.scene);
        }
        let aspect = dessins.first().map(|d| d.aspect).unwrap_or_default();
        // Proche et loin d'après les coins des décors placés : la profondeur garde sa précision.
        let (oeil, _, _, avant, _, _, ortho) = vue.axes();
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        let (mut dmin, mut dmax) = (f32::MAX, 0.0f32);
        for d in dessins {
            let (a, b) = d.scene.bornes();
            for i in 0..8 {
                let c = m::point(&d.matrice, [0, 1, 2].map(|k| if i >> k & 1 == 0 { a[k] } else { b[k] }));
                for k in 0..3 {
                    lo[k] = lo[k].min(c[k]);
                    hi[k] = hi[k].max(c[k]);
                }
                let z = m::dot(m::sub(c, oeil), avant);
                dmin = dmin.min(z);
                dmax = dmax.max(z);
                let _ = ortho;
            }
        }
        let loin = (dmax * 1.05).max(1.0);
        let proche = if ortho { dmin.min(0.0) - 1.0 } else { (dmin * 0.9).max(loin * 2e-5).max(0.005) };
        let vp = vue.decoupe(taille_unites, proche, loin);
        // La brume : de la distance du plus proche coin jusqu'au plus loin (relative au décor, quel que soit son échelle).
        let dist_max = (0..8).map(|i| m::length(m::sub([0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] }), oeil))).fold(0.0f32, f32::max).max(1e-3);
        let (brume_debut, brume_fin) = aspect.brume_de(dist_max);
        // Les ombres du soleil : une carte d'ombres, à l'orthographique sur la boîte des décors.
        let soleil = aspect.soleil();
        let (taps, cote_ombre) = qualite.ombres();
        let ombres = aspect.ombres && taps > 0 && lo[0] <= hi[0];
        let centre = m::scale(m::add(lo, hi), 0.5);
        let rayon = (m::length(m::sub(hi, lo)) * 0.5).max(1e-3);
        let light_vp = {
            let f = m::scale(soleil, -1.0);
            let r = m::normalize(m::cross(f, if f[1].abs() > 0.99 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }));
            let u = m::cross(r, f);
            let oeil_l = m::sub(centre, m::scale(f, rayon * 2.0));
            let v = Vue::Ortho { oeil: oeil_l, droite: r, haut: u, avant: f, px_par_unite: 1.0 / rayon, centre: [1.0, 1.0] };
            v.decoupe([2.0, 2.0], 0.0, rayon * 4.0)
        };
        let n: u64 = dessins.iter().map(|d| self.modeles.get(&d.id).map_or(0, |mg| mg.matieres.len() as u64)).sum::<u64>().max(1);
        // Les réglages de la passe.
        let (oe, r, u, f, s, c, est_ortho) = vue.axes();
        let mut g = vec![0u8; GLOBAL as usize];
        put(&mut g, 0, &vp);
        put(&mut g, 64, &light_vp);
        put(&mut g, 128, &[oe[0], oe[1], oe[2], if aspect.facettes { 1.0 } else { 0.0 }]);
        put(&mut g, 144, &[soleil[0], soleil[1], soleil[2], if ombres { taps as f32 } else { 0.0 }]);
        let kc = aspect.couleur_soleil();
        put(&mut g, 160, &[kc[0], kc[1], kc[2], 1.0 / cote_ombre.max(1) as f32]);
        let (sa, ga) = aspect.ambiances();
        put(&mut g, 176, &[sa[0], sa[1], sa[2], 0.0]);
        put(&mut g, 192, &[ga[0], ga[1], ga[2], 0.0]);
        let hz = aspect.horizon();
        put(&mut g, 208, &[hz[0], hz[1], hz[2], if aspect.brume { crate::decor3drendu::BRUME_MAX } else { 0.0 }]);
        put(&mut g, 224, &[brume_debut, brume_fin, if ciel { 1.0 } else { 0.0 }, 1.0]);
        let ze = aspect.zenith();
        put(&mut g, 240, &[ze[0], ze[1], ze[2], 0.0]);
        // (Le ciel lit les pixels de la cible : l'échelle et le centre y sont remis en pixels.)
        let kpx = taille_px[0] as f32 / taille_unites[0].max(1e-6);
        put(&mut g, 256, &[r[0], r[1], r[2], s * kpx]);
        put(&mut g, 272, &[u[0], u[1], u[2], c[0] * kpx]);
        put(&mut g, 288, &[f[0], f[1], f[2], c[1] * kpx]);
        put(&mut g, 304, &[taille_px[0] as f32, taille_px[1] as f32, if est_ortho { 1.0 } else { 0.0 }, 0.0]);
        let mut objets = vec![0u8; (n * PAS_OBJET) as usize];
        let mut rang = 0usize;
        for d in dessins {
            let Some(mg) = self.modeles.get(&d.id) else { continue };
            let nm = crate::decor3drendu::normal_matrix(&d.matrice);
            for &(couleur, tex, seuil) in &mg.matieres {
                let at = rang * PAS_OBJET as usize;
                put(&mut objets, at, &d.matrice);
                put(&mut objets, at + 64, &nm);
                put(&mut objets, at + 128, &couleur);
                put(&mut objets, at + 144, &[if tex.is_some() { 1.0 } else { 0.0 }, seuil, 0.0, 0.0]);
                rang += 1;
            }
        }
        // La carte d'ombres de cette prise.
        if ombres {
            let (layout, echant) = (self.layout_global.clone(), self.echant_ombre.clone());
            let p = self.prise(device, cle, n);
            if p.ombre.as_ref().is_none_or(|(c, _)| *c != cote_ombre) {
                p.ombre = Some((cote_ombre, texture_profondeur(device, [cote_ombre, cote_ombre], 1, true)));
                p.groupe_global = groupe_global(device, &layout, &p.globals, &p.ombre.as_ref().expect("ombre").1, &echant);
            }
        }
        let p = self.prise(device, cle, n);
        queue.write_buffer(&p.globals, 0, &g);
        queue.write_buffer(&p.objets, 0, &objets);
        let (groupe_global, groupe_objets) = (p.groupe_global.clone(), p.groupe_objets.clone());
        let ombre_vue = p.ombre.as_ref().map(|(_, v)| v.clone());
        let mut dessines = 0;
        // Les passes : d'abord la carte d'ombres, puis le ciel et le décor.
        if let (true, Some(ov)) = (ombres, ombre_vue) {
            // (La carte d'ombres ne peut pas être lue pendant qu'on y écrit : le groupe global lit la vide le temps de la passe.)
            let groupe_vide = groupe_global_vide(device, &self.layout_global, &self.prises[&cle].globals, &self.ombre_vide, &self.echant_ombre);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("décor 3D : ombres"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &ov, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.ombre_pipe);
            pass.set_bind_group(0, &groupe_vide, &[]);
            pass.set_bind_group(2, &self.blanc, &[]);
            self.poser(&mut pass, dessins, &groupe_objets, &mut 0, false);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("décor 3D"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: couleur,
                resolve_target: resolue,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: if resolue.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: profondeur, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &groupe_global, &[]);
        if ciel {
            pass.set_pipeline(if echantillons > 1 { &self.ciel4 } else { &self.ciel1 });
            pass.set_bind_group(1, &groupe_objets, &[0]);
            pass.set_bind_group(2, &self.blanc, &[]);
            pass.draw(0..3, 0..1);
        }
        pass.set_pipeline(if echantillons > 1 { &self.main4 } else { &self.main1 });
        self.poser(&mut pass, dessins, &groupe_objets, &mut dessines, true);
        drop(pass);
        self.dessines = dessines;
    }

    fn poser(&self, pass: &mut wgpu::RenderPass<'_>, dessins: &[Dessin], groupe_objets: &wgpu::BindGroup, dessines: &mut usize, textures: bool) {
        let mut base = 0usize;
        for d in dessins {
            let Some(mg) = self.modeles.get(&d.id) else { continue };
            for mc in &mg.morceaux {
                let rang = base + mc.prim;
                pass.set_bind_group(1, groupe_objets, &[(rang as u64 * PAS_OBJET) as u32]);
                if textures {
                    let groupe = mg.matieres[mc.prim].1.and_then(|t| mg.textures.get(t)).and_then(|t| t.as_ref()).map_or(&self.blanc, |t| &t.groupe);
                    pass.set_bind_group(2, groupe, &[]);
                }
                pass.set_vertex_buffer(0, mc.sommets.slice(..));
                pass.set_index_buffer(mc.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mc.nombre, 0, 0..1);
                *dessines += mc.nombre as usize / 3;
            }
            base += mg.matieres.len();
        }
    }

    /// L'image du décor pour la composition de l'image du plan (`gpu.rs`) : dans `cible` (une texture de la taille de la
    /// feuille, `gpufx::target_texture`), vue de l'œil de la caméra, alignée sur la feuille. Pas de lissage : l'image est
    /// réduite à l'écran.
    #[allow(clippy::too_many_arguments)]
    pub fn dessiner_feuille(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, cle: u64, dessins: &[Dessin], prise: &Prise, taille: [u32; 2], cible: &wgpu::TextureView, ciel: bool, qualite: crate::decor3dbudget::Qualite) {
        let profondeur = self.cibles(device, cle, taille, 1, false).profondeur.clone();
        self.dessiner_dans(device, queue, encoder, cle, dessins, &Vue::Prise(*prise), [taille[0] as f32, taille[1] as f32], taille, cible, None, &profondeur, 1, ciel, qualite);
    }

    fn cibles(&mut self, device: &wgpu::Device, cle: u64, taille: [u32; 2], echantillons: u32, avec_couleur: bool) -> &Cibles {
        let layout = self.blit_layout.clone();
        let (uniform, echant) = (self.blit_uniform.clone(), self.echant_blit.clone());
        let p = self.prise(device, cle, 1);
        if p.cibles.as_ref().is_none_or(|c| c.taille != taille || c.echantillons != echantillons) {
            let profondeur = texture_profondeur(device, taille, echantillons, false);
            let (couleur, resolue) = if avec_couleur {
                let couleur = texture_couleur(device, taille, echantillons, false);
                let resolue = texture_couleur(device, taille, 1, true);
                let groupe = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("décor 3D poser"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&resolue) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&echant) },
                        wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
                    ],
                });
                (couleur, Some((resolue, groupe)))
            } else {
                (profondeur.clone(), None)
            };
            p.cibles = Some(Cibles { taille, echantillons, couleur, profondeur, resolue });
        }
        p.cibles.as_ref().expect("cibles")
    }

    /// L'image de la vue Espace 3D : dans les cibles lissées de la prise `cle`, à poser ensuite par `poser_vue`.
    #[allow(clippy::too_many_arguments)]
    pub fn dessiner_vue(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, cle: u64, dessins: &[Dessin], vue: &Vue, taille_pts: [f32; 2], taille_px: [u32; 2], qualite: crate::decor3dbudget::Qualite) {
        let c = self.cibles(device, cle, taille_px, ECHANTILLONS_VUE, true);
        let (couleur, profondeur, resolue) = (c.couleur.clone(), c.profondeur.clone(), c.resolue.as_ref().map(|r| r.0.clone()));
        self.dessiner_dans(device, queue, encoder, cle, dessins, vue, taille_pts, taille_px, &couleur, resolue.as_ref(), &profondeur, ECHANTILLONS_VUE, false, qualite);
    }

    /// Pose l'image de la vue `cle` dans la passe d'egui.
    pub fn poser_vue(&self, pass: &mut wgpu::RenderPass<'static>, cle: u64) {
        let (Some((_, blit)), Some(groupe)) = (self.blit.as_ref(), self.prises.get(&cle).and_then(|p| p.cibles.as_ref()).and_then(|c| c.resolue.as_ref()).map(|r| &r.1)) else { return };
        pass.set_pipeline(blit);
        pass.set_bind_group(0, groupe, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Les prises qui ne servent plus (une vue multiple fermée) rendent leurs cibles.
    pub fn menage(&mut self) {
        let tic = self.tic;
        self.prises.retain(|_, p| tic.saturating_sub(p.utilise) < 600);
    }
}

fn groupe_texture(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, view: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("décor 3D texture"),
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) }],
    })
}

fn groupe_global(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, buffer: &wgpu::Buffer, ombre: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("décor 3D globale"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(ombre) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn groupe_global_vide(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, buffer: &wgpu::Buffer, vide: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
    groupe_global(device, layout, buffer, vide, sampler)
}

fn tampon_objets(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, capacite: u64) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("décor 3D objets"), size: PAS_OBJET * capacite, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("décor 3D objets"),
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buffer, offset: 0, size: NonZeroU64::new(OBJET) }) }],
    });
    (buffer, group)
}

fn texture_profondeur(device: &wgpu::Device, taille: [u32; 2], echantillons: u32, lue: bool) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("décor 3D profondeur"),
            size: wgpu::Extent3d { width: taille[0].max(1), height: taille[1].max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: echantillons,
            dimension: wgpu::TextureDimension::D2,
            format: PROFONDEUR,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | if lue { wgpu::TextureUsages::TEXTURE_BINDING } else { wgpu::TextureUsages::empty() },
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn texture_couleur(device: &wgpu::Device, taille: [u32; 2], echantillons: u32, lue: bool) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("décor 3D couleur"),
            size: wgpu::Extent3d { width: taille[0].max(1), height: taille[1].max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: echantillons,
            dimension: wgpu::TextureDimension::D2,
            format: COULEUR,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | if lue { wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC } else { wgpu::TextureUsages::empty() },
            view_formats: &[],
        })
        .create_view(&Default::default())
}

// ------------------------------------------------------------------------------------------- la vue Espace 3D

/// Ce qu'une vue Espace 3D demande à la carte pour son décor (`App::e3_peindre_decor`).
pub struct VueDecor {
    pub cle: u64,
    pub dessins: Vec<Dessin>,
    pub vue: Vue,
    pub taille_pts: [f32; 2],
    pub cible_egui: wgpu::TextureFormat,
    pub qualite: crate::decor3dbudget::Qualite,
}

impl CallbackTrait for VueDecor {
    fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue, screen: &egui_wgpu::ScreenDescriptor, encoder: &mut wgpu::CommandEncoder, resources: &mut CallbackResources) -> Vec<wgpu::CommandBuffer> {
        if resources.get::<Moteur>().is_none() {
            resources.insert(Moteur::new(device, queue));
        }
        let Some(mt) = resources.get_mut::<Moteur>() else { return Vec::new() };
        mt.assurer_blit(device, queue, self.cible_egui);
        let ppp = screen.pixels_per_point;
        let px = [((self.taille_pts[0] * ppp).round() as u32).max(1), ((self.taille_pts[1] * ppp).round() as u32).max(1)];
        mt.dessiner_vue(device, queue, encoder, self.cle, &self.dessins, &self.vue, self.taille_pts, px, self.qualite);
        mt.menage();
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        if let Some(mt) = resources.get::<Moteur>() {
            mt.poser_vue(pass, self.cle);
        }
    }
}

// ------------------------------------------------------------------------------------------- rendu hors écran, mesures

/// Rend `dessins` vus par `prise` à `taille` pixels, hors de l'écran, et relit l'image (RGBA prémultiplié, sRGB) : les exports
/// avec une carte graphique (la même image qu'à l'écran) et les essais. `fois` : nombre d'images à rendre (mesures) ; renvoie
/// aussi la durée d'une image, en millisecondes (si `fois` > 1 ; première image, celle de l'envoi des maillages, exclue).
#[allow(clippy::too_many_arguments)]
pub fn rendre_hors_ecran(device: &wgpu::Device, queue: &wgpu::Queue, mt: &mut Moteur, dessins: &[Dessin], prise: &Prise, taille: [u32; 2], ciel: bool, qualite: crate::decor3dbudget::Qualite, fois: usize) -> Option<(Vec<u8>, f32)> {
    let max = device.limits().max_texture_dimension_2d;
    if taille[0] > max || taille[1] > max || taille[0] == 0 || taille[1] == 0 {
        return None;
    }
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("décor 3D hors écran"),
        size: wgpu::Extent3d { width: taille[0], height: taille[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COULEUR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    // Une première image : les maillages et les textures partent à la carte (une fois), rien n'est compté.
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("décor 3D hors écran") });
    mt.dessiner_feuille(device, queue, &mut enc, u64::MAX - 7, dessins, prise, taille, &view, ciel, qualite);
    queue.submit([enc.finish()]);
    let _ = device.poll(wgpu::Maintain::Wait);
    // Les mesures : `fois` images dans une seule soumission (comme à l'écran, où la carte enchaîne les images sans attendre le
    // processeur : l'attente de fin de soumission, une milliseconde ou plus selon le pilote, n'est pas comptée `fois` fois),
    // trois fois, la plus rapide gardée (une autre application qui dessine au même moment ne fausse pas tout).
    let mut meilleur = f32::MAX;
    if fois > 1 {
        for _ in 0..3 {
            let debut = std::time::Instant::now();
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("décor 3D mesure") });
            for _ in 0..fois {
                mt.dessiner_feuille(device, queue, &mut enc, u64::MAX - 7, dessins, prise, taille, &view, ciel, qualite);
            }
            queue.submit([enc.finish()]);
            let _ = device.poll(wgpu::Maintain::Wait);
            meilleur = meilleur.min(debut.elapsed().as_secs_f32() * 1000.0 / fois as f32);
        }
    }
    let total = if meilleur.is_finite() { meilleur } else { 0.0 };
    let ligne = (taille[0] * 4).div_ceil(256) * 256;
    let lecture = device.create_buffer(&wgpu::BufferDescriptor { label: Some("décor 3D relu"), size: ligne as u64 * taille[1] as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("décor 3D relu") });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &lecture, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(ligne), rows_per_image: Some(taille[1]) } },
        wgpu::Extent3d { width: taille[0], height: taille[1], depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    let tranche = lecture.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    tranche.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = device.poll(wgpu::Maintain::Wait);
    rx.recv().ok()?.ok()?;
    let data = tranche.get_mapped_range();
    let mut out = Vec::with_capacity((taille[0] * taille[1] * 4) as usize);
    for y in 0..taille[1] as usize {
        out.extend_from_slice(&data[y * ligne as usize..y * ligne as usize + taille[0] as usize * 4]);
    }
    drop(data);
    lecture.unmap();
    Some((out, total))
}
