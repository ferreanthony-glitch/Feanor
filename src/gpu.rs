//! Superposition des calques par la carte graphique (wgpu).
//!
//! Deux temps, comme le calcul sur processeur qu'il remplace à l'écran :
//! 1. **composer** : le papier puis les calques (opacité, teinte de la pelure d'oignon) sont
//!    empilés dans une image à la taille du projet, pixel pour pixel ;
//! 2. **afficher** : cette image est posée à l'écran avec la vue (zoom, rotation, miroir), lissée
//!    comme egui lisse ses images.
//!
//! Chaque dessin affiché est une texture, **à la taille de son contenu** (la boîte des tuiles
//! dessinées, avec un pixel transparent de marge, `texture_rect`) et non de la feuille entière :
//! 1 Mo au lieu de 33 pour un dessin au trait, donc 30 à 40 fois plus de dessins gardés. Son
//! rectangle (`rect`) dit quelle partie du dessin elle montre ; le dessin sur l'image composée
//! (`paint`) en tient compte pour les coins du rectangle (transformation du calque, feuille
//! flottante, aperçu réduit d'effet) et pour les coordonnées de texture des maillages. Un dessin qui
//! sort de sa boîte, on le peint ou on l'agrandit, refait sa texture un peu plus grande et renvoie
//! le dessin entier (`place_texture`). On n'envoie que ses tuiles dessinées (et on efface
//! celles qui ne le sont plus), toutes regroupées en une seule copie ; pendant un trait, seulement
//! les tuiles touchées. Pendant la lecture, les dessins des images suivantes partent d'avance, par
//! petits morceaux (`playback.rs`). Le calcul sur processeur (`render.rs`) reste pour l'export,
//! les vignettes, la pipette, le pot de peinture, et en secours. L'étape « composer » est aussi
//! celle où viendront la caméra et le multiplan.

use std::collections::HashMap;
use std::num::NonZeroU64;

use eframe::egui::{self, Pos2, Rect};
use eframe::egui_wgpu::{self, wgpu, CallbackResources, CallbackTrait, RenderState};

use crate::document::{CelId, Document};
use crate::raster::{tile_bounds, Raster, TILE};
use crate::render::Plan;

/// Réglages d'un dessin dans le tampon : 4 coins (position, coordonnées de texture), une couleur,
/// des paramètres = 6 vec4 = 96 octets.
const ITEM_BYTES: u64 = 96;
/// Au-delà, les textures les moins récemment affichées sont libérées.
const CACHE_BYTES: u64 = 1 << 30;
/// Envoi d'avance, par image d'écran : 4 Mo de tuiles (256 tuiles), quelques millisecondes.
const AHEAD_BYTES: usize = 4 << 20;
/// Taille au-delà de laquelle le tampon d'envoi n'est pas gardé pour l'image suivante.
const STAGING_KEEP: usize = 48 << 20;
/// Un sommet de maillage : position normalisée et coordonnées de texture, 4 flottants.
const MESH_VERTEX_BYTES: u64 = 16;
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
    let k = order[i];
    let c = item.corners[k];
    // Affichage en perspective (caméra tournée, 9f) : params.w = 1 et le w de chaque coin dans
    // color ; la carte graphique interpole alors la texture en perspective et coupe ce qui
    // passe derrière la caméra.
    var w = 1.0;
    if (item.params.w > 0.5) {
        w = item.color[k];
    }
    var out: VOut;
    out.pos = vec4<f32>(c.xy * w, 0.0, w);
    out.uv = c.zw;
    return out;
}

// Feuille déformée par un maillage (enveloppe, perspective ; `warp.rs`) : les sommets portent leur
// place normalisée dans l'image composée et leurs coordonnées de texture ; mêmes fragments.
struct MeshIn {
    @location(0) v: vec4<f32>,
};

@vertex
fn vs_mesh(m: MeshIn) -> VOut {
    var out: VOut;
    out.pos = vec4<f32>(m.v.xy, 0.0, 1.0);
    out.uv = m.v.zw;
    return out;
}

// Composer. Couleurs prémultipliées, en valeurs gamma, comme les dessins. params.x : opacité ;
// params.y : 1 = dessin, 0 = papier ; color.w : 1 = teinte de pelure d'oignon (color.rgb).
// Fond pour les modes de fusion (8) : ce qui est déjà composé, copié avant le calque.
@group(2) @binding(0) var back_tex: texture_2d<f32>;

// Modes de fusion : formules de la norme W3C « Compositing and Blending », comme `blend.rs`
// (même numérotation que `BlendMode::ALL`).
fn lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.3, 0.59, 0.11));
}
fn clip_color(c: vec3<f32>) -> vec3<f32> {
    let l = lum(c);
    let n = min(min(c.r, c.g), c.b);
    let x = max(max(c.r, c.g), c.b);
    var o = c;
    if (n < 0.0) {
        o = vec3<f32>(l) + (o - vec3<f32>(l)) * l / max(l - n, 1e-6);
    }
    if (x > 1.0) {
        o = vec3<f32>(l) + (o - vec3<f32>(l)) * (1.0 - l) / max(x - l, 1e-6);
    }
    return o;
}
fn set_lum(c: vec3<f32>, l: f32) -> vec3<f32> {
    return clip_color(c + vec3<f32>(l - lum(c)));
}
fn sat(c: vec3<f32>) -> f32 {
    return max(max(c.r, c.g), c.b) - min(min(c.r, c.g), c.b);
}
fn set_sat(c: vec3<f32>, s: f32) -> vec3<f32> {
    let mx = max(max(c.r, c.g), c.b);
    let mn = min(min(c.r, c.g), c.b);
    if (mx <= mn) {
        return vec3<f32>(0.0);
    }
    return (c - vec3<f32>(mn)) * s / (mx - mn);
}
fn hard_light(cb: f32, cs: f32) -> f32 {
    if (cs <= 0.5) {
        return cb * 2.0 * cs;
    }
    let s = 2.0 * cs - 1.0;
    return cb + s - cb * s;
}
fn color_burn(cb: f32, cs: f32) -> f32 {
    if (cb >= 1.0) { return 1.0; }
    if (cs <= 0.0) { return 0.0; }
    return 1.0 - min((1.0 - cb) / cs, 1.0);
}
fn color_dodge(cb: f32, cs: f32) -> f32 {
    if (cb <= 0.0) { return 0.0; }
    if (cs >= 1.0) { return 1.0; }
    return min(cb / (1.0 - cs), 1.0);
}
fn channel(mode: u32, cb: f32, cs: f32) -> f32 {
    switch mode {
        case 2u: { return min(cb, cs); }
        case 3u: { return cb * cs; }
        case 4u: { return color_burn(cb, cs); }
        case 5u: { return max(cb + cs - 1.0, 0.0); }
        case 7u: { return max(cb, cs); }
        case 8u: { return cb + cs - cb * cs; }
        case 9u: { return color_dodge(cb, cs); }
        case 10u: { return min(cb + cs, 1.0); }
        case 12u: { return hard_light(cs, cb); }
        case 13u: {
            if (cs <= 0.5) {
                return cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb);
            }
            var d = sqrt(cb);
            if (cb <= 0.25) {
                d = ((16.0 * cb - 12.0) * cb + 4.0) * cb;
            }
            return cb + (2.0 * cs - 1.0) * (d - cb);
        }
        case 14u: { return hard_light(cb, cs); }
        case 15u: {
            if (cs <= 0.5) { return color_burn(cb, 2.0 * cs); }
            return color_dodge(cb, 2.0 * cs - 1.0);
        }
        case 16u: { return clamp(cb + 2.0 * cs - 1.0, 0.0, 1.0); }
        case 17u: {
            if (cs <= 0.5) { return min(cb, 2.0 * cs); }
            return max(cb, 2.0 * cs - 1.0);
        }
        case 18u: {
            if (cb + cs >= 1.0) { return 1.0; }
            return 0.0;
        }
        case 19u: { return abs(cb - cs); }
        case 20u: { return cb + cs - 2.0 * cb * cs; }
        case 21u: { return max(cb - cs, 0.0); }
        case 22u: {
            if (cs <= 0.0) {
                if (cb > 0.0) { return 1.0; }
                return 0.0;
            }
            return min(cb / cs, 1.0);
        }
        default: { return cs; }
    }
}
fn mix_rgb(mode: u32, cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    switch mode {
        case 6u: {
            if (lum(cs) < lum(cb)) { return cs; }
            return cb;
        }
        case 11u: {
            if (lum(cs) > lum(cb)) { return cs; }
            return cb;
        }
        case 23u: { return set_lum(set_sat(cs, sat(cb)), lum(cb)); }
        case 24u: { return set_lum(set_sat(cb, sat(cs)), lum(cb)); }
        case 25u: { return set_lum(cs, lum(cb)); }
        case 26u: { return set_lum(cb, lum(cs)); }
        default: {
            return vec3<f32>(channel(mode, cb.r, cs.r), channel(mode, cb.g, cs.g), channel(mode, cb.b, cs.b));
        }
    }
}
// Bruit du Fondu au pixel (x, y) : le même calcul que `blend::dissolve_noise`.
fn dissolve_noise(x: u32, y: u32) -> f32 {
    var h = (x * 0x8da6b343u) ^ (y * 0xd8163841u);
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) / 16777216.0;
}

// Composer un calque en mode de fusion : le fond est lu au même pixel et le résultat le
// remplace (pas de mélange matériel). params.x : opacité ; params.z : le mode.
@fragment
fn fs_blend(v: VOut) -> @location(0) vec4<f32> {
    let raw = textureSample(tex, samp, v.uv);
    let b = textureLoad(back_tex, vec2<i32>(floor(v.pos.xy)), 0);
    let sa = raw.a * item.params.x;
    if (sa <= 0.0) {
        return b;
    }
    let cs = min(raw.rgb / raw.a, vec3<f32>(1.0));
    let mode = u32(item.params.z + 0.5);
    // Fondu : un point plein, ou le fond.
    if (mode == 1u) {
        let p = vec2<u32>(floor(v.pos.xy));
        if (dissolve_noise(p.x, p.y) < sa) {
            return vec4<f32>(cs, 1.0);
        }
        return b;
    }
    let ab = b.a;
    var cb = vec3<f32>(0.0);
    if (ab > 0.0) {
        cb = min(b.rgb / ab, vec3<f32>(1.0));
    }
    let mixed = clamp(mix_rgb(mode, cb, cs), vec3<f32>(0.0), vec3<f32>(1.0));
    let c = (1.0 - ab) * cs + ab * mixed;
    let co = sa * c + (1.0 - sa) * ab * cb;
    let ao = sa + ab * (1.0 - sa);
    return vec4<f32>(co, ao);
}

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

// En réduisant fort (vignette, vue très éloignée), une seule lecture par pixel d'écran sauterait
// des pixels de l'image et les traits fins disparaîtraient : on moyenne n×n lectures réparties
// sur ce que couvre le pixel (n selon la réduction, 8 au plus).
// Transitions (`transitions.rs`) : params.x est l'opacité de l'image posée ; params.y le mode
// (1 : image, 0 : aplat de gris params.z, par exemple le noir d'un fondu au noir, 2 : volet, l'image
// ne se voit qu'à gauche de la colonne params.z de la feuille). Une image seule : x = 1, y = 1.
@fragment
fn fs_display(v: VOut) -> @location(0) vec4<f32> {
    if (item.params.y < 0.5) {
        return vec4<f32>(vec3<f32>(item.params.z), 1.0) * item.params.x;
    }
    let dx = dpdx(v.uv);
    let dy = dpdy(v.uv);
    let size = vec2<f32>(textureDimensions(tex));
    let texels = max(length(dx * size), length(dy * size));
    let n = i32(clamp(ceil(texels / 1.5), 1.0, 8.0));
    var acc = vec4<f32>(0.0);
    for (var j = 0; j < n; j++) {
        for (var i = 0; i < n; i++) {
            let o = (vec2<f32>(f32(i), f32(j)) + 0.5) / f32(n) - 0.5;
            acc += textureSampleLevel(tex, samp, v.uv + dx * o.x + dy * o.y, 0.0);
        }
    }
    let lin = acc / f32(n * n);
    // (Le volet écarte les pixels après les dérivées : elles se calculent en flot uniforme.)
    if (item.params.y > 1.5 && v.uv.x > item.params.z) {
        discard;
    }
    return vec4<f32>(gamma_from_linear_rgb(lin.rgb), lin.a) * item.params.x;
}
"#;

struct CelTexture {
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    /// La partie du dessin que la texture montre : coin haut-gauche (x, y) et taille (largeur,
    /// hauteur), en pixels du dessin. Une texture n'a que la place du contenu (`texture_rect`), pas
    /// celle de la feuille entière.
    rect: [u32; 4],
    /// Taille de l'image du dessin (feuille, aperçu réduit d'un effet, feuille flottante agrandie).
    dims: (u32, u32),
    /// Version du dessin envoyée (`u64::MAX` : rien d'envoyé).
    version: u64,
    last_used: u64,
    /// Dernière image où elle a reçu des pixels, ou été refaite : l'image composée gardée n'est plus
    /// bonne si elle change (`ItemKey`).
    generation: u64,
    /// Tuiles non vides dans la texture (une texture neuve est transparente) : seules celles-ci
    /// sont à effacer quand le dessin en perd. Rangées comme les tuiles du dessin.
    filled: Vec<bool>,
    /// Envoi d'avance en cours : version visée et prochaine tuile.
    partial: Option<(u64, usize)>,
}

impl CelTexture {
    /// Poids de la texture sur la carte.
    fn bytes(&self) -> u64 {
        self.rect[2] as u64 * self.rect[3] as u64 * 4
    }
}

/// Multiple de pixels auquel la taille d'une texture est arrondie (vers le côté libre) : des
/// dessins de contenu voisin ont ainsi la même taille et se recyclent l'un l'autre.
const BUCKET: usize = 128;

/// Les tuiles non vides d'un dessin : (tuile gauche, haute, droite, basse), bornes comprises.
pub(crate) fn content_tiles(raster: &Raster) -> Option<[usize; 4]> {
    let mut found: Option<[usize; 4]> = None;
    for (idx, t) in raster.tiles.iter().enumerate() {
        if t.is_some() {
            let (tx, ty) = (idx % raster.tiles_x, idx / raster.tiles_x);
            found = Some(match found {
                None => [tx, ty, tx, ty],
                Some([a, b, c, d]) => [a.min(tx), b.min(ty), c.max(tx), d.max(ty)],
            });
        }
    }
    found
}

/// Rectangle de pixels d'un tel contenu : [x0, y0, x1, y1) (x1 et y1 exclus), tuiles entières.
pub(crate) fn content_pixels(dims: (u32, u32), tiles: [usize; 4]) -> [usize; 4] {
    [tiles[0] * TILE, tiles[1] * TILE, ((tiles[2] + 1) * TILE).min(dims.0 as usize), ((tiles[3] + 1) * TILE).min(dims.1 as usize)]
}

/// Un axe de la texture : de `lo` à `hi` (exclu) dans `total` pixels. La texture déborde d'un pixel
/// transparent de chaque côté qui n'est pas le bord de la feuille (le lissage d'un calque transformé
/// y lit du transparent, comme dans la feuille entière ; au bord de la feuille, il lit le bord, comme
/// avant), puis la taille est arrondie à `BUCKET` du côté libre, sans sortir de la feuille.
fn texture_axis(lo: usize, hi: usize, total: usize) -> (usize, usize) {
    let (before, after) = (usize::from(lo > 0), usize::from(hi < total));
    let (mut origin, mut len) = (lo - before, hi - lo + before + after);
    let want = len.div_ceil(BUCKET) * BUCKET;
    if want > len {
        if after == 1 {
            len = want.min(total - origin);
        } else if before == 1 && origin >= want - len {
            origin -= want - len;
            len = want;
        }
    }
    (origin, len)
}

/// Le rectangle de la texture d'un dessin de taille `dims` dont le contenu occupe les tuiles
/// `tiles`, avec `slack` pixels de plus de chaque côté (un dessin qu'on est en train d'agrandir :
/// la texture ne se refait pas à chaque tuile de plus). Dessin vide : un pixel.
pub(crate) fn texture_rect(dims: (u32, u32), tiles: Option<[usize; 4]>, slack: usize) -> [u32; 4] {
    let Some(tiles) = tiles else { return [0, 0, 1, 1] };
    let [x0, y0, x1, y1] = content_pixels(dims, tiles);
    let (x0, y0, x1, y1) = (x0.saturating_sub(slack), y0.saturating_sub(slack), (x1 + slack).min(dims.0 as usize), (y1 + slack).min(dims.1 as usize));
    let (ox, w) = texture_axis(x0, x1, dims.0 as usize);
    let (oy, h) = texture_axis(y0, y1, dims.1 as usize);
    [ox as u32, oy as u32, w as u32, h as u32]
}

/// La texture `rect` montre-t-elle tout ce contenu (pixels [x0, y0, x1, y1)) ? Il doit rester un
/// pixel de transparent entre lui et chaque bord de la texture qui n'est pas celui de la feuille.
pub(crate) fn covers(rect: [u32; 4], dims: (u32, u32), content: Option<[usize; 4]>) -> bool {
    let Some([x0, y0, x1, y1]) = content else { return true };
    let axis = |lo: usize, hi: usize, o: u32, len: u32, total: u32| (o == 0 || (o as usize) < lo) && (o + len == total || hi < (o + len) as usize);
    axis(x0, x1, rect[0], rect[2], dims.0) && axis(y0, y1, rect[1], rect[3], dims.1)
}

/// Image composée, à la taille du projet.
struct Frame {
    size: (u32, u32),
    texture: wgpu::Texture,
    target: wgpu::TextureView,
    /// Lecture en sRGB pour l'affichage.
    display_group: wgpu::BindGroup,
    /// Copie de l'image composée jusque-là, lue par les calques en mode de fusion (8).
    backdrop: wgpu::Texture,
    backdrop_group: wgpu::BindGroup,
    /// Ce qui a servi à composer l'image qu'elle contient (rien : elle est vide ou à refaire).
    composed: Option<Vec<ItemKey>>,
}

/// Ressources gardées par egui-wgpu d'une image à l'autre.
struct CanvasResources {
    compose: wgpu::RenderPipeline,
    /// Composer un calque en mode de fusion (lit le fond, remplace).
    blend: wgpu::RenderPipeline,
    /// Les mêmes, pour une feuille posée par un maillage (sommets au lieu de quatre coins).
    compose_mesh: wgpu::RenderPipeline,
    blend_mesh: wgpu::RenderPipeline,
    /// Sommets des maillages de l'image, et sa taille en octets.
    mesh_vertices: Option<(wgpu::Buffer, u64)>,
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
    /// Mémoire visée pour les textures des dessins (`CACHE_BYTES`, plus petite dans les essais).
    cache_bytes: u64,
    /// Essais : chaque texture couvre l'image entière de son dessin, comme avant la taille au
    /// contenu (pour comparer les deux, pixel pour pixel).
    whole_textures: bool,
    /// Tampon des tuiles à envoyer, gardé d'une image à l'autre (vide entre deux).
    staging: Vec<u8>,
    /// Essais : un tampon neuf à chaque image, comme avant (pour mesurer ce que le garder rapporte).
    fresh_staging: bool,
    /// Tuiles envoyées depuis le début (pour les mesures).
    copies: u64,
    /// Compositions faites depuis le début (celles qu'une image composée gardée évite ne comptent pas).
    composes: u64,
    frame: Option<Frame>,
    /// Le second plan d'une transition (fondu enchaîné, volet), composé à part ; créé à la demande.
    frame2: Option<Frame>,
    tick: u64,
}

/// Le contenu du cache de textures (`GpuCanvas::stats`).
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuStats {
    pub textures: usize,
    pub bytes: u64,
    /// Tuiles envoyées à la carte depuis le début.
    pub copies: u64,
    /// Compositions des calques faites depuis le début.
    pub composes: u64,
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
        // Modes de fusion : un troisième groupe pour le fond, et pas de mélange matériel.
        let blend_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("calques fusion"),
            bind_group_layouts: &[&uniform_layout, &texture_layout, &texture_layout],
            push_constant_ranges: &[],
        });
        let blend = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fs_blend"),
            layout: Some(&blend_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_blend"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FRAME_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        // Feuille posée par un maillage : sommets (x, y, u, v) en lieu de quatre coins.
        let mesh_attributes = wgpu::vertex_attr_array![0 => Float32x4];
        let mesh_buffers = [wgpu::VertexBufferLayout {
            array_stride: MESH_VERTEX_BYTES,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &mesh_attributes,
        }];
        let mesh_pipeline = |entry: &str, layout: &wgpu::PipelineLayout, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_mesh"),
                    buffers: &mesh_buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState { format: FRAME_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let compose_mesh = mesh_pipeline("fs_compose", &layout, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        let blend_mesh = mesh_pipeline("fs_blend", &blend_layout, None);
        // Bilinéaire : un calque transformé est lissé ; un calque tel quel tombe pile sur les pixels.
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("composer"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
            blend,
            compose_mesh,
            blend_mesh,
            mesh_vertices: None,
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
            cache_bytes: CACHE_BYTES,
            whole_textures: false,
            staging: Vec::new(),
            fresh_staging: false,
            copies: 0,
            composes: 0,
            frame: None,
            frame2: None,
            tick: 0,
        });
        Self {
            render_state: rs.clone(),
        }
    }

    /// L'état de rendu partagé avec eframe (la scène 3D y installe ses propres ressources).
    pub fn render_state(&self) -> &RenderState {
        &self.render_state
    }

    /// La mémoire visée pour les textures des dessins (un Go d'ordinaire ; les Préférences et le
    /// diagnostic des performances la règlent). Les plus anciennes partent à la prochaine image si
    /// elle est dépassée.
    pub fn set_cache_bytes(&self, bytes: u64) {
        if let Some(res) = self.render_state.renderer.write().callback_resources.get_mut::<CanvasResources>() {
            res.cache_bytes = bytes;
        }
    }

    /// Essais : les textures couvrent l'image entière de leur dessin (le comportement d'avant la
    /// taille au contenu). Les textures déjà là sont jetées.
    #[cfg(test)]
    pub fn set_whole_textures(&self, whole: bool) {
        if let Some(res) = self.render_state.renderer.write().callback_resources.get_mut::<CanvasResources>() {
            res.whole_textures = whole;
            res.textures.clear();
        }
    }

    /// Essais : un tampon d'envoi neuf à chaque image (le comportement d'avant).
    #[cfg(test)]
    pub fn set_fresh_staging(&self, fresh: bool) {
        if let Some(res) = self.render_state.renderer.write().callback_resources.get_mut::<CanvasResources>() {
            res.fresh_staging = fresh;
        }
    }

    /// Ce que le cache garde : nombre de textures de dessin et leur poids (octets de la carte).
    /// Sert aux mesures (`perf_tests.rs`) et aux essais.
    #[cfg(test)]
    pub fn stats(&self) -> GpuStats {
        let renderer = self.render_state.renderer.read();
        let Some(res) = renderer.callback_resources.get::<CanvasResources>() else { return GpuStats::default() };
        GpuStats { textures: res.textures.len(), bytes: res.textures.values().map(CelTexture::bytes).sum(), copies: res.copies, composes: res.composes }
    }

    /// Nombre de dessins qu'on peut envoyer d'avance sans chasser ceux qui sont affichés. Le poids
    /// d'un dessin est celui des textures que le cache garde (elles n'ont que la place du contenu) ;
    /// un cache encore presque vide compte un quart de feuille.
    pub fn ahead_capacity(&self, doc: &Document, shown: usize) -> usize {
        let sheet = (doc.width as u64 * doc.height as u64 * 4).max(1);
        let renderer = self.render_state.renderer.read();
        let Some(res) = renderer.callback_resources.get::<CanvasResources>() else { return 0 };
        let n = res.textures.len() as u64;
        let per = if n >= 8 { (res.textures.values().map(CelTexture::bytes).sum::<u64>() / n).clamp(64 << 10, sheet) } else { sheet / 4 };
        ((res.cache_bytes / per) as usize).saturating_sub(shown + 4) / 2
    }

    /// Met à jour les textures des dessins du plan. `all` : tout a pu changer ; `tiles` : tuiles
    /// touchées depuis l'image précédente (dans le dessin en cours de modification). `ahead` :
    /// dessins des images qui suivent (lecture, défilement), envoyés d'avance par petits morceaux,
    /// pour qu'un changement de panneau ne fige pas l'image. `level` : aperçu des effets (0 : pleine
    /// résolution, `fx::draft_level`). Renvoie, pour chaque élément du plan, le numéro de la
    /// texture à composer (celui du dessin dérivé, ou du dernier résultat tant que le fil de fond
    /// calcule les effets, `fx::display`) ; rien : élément sans dessin.
    pub fn sync(&mut self, doc: &Document, plan: &Plan, all: bool, tiles: &[usize], ahead: &[Ahead], level: u8) -> Vec<Option<CelId>> {
        let rs = &self.render_state;
        let mut renderer = rs.renderer.write();
        let Some(res) = renderer.callback_resources.get_mut::<CanvasResources>() else { return Vec::new() };
        res.tick += 1;
        let tick = res.tick;
        let size = (doc.width as u32, doc.height as u32);
        let mut up = Uploads { tick, data: std::mem::take(&mut res.staging), ..Default::default() };
        // Poids des textures gardées (mis à jour à chaque création, recyclage et départ).
        let mut bytes: u64 = res.textures.values().map(CelTexture::bytes).sum();
        // Un dessin passé par des effets (13i) : son dessin dérivé, sous son propre numéro ; en
        // aperçu ou en attente du fil de fond, une texture plus petite ou plus ancienne. Cherché
        // une fois par élément.
        let displays: Vec<Option<crate::fx::Display>> = plan.items.iter().map(|item| crate::fx::display(doc, item, level)).collect();
        // Les dessins de cette image sont les plus récents avant tout : en créer un ne chasse pas
        // un autre dessin de la même image.
        let mut resolved = Vec::with_capacity(plan.items.len());
        for shown in displays.iter().flatten() {
            if let Some(t) = res.textures.get_mut(&shown.id) {
                t.last_used = tick;
            }
        }
        for (item, shown) in plan.items.iter().zip(&displays) {
            let Some(shown) = shown else {
                resolved.push(None);
                continue;
            };
            let (cel, id) = (&shown.cel, shown.id);
            resolved.push(Some(id));
            // (À la taille de son image : la feuille flottante d'une sélection peut dépasser la feuille.)
            let own = (cel.raster.width as u32, cel.raster.height as u32);
            let n = cel.raster.tiles.len();
            let known = res.textures.get(&id).filter(|e| e.dims == own).map(|e| (e.version, e.partial, e.rect));
            if all || known.is_none_or(|(version, _, _)| version != cel.version) {
                // Un envoi d'avance commencé pour cette version reprend où il en était.
                let mut start = match known {
                    Some((_, Some((v, next)), _)) if v == cel.version && !all => next,
                    _ => 0,
                };
                // La texture n'a que la place du contenu : elle se refait si le contenu en sort.
                let content = content_tiles(&cel.raster);
                if !known.is_some_and(|(_, _, rect)| covers(rect, own, content.map(|c| content_pixels(own, c)))) {
                    let grow = if known.is_some() { growth(content, own) } else { 0 };
                    place_texture(rs, res, id, own, texture_rect(own, content, grow), n, tick, &mut bytes, &mut up);
                    start = 0;
                }
                let entry = res.textures.get_mut(&id).expect("texture présente");
                let rest: Vec<usize> = (start..n).collect();
                let _t = crate::perf::section("  gpu : tuiles copiées");
                up.add(id, entry, &cel.raster, &rest);
                entry.version = cel.version;
                entry.partial = None;
            } else if !tiles.is_empty() && (id == item.cel || item.fond.is_some()) {
                // (Un fond, pendant qu'on peint dedans, se suit aussi tuile par tuile : `fx::fond_patch`.)
                // Un trait qui sort de la texture la refait, plus grande, et renvoie le dessin entier.
                let touched = content_of(&cel.raster, tiles).map(|c| content_pixels(own, c));
                let rect = known.map_or([0; 4], |(_, _, rect)| rect);
                if covers(rect, own, touched) {
                    up.add(id, res.textures.get_mut(&id).expect("texture présente"), &cel.raster, tiles);
                } else {
                    let content = content_tiles(&cel.raster);
                    place_texture(rs, res, id, own, texture_rect(own, content, growth(content, own)), n, tick, &mut bytes, &mut up);
                    let entry = res.textures.get_mut(&id).expect("texture présente");
                    entry.version = cel.version;
                    up.add(id, entry, &cel.raster, &(0..n).collect::<Vec<_>>());
                }
            }
            if let Some(entry) = res.textures.get_mut(&id) {
                entry.last_used = tick;
            }
        }
        // Envoi d'avance, borné à AHEAD_BYTES par image d'écran ; un dessin entamé reprend à sa
        // prochaine tuile à l'image suivante.
        let shown = up.data.len();
        for a in ahead {
            if up.data.len() >= shown + AHEAD_BYTES {
                break;
            }
            let id = a.id;
            // Un dessin dérivé déjà calculé par le fil de fond, ou un dessin du projet.
            let Some(cel) = a.cel.as_deref().or_else(|| doc.cels.get(&id)) else { continue };
            let own = (cel.raster.width as u32, cel.raster.height as u32);
            let n = cel.raster.tiles.len();
            let known = res.textures.get_mut(&id).filter(|e| e.dims == own);
            if let Some(e) = known {
                e.last_used = tick;
                if e.version == cel.version {
                    continue;
                }
            }
            let known = res.textures.get(&id).filter(|e| e.dims == own).map(|e| (e.partial, e.rect));
            let content = content_tiles(&cel.raster);
            if !known.is_some_and(|(_, rect)| covers(rect, own, content.map(|c| content_pixels(own, c)))) {
                let grow = if known.is_some() { growth(content, own) } else { 0 };
                place_texture(rs, res, id, own, texture_rect(own, content, grow), n, tick, &mut bytes, &mut up);
            }
            let entry = res.textures.get_mut(&id).expect("texture présente");
            entry.last_used = tick;
            let mut next = match entry.partial {
                Some((v, next)) if v == cel.version => next,
                _ => 0,
            };
            while next < n && up.data.len() < shown + AHEAD_BYTES {
                up.add(id, entry, &cel.raster, &[next]);
                next += 1;
            }
            if next >= n {
                entry.version = cel.version;
                entry.partial = None;
            } else {
                entry.partial = Some((cel.version, next));
            }
        }
        res.copies += up.copies.len() as u64;        let staging = up.submit(rs, &res.textures);
        // (Le tampon d'envoi est gardé d'une image à l'autre, sauf s'il est devenu énorme.)
        if staging.capacity() <= STAGING_KEEP && !res.fresh_staging {
            res.staging = staging;
        }
        // Mémoire de la carte graphique : on libère les textures les plus anciennes au-delà du budget
        // (en octets : une texture n'a que la place du contenu de son dessin, un aperçu réduit
        // encore moins) ; celles de cette image restent. Un cache plein recycle déjà ses textures
        // de même taille (`place_texture`) ; ceci rattrape le reste.
        let budget = res.cache_bytes;
        let mut total: u64 = res.textures.values().map(CelTexture::bytes).sum();
        if total > budget {
            let mut ages: Vec<(u64, CelId)> = res.textures.iter().map(|(&id, t)| (t.last_used, id)).collect();
            ages.sort_unstable();
            for (used, id) in ages {
                if total <= budget || used == tick {
                    break;
                }
                if let Some(t) = res.textures.remove(&id) {
                    total -= t.bytes();
                }
            }
        }
        // Image composée à la taille du projet.
        if res.frame.as_ref().is_none_or(|f| f.size != size) {
            res.frame = Some(make_frame(&rs.device, &res.texture_layout, &res.smooth, &res.nearest, size));
            // (Celle de la transition suit la taille : elle est refaite à la demande.)
            res.frame2 = None;
        }
        resolved
    }

    /// Commande de dessin : composer le papier et les calques du plan, puis afficher l'image.
    /// `resolved` : les textures des éléments du plan (renvoyées par `sync`). `corners` : les coins
    /// du dessin à l'écran, en points (haut-gauche, haut-droite, bas-droite, bas-gauche), avec
    /// leur w si la vue est en perspective (9f) ; `rect` : la zone d'affichage (ce qui dépasse est
    /// coupé).
    pub fn paint(
        &self,
        plan: &Plan,
        resolved: &[Option<CelId>],
        corners: [Pos2; 4],
        ws: Option<[f32; 4]>,
        rect: Rect,
        stage: [usize; 2],
    ) -> egui::PaintCallback {
        self.paint_mix(MixLayer { plan, resolved, corners, ws }, None, None, rect, stage)
    }

    /// Comme `paint`, avec une transition (`transitions.rs`) : `first` est le plan montré (le plan
    /// sortant s'il y en a deux), `second` le plan entrant d'un fondu enchaîné ou d'un volet (composé
    /// à part, avec sa propre caméra), `mix` dit comment les poser l'un sur l'autre, et la couche de
    /// couleur d'un fondu au noir ou au blanc.
    /// Tout est fait par l'affichage : deux images posées l'une sur l'autre, sans recomposer.
    pub fn paint_mix(&self, first: MixLayer, second: Option<MixLayer>, mix: Option<&DisplayMix>, rect: Rect, stage: [usize; 2]) -> egui::PaintCallback {
        let mut items = self.gpu_items(first.plan, first.resolved, stage);
        let split = items.len();
        if let Some(s) = &second {
            items.extend(self.gpu_items(s.plan, s.resolved, stage));
        }
        // w ramenés autour de 1 (une homographie vaut à un facteur positif près).
        let norm = |ws: Option<[f32; 4]>| {
            ws.map(|w| {
                let m = w.iter().fold(0.0_f32, |m, v| m.max(v.abs())).max(1e-12);
                w.map(|v| {
                    let v = v / m;
                    if v.abs() < 1e-6 { 1e-6_f32.copysign(v) } else { v }
                })
            })
        };
        // Afficher : positions en points ; elles passent en coordonnées de l'écran entier au moment
        // du dessin (taille exacte en pixels), pour tomber sur les mêmes pixels que l'ancien affichage.
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let quad = |layer: &MixLayer, second: bool, params: [f32; 3]| Show {
            screen: std::array::from_fn(|i| [layer.corners[i].x, layer.corners[i].y, uv[i][0], uv[i][1]]),
            ws: norm(layer.ws),
            params,
            second,
        };
        let mut shows = Vec::new();
        match (&second, mix) {
            (Some(s), Some(m)) => {
                // Le plan sortant (`first`) d'abord, l'entrant par-dessus (poids `m`, ou volet).
                shows.push(quad(&first, false, [1.0, 1.0, 0.0]));
                if m.wipe {
                    shows.push(quad(s, true, [1.0, 2.0, m.wipe_u[0] + m.m.clamp(0.0, 1.0) * (m.wipe_u[1] - m.wipe_u[0])]));
                } else {
                    shows.push(quad(s, true, [m.m.clamp(0.0, 1.0), 1.0, 0.0]));
                }
            }
            _ => shows.push(quad(&first, false, [1.0, 1.0, 0.0])),
        }
        if let Some(m) = mix.filter(|m| m.k > 0.0) {
            // La couche de couleur : un aplat sur la feuille, d'opacité `k`.
            shows.push(quad(&first, false, [m.k.clamp(0.0, 1.0), 0.0, m.grey]));
        }
        egui_wgpu::Callback::new_paint_callback(rect, CanvasPaint { items, split, shows })
    }

    /// Les éléments à composer pour un plan : papier, puis un par dessin (voir `paint`).
    fn gpu_items(&self, plan: &Plan, resolved: &[Option<CelId>], stage: [usize; 2]) -> Vec<GpuItem> {
        // Composer : l'image entière, en coordonnées normalisées.
        let full = [[-1.0, 1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 0.0], [1.0, -1.0, 1.0, 1.0], [-1.0, -1.0, 0.0, 1.0]];
        let (w, h) = (stage[0] as f32, stage[1] as f32);
        let pivot = egui::pos2(w / 2.0, h / 2.0);
        // Calque transformé : les coins de la feuille, déplacés, en coordonnées normalisées.
        // (`dims` : l'image du dessin si elle n'est pas à la taille de la feuille.)
        let placed = |aff: crate::vector::Affine, dims: Option<[usize; 2]>| -> [[f32; 4]; 4] {
            let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
            let (dw, dh) = dims.map_or((w, h), |d| (d[0] as f32, d[1] as f32));
            std::array::from_fn(|i| {
                let p = aff.apply(egui::pos2(uv[i][0] * dw, uv[i][1] * dh));
                [p.x / w * 2.0 - 1.0, 1.0 - p.y / h * 2.0, uv[i][0], uv[i][1]]
            })
        };
        // Ce que montre la texture de chaque dessin : sa place dans l'image du dessin (`rect`) et la
        // taille de cette image.
        let places: Vec<Option<([u32; 4], (u32, u32))>> = {
            let renderer = self.render_state.renderer.read();
            let textures = renderer.callback_resources.get::<CanvasResources>().map(|res| &res.textures);
            resolved.iter().map(|id| id.and_then(|id| textures?.get(&id)).map(|t| (t.rect, t.dims))).collect()
        };
        let mut items = Vec::with_capacity(plan.items.len() + 1);
        if let Some(p) = plan.paper {
            items.push(GpuItem {
                cel: None,
                corners: full,
                color: [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, 0.0],
                params: [1.0, 0.0, 0.0, 0.0],
                blend: 0,
                mesh: None,
            });
        }
        for ((item, id), place) in plan.items.iter().zip(resolved).zip(&places) {
            // Un élément sans dessin n'est pas composé.
            let Some(id) = *id else { continue };
            // (Sans texture, rien à poser : la passe de composition saute l'élément.)
            let (rect, rdims) = place.unwrap_or(([0, 0, stage[0] as u32, stage[1] as u32], (stage[0] as u32, stage[1] as u32)));
            let whole = rect == [0, 0, rdims.0, rdims.1];
            let (color, tinted) = match item.tint {
                Some(t) => ([t[0] as f32 / 255.0, t[1] as f32 / 255.0, t[2] as f32 / 255.0], 1.0),
                None => ([0.0; 3], 0.0),
            };
            // La pelure d'oignon (teinte) reste en mode normal.
            let blend = if item.tint.is_some() { 0 } else { item.blend.index() };
            // Feuille posée par un maillage : ses sommets, déjà à leur place dans l'image composée.
            let mesh = item.warp(pivot).map(|(warp, layer)| {
                let dims = item.pre.as_ref().and_then(|p| p.dims).map_or((stage[0], stage[1]), |d| (d[0], d[1]));
                let mut vertices = warp.gpu_vertices(dims, layer.as_ref(), |p| [p.x / w * 2.0 - 1.0, 1.0 - p.y / h * 2.0]);
                if !whole {
                    // Les coordonnées de texture étaient celles de l'image entière du dessin : elles
                    // deviennent celles de la texture, qui n'en montre qu'une partie (le reste, hors
                    // du contenu, est transparent : la lecture hors de la texture tombe sur son bord
                    // transparent).
                    let (rw, rh) = (rdims.0 as f32, rdims.1 as f32);
                    for v in &mut vertices {
                        v[2] = (v[2] * rw - rect[0] as f32) / rect[2] as f32;
                        v[3] = (v[3] * rh - rect[1] as f32) / rect[3] as f32;
                    }
                }
                vertices
            });
            let aff = item.affine(pivot);
            let corners = if whole {
                aff.map_or(full, |aff| placed(aff, item.pre.as_ref().and_then(|p| p.dims)))
            } else {
                // La texture ne couvre qu'une partie de l'image du dessin : ses quatre coins sont
                // ceux de ce rectangle, placés comme l'image entière l'aurait été.
                let logical = item.pre.as_ref().and_then(|p| p.dims).map_or((w, h), |d| (d[0] as f32, d[1] as f32));
                let (rw, rh) = (rdims.0 as f32, rdims.1 as f32);
                std::array::from_fn(|i| {
                    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]][i];
                    let (px, py) = (rect[0] as f32 + uv[0] * rect[2] as f32, rect[1] as f32 + uv[1] * rect[3] as f32);
                    match aff {
                        Some(aff) => {
                            let p = aff.apply(egui::pos2(px / rw * logical.0, py / rh * logical.1));
                            [p.x / w * 2.0 - 1.0, 1.0 - p.y / h * 2.0, uv[0], uv[1]]
                        }
                        None => [px / rw * 2.0 - 1.0, 1.0 - py / rh * 2.0, uv[0], uv[1]],
                    }
                })
            };
            items.push(GpuItem {
                cel: Some(id),
                corners,
                color: [color[0], color[1], color[2], tinted],
                params: [item.opacity as f32 / 255.0, 1.0, blend as f32, 0.0],
                blend,
                mesh,
            });
        }
        items
    }
}

/// Un plan à montrer : ses éléments (`Plan`), leurs textures (`GpuCanvas::sync`), et où est la
/// feuille à l'écran (coins en points, avec leur w si la vue est en perspective).
pub struct MixLayer<'a> {
    pub plan: &'a Plan,
    pub resolved: &'a [Option<CelId>],
    pub corners: [Pos2; 4],
    pub ws: Option<[f32; 4]>,
}

/// Comment poser les images d'une transition à l'écran (voir `GpuCanvas::paint_mix`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayMix {
    pub wipe: bool,
    /// Poids du plan entrant (ou avancée du volet), de 0 à 1.
    pub m: f32,
    /// Couche de couleur : gris (0 noir, 1 blanc) et opacité.
    pub grey: f32,
    pub k: f32,
    /// Colonnes de la feuille (0 à 1) où le cadre filmé commence et finit : le volet y avance.
    pub wipe_u: [f32; 2],
}

/// Un dessin à envoyer d'avance à la carte graphique : un dessin du projet (`cel` rien), ou un
/// dessin dérivé déjà calculé par le fil de fond des effets.
pub struct Ahead {
    pub id: CelId,
    pub cel: Option<std::sync::Arc<crate::document::Cel>>,
}

struct GpuItem {
    cel: Option<CelId>,
    corners: [[f32; 4]; 4],
    color: [f32; 4],
    params: [f32; 4],
    /// Mode de fusion (0 : normal, voir `BlendMode::index`).
    blend: u32,
    /// Sommets du maillage qui pose la feuille (`warp.rs`) : à la place des quatre coins.
    mesh: Option<Vec<[f32; 4]>>,
}

/// Ce qui fait un élément de l'image composée : le dessin et la version de sa texture, sa place, sa
/// couleur, ses réglages, son mode et son maillage. Deux listes égales donnent la même image.
#[derive(Clone, PartialEq, Debug)]
struct ItemKey {
    cel: Option<CelId>,
    /// Dernière image d'écran où la texture a reçu des pixels (ou a été refaite).
    generation: Option<u64>,
    numbers: [u32; 24],
    blend: u32,
    /// Nombre de sommets du maillage et leur empreinte.
    mesh: Option<(usize, u64)>,
}

impl ItemKey {
    fn of(item: &GpuItem, generation: Option<u64>) -> Self {
        let mut numbers = [0u32; 24];
        for (n, v) in numbers.iter_mut().zip(item.corners.iter().flatten().chain(&item.color).chain(&item.params)) {
            *n = v.to_bits();
        }
        let mesh = item.mesh.as_ref().map(|m| {
            // (FNV-1a sur les bits des sommets.)
            let hash = m.iter().flatten().fold(0xcbf2_9ce4_8422_2325_u64, |h, v| (h ^ v.to_bits() as u64).wrapping_mul(0x0000_0100_0000_01b3));
            (m.len(), hash)
        });
        ItemKey { cel: item.cel, generation, numbers, blend: item.blend, mesh }
    }
}

/// Passe qui compose dans l'image `target` (effacée d'abord, ou reprise telle quelle).
fn compose_pass<'a>(encoder: &'a mut wgpu::CommandEncoder, target: &'a wgpu::TextureView, clear: bool) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("composer les calques"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load: if clear { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load },
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

/// Une image composée posée à l'écran (une, ou deux avec une couche pour une transition).
struct Show {
    /// Coins de l'image à l'écran, en points.
    screen: [[f32; 4]; 4],
    /// Leur w, en perspective.
    ws: Option<[f32; 4]>,
    /// Opacité, mode et réglage de `fs_display` (voir le shader).
    params: [f32; 3],
    /// L'image du second plan d'une transition, au lieu de la première.
    second: bool,
}

struct CanvasPaint {
    /// Ce qu'on compose, du fond vers le dessus : les éléments du premier plan, puis ceux du second
    /// (à partir de `split`).
    items: Vec<GpuItem>,
    split: usize,
    /// Ce qu'on pose à l'écran, dans l'ordre.
    shows: Vec<Show>,
}

/// Compose les éléments `span` de `items` dans `frame` (effacée d'abord). Un calque en mode de
/// fusion lit ce qui est déjà composé : on le copie à part avant (fin de passe, copie, nouvelle
/// passe).
fn compose_span(encoder: &mut wgpu::CommandEncoder, res: &CanvasResources, frame: &Frame, items: &[GpuItem], ranges: &[Option<std::ops::Range<u32>>], span: std::ops::Range<usize>) {
    let stride = res.stride as usize;
    let mut pass = compose_pass(encoder, &frame.target, true);
    for i in span {
        let item = &items[i];
        let group = match item.cel {
            None => &res.white,
            Some(id) => match res.textures.get(&id) {
                Some(t) => &t.bind_group,
                None => continue,
            },
        };
        // (Une feuille posée par un maillage sans rien à dessiner : rien.)
        let meshed = item.mesh.is_some();
        let range = ranges[i].clone();
        if meshed && range.is_none() {
            continue;
        }
        if item.blend != 0 {
            drop(pass);
            encoder.copy_texture_to_texture(frame.texture.as_image_copy(), frame.backdrop.as_image_copy(), extent(frame.size.0, frame.size.1));
            pass = compose_pass(encoder, &frame.target, false);
            pass.set_bind_group(2, &frame.backdrop_group, &[]);
        }
        pass.set_pipeline(match (item.blend != 0, meshed) {
            (false, false) => &res.compose,
            (false, true) => &res.compose_mesh,
            (true, false) => &res.blend,
            (true, true) => &res.blend_mesh,
        });
        pass.set_bind_group(0, &res.uniform_group, &[(i * stride) as u32]);
        pass.set_bind_group(1, group, &[]);
        match range {
            Some(range) => {
                let Some((buffer, _)) = &res.mesh_vertices else { continue };
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(range, 0..1);
            }
            None => pass.draw(0..6, 0..1),
        }
    }
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
        let n = (self.items.len() + self.shows.len()) as u64;
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
        for (k, show) in self.shows.iter().enumerate() {
            let on_screen = show.screen.map(|c| [c[0] * ppp / w * 2.0 - 1.0, 1.0 - c[1] * ppp / h * 2.0, c[2], c[3]]);
            let (ws, persp) = match show.ws {
                Some(w) => (w, 1.0),
                None => ([0.0; 4], 0.0),
            };
            put_item(&mut bytes[(self.items.len() + k) * stride..], &on_screen, &ws, &[show.params[0], show.params[1], show.params[2], persp]);
        }
        queue.write_buffer(&res.uniforms, 0, &bytes);

        // L'image composée est gardée : si rien de ce qui la fait n'a changé depuis la dernière fois
        // (mêmes dessins aux mêmes versions, mêmes places, opacités et modes), elle est telle quelle ;
        // seul l'affichage (la vue, le zoom) est refait. Souris qui bouge, panneaux qui s'animent,
        // curseur : la pile de calques n'est plus recomposée à chaque image d'écran. Le second plan
        // d'une transition a la sienne, gardée de même (le plan tenu d'un fondu ne se recompose pas).
        let keys: Vec<ItemKey> = self.items.iter().map(|item| ItemKey::of(item, item.cel.and_then(|id| res.textures.get(&id)).map(|t| t.generation))).collect();
        let (keys1, keys2) = keys.split_at(self.split.min(keys.len()));
        let two = self.shows.iter().any(|s| s.second);
        if two && res.frame2.as_ref().map(|f| f.size) != res.frame.as_ref().map(|f| f.size) {
            if let Some(size) = res.frame.as_ref().map(|f| f.size) {
                res.frame2 = Some(make_frame(device, &res.texture_layout, &res.smooth, &res.nearest, size));
            }
        }
        let fresh1 = !res.frame.as_ref().is_some_and(|f| f.composed.as_deref() == Some(keys1));
        let fresh2 = two && !res.frame2.as_ref().is_some_and(|f| f.composed.as_deref() == Some(keys2));
        if !fresh1 && !fresh2 {
            return Vec::new();
        }

        // Les sommets des maillages (feuilles déformées), bout à bout dans un seul tampon : pour
        // chaque dessin, sa plage de sommets.
        let mut vertices: Vec<[f32; 4]> = Vec::new();
        let ranges: Vec<Option<std::ops::Range<u32>>> = self
            .items
            .iter()
            .map(|item| {
                let mesh = item.mesh.as_ref().filter(|m| !m.is_empty())?;
                let first = vertices.len() as u32;
                vertices.extend_from_slice(mesh);
                Some(first..vertices.len() as u32)
            })
            .collect();
        if !vertices.is_empty() {
            let bytes = vertices.len() as u64 * MESH_VERTEX_BYTES;
            if res.mesh_vertices.as_ref().is_none_or(|(_, size)| *size < bytes) {
                let size = bytes.next_power_of_two().max(1 << 16);
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("maillages des feuilles déformées"),
                    size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                res.mesh_vertices = Some((buffer, size));
            }
            if let Some((buffer, _)) = &res.mesh_vertices {
                queue.write_buffer(buffer, 0, bytemuck::cast_slice(&vertices));
            }
        }

        // Composer dans l'image à la taille du projet (et dans celle du second plan d'une transition).
        let split = self.split.min(self.items.len());
        if fresh1 {
            let Some(frame) = &res.frame else { return Vec::new() };
            compose_span(encoder, res, frame, &self.items, &ranges, 0..split);
            res.composes += 1;
        }
        if fresh2 {
            let Some(frame) = &res.frame2 else { return Vec::new() };
            compose_span(encoder, res, frame, &self.items, &ranges, split..self.items.len());
            res.composes += 1;
        }
        if fresh1 {
            if let Some(frame) = res.frame.as_mut() {
                frame.composed = Some(keys1.to_vec());
            }
        }
        if fresh2 {
            if let Some(frame) = res.frame2.as_mut() {
                frame.composed = Some(keys2.to_vec());
            }
        }
        Vec::new()
    }

    fn paint(&self, info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        let Some(res) = resources.get::<CanvasResources>() else { return };
        // Tout l'écran comme repère (la zone d'affichage reste la limite, par le découpage d'egui).
        let [w, h] = info.screen_size_px;
        pass.set_viewport(0.0, 0.0, w as f32, h as f32, 0.0, 1.0);
        pass.set_pipeline(&res.display);
        for (k, show) in self.shows.iter().enumerate() {
            let Some(frame) = (if show.second { &res.frame2 } else { &res.frame }) else { continue };
            pass.set_bind_group(0, &res.uniform_group, &[((self.items.len() + k) as u64 * res.stride) as u32]);
            pass.set_bind_group(1, &frame.display_group, &[]);
            pass.draw(0..6, 0..1);
        }
    }
}

/// Une image composée neuve, à la taille du projet.
fn make_frame(device: &wgpu::Device, texture_layout: &wgpu::BindGroupLayout, smooth: &wgpu::Sampler, nearest: &wgpu::Sampler, size: (u32, u32)) -> Frame {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("image composée"),
        size: extent(size.0, size.1),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FRAME_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
    });
    let target = texture.create_view(&Default::default());
    let srgb = texture.create_view(&wgpu::TextureViewDescriptor {
        format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        ..Default::default()
    });
    let display_group = texture_group(device, texture_layout, &srgb, smooth);
    let backdrop = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fond des modes de fusion"),
        size: extent(size.0, size.1),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FRAME_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let backdrop_group = texture_group(device, texture_layout, &backdrop.create_view(&Default::default()), nearest);
    Frame { size, texture, target, display_group, backdrop, backdrop_group, composed: None }
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

/// Combien de pixels de plus de chaque côté pour la texture d'un dessin qui vient de sortir de la
/// sienne (on le peint, ou on l'agrandit) : le quart de son contenu, au moins deux tuiles, pour que
/// les tuiles suivantes d'un trait n'obligent pas à la refaire chaque fois.
fn growth(content: Option<[usize; 4]>, dims: (u32, u32)) -> usize {
    let Some(c) = content else { return 2 * TILE };
    let (w, h) = (content_pixels(dims, c)[2] - c[0] * TILE, content_pixels(dims, c)[3] - c[1] * TILE);
    (w.max(h) / 4).div_ceil(TILE).max(2) * TILE
}

/// Les tuiles non vides, parmi `tiles`, de ce dessin : leur boîte.
fn content_of(raster: &Raster, tiles: &[usize]) -> Option<[usize; 4]> {
    let mut found: Option<[usize; 4]> = None;
    for &idx in tiles {
        if raster.tiles.get(idx).is_some_and(Option::is_some) {
            let (tx, ty) = (idx % raster.tiles_x, idx / raster.tiles_x);
            found = Some(match found {
                None => [tx, ty, tx, ty],
                Some([a, b, c, d]) => [a.min(tx), b.min(ty), c.max(tx), d.max(ty)],
            });
        }
    }
    found
}

/// La texture du dessin `id` (image de taille `dims`, `tiles` tuiles), à la place `rect` de cette
/// image : une neuve (transparente), ou, quand la carte est pleine, celle du dessin le moins récent
/// qui a la même taille (en créer une oblige la carte à la remplir de zéros) ; les tuiles de son
/// ancien dessin sont alors effacées par le prochain envoi. Un dessin de cette image d'écran
/// (`last_used == tick`) n'est jamais pris. L'ancienne texture de `id`, s'il en avait une, est
/// libérée. `bytes` est le poids du cache, tenu à jour.
#[allow(clippy::too_many_arguments)]
fn place_texture(rs: &RenderState, res: &mut CanvasResources, id: CelId, dims: (u32, u32), rect: [u32; 4], tiles: usize, tick: u64, bytes: &mut u64, up: &mut Uploads) {
    let _t = crate::perf::section("  gpu : textures créées");
    let rect = if res.whole_textures { [0, 0, dims.0, dims.1] } else { rect };
    if let Some(old) = res.textures.remove(&id) {
        *bytes -= old.bytes();
    }
    let new_bytes = rect[2] as u64 * rect[3] as u64 * 4;
    let mut recycled = None;
    if *bytes + new_bytes > res.cache_bytes {
        let oldest = res
            .textures
            .iter()
            .filter(|(_, t)| t.rect[2] == rect[2] && t.rect[3] == rect[3] && t.last_used < tick)
            .min_by_key(|(_, t)| t.last_used)
            .map(|(&k, _)| k);
        if let Some(mut t) = oldest.and_then(|k| res.textures.remove(&k)) {
            *bytes -= t.bytes();
            // Les tuiles de l'ancien dessin sont encore dedans : on les efface, avant tout envoi.
            for (idx, _) in t.filled.iter().enumerate().filter(|(_, f)| **f) {
                let (ox, oy, tw, th) = tile_bounds(t.dims.0 as usize, t.dims.1 as usize, idx);
                up.clear_rect(id, [(ox as u32).saturating_sub(t.rect[0]), (oy as u32).saturating_sub(t.rect[1]), tw as u32, th as u32]);
            }
            t.rect = rect;
            t.dims = dims;
            t.filled = vec![false; tiles];
            t.version = u64::MAX;
            t.last_used = tick;
            t.generation = tick;
            t.partial = None;
            recycled = Some(t);
        }
    }
    let entry = recycled.unwrap_or_else(|| {
        let texture = cel_texture(&rs.device, rect[2], rect[3]);
        let view = texture.create_view(&Default::default());
        let bind_group = texture_group(&rs.device, &res.texture_layout, &view, &res.nearest);
        CelTexture { bind_group, texture, rect, dims, version: u64::MAX, last_used: tick, generation: tick, filled: vec![false; tiles], partial: None }
    });
    *bytes += entry.bytes();
    res.textures.insert(id, entry);
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

/// Tuiles à envoyer, rangées bout à bout dans un seul tampon : un envoi par passage au lieu d'un
/// par tuile (chaque `write_texture` crée son propre tampon intermédiaire, trop lent par milliers).
/// Une tuile garde ses lignes de 64 pixels (256 octets), l'alignement que demande la copie.
#[derive(Default)]
struct Uploads {
    /// L'image d'écran (`CanvasResources::tick`) : la `generation` des textures qui reçoivent des pixels.
    tick: u64,
    data: Vec<u8>,
    /// Emplacement d'une tuile transparente, partagée par toutes les tuiles à effacer.
    zero: Option<u64>,
    /// Dessin, position dans `data`, puis rectangle de destination.
    copies: Vec<(CelId, u64, [u32; 4])>,
}

impl Uploads {
    /// Ajoute les tuiles `tiles` du dessin `id` : pleines si dessinées, effacées si elles l'étaient.
    fn add(&mut self, id: CelId, entry: &mut CelTexture, raster: &Raster, tiles: &[usize]) {
        let before = self.copies.len();
        for &idx in tiles {
            if idx >= raster.tiles.len() || idx >= entry.filled.len() {
                continue;
            }
            let (ox, oy, tw, th) = raster.tile_bounds(idx);
            // La tuile doit être dans la texture (le contenu y est toujours : `covers`).
            let [rx, ry, rw, rh] = entry.rect.map(|v| v as usize);
            let inside = ox >= rx && oy >= ry && ox + tw <= rx + rw && oy + th <= ry + rh;
            let offset = match &raster.tiles[idx] {
                Some(_) if !inside => {
                    debug_assert!(false, "tuile {idx} hors de la texture");
                    continue;
                }
                Some(tile) => {
                    entry.filled[idx] = true;
                    let at = self.data.len() as u64;
                    self.data.extend_from_slice(bytemuck::cast_slice(&tile[..]));
                    at
                }
                None if entry.filled[idx] => {
                    entry.filled[idx] = false;
                    let data = &mut self.data;
                    *self.zero.get_or_insert_with(|| {
                        let at = data.len() as u64;
                        data.resize(data.len() + TILE * TILE * 4, 0);
                        at
                    })
                }
                None => continue,
            };
            self.copies.push((id, offset, [(ox - rx) as u32, (oy - ry) as u32, tw as u32, th as u32]));
        }
        if self.copies.len() > before {
            entry.generation = self.tick;
        }
    }

    /// Efface le rectangle `rect` (de la taille d'une tuile au plus, dans la texture) du dessin `id`.
    fn clear_rect(&mut self, id: CelId, rect: [u32; 4]) {
        let data = &mut self.data;
        let at = *self.zero.get_or_insert_with(|| {
            let at = data.len() as u64;
            data.resize(data.len() + TILE * TILE * 4, 0);
            at
        });
        self.copies.push((id, at, rect));
    }

    /// Envoie tout. Rend le tampon, vidé, pour le prochain passage (le garder évite de redemander à
    /// Windows des Mo de mémoire neuve, que le premier accès paie en fautes de page).
    fn submit(mut self, rs: &RenderState, textures: &HashMap<CelId, CelTexture>) -> Vec<u8> {
        if self.copies.is_empty() {
            self.data.clear();
            return self.data;
        }
        let _t = crate::perf::section("  gpu : tuiles envoyées");
        use wgpu::util::DeviceExt;
        let buffer = rs.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tuiles à envoyer"),
            contents: &self.data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        let mut encoder = rs.device.create_command_encoder(&Default::default());
        for &(id, offset, [x, y, w, h]) in &self.copies {
            let Some(t) = textures.get(&id) else { continue };
            encoder.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset,
                        bytes_per_row: Some((TILE * 4) as u32),
                        rows_per_image: Some(TILE as u32),
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &t.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                extent(w, h),
            );
        }
        rs.queue.submit([encoder.finish()]);
        self.data.clear();
        self.data
    }
}
