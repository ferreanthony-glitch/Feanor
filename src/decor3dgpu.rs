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
/// La profondeur partagée avec les cartes (partie 183, `profondeur.rs`) : l'inverse de la distance à l'œil, le long de son axe, de ce
/// que le décor montre à chaque pixel de la feuille (0 : rien). Sur 16 bits : à 0,05 % près (la tolérance est de 0,4 %).
///
/// **En quatre échantillons par pixel** (partie 193, les bords doux), à côté de `PROF1`, seulement quand une carte du plan partage
/// la profondeur et que le décor tient dans le budget avec elle : après la passe de l'image du plan (inchangée), une passe de la
/// seule profondeur à quatre échantillons (`fs_prof` : pas de couleur, pas de lumière), ramenée
/// ensuite à l'inverse de la distance de chacun de ses quatre échantillons (`PROF_DES_ECHANTILLONS`), un par canal, aux places de
/// `profondeur::MOTIF_CARTE`. La composition compte ceux où le décor est plus près que la carte.
pub const PROF: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// La profondeur partagée en une valeur par pixel, celle de la partie 183 (`R16Float`, seconde cible de la passe de couleur), toujours
/// dessinée : les ombres des cartes la lisent (partie 185), et c'est elle qui cache les cartes quand aucune carte du plan ne partage
/// la profondeur du décor ou qu'un décor trop lourd n'a pas la place des bords doux dans le budget de cette machine (partie 193,
/// allégée) : le coût d'avant.
pub const PROF1: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
/// Les ombres des cartes sur le décor (partie 185, `ombres.rs`) : à chaque pixel de la feuille, la part du soleil que le décor y a
/// reçue (`max(n·soleil, 0)` × ses propres ombres) et le mélange de la lumière hémisphère (`n.y / 2 + 1/2`). Une troisième cible
/// de la passe de l'image du plan, seulement quand le décor a ses ombres : c'est ce qu'il faut pour retirer le soleil là où une
/// carte le cache, exactement comme le décor ombre ses propres faces.
pub const SOLEIL: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;
/// Lissage des bords de la vue Espace 3D (l'image du plan, à la taille de la feuille, n'en a pas besoin : elle est réduite à
/// l'écran ; sa profondeur partagée avec les cartes a quatre échantillons, partie 193, `PROF`).
const ECHANTILLONS_VUE: u32 = 4;
const GLOBAL: u64 = 512;
/// Les réglages du voile d'une carte (`VOILE`) et leur pas dans le tampon (partie 183).
const VOILE_OCTETS: u64 = 192;
const PAS_VOILE: u64 = 256;
const OBJET: u64 = 160;
const PAS_OBJET: u64 = 256;
/// Les ombres reçues par une carte de la vue (partie 189, `RECUE`) : ses réglages et leur pas dans le tampon ; ses sommets (le
/// maillage d'egui, `espace3dview::card_mesh` : (n + 1)² sommets, 6 n² indices au plus) et leur taille.
const RECUE_OCTETS: u64 = 16;
const PAS_RECUE: u64 = 256;
const SOMMETS_RECUE: u64 = ((crate::espace3dview::SUBDIV_CARTE + 1) * (crate::espace3dview::SUBDIV_CARTE + 1)) as u64;
const INDICES_RECUE: u64 = (crate::espace3dview::SUBDIV_CARTE * crate::espace3dview::SUBDIV_CARTE * 6) as u64;
const SOMMET_RECUE: u64 = 32;
/// Un sommet de la carte d'ombres des cartes de la vue : sa place dans le monde, sa coordonnée de dessin, l'opacité du calque.
const SOMMET_PORTEUSE: u64 = 24;
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
    flags: vec4<f32>,      // texture (1/0), seuil d'alpha (0 : opaque), couleur de la texture (1/0), à plat (1/0)
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
    return ombrer(in);
}

// La profondeur seule de l'image du plan à quatre échantillons (partie 193, `PROF`) : rien n'est éclairé. La place seule
// (`vs_prof_seul`, sans fragment), et, pour les matières à seuil, la coordonnée de texture et la découpe de `eclairer`.
struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_prof_seul(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) uv: vec2<f32>) -> @builtin(position) vec4<f32> {
    return g.vp * (o.model * vec4<f32>(p, 1.0));
}

@vertex
fn vs_prof(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) uv: vec2<f32>) -> POut {
    var out: POut;
    out.pos = g.vp * (o.model * vec4<f32>(p, 1.0));
    out.uv = uv;
    return out;
}

@fragment
fn fs_prof(in: POut) {
    if (o.flags.y > 0.0) {
        var a = o.color.a;
        if (o.flags.x > 0.5) {
            a = a * textureSampleLevel(t_base, s_base, in.uv, 0.0).a;
        }
        if (a < o.flags.y) {
            discard;
        }
    }
}

// L'image du plan (partie 183) : la couleur, et l'inverse de la distance à l'œil le long de son axe (la profondeur que les
// cartes partagent avec le décor, `profondeur.rs`), en une valeur par pixel (`PROF1`).
struct Feuille {
    @location(0) c: vec4<f32>,
    @location(1) p: vec4<f32>,
};

@fragment
fn fs_main_feuille_p(in: VOut) -> Feuille {
    var out: Feuille;
    out.c = ombrer(in);
    out.p = vec4<f32>(1.0 / max(dot(in.wpos - g.eye.xyz, g.cam_f.xyz), 1e-6), 0.0, 0.0, 1.0);
    return out;
}

// Le ciel de l'image du plan : il ne cache rien (profondeur 0).
@fragment
fn fs_sky_feuille_p(in: SkyOut) -> Feuille {
    var out: Feuille;
    out.c = ciel(in);
    out.p = vec4<f32>(0.0);
    return out;
}

// Et quand le décor a ses ombres (partie 185) : en plus, la part du soleil et le mélange de l'hémisphère, pour que les ombres des
// cartes retirent le soleil comme le décor le retire de ses faces à l'ombre (`ombres.rs`).
struct FeuillePS {
    @location(0) c: vec4<f32>,
    @location(1) p: vec4<f32>,
    @location(2) s: vec4<f32>,
};

@fragment
fn fs_main_feuille_ps(in: VOut) -> FeuillePS {
    var out: FeuillePS;
    let e = eclairer(in);
    out.c = e.c;
    out.p = vec4<f32>(1.0 / max(dot(in.wpos - g.eye.xyz, g.cam_f.xyz), 1e-6), 0.0, 0.0, 1.0);
    out.s = vec4<f32>(e.sun, e.t, 0.0, 1.0);
    return out;
}

@fragment
fn fs_sky_feuille_ps(in: SkyOut) -> FeuillePS {
    var out: FeuillePS;
    out.c = ciel(in);
    out.p = vec4<f32>(0.0);
    out.s = vec4<f32>(0.0);
    return out;
}

fn ombrer(in: VOut) -> vec4<f32> {
    return eclairer(in).c;
}

struct Eclaire {
    c: vec4<f32>,
    sun: f32,
    t: f32,
};

fn eclairer(in: VOut) -> Eclaire {
    // (Calculés avant tout `discard` : les dérivées veulent un flot uniforme.)
    let flat_n = normalize(cross(dpdx(in.wpos), dpdy(in.wpos)));
    let tex = textureSampleLevel(t_base, s_base, in.uv, 0.0);
    var base = o.color;
    if (o.flags.x > 0.5) {
        if (o.flags.z > 0.5) { base = base * tex; } else { base.a = base.a * tex.a; }
    }
    if (o.flags.y > 0.0 && base.a < o.flags.y) { discard; }
    var n = normalize(in.n);
    if (g.eye.w > 0.5) { n = flat_n; }
    let v = g.eye.xyz - in.wpos;
    if (dot(n, v) < 0.0) { n = -n; }
    let t = n.y * 0.5 + 0.5;
    let hemi = mix(g.ground_amb.rgb, g.sky_amb.rgb, t);
    // (Partie 189 : et le soleil que les cartes laissent au décor de la vue Espace 3D ; 1 partout ailleurs, `SANS_CARTES`.)
    let sun = max(dot(n, g.key_dir.xyz), 0.0) * shadow(in.wpos, n) * soleil_des_cartes(in.wpos);
    var c = base.rgb * (hemi + g.key_col.rgb * sun);
    // (Partie 199 : l'aspect Couleurs, éclairé à plat : tiré vers la couleur d'identification, `couleurs3d::PLAT`.)
    if (o.flags.w > 0.5) { c = mix(c, base.rgb, 0.55); }
    let d = length(v);
    let f = clamp((d - g.fog.x) / max(g.fog.y - g.fog.x, 0.0001), 0.0, 1.0) * g.fog_col.w;
    c = mix(c, g.fog_col.rgb, f * f * (3.0 - 2.0 * f));
    var e: Eclaire;
    e.c = vec4<f32>(finish(c), 1.0);
    e.sun = sun;
    e.t = t;
    return e;
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
    return ciel(in);
}

fn ciel(in: SkyOut) -> vec4<f32> {
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

/// Le décor sans ombres de cartes (l'image du plan, et la vue quand ses cartes n'en portent pas) : le soleil reste le sien.
const SANS_CARTES: &str = r#"
fn soleil_des_cartes(w: vec3<f32>) -> f32 {
    return 1.0;
}
"#;

/// Le décor de la vue Espace 3D sous les ombres des cartes (partie 189, `ombres3d.rs`) : à la suite de `SHADER`, de la lumière du
/// décor et du calcul des ombres de la composition (`gpu::LUMIERE`, `gpu::OMBRES_COMMUN`) : chaque point du décor perd le soleil que
/// les cartes lui cachent, avec le filtre du décor, comme dans l'image du plan (`fs_decor_o`). Ici le point du monde est celui du
/// fragment lui-même, et le soleil est retiré avant la brume, là où le décor le met.
const AVEC_CARTES: &str = r#"
@group(3) @binding(0) var<uniform> eclair: Lumiere;
@group(3) @binding(1) var ombres_decor: texture_depth_2d;
@group(3) @binding(2) var ombres_cartes: texture_depth_2d;
@group(3) @binding(3) var s_cmp: sampler_comparison;
@group(3) @binding(4) var cartes_au_plus_pres: texture_2d<f32>;

fn soleil_des_cartes(w: vec3<f32>) -> f32 {
    let l = lum_vers(w);
    if (eclair.carte.w < 0.5 || !dans_boite(l)) {
        return 1.0;
    }
    // (La carte grossière, `GROSSIERE` : aucune carte n'est plus près du soleil que ce point dans tout ce que le filtre lit autour de
    // lui, les cases qui couvrent 4 texels de chaque côté ; chaque lecture le verrait au soleil, le filtre rendrait 1 : quatre
    // lectures au lieu de 9 ou 25 comparées.)
    let n = vec2<i32>(textureDimensions(cartes_au_plus_pres));
    let t = l.xy * vec2<f32>(n * 8);
    let a = clamp(vec2<i32>(floor((t - 4.0) / 8.0)), vec2<i32>(0), n - vec2<i32>(1));
    let b = clamp(vec2<i32>(floor((t + 4.0) / 8.0)), vec2<i32>(0), n - vec2<i32>(1));
    let m = min(
        min(textureLoad(cartes_au_plus_pres, a, 0).r, textureLoad(cartes_au_plus_pres, vec2<i32>(b.x, a.y), 0).r),
        min(textureLoad(cartes_au_plus_pres, vec2<i32>(a.x, b.y), 0).r, textureLoad(cartes_au_plus_pres, b, 0).r),
    );
    if (m >= l.z - BIAIS) {
        return 1.0;
    }
    return soleil_des_cartes_sur_le_decor(l);
}
"#;

/// Le côté d'une case de la carte grossière (`GROSSIERE`), en texels de la carte d'ombres des cartes (le décor de la vue y lit les
/// cases qui couvrent 4 texels de chaque côté de son point : toute lecture du filtre du décor, au plus 2 × `ombres::PAS` = 3 texels
/// du point, plus le texel voisin de la lecture bilinéaire, y tombe).
const CASE_GROSSIERE: u32 = 8;

/// La carte grossière des cartes de la vue (partie 189) : pour chaque case de `CASE_GROSSIERE` texels de côté de la carte d'ombres des
/// cartes, la profondeur la plus proche du soleil (16 lectures de quatre texels). Le décor de la vue la lit d'abord : là où aucune
/// carte n'est plus près du soleil que lui, le filtre rendrait 1 à coup sûr (le même résultat, sans ses lectures). Refaite avec la
/// carte d'ombres des cartes, sur la boîte des cartes.
const GROSSIERE: &str = r#"
@group(0) @binding(0) var carte: texture_depth_2d;
@group(0) @binding(1) var s_carte: sampler;

@vertex
fn vs_g(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_g(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(carte));
    let c = floor(pos.xy) * 8.0;
    var m = 1.0;
    for (var y = 0; y < 4; y = y + 1) {
        for (var x = 0; x < 4; x = x + 1) {
            // (Les quatre texels autour du coin (2x + 1, 2y + 1) de la case.)
            let q = textureGather(carte, s_carte, (c + vec2<f32>(f32(2 * x + 1), f32(2 * y + 1))) * texel);
            m = min(m, min(min(q.x, q.y), min(q.z, q.w)));
        }
    }
    return vec4<f32>(m, 0.0, 0.0, 1.0);
}
"#;

/// La carte d'ombres des cartes de la vue Espace 3D (partie 189) : chaque carte qui porte une ombre, ses quatre coins dans le monde
/// vus du soleil (la matrice de la carte d'ombres du décor), là où son encre est au moins à moitié opaque. Comme `fs_ombre` de
/// `gpu.rs`, qui la fait pour l'image du plan (où les cartes sont dans la feuille et relevées sur leur plan).
const PORTEUSES: &str = r#"
@group(0) @binding(0) var<uniform> eclair: Lumiere;
@group(1) @binding(0) var t_carte: texture_2d<f32>;
@group(1) @binding(1) var s_carte: sampler;

// `ombres::SEUIL_ALPHA`.
const SEUIL_ALPHA: f32 = 0.5;

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacite: f32,
};

@vertex
fn vs_porteuse(@location(0) p: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) opacite: f32) -> POut {
    let q = eclair.light_vp * vec4<f32>(p, 1.0);
    var o: POut;
    // (Ce qui est entre le soleil et la carte d'ombres cache tout : ramené au plus près, comme `gpu.rs`.)
    o.pos = vec4<f32>(q.x, q.y, clamp(q.z, 0.0, 1.0), 1.0);
    o.uv = uv;
    o.opacite = opacite;
    return o;
}

@fragment
fn fs_porteuse(o: POut) {
    if (textureSampleLevel(t_carte, s_carte, o.uv, 0.0).a * o.opacite < SEUIL_ALPHA) {
        discard;
    }
}
"#;

/// L'ombre reçue par une carte de la vue Espace 3D (partie 189), posée juste après la carte dans la passe d'egui : sur le même
/// maillage (les mêmes triangles, la même lecture du dessin, `@interpolate(linear)` comme les sommets d'egui), la couleur qu'egui y
/// a mise, assombrie comme dans l'image du plan (`soleil_vu_par_la_carte`, `assombrir_carte` : le décor et les autres cartes, la
/// pente de la carte dans la carte d'ombres), est **retranchée** de l'écran (`ReverseSubtract`) : ce qui est sous une encre à demi
/// transparente garde sa part, au pixel près.
const RECUE: &str = r#"
@group(0) @binding(0) var<uniform> eclair: Lumiere;
@group(0) @binding(1) var ombres_decor: texture_depth_2d;
@group(0) @binding(2) var ombres_cartes: texture_depth_2d;
@group(0) @binding(3) var s_cmp: sampler_comparison;

struct Recue {
    reglage: vec4<f32>,   // pente de la carte dans la carte d'ombres (x, y) ; opacité du calque ; écran en sRGB (1/0)
};
@group(1) @binding(0) var<uniform> recue: Recue;
@group(2) @binding(0) var t_carte: texture_2d<f32>;
@group(2) @binding(1) var s_carte: sampler;

struct ROut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(linear) uv: vec2<f32>,
    @location(1) monde: vec3<f32>,
};

@vertex
fn vs_recue(@location(0) ndc: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) monde: vec3<f32>, @location(3) w: f32) -> ROut {
    var o: ROut;
    // (Le w de la vue : le point du monde est interpolé en perspective, juste en chaque pixel ; la lecture du dessin, elle, est
    // linéaire à l'écran comme celle d'egui.)
    o.pos = vec4<f32>(ndc * w, 0.5 * w, w);
    o.uv = uv;
    o.monde = monde;
    return o;
}

@fragment
fn fs_recue(o: ROut) -> @location(0) vec4<f32> {
    let t = textureSample(t_carte, s_carte, o.uv);
    // Ce qu'egui a posé : le dessin en valeurs gamma, prémultiplié, à l'opacité du calque.
    let p = vec4<f32>(vers_srgb(t.rgb), t.a) * recue.reglage.z;
    if (p.a <= 0.0) {
        discard;
    }
    let vis = soleil_vu_par_la_carte(lum_vers(o.monde), recue.reglage.xy);
    if (vis >= 1.0) {
        discard;
    }
    let s = assombrir_carte(p, 1.0 - vis);
    var d = p.rgb - s.rgb;
    if (recue.reglage.w > 0.5) {
        d = vers_lin(p.rgb) - vers_lin(s.rgb);
    }
    return vec4<f32>(max(d, vec3<f32>(0.0)), 0.0);
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

/// La profondeur de l'image du plan en quatre échantillons (partie 193, `PROF`) : après la passe de la profondeur seule (`fs_prof`),
/// chaque pixel de la feuille lit les quatre échantillons de son tampon et y met l'inverse de leur distance à l'œil, le long de son axe (0 là où
/// rien n'est dessiné : le ciel ne cache rien). Le proche, le loin et l'orthographique de la passe sont dans ses réglages (après
/// les 320 premiers octets, `dessiner_dans`).
const PROF_DES_ECHANTILLONS: &str = r#"
struct Reglages {
    avant: array<vec4<f32>, 20>,
    conv: vec4<f32>,     // proche, loin, orthographique (1/0)
};
@group(0) @binding(0) var<uniform> g: Reglages;
@group(0) @binding(1) var t_prof: texture_depth_multisampled_2d;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

// L'inverse de la distance d'une profondeur du tampon (`Vue::decoupe` : de 0 au proche à 1 au loin).
fn inverse(z: f32) -> f32 {
    if (z >= 1.0) {
        return 0.0;
    }
    let proche = g.conv.x;
    let loin = g.conv.y;
    if (g.conv.z > 0.5) {
        return 1.0 / max(proche + z * (loin - proche), 1e-6);
    }
    let k = loin / (loin - proche);
    return max(1.0 - z / k, 0.0) / proche;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = vec2<i32>(floor(pos.xy));
    return vec4<f32>(inverse(textureLoad(t_prof, q, 0)), inverse(textureLoad(t_prof, q, 1)), inverse(textureLoad(t_prof, q, 2)), inverse(textureLoad(t_prof, q, 3)));
}
"#;

/// Le voile d'une carte de la vue Espace 3D (partie 183) : sur le quadrilatère de la carte, le décor là où il est plus près
/// qu'elle, lu dans les cibles de la passe du décor de cette vue (sa couleur lissée et son tampon de profondeur à quatre
/// échantillons). Posé juste après le dessin de la carte : le décor qui est devant la carte la recouvre.
const VOILE: &str = r#"
struct Voile {
    vp: mat4x4<f32>,
    coins: array<vec4<f32>, 4>,
    oeil: vec4<f32>,     // xyz ; w : orthographique (1/0)
    avant: vec4<f32>,    // xyz ; w : proche
    lin: vec4<f32>,      // loin, taille de la cible (pixels), sortie sRGB (1/0)
};
@group(0) @binding(0) var<uniform> v: Voile;
@group(1) @binding(0) var t_couleur: texture_2d<f32>;
@group(1) @binding(1) var t_prof: texture_depth_multisampled_2d;

// Les tolérances de `profondeur.rs` (`TOLERANCE_RELATIVE`, `TOLERANCE_METRES`).
const TOLERANCE_RELATIVE: f32 = 0.004;
const TOLERANCE_METRES: f32 = 0.01;

struct Out {
    @builtin(position) pos: vec4<f32>,
    // (La position de découpe et la distance à l'œil, linéaires dans l'espace : interpolées en perspective, elles sont justes en
    // chaque point de la carte.)
    @location(0) decoupe: vec3<f32>,
    @location(1) dist: f32,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    var ordre = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let p = v.coins[ordre[i]].xyz;
    let c = v.vp * vec4<f32>(p, 1.0);
    var out: Out;
    // (La profondeur de découpe au milieu : une carte plus loin que le décor, au-delà de son plan lointain, n'est pas coupée ;
    // ce qui passe derrière l'œil l'est toujours.)
    out.pos = vec4<f32>(c.x, c.y, 0.5 * c.w, c.w);
    out.decoupe = vec3<f32>(c.x, c.y, c.w);
    out.dist = dot(p - v.oeil.xyz, v.avant.xyz);
    return out;
}

// La distance à l'œil d'une profondeur du tampon (`Vue::decoupe`).
fn distance_du_decor(z: f32) -> f32 {
    let proche = v.avant.w;
    let loin = v.lin.x;
    if (v.oeil.w > 0.5) {
        return proche + z * (loin - proche);
    }
    let k = loin / (loin - proche);
    return proche / max(1.0 - z / k, 1e-9);
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    // (Partie 193 : la distance de la carte à la place de chacun des quatre échantillons du décor, d'après sa pente à l'écran ; une
    // carte inclinée qui entre dans un objet a un bord lissé. Les dérivées avant tout `discard`.)
    let pente = vec2<f32>(dpdx(in.dist), dpdy(in.dist));
    var places = array<vec2<f32>, 4>(vec2<f32>(0.375, 0.125), vec2<f32>(0.875, 0.375), vec2<f32>(0.125, 0.625), vec2<f32>(0.625, 0.875));
    let ndc = in.decoupe.xy / in.decoupe.z;
    let taille = vec2<f32>(v.lin.y, v.lin.z);
    let q = vec2<i32>(floor(vec2<f32>((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5) * taille));
    if (q.x < 0 || q.y < 0 || q.x >= i32(taille.x) || q.y >= i32(taille.y)) {
        discard;
    }
    var devant = 0.0;
    for (var s = 0; s < 4; s = s + 1) {
        let d = in.dist + dot(pente, places[s] - vec2<f32>(0.5));
        let seuil = d * (1.0 - TOLERANCE_RELATIVE) - TOLERANCE_METRES;
        let z = textureLoad(t_prof, q, s);
        if (z < 1.0 && distance_du_decor(z) < seuil) {
            devant = devant + 1.0;
        }
    }
    let c = textureLoad(t_couleur, q, 0);
    if (devant <= 0.0 || c.a <= 0.0) {
        discard;
    }
    var rgb = min(c.rgb / c.a, vec3<f32>(1.0));
    if (v.lin.w > 0.5) {
        rgb = pow(rgb, vec3<f32>(2.2));
    }
    let f = devant / 4.0;
    return vec4<f32>(rgb * f, f);
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
    /// Sa texture (partie 185 : l'export la relit pour les cartes qui reçoivent les ombres du décor).
    ombre_tex: Option<wgpu::Texture>,
    /// La dernière passe de cette prise a dessiné sa carte d'ombres (partie 185 : les cartes la lisent, `carte_ombres`).
    ombre_faite: bool,
    utilise: u64,
    /// La dernière passe de cette prise (partie 183) : sa matrice monde → découpe, son œil et son axe, et de quoi ramener sa
    /// profondeur à une distance (proche, loin, orthographique) ; le voile des cartes de la vue s'en sert (`VoileCarte`).
    passe: Option<Passe>,
    /// Les réglages des voiles des cartes de cette vue, un pas de `PAS_VOILE` par carte, et leur capacité (partie 183).
    voiles: Option<(wgpu::Buffer, wgpu::BindGroup, u64)>,
    /// Les ombres des cartes de cette vue (partie 189) : la lumière de sa dernière passe et le groupe des deux cartes d'ombres (rien :
    /// pas d'ombres de cartes à cette image) ; les ombres reçues par ses cartes.
    ombres_vue: Option<(crate::ombres::Lumiere, wgpu::BindGroup)>,
    recues: Option<Recues>,
}

/// Les ombres reçues par les cartes d'une vue (partie 189) : un pas par carte dans chaque tampon, et leur capacité.
struct Recues {
    reglages: wgpu::Buffer,
    groupe: wgpu::BindGroup,
    sommets: wgpu::Buffer,
    indices: wgpu::Buffer,
    capacite: u64,
    /// Pour chaque carte de cette image : le groupe de son dessin et le nombre de ses indices.
    cartes: Vec<Option<(wgpu::BindGroup, u32)>>,
}

/// Ce que les ombres des cartes de la vue Espace 3D partagent entre les vues (partie 189), fait à la première qui en a besoin : les
/// pipelines, la lumière, et **la carte d'ombres des cartes** (le soleil est le même de partout : elle ne dépend pas du point de vue),
/// refaite seulement quand une carte, son dessin ou le soleil a changé (`cle`).
struct OmbresVue {
    main4c: wgpu::RenderPipeline,
    porteuses: wgpu::RenderPipeline,
    recue_shader: wgpu::ShaderModule,
    recue: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    layout_ombres: wgpu::BindGroupLayout,
    layout_recue: wgpu::BindGroupLayout,
    layout_carte: wgpu::BindGroupLayout,
    lumiere: wgpu::Buffer,
    groupe_lumiere: wgpu::BindGroup,
    echant_carte: wgpu::Sampler,
    carte: Option<(u32, wgpu::TextureView)>,
    /// La carte grossière (`GROSSIERE`), son pipeline et ce qu'il lit ; une vide (aucune carte ne porte d'ombre).
    grossiere: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    grossiere_pipe: wgpu::RenderPipeline,
    layout_grossiere: wgpu::BindGroupLayout,
    grossiere_vide: wgpu::TextureView,
    echant_proche: wgpu::Sampler,
    cle: Option<u64>,
    sommets: Option<(wgpu::Buffer, u64)>,
}

/// Ce que le voile d'une carte de la vue lit de la passe du décor (`Prise3::passe`).
#[derive(Clone, Copy, Debug)]
struct Passe {
    vp: M4,
    oeil: V3,
    avant: V3,
    proche: f32,
    loin: f32,
    ortho: bool,
    taille_px: [u32; 2],
}

struct Cibles {
    taille: [u32; 2],
    echantillons: u32,
    couleur: wgpu::TextureView,
    profondeur: wgpu::TextureView,
    resolue: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    /// La vue (partie 183) : sa couleur lissée et son tampon de profondeur, tels que le voile des cartes les lit (`VoileCarte`).
    voile: Option<wgpu::BindGroup>,
}

/// Les tampons de profondeur de l'image du plan (partie 193, `Moteur::feuille4`) : celui de sa passe de couleur (un échantillon,
/// jeté après elle) et celui de sa passe de profondeur seule (quatre échantillons, gardé : la profondeur des cartes en est tirée).
struct Feuille4 {
    taille: [u32; 2],
    profondeur1: wgpu::TextureView,
    /// (Fait la première fois qu'une carte partage la profondeur d'un décor : sans elle, rien de plus que la partie 183.)
    profondeur4: Option<wgpu::TextureView>,
}

/// Le moteur du décor : pipelines, décors envoyés, prises.
pub struct Moteur {
    /// La vue Espace 3D (quatre échantillons).
    main4: wgpu::RenderPipeline,
    ciel4: wgpu::RenderPipeline,
    /// L'image du plan, avec la profondeur en une valeur pour seconde cible (`PROF1`, partie 183) ; et la part du soleil (partie 185,
    /// `SOLEIL`).
    main1p: wgpu::RenderPipeline,
    ciel1p: wgpu::RenderPipeline,
    main1ps: wgpu::RenderPipeline,
    ciel1ps: wgpu::RenderPipeline,
    /// Passes de couleur de l'image du plan et passes de la profondeur à quatre échantillons faites depuis le début (essais).
    pub passes_couleur: u64,
    pub passes_quatre: u64,
    /// La profondeur seule de l'image du plan à quatre échantillons (partie 193, `fs_prof`), et ce qui la ramène à `PROF`
    /// (`PROF_DES_ECHANTILLONS`).
    prof4: wgpu::RenderPipeline,
    /// La même, pour les matières à seuil d'alpha (découpées comme dans la passe de couleur).
    prof4s: wgpu::RenderPipeline,
    prof_pipe: wgpu::RenderPipeline,
    prof_layout: wgpu::BindGroupLayout,
    /// Les tampons de profondeur de l'image du plan (partie 193), partagés par toutes ses prises (chaque passe les reprend tout
    /// entiers), refaits quand la taille change.
    feuille4: Option<Feuille4>,
    ombre_pipe: wgpu::RenderPipeline,
    /// Le poser de la vue sur l'écran d'egui, fait à la première vue (il dépend du format de l'écran).
    blit: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    blit_shader: wgpu::ShaderModule,
    blit_layout: wgpu::BindGroupLayout,
    blit_uniform: wgpu::Buffer,
    /// Le voile des cartes de la vue (partie 183), fait à la première vue comme le poser : son format, son pipeline.
    voile: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    voile_shader: wgpu::ShaderModule,
    voile_layout_reglages: wgpu::BindGroupLayout,
    voile_layout_cibles: wgpu::BindGroupLayout,
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
    /// Le tic au début de la passe en cours : un modèle qu'elle dessine n'est jamais renvoyé de la carte pour en faire entrer un
    /// autre (un décor de plus de huit modèles ne les renverrait pas à chaque image).
    debut: u64,
    max_texture: u32,
    /// Triangles dessinés par la dernière passe (essais, mesures).
    pub dessines: usize,
    /// Les ombres des cartes de la vue Espace 3D (partie 189).
    vue_ombres: Option<Box<OmbresVue>>,
    /// Cartes d'ombres des cartes de la vue dessinées depuis le début (essais : gardée quand rien n'a bougé).
    pub cartes_d_ombres_faites: u64,
}

/// Une chose à dessiner : un décor (sa scène, pour l'envoyer s'il n'est pas encore dans la carte) et sa matrice.
#[derive(Clone)]
pub struct Dessin {
    pub id: u64,
    pub scene: Arc<Scene3d>,
    pub matrice: M4,
    pub aspect: Aspect,
    /// Partie 199 : ce que l'aspect fait de cet objet (ses matières, sa couleur d'identification à plat, l'argile) : seulement la
    /// couleur de base de ses matières, écrite à chaque passe ; le modèle à la carte ne change pas.
    pub teinte: crate::couleurs3d::Teinte,
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

/// Le soleil et la brume d'une passe du décor (partie 185) : ce que `dessiner_dans` calcule pour ses ombres et sa brume, et que les
/// ombres des cartes reprennent tel quel (`ombres.rs`) : la même carte d'ombres (sa matrice, son côté, son filtre), la même brume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eclairage {
    /// Monde (le repère où l'on regarde) → découpe de la carte d'ombres : orthographique, vue du soleil, sur la boîte des décors.
    pub light_vp: M4,
    /// Début et fin de la brume (mètres de l'œil).
    pub brume: (f32, f32),
    /// Le décor dessine ses ombres (l'aspect « Ombres », une qualité de cet ordinateur, un décor non vide).
    pub ombres: bool,
    /// Lectures du filtre (0 : sans ombres) et côté de la carte d'ombres (`Qualite::ombres`).
    pub taps: u32,
    pub cote: u32,
}

/// Le soleil et la brume des décors `dessins` vus de l'œil `oeil`, à la qualité d'ombres `qualite`.
pub fn eclairage(dessins: &[Dessin], oeil: V3, aspect: &Aspect, qualite: crate::decor3dbudget::Qualite) -> Eclairage {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for d in dessins {
        let (a, b) = d.scene.bornes();
        for i in 0..8 {
            let c = m::point(&d.matrice, [0, 1, 2].map(|k| if i >> k & 1 == 0 { a[k] } else { b[k] }));
            for k in 0..3 {
                lo[k] = lo[k].min(c[k]);
                hi[k] = hi[k].max(c[k]);
            }
        }
    }
    // La brume : de la distance du plus proche coin jusqu'au plus loin (relative au décor, quel que soit son échelle).
    let dist_max = (0..8).map(|i| m::length(m::sub([0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] }), oeil))).fold(0.0f32, f32::max).max(1e-3);
    let brume = aspect.brume_de(dist_max);
    // Les ombres du soleil : une carte d'ombres, à l'orthographique sur la boîte des décors.
    let soleil = aspect.soleil();
    let (taps, cote) = qualite.ombres();
    let ombres = aspect.ombres && taps > 0 && lo[0] <= hi[0];
    let (centre, rayon) = if lo[0] <= hi[0] { (m::scale(m::add(lo, hi), 0.5), (m::length(m::sub(hi, lo)) * 0.5).max(1e-3)) } else { ([0.0; 3], 1.0) };
    let light_vp = {
        let f = m::scale(soleil, -1.0);
        let r = m::normalize(m::cross(f, if f[1].abs() > 0.99 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }));
        let u = m::cross(r, f);
        let oeil_l = m::sub(centre, m::scale(f, rayon * 2.0));
        let v = Vue::Ortho { oeil: oeil_l, droite: r, haut: u, avant: f, px_par_unite: 1.0 / rayon, centre: [1.0, 1.0] };
        v.decoupe([2.0, 2.0], 0.0, rayon * 4.0)
    };
    Eclairage { light_vp, brume, ombres, taps: if ombres { taps } else { 0 }, cote: if ombres { cote } else { 1 } }
}

fn put(bytes: &mut [u8], at: usize, values: &[f32]) {
    for (k, v) in values.iter().enumerate() {
        bytes[at + k * 4..at + k * 4 + 4].copy_from_slice(&v.to_ne_bytes());
    }
}

impl Moteur {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Moteur {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D"), source: wgpu::ShaderSource::Wgsl(format!("{SHADER}{SANS_CARTES}").into()) });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D : poser"), source: wgpu::ShaderSource::Wgsl(BLIT.into()) });
        let voile_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D : voile des cartes"), source: wgpu::ShaderSource::Wgsl(VOILE.into()) });
        let voile_layout_reglages = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D : voile, réglages"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: NonZeroU64::new(VOILE_OCTETS) },
                count: None,
            }],
        });
        let voile_layout_cibles = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D : voile, cibles"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: true },
                    count: None,
                },
            ],
        });
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
        // (Une échantillon : l'image du plan, avec sa profondeur en une valeur, partie 183 ; quatre : la vue Espace 3D.)
        let cibles_vue = [Some(wgpu::ColorTargetState { format: COULEUR, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        // (Partie 185 : une seconde cible, la part du soleil, quand le décor de l'image du plan a ses ombres.)
        let cibles_soleil = [cibles_vue[0].clone(), Some(wgpu::ColorTargetState { format: SOLEIL, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        // (Et la profondeur en une valeur comme seconde cible, celle de la partie 183 : `PROF1`.)
        let cible_prof1 = Some(wgpu::ColorTargetState { format: PROF1, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let cibles_p = [cibles_vue[0].clone(), cible_prof1.clone()];
        let cibles_ps = [cibles_vue[0].clone(), cible_prof1, cibles_soleil[1].clone()];
        let feuille_p = |vertex: &str, fragment: &str, soleil: bool, profondeur: wgpu::DepthStencilState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("décor 3D : image du plan, profondeur en une valeur"),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(vertex), buffers: if vertex == "vs_main" { &tampons } else { &[] }, compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fragment), targets: if soleil { &cibles_ps } else { &cibles_p }, compilation_options: Default::default() }),
                primitive: if vertex == "vs_main" { wgpu::PrimitiveState { cull_mode: None, ..Default::default() } } else { wgpu::PrimitiveState::default() },
                depth_stencil: Some(profondeur),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let ecrit = wgpu::DepthStencilState { format: PROFONDEUR, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: Default::default() };
        let fond = wgpu::DepthStencilState { format: PROFONDEUR, depth_write_enabled: false, depth_compare: wgpu::CompareFunction::Always, stencil: Default::default(), bias: Default::default() };
        let (main1p, ciel1p) = (feuille_p("vs_main", "fs_main_feuille_p", false, ecrit.clone()), feuille_p("vs_sky", "fs_sky_feuille_p", false, fond.clone()));
        let (main1ps, ciel1ps) = (feuille_p("vs_main", "fs_main_feuille_ps", true, ecrit), feuille_p("vs_sky", "fs_sky_feuille_ps", true, fond));
        let principal = |samples: u32| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("décor 3D"),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &tampons, compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &cibles_vue,
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
                    targets: &cibles_vue,
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
        // La profondeur de l'image du plan en quatre échantillons (partie 193) : ses réglages (ceux de la passe) et son tampon.
        let prof_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D : profondeur des échantillons"), source: wgpu::ShaderSource::Wgsl(PROF_DES_ECHANTILLONS.into()) });
        let prof_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D : profondeur des échantillons"),
            entries: &[
                uniforme(0, GLOBAL, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: true },
                    count: None,
                },
            ],
        });
        let prof_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("décor 3D : profondeur des échantillons"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("décor 3D : profondeur des échantillons"), bind_group_layouts: &[&prof_layout], push_constant_ranges: &[] })),
            vertex: wgpu::VertexState { module: &prof_shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &prof_shader, entry_point: Some("fs"), targets: &[Some(wgpu::ColorTargetState { format: PROF, blend: None, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let profondeur_seule = |seuil: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("décor 3D : profondeur seule"),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(if seuil { "vs_prof" } else { "vs_prof_seul" }), buffers: &tampons, compilation_options: Default::default() },
                fragment: seuil.then(|| wgpu::FragmentState { module: &shader, entry_point: Some("fs_prof"), targets: &[], compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: PROFONDEUR, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: Default::default() }),
                multisample: wgpu::MultisampleState { count: ECHANTILLONS_VUE, mask: !0, alpha_to_coverage_enabled: false },
                multiview: None,
                cache: None,
            })
        };
        Moteur {
            main4: principal(ECHANTILLONS_VUE),
            ciel4: ciel(ECHANTILLONS_VUE),
            main1p,
            ciel1p,
            main1ps,
            ciel1ps,
            passes_couleur: 0,
            passes_quatre: 0,
            prof4: profondeur_seule(false),
            prof4s: profondeur_seule(true),
            prof_pipe,
            prof_layout,
            feuille4: None,
            ombre_pipe,
            blit: None,
            blit_shader,
            blit_layout,
            blit_uniform,
            voile: None,
            voile_shader,
            voile_layout_reglages,
            voile_layout_cibles,
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
            debut: 0,
            max_texture: device.limits().max_texture_dimension_2d,
            dessines: 0,
            vue_ombres: None,
            cartes_d_ombres_faites: 0,
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
        // Les modèles qui ne se montrent plus depuis longtemps partent (un autre projet, un objet enlevé) ; jamais un de la
        // passe en cours.
        if self.modeles.len() > 8 {
            let debut = self.debut;
            let plus_vieux = self.modeles.iter().filter(|(k, mg)| **k != id && mg.utilise < debut).min_by_key(|(_, mg)| mg.utilise).map(|(k, _)| *k);
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
            Prise3 { globals, groupe_global, objets, groupe_objets, capacite: 16, cibles: None, ombre: None, ombre_tex: None, ombre_faite: false, utilise: tic, passe: None, voiles: None, ombres_vue: None, recues: None }
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
        prof1: Option<&wgpu::TextureView>,
        soleil_cible: Option<&wgpu::TextureView>,
        echantillons: u32,
        ciel: bool,
        qualite: crate::decor3dbudget::Qualite,
        cartes: Option<&CartesVue>,
    ) {
        self.debut = self.tic + 1;
        for d in dessins {
            self.assurer(device, queue, d.id, &d.scene);
        }
        let aspect = dessins.first().map(|d| d.aspect).unwrap_or_default();
        // Proche et loin d'après les coins des décors placés : la profondeur garde sa précision.
        let (oeil, _, _, avant, _, _, ortho) = vue.axes();
        let (mut dmin, mut dmax) = (f32::MAX, 0.0f32);
        for d in dessins {
            let (a, b) = d.scene.bornes();
            for i in 0..8 {
                let c = m::point(&d.matrice, [0, 1, 2].map(|k| if i >> k & 1 == 0 { a[k] } else { b[k] }));
                let z = m::dot(m::sub(c, oeil), avant);
                dmin = dmin.min(z);
                dmax = dmax.max(z);
            }
        }
        let loin = (dmax * 1.05).max(1.0);
        let proche = if ortho { dmin.min(0.0) - 1.0 } else { (dmin * 0.9).max(loin * 2e-5).max(0.005) };
        let vp = vue.decoupe(taille_unites, proche, loin);
        // La brume et le soleil (sa carte d'ombres, à l'orthographique sur la boîte des décors) : `eclairage`, que les ombres des
        // cartes reprennent (partie 185).
        let ec = eclairage(dessins, oeil, &aspect, qualite);
        let (brume_debut, brume_fin) = ec.brume;
        let soleil = aspect.soleil();
        let (taps, cote_ombre) = (ec.taps, ec.cote);
        let ombres = ec.ombres;
        let light_vp = ec.light_vp;
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
        // (Partie 193 : de quoi ramener le tampon de profondeur à l'inverse d'une distance, `PROF_DES_ECHANTILLONS`.)
        put(&mut g, 320, &[proche, loin, if est_ortho { 1.0 } else { 0.0 }, 0.0]);
        let mut objets = vec![0u8; (n * PAS_OBJET) as usize];
        let mut rang = 0usize;
        for d in dessins {
            let Some(mg) = self.modeles.get(&d.id) else { continue };
            let nm = crate::decor3drendu::normal_matrix(&d.matrice);
            for &(couleur, tex, seuil) in &mg.matieres {
                let at = rang * PAS_OBJET as usize;
                let (couleur, couleur_de_texture) = d.teinte.base(couleur);
                put(&mut objets, at, &d.matrice);
                put(&mut objets, at + 64, &nm);
                put(&mut objets, at + 128, &couleur);
                // (z : la texture garde sa couleur, ou seulement sa découpe ; w : éclairé à plat, l'aspect Couleurs, partie 199.)
                put(&mut objets, at + 144, &[if tex.is_some() { 1.0 } else { 0.0 }, seuil, if couleur_de_texture { 1.0 } else { 0.0 }, if d.teinte.plate() { 1.0 } else { 0.0 }]);
                rang += 1;
            }
        }
        // La carte d'ombres de cette prise.
        if ombres {
            let (layout, echant) = (self.layout_global.clone(), self.echant_ombre.clone());
            let p = self.prise(device, cle, n);
            if p.ombre.as_ref().is_none_or(|(c, _)| *c != cote_ombre) {
                let tex = texture_profondeur_t(device, [cote_ombre, cote_ombre], 1, true);
                p.ombre = Some((cote_ombre, tex.create_view(&Default::default())));
                p.ombre_tex = Some(tex);
                p.groupe_global = groupe_global(device, &layout, &p.globals, &p.ombre.as_ref().expect("ombre").1, &echant);
            }
        }
        // Les ombres des cartes de la vue (partie 189) : seulement quand le décor a ses ombres ; la carte d'ombres des cartes (gardée
        // si rien n'a bougé), et le groupe que le décor et les cartes de cette vue en lisent.
        let lum = cartes.filter(|_| ombres && echantillons == ECHANTILLONS_VUE).and_then(|_| crate::ombres::Lumiere::de(&ec, &aspect, &Prise { oeil: oe, droite: r, haut: u, avant: f, px_par_unite: 1.0, centre: [0.0; 2], proche: 0.01 }));
        let decor_cartes = match (lum, cartes) {
            (Some(lum), Some(cv)) => self.preparer_ombres_vue(device, queue, encoder, cle, n, cv, lum),
            _ => {
                self.prise(device, cle, n).ombres_vue = None;
                None
            }
        };
        let p = self.prise(device, cle, n);
        p.passe = Some(Passe { vp, oeil: oe, avant: f, proche, loin, ortho: est_ortho, taille_px });
        p.ombre_faite = ombres;
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
        // (Quand le décor de l'image du plan a ses ombres, la part du soleil : partie 185. La vue garde son tampon de profondeur : le
        // voile de ses cartes le lit, `VoileCarte`.)
        let cible_soleil = soleil_cible.map(|v| wgpu::RenderPassColorAttachment { view: v, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } });
        // (Sans les bords doux, la profondeur en une valeur, seconde cible comme à la partie 183 : `PROF1`.)
        let cible_prof1 = prof1.map(|v| wgpu::RenderPassColorAttachment { view: v, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } });
        let couleur_att = Some(wgpu::RenderPassColorAttachment {
            view: couleur,
            resolve_target: resolue,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: if resolue.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store } },
        });
        let attaches: Vec<Option<wgpu::RenderPassColorAttachment>> = if cible_prof1.is_some() { [couleur_att, cible_prof1, cible_soleil].into_iter().take(if soleil_cible.is_some() { 3 } else { 2 }).collect() } else { [couleur_att, cible_soleil].into_iter().take(if soleil_cible.is_some() { 2 } else { 1 }).collect() };
        debug_assert!(prof1.is_some() == (echantillons == 1), "l'image du plan a sa profondeur, la vue non");
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("décor 3D"),
            color_attachments: &attaches,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: profondeur, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: if echantillons > 1 { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard } }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &groupe_global, &[]);
        let avec_soleil = soleil_cible.is_some();
        if ciel {
            pass.set_pipeline(match (echantillons > 1, avec_soleil) {
                (true, _) => &self.ciel4,
                (false, true) => &self.ciel1ps,
                (false, false) => &self.ciel1p,
            });
            pass.set_bind_group(1, &groupe_objets, &[0]);
            pass.set_bind_group(2, &self.blanc, &[]);
            pass.draw(0..3, 0..1);
        }
        match (&decor_cartes, self.vue_ombres.as_ref()) {
            // (Partie 189 : le décor de la vue sous les ombres des cartes.)
            (Some(groupe), Some(ov)) if echantillons > 1 => {
                pass.set_pipeline(&ov.main4c);
                pass.set_bind_group(3, groupe, &[]);
            }
            _ => pass.set_pipeline(match (echantillons > 1, avec_soleil) {
                (true, _) => &self.main4,
                (false, true) => &self.main1ps,
                (false, false) => &self.main1p,
            }),
        }
        self.poser(&mut pass, dessins, &groupe_objets, &mut dessines, true);
        drop(pass);
        self.dessines = dessines;
    }

    /// Comme `poser`, pour la profondeur seule de l'image du plan (partie 193) : sans fragment, sauf pour les matières à seuil.
    fn poser_profondeur(&self, pass: &mut wgpu::RenderPass<'_>, dessins: &[Dessin], groupe_objets: &wgpu::BindGroup) {
        let mut base = 0usize;
        let mut seuil_pose = None;
        for d in dessins {
            let Some(mg) = self.modeles.get(&d.id) else { continue };
            for mc in &mg.morceaux {
                let (_, tex, seuil) = mg.matieres[mc.prim];
                let a_seuil = seuil > 0.0;
                if seuil_pose != Some(a_seuil) {
                    pass.set_pipeline(if a_seuil { &self.prof4s } else { &self.prof4 });
                    seuil_pose = Some(a_seuil);
                }
                pass.set_bind_group(1, groupe_objets, &[((base + mc.prim) as u64 * PAS_OBJET) as u32]);
                if a_seuil {
                    let groupe = tex.and_then(|t| mg.textures.get(t)).and_then(|t| t.as_ref()).map_or(&self.blanc, |t| &t.groupe);
                    pass.set_bind_group(2, groupe, &[]);
                }
                pass.set_vertex_buffer(0, mc.sommets.slice(..));
                pass.set_index_buffer(mc.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mc.nombre, 0, 0..1);
            }
            base += mg.matieres.len();
        }
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
    ///
    /// `prof` : la texture de même taille (`PROF1`) où va la profondeur que les cartes partagent avec le décor (partie 183), en une
    /// valeur par pixel, seconde cible de la passe de couleur. `prof4` (partie 193, les bords doux : une carte du plan partage la
    /// profondeur) : la texture (`PROF`) où va aussi la profondeur en quatre échantillons, après la couleur (`profondeur_quatre`).
    ///
    /// `soleil` (partie 185) : la texture de même taille (`SOLEIL`) où va la part du soleil, quand le décor a ses ombres et que
    /// les cartes peuvent en porter ; rien : sans elle.
    pub fn dessiner_feuille(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, cle: u64, dessins: &[Dessin], prise: &Prise, taille: [u32; 2], cible: &wgpu::TextureView, prof: &wgpu::TextureView, prof4: Option<&wgpu::TextureView>, soleil: Option<&wgpu::TextureView>, ciel: bool, qualite: crate::decor3dbudget::Qualite) {
        if self.feuille4.as_ref().is_none_or(|f| f.taille != taille) {
            self.feuille4 = Some(Feuille4 { taille, profondeur1: texture_profondeur(device, taille, 1, false), profondeur4: None });
        }
        let profondeur1 = self.feuille4.as_ref().expect("feuille4").profondeur1.clone();
        self.passes_couleur += 1;
        self.dessiner_dans(device, queue, encoder, cle, dessins, &Vue::Prise(*prise), [taille[0] as f32, taille[1] as f32], taille, cible, None, &profondeur1, Some(prof), soleil, 1, ciel, qualite, None);
        if let Some(p4) = prof4 {
            self.profondeur_quatre(device, queue, encoder, cle, dessins, taille, p4);
        }
    }

    /// **La profondeur de l'image du plan en quatre échantillons** (partie 193, `PROF`) : la passe de la profondeur seule à quatre
    /// échantillons, avec les réglages et les objets de la dernière passe de couleur de la prise `cle` (le même décor vu du même œil),
    /// puis ramenée à l'inverse des distances dans `prof`. Seule, elle sert quand une carte se met à partager la profondeur d'un décor
    /// déjà dessiné (la couleur n'est pas refaite). Faux si la prise n'a pas encore de passe de couleur.
    pub fn profondeur_quatre(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, cle: u64, dessins: &[Dessin], taille: [u32; 2], prof: &wgpu::TextureView) -> bool {
        if !self.prises.get(&cle).is_some_and(|p| p.passe.is_some_and(|ps| ps.taille_px == taille)) {
            return false;
        }
        // (Les modèles sont dans la carte depuis la passe de couleur ; s'il en manquait un, il y revient.)
        for d in dessins {
            self.assurer(device, queue, d.id, &d.scene);
        }
        if self.feuille4.as_ref().is_none_or(|f| f.taille != taille) {
            self.feuille4 = Some(Feuille4 { taille, profondeur1: texture_profondeur(device, taille, 1, false), profondeur4: None });
        }
        let f = self.feuille4.as_mut().expect("feuille4");
        let profondeur4 = f.profondeur4.get_or_insert_with(|| texture_profondeur(device, taille, ECHANTILLONS_VUE, true)).clone();
        self.passes_quatre += 1;
        let p = &self.prises[&cle];
        let (groupe_global, groupe_objets) = (p.groupe_global.clone(), p.groupe_objets.clone());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("décor 3D : profondeur seule"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &profondeur4, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &groupe_global, &[]);
            pass.set_bind_group(2, &self.blanc, &[]);
            self.poser_profondeur(&mut pass, dessins, &groupe_objets);
        }
        // Ramenée à l'inverse des distances des quatre échantillons (avec les réglages de la passe : chaque prise a les siens).
        let groupe = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("décor 3D : profondeur des échantillons"),
            layout: &self.prof_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: p.globals.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&profondeur4) }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("décor 3D : profondeur des échantillons"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: prof, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.prof_pipe);
        pass.set_bind_group(0, &groupe, &[]);
        pass.draw(0..3, 0..1);
        true
    }


    /// La carte d'ombres du soleil de la dernière passe de la prise `cle`, si elle en a dessiné une (partie 185 : les cartes qui
    /// reçoivent les ombres du décor la lisent ; la vue garde la texture tant qu'on la tient).
    pub fn carte_ombres(&self, cle: u64) -> Option<(u32, wgpu::TextureView)> {
        let p = self.prises.get(&cle)?;
        p.ombre.clone().filter(|_| p.ombre_faite)
    }

    fn cibles(&mut self, device: &wgpu::Device, cle: u64, taille: [u32; 2], echantillons: u32, avec_couleur: bool) -> &Cibles {
        let layout = self.blit_layout.clone();
        let voile_cibles = self.voile_layout_cibles.clone();
        let (uniform, echant) = (self.blit_uniform.clone(), self.echant_blit.clone());
        let p = self.prise(device, cle, 1);
        if p.cibles.as_ref().is_none_or(|c| c.taille != taille || c.echantillons != echantillons) {
            // (La vue garde son tampon de profondeur lisible : le voile de ses cartes le lit, partie 183.)
            let profondeur = texture_profondeur(device, taille, echantillons, avec_couleur);
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
            let voile = resolue.as_ref().filter(|_| echantillons == ECHANTILLONS_VUE).map(|(r, _)| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("décor 3D : voile, cibles"),
                    layout: &voile_cibles,
                    entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(r) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&profondeur) }],
                })
            });
            p.cibles = Some(Cibles { taille, echantillons, couleur, profondeur, resolue, voile });
        }
        p.cibles.as_ref().expect("cibles")
    }

    /// L'image de la vue Espace 3D : dans les cibles lissées de la prise `cle`, à poser ensuite par `poser_vue`.
    #[allow(clippy::too_many_arguments)]
    ///
    /// `cartes` (partie 189) : les cartes de la vue qui portent une ombre, quand le décor a ses ombres ; rien : comme avant.
    pub fn dessiner_vue(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, cle: u64, dessins: &[Dessin], vue: &Vue, taille_pts: [f32; 2], taille_px: [u32; 2], qualite: crate::decor3dbudget::Qualite, cartes: Option<&CartesVue>) {
        let c = self.cibles(device, cle, taille_px, ECHANTILLONS_VUE, true);
        let (couleur, profondeur, resolue) = (c.couleur.clone(), c.profondeur.clone(), c.resolue.as_ref().map(|r| r.0.clone()));
        self.dessiner_dans(device, queue, encoder, cle, dessins, vue, taille_pts, taille_px, &couleur, resolue.as_ref(), &profondeur, None, None, ECHANTILLONS_VUE, false, qualite, cartes);
    }

    /// Pose l'image de la vue `cle` dans la passe d'egui.
    pub fn poser_vue(&self, pass: &mut wgpu::RenderPass<'static>, cle: u64) {
        let (Some((_, blit)), Some(groupe)) = (self.blit.as_ref(), self.prises.get(&cle).and_then(|p| p.cibles.as_ref()).and_then(|c| c.resolue.as_ref()).map(|r| &r.1)) else { return };
        pass.set_pipeline(blit);
        pass.set_bind_group(0, groupe, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Le voile des cartes de la vue, au format `format` de l'écran d'egui (la première fois, ou si le format change).
    fn assurer_voile(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.voile.as_ref().is_some_and(|v| v.0 == format) {
            return;
        }
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("décor 3D : voile"), bind_group_layouts: &[&self.voile_layout_reglages, &self.voile_layout_cibles], push_constant_ranges: &[] });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("décor 3D : voile des cartes"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &self.voile_shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &self.voile_shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        self.voile = Some((format, pipe));
    }

    /// Les réglages du voile de la carte `rang` de la vue `cle` (ses quatre coins dans le monde), d'après la dernière passe du décor
    /// de cette vue. Faux : la vue n'a pas de décor dessiné (rien à voiler).
    fn preparer_voile(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, cle: u64, rang: u32, coins: &[V3; 4], srgb: bool) -> bool {
        let layout = self.voile_layout_reglages.clone();
        let Some(p) = self.prises.get_mut(&cle) else { return false };
        let (Some(passe), Some(_)) = (p.passe, p.cibles.as_ref().and_then(|c| c.voile.as_ref())) else { return false };
        let besoin = rang as u64 + 1;
        if p.voiles.as_ref().is_none_or(|v| v.2 < besoin) {
            let cap = besoin.next_power_of_two().max(8);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("décor 3D : voiles"), size: PAS_VOILE * cap, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let groupe = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("décor 3D : voiles"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buffer, offset: 0, size: NonZeroU64::new(VOILE_OCTETS) }) }],
            });
            p.voiles = Some((buffer, groupe, cap));
        }
        let mut b = vec![0u8; VOILE_OCTETS as usize];
        put(&mut b, 0, &passe.vp);
        for (k, c) in coins.iter().enumerate() {
            put(&mut b, 64 + k * 16, &[c[0], c[1], c[2], 1.0]);
        }
        put(&mut b, 128, &[passe.oeil[0], passe.oeil[1], passe.oeil[2], if passe.ortho { 1.0 } else { 0.0 }]);
        put(&mut b, 144, &[passe.avant[0], passe.avant[1], passe.avant[2], passe.proche]);
        put(&mut b, 160, &[passe.loin, passe.taille_px[0] as f32, passe.taille_px[1] as f32, if srgb { 1.0 } else { 0.0 }]);
        if let Some((buffer, _, _)) = &p.voiles {
            queue.write_buffer(buffer, rang as u64 * PAS_VOILE, &b);
        }
        true
    }

    /// Pose le voile de la carte `rang` de la vue `cle` dans la passe d'egui.
    fn poser_voile(&self, pass: &mut wgpu::RenderPass<'static>, cle: u64, rang: u32) {
        let Some((_, pipe)) = self.voile.as_ref() else { return };
        let Some(p) = self.prises.get(&cle) else { return };
        let (Some((_, reglages, _)), Some(cibles)) = (p.voiles.as_ref(), p.cibles.as_ref().and_then(|c| c.voile.as_ref())) else { return };
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, reglages, &[(rang as u64 * PAS_VOILE) as u32]);
        pass.set_bind_group(1, cibles, &[]);
        pass.draw(0..6, 0..1);
    }

    /// Les pipelines et la lumière des ombres des cartes de la vue (partie 189), faits la première fois qu'une vue en a besoin.
    fn assurer_ombres_vue(&mut self, device: &wgpu::Device) {
        if self.vue_ombres.is_some() {
            return;
        }
        let commun = format!("{}{}", crate::gpu::LUMIERE, crate::gpu::OMBRES_COMMUN);
        let shader_decor = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D : ombres des cartes"), source: wgpu::ShaderSource::Wgsl(format!("{SHADER}{commun}{AVEC_CARTES}").into()) });
        let shader_porteuses = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D : carte d'ombres des cartes"), source: wgpu::ShaderSource::Wgsl(format!("{}{PORTEUSES}", crate::gpu::LUMIERE).into()) });
        let recue_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D : ombres reçues par les cartes"), source: wgpu::ShaderSource::Wgsl(format!("{commun}{RECUE}").into()) });
        let lumiere_entree = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: NonZeroU64::new(crate::ombres::OCTETS) },
            count: None,
        };
        let profondeur = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let layout_ombres = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D : ombres des cartes de la vue"),
            entries: &[
                lumiere_entree(0),
                profondeur(1),
                profondeur(2),
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
            ],
        });
        let layout_grossiere = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D : carte grossière des cartes"),
            entries: &[profondeur(0), wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering), count: None }],
        });
        let echant_proche = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("décor 3D : carte grossière des cartes"), ..Default::default() });
        let shader_grossiere = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("décor 3D : carte grossière des cartes"), source: wgpu::ShaderSource::Wgsl(GROSSIERE.into()) });
        let layout_g = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("décor 3D : carte grossière des cartes"), bind_group_layouts: &[&layout_grossiere], push_constant_ranges: &[] });
        let grossiere_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("décor 3D : carte grossière des cartes"),
            layout: Some(&layout_g),
            vertex: wgpu::VertexState { module: &shader_grossiere, entry_point: Some("vs_g"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader_grossiere, entry_point: Some("fs_g"), targets: &[Some(wgpu::ColorTargetState { format: GROSSIERE_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let grossiere_vide = texture_grossiere(device, 1);
        let layout_lumiere = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("décor 3D : lumière des cartes"), entries: &[lumiere_entree(0)] });
        let layout_carte = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D : dessin d'une carte"),
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
        let layout_recue = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("décor 3D : ombres reçues, réglages"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: NonZeroU64::new(RECUE_OCTETS) },
                count: None,
            }],
        });
        // Le décor de la vue, comme `main4`, avec le groupe des ombres des cartes.
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("décor 3D : ombres des cartes"), bind_group_layouts: &[&self.layout_global, &self.layout_objet, &self.layout_texture, &layout_ombres], push_constant_ranges: &[] });
        const ATTR: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let main4c = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("décor 3D : vue sous les ombres des cartes"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader_decor, entry_point: Some("vs_main"), buffers: &[wgpu::VertexBufferLayout { array_stride: 32, step_mode: wgpu::VertexStepMode::Vertex, attributes: &ATTR }], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader_decor, entry_point: Some("fs_main"), targets: &[Some(wgpu::ColorTargetState { format: COULEUR, blend: None, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: PROFONDEUR, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: Default::default() }),
            multisample: wgpu::MultisampleState { count: ECHANTILLONS_VUE, mask: !0, alpha_to_coverage_enabled: false },
            multiview: None,
            cache: None,
        });
        let layout_p = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("décor 3D : carte d'ombres des cartes"), bind_group_layouts: &[&layout_lumiere, &layout_carte], push_constant_ranges: &[] });
        const ATTR_P: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32];
        let porteuses = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("décor 3D : carte d'ombres des cartes"),
            layout: Some(&layout_p),
            vertex: wgpu::VertexState { module: &shader_porteuses, entry_point: Some("vs_porteuse"), buffers: &[wgpu::VertexBufferLayout { array_stride: SOMMET_PORTEUSE, step_mode: wgpu::VertexStepMode::Vertex, attributes: &ATTR_P }], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader_porteuses, entry_point: Some("fs_porteuse"), targets: &[], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: PROFONDEUR, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: Default::default() }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let lumiere = device.create_buffer(&wgpu::BufferDescriptor { label: Some("décor 3D : lumière des cartes"), size: crate::ombres::OCTETS, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let groupe_lumiere = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("décor 3D : lumière des cartes"), layout: &layout_lumiere, entries: &[wgpu::BindGroupEntry { binding: 0, resource: lumiere.as_entire_binding() }] });
        // (Le dessin d'une carte lu comme egui le lit : bilinéaire, bords répétés.)
        let echant_carte = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("décor 3D : dessin d'une carte"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        self.vue_ombres = Some(Box::new(OmbresVue { main4c, porteuses, recue_shader, recue: None, layout_ombres, layout_recue, layout_carte, lumiere, groupe_lumiere, echant_carte, carte: None, grossiere: None, grossiere_pipe, layout_grossiere, grossiere_vide, echant_proche, cle: None, sommets: None }));
    }

    /// Les ombres des cartes de la vue `cle` à cette image (partie 189), sous la lumière `lum` de sa passe : la carte d'ombres des
    /// cartes (refaite seulement si une carte, son dessin ou le soleil a bougé depuis la dernière, toutes vues comprises), les
    /// réglages de la lumière, et le groupe que la vue en lit. Rend ce groupe si des cartes portent une ombre (le décor les lit).
    #[allow(clippy::too_many_arguments)]
    fn preparer_ombres_vue(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, cle: u64, n: u64, cv: &CartesVue, lum: crate::ombres::Lumiere) -> Option<wgpu::BindGroup> {
        self.assurer_ombres_vue(device);
        let cote = lum.cote.max(1);
        let boite = if cv.porteuses.is_empty() { None } else { Some(crate::ombres::boite_de(&lum, cv.porteuses.iter().flat_map(|p| p.coins)).unwrap_or([0.0, 0.0, 1.0, 1.0])) };
        let cle_carte = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (cv.cle, cote, lum.light_vp.map(f32::to_bits)).hash(&mut h);
            #[cfg(test)]
            ESSAI_SANS_GROSSIERE.with(|e| e.get()).hash(&mut h);
            h.finish()
        };
        let ov = self.vue_ombres.as_mut().expect("ombres de la vue");
        if boite.is_some() && (ov.cle != Some(cle_carte) || ov.carte.as_ref().is_none_or(|(c, _)| *c != cote)) {
            if ov.carte.as_ref().is_none_or(|(c, _)| *c != cote) {
                let tex = texture_profondeur_t(device, [cote, cote], 1, true);
                let vue = tex.create_view(&Default::default());
                let g = texture_grossiere(device, cote.div_ceil(CASE_GROSSIERE));
                let groupe = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("décor 3D : carte grossière des cartes"), layout: &ov.layout_grossiere, entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&vue) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&ov.echant_proche) }] });
                ov.carte = Some((cote, vue));
                ov.grossiere = Some((g, groupe));
            }
            // Les sommets des cartes : six par carte (deux triangles), leurs coins dans le monde.
            let mut data: Vec<f32> = Vec::with_capacity(cv.porteuses.len() * 36);
            for p in &cv.porteuses {
                let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                for k in [0usize, 1, 2, 0, 2, 3] {
                    data.extend_from_slice(&p.coins[k]);
                    data.extend_from_slice(&uv[k]);
                    data.push(p.opacite);
                }
            }
            let octets = (data.len() as u64 * 4).max(SOMMET_PORTEUSE);
            if ov.sommets.as_ref().is_none_or(|(_, c)| *c < octets) {
                let cap = octets.next_power_of_two();
                ov.sommets = Some((device.create_buffer(&wgpu::BufferDescriptor { label: Some("décor 3D : cartes vues du soleil"), size: cap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }), cap));
            }
            let (sommets, _) = ov.sommets.as_ref().expect("sommets");
            queue.write_buffer(sommets, 0, bytemuck::cast_slice(&data));
            let groupes: Vec<wgpu::BindGroup> = cv.porteuses.iter().map(|p| groupe_texture(device, &ov.layout_carte, &p.texture.create_view(&Default::default()), &ov.echant_carte)).collect();
            let (_, vue) = ov.carte.as_ref().expect("carte d'ombres des cartes");
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("décor 3D : carte d'ombres des cartes"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: vue, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&ov.porteuses);
            pass.set_bind_group(0, &ov.groupe_lumiere, &[]);
            pass.set_vertex_buffer(0, sommets.slice(..));
            for (k, g) in groupes.iter().enumerate() {
                pass.set_bind_group(1, g, &[]);
                pass.draw(k as u32 * 6..k as u32 * 6 + 6, 0..1);
            }
            drop(pass);
            // Puis la carte grossière, d'après elle.
            let (gv, gg) = ov.grossiere.as_ref().expect("carte grossière");
            #[cfg(not(test))]
            let sans = false;
            #[cfg(test)]
            let sans = ESSAI_SANS_GROSSIERE.with(|e| e.get());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("décor 3D : carte grossière des cartes"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: gv, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(if sans { wgpu::Color::BLACK } else { wgpu::Color::WHITE }), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // (Seulement sur la boîte des cartes : le décor ne la lit pas ailleurs.)
            let nc = cote.div_ceil(CASE_GROSSIERE);
            let b = boite.expect("boîte des cartes").map(|v| v.clamp(0.0, 1.0) * nc as f32);
            let (x0, y0) = ((b[0].floor() as u32).min(nc - 1), (b[1].floor() as u32).min(nc - 1));
            let (x1, y1) = ((b[2].ceil() as u32).clamp(x0 + 1, nc), (b[3].ceil() as u32).clamp(y0 + 1, nc));
            pass.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
            pass.set_pipeline(&ov.grossiere_pipe);
            pass.set_bind_group(0, gg, &[]);
            if !sans {
                pass.draw(0..3, 0..1);
            }
            drop(pass);
            ov.cle = Some(cle_carte);
            self.cartes_d_ombres_faites += 1;
        }
        let ov = self.vue_ombres.as_ref().expect("ombres de la vue");
        // (Le même de toutes les vues : la carte d'ombres et sa boîte ne dépendent pas du point de vue.)
        queue.write_buffer(&ov.lumiere, 0, &lum.octets([1.0, 1.0], boite, true));
        let (cartes, grossiere) = match (boite, ov.carte.as_ref(), ov.grossiere.as_ref()) {
            (Some(_), Some((_, v)), Some((g, _))) => (v.clone(), g.clone()),
            _ => (self.ombre_vide.clone(), ov.grossiere_vide.clone()),
        };
        let (layout, lumiere) = (ov.layout_ombres.clone(), ov.lumiere.clone());
        let echant = self.echant_ombre.clone();
        let p = self.prise(device, cle, n);
        let decor = p.ombre.as_ref().map(|(_, v)| v.clone())?;
        let groupe = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("décor 3D : ombres des cartes de la vue"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: lumiere.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&decor) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&cartes) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&echant) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&grossiere) },
            ],
        });
        p.ombres_vue = Some((lum, groupe.clone()));
        boite.map(|_| groupe)
    }

    /// Les réglages de l'ombre reçue par la carte `rang` de la vue `cle` (partie 189), d'après la dernière passe du décor de cette
    /// vue. Faux : la vue n'a pas d'ombres de cartes à cette image (rien à poser).
    fn preparer_recue(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, r: &RecueCarte) -> bool {
        let format = r.cible_egui;
        let Some(ov) = self.vue_ombres.as_mut() else { return false };
        if ov.recue.as_ref().is_none_or(|(f, _)| *f != format) {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("décor 3D : ombres reçues"), bind_group_layouts: &[&ov.layout_ombres, &ov.layout_recue, &ov.layout_carte], push_constant_ranges: &[] });
            const ATTR: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x3, 3 => Float32];
            // (L'écran moins ce que l'ombre retire de la carte ; l'alpha de l'écran ne change pas.)
            let blend = wgpu::BlendState {
                color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::ReverseSubtract },
                alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
            };
            let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("décor 3D : ombres reçues par les cartes"),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &ov.recue_shader, entry_point: Some("vs_recue"), buffers: &[wgpu::VertexBufferLayout { array_stride: SOMMET_RECUE, step_mode: wgpu::VertexStepMode::Vertex, attributes: &ATTR }], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: &ov.recue_shader, entry_point: Some("fs_recue"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
            ov.recue = Some((format, pipe));
        }
        let (layout_recue, layout_carte, echant) = (ov.layout_recue.clone(), ov.layout_carte.clone(), ov.echant_carte.clone());
        let Some(p) = self.prises.get_mut(&r.cle) else { return false };
        let Some((lum, _)) = p.ombres_vue.as_ref() else { return false };
        let n = r.indices.len().min(INDICES_RECUE as usize);
        if r.sommets.len() as u64 > SOMMETS_RECUE * SOMMET_RECUE / 4 || n == 0 {
            return false;
        }
        let g = lum.gradient_de(r.normale);
        let besoin = (r.rang.max(r.total.saturating_sub(1)) as u64 + 1).max(1);
        if p.recues.as_ref().is_none_or(|v| v.capacite < besoin) {
            let cap = besoin.next_power_of_two().max(4);
            let tampon = |label: &str, size: u64, usage: wgpu::BufferUsages| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
            let reglages = tampon("décor 3D : ombres reçues, réglages", PAS_RECUE * cap, wgpu::BufferUsages::UNIFORM);
            let groupe = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("décor 3D : ombres reçues, réglages"),
                layout: &layout_recue,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &reglages, offset: 0, size: NonZeroU64::new(RECUE_OCTETS) }) }],
            });
            let sommets = tampon("décor 3D : ombres reçues, sommets", SOMMETS_RECUE * SOMMET_RECUE * cap, wgpu::BufferUsages::VERTEX);
            let indices = tampon("décor 3D : ombres reçues, indices", INDICES_RECUE * 4 * cap, wgpu::BufferUsages::INDEX);
            p.recues = Some(Recues { reglages, groupe, sommets, indices, capacite: cap, cartes: Vec::new() });
        }
        let v = p.recues.as_mut().expect("ombres reçues");
        let rang = r.rang as u64;
        queue.write_buffer(&v.reglages, rang * PAS_RECUE, bytemuck::cast_slice(&[g[0], g[1], r.opacite, if format.is_srgb() { 1.0f32 } else { 0.0 }]));
        queue.write_buffer(&v.sommets, rang * SOMMETS_RECUE * SOMMET_RECUE, bytemuck::cast_slice(&r.sommets));
        queue.write_buffer(&v.indices, rang * INDICES_RECUE * 4, bytemuck::cast_slice(&r.indices[..n]));
        if v.cartes.len() <= r.rang as usize {
            v.cartes.resize(r.rang as usize + 1, None);
        }
        v.cartes[r.rang as usize] = Some((groupe_texture(device, &layout_carte, &r.texture.create_view(&Default::default()), &echant), n as u32));
        true
    }

    /// Pose l'ombre reçue par la carte `rang` de la vue `cle` dans la passe d'egui.
    fn poser_recue(&self, pass: &mut wgpu::RenderPass<'static>, cle: u64, rang: u32) {
        let Some((_, pipe)) = self.vue_ombres.as_ref().and_then(|o| o.recue.as_ref()) else { return };
        let Some(p) = self.prises.get(&cle) else { return };
        let (Some((_, groupe)), Some(v)) = (p.ombres_vue.as_ref(), p.recues.as_ref()) else { return };
        let Some(Some((dessin, n))) = v.cartes.get(rang as usize) else { return };
        let r = rang as u64;
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, groupe, &[]);
        pass.set_bind_group(1, &v.groupe, &[(r * PAS_RECUE) as u32]);
        pass.set_bind_group(2, dessin, &[]);
        pass.set_vertex_buffer(0, v.sommets.slice(r * SOMMETS_RECUE * SOMMET_RECUE..(r + 1) * SOMMETS_RECUE * SOMMET_RECUE));
        pass.set_index_buffer(v.indices.slice(r * INDICES_RECUE * 4..(r + 1) * INDICES_RECUE * 4), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..*n, 0, 0..1);
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
    texture_profondeur_t(device, taille, echantillons, lue).create_view(&Default::default())
}

fn texture_profondeur_t(device: &wgpu::Device, taille: [u32; 2], echantillons: u32, lue: bool) -> wgpu::Texture {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("décor 3D profondeur"),
            size: wgpu::Extent3d { width: taille[0].max(1), height: taille[1].max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: echantillons,
            dimension: wgpu::TextureDimension::D2,
            format: PROFONDEUR,
            // (Lue : la carte d'ombres et la profondeur de la vue ; la carte d'ombres se relit aussi pour l'export, partie 185.)
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | if lue { wgpu::TextureUsages::TEXTURE_BINDING | if echantillons == 1 { wgpu::TextureUsages::COPY_SRC } else { wgpu::TextureUsages::empty() } } else { wgpu::TextureUsages::empty() },
            view_formats: &[],
        })
}

#[cfg(test)]
thread_local! {
    /// Essais (partie 189) : la carte grossière à zéro (le décor fait toujours son filtre entier), pour vérifier qu'elle ne change
    /// rien à l'image.
    pub(crate) static ESSAI_SANS_GROSSIERE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Le format de la carte grossière des cartes de la vue (partie 189, `GROSSIERE`) : une profondeur par case.
const GROSSIERE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// La carte grossière d'une carte d'ombres des cartes : `cote` cases de côté, vide (1 : rien près du soleil).
fn texture_grossiere(device: &wgpu::Device, cote: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("décor 3D : carte grossière des cartes"),
            size: wgpu::Extent3d { width: cote.max(1), height: cote.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: GROSSIERE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
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
    /// Les cartes qui portent une ombre et celles qui la reçoivent (partie 189, `ombres3d.rs`), quand le décor a ses ombres ; rien :
    /// le décor de la vue comme avant, au même coût.
    pub cartes: Option<Arc<CartesVue>>,
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
        mt.dessiner_vue(device, queue, encoder, self.cle, &self.dessins, &self.vue, self.taille_pts, px, self.qualite, self.cartes.as_deref());
        mt.menage();
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        if let Some(mt) = resources.get::<Moteur>() {
            mt.poser_vue(pass, self.cle);
        }
    }
}

/// Le voile d'une carte de la vue Espace 3D (partie 183, `profondeur.rs`) : posé juste après le dessin de la carte, il y remet le
/// décor de la vue `cle` là où le décor est plus près que la carte. Les cartes et les objets du décor se cachent ainsi selon leur
/// distance dans la vue comme dans l'image du plan.
pub struct VoileCarte {
    pub cle: u64,
    /// Le rang de la carte parmi celles de la vue à cette image (chacune son pas dans le tampon des réglages).
    pub rang: u32,
    /// Ses quatre coins dans le monde.
    pub coins: [V3; 4],
    pub cible_egui: wgpu::TextureFormat,
}

impl CallbackTrait for VoileCarte {
    fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue, _screen: &egui_wgpu::ScreenDescriptor, _encoder: &mut wgpu::CommandEncoder, resources: &mut CallbackResources) -> Vec<wgpu::CommandBuffer> {
        if let Some(mt) = resources.get_mut::<Moteur>() {
            mt.assurer_voile(device, self.cible_egui);
            mt.preparer_voile(device, queue, self.cle, self.rang, &self.coins, self.cible_egui.is_srgb());
        }
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        if let Some(mt) = resources.get::<Moteur>() {
            mt.poser_voile(pass, self.cle, self.rang);
        }
    }
}

/// Une carte de la vue Espace 3D qui porte une ombre (partie 189) : ses quatre coins dans le monde (haut gauche, haut droit, bas droit,
/// bas gauche : `Monde::card_corners`), son dessin tel que la vue le montre (la texture d'egui) et l'opacité de son calque.
#[derive(Clone)]
pub struct PorteuseVue {
    pub coins: [V3; 4],
    pub texture: wgpu::Texture,
    pub opacite: f32,
}

/// Les cartes de la vue Espace 3D qui portent une ombre (partie 189, `ombres3d.rs`), et l'empreinte de ce qui fait leur ombre (leurs
/// coins, leur dessin, leur opacité) : la carte d'ombres des cartes n'est refaite que si elle change.
#[derive(Clone)]
pub struct CartesVue {
    pub porteuses: Vec<PorteuseVue>,
    pub cle: u64,
}

/// L'ombre reçue par une carte de la vue Espace 3D (partie 189, `ombres3d.rs`) : posée juste après le dessin de la carte (et avant
/// son voile, partie 183), elle retire de la carte le soleil que le décor et les autres cartes lui cachent, comme dans l'image du
/// plan. Sur le maillage même qu'egui vient de dessiner.
pub struct RecueCarte {
    pub cle: u64,
    /// Le rang de la carte parmi celles qui reçoivent les ombres dans cette vue à cette image, et leur nombre (la place à prévoir).
    pub rang: u32,
    pub total: u32,
    /// Les sommets du maillage d'egui (`espace3dview::card_mesh`) : place dans la zone de la vue (de −1 à 1), coordonnée du dessin,
    /// point du monde, w de la vue (1 en orthographique) ; et ses triangles.
    pub sommets: Vec<f32>,
    pub indices: Vec<u32>,
    pub texture: wgpu::Texture,
    pub opacite: f32,
    /// La normale de la carte (unitaire, dans le monde).
    pub normale: V3,
    pub cible_egui: wgpu::TextureFormat,
}

impl CallbackTrait for RecueCarte {
    fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue, _screen: &egui_wgpu::ScreenDescriptor, _encoder: &mut wgpu::CommandEncoder, resources: &mut CallbackResources) -> Vec<wgpu::CommandBuffer> {
        if let Some(mt) = resources.get_mut::<Moteur>() {
            if !mt.preparer_recue(device, queue, self) {
                if let Some(v) = mt.prises.get_mut(&self.cle).and_then(|p| p.recues.as_mut()) {
                    if let Some(c) = v.cartes.get_mut(self.rang as usize) {
                        *c = None;
                    }
                }
            }
        }
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        if let Some(mt) = resources.get::<Moteur>() {
            mt.poser_recue(pass, self.cle, self.rang);
        }
    }
}

// ------------------------------------------------------------------------------------------- rendu hors écran, mesures

/// Rend `dessins` vus par `prise` à `taille` pixels, hors de l'écran, et relit l'image (RGBA prémultiplié, sRGB) : les exports
/// avec une carte graphique (la même image qu'à l'écran) et les essais. `fois` : nombre d'images à rendre (mesures) ; renvoie
/// aussi la durée d'une image, en millisecondes (si `fois` > 1 ; première image, celle de l'envoi des maillages, exclue).
#[allow(clippy::too_many_arguments)]
pub fn rendre_hors_ecran(device: &wgpu::Device, queue: &wgpu::Queue, mt: &mut Moteur, dessins: &[Dessin], prise: &Prise, taille: [u32; 2], ciel: bool, qualite: crate::decor3dbudget::Qualite, fois: usize) -> Option<(Vec<u8>, f32)> {
    // (Avec la part du soleil quand le décor a ses ombres, comme à l'écran, partie 185 : l'étalonnage de la machine mesure la passe
    // telle qu'elle est dessinée.)
    rendre_hors_ecran_tout(device, queue, mt, dessins, prise, taille, ciel, qualite, fois, false, true, false).map(|(c, t, _, _)| (c, t))
}

/// Comme `rendre_hors_ecran`, et, si `avec_prof`, relit aussi la profondeur que les cartes partagent avec le décor (partie 183 :
/// l'inverse de la distance à l'œil à chaque pixel, 0 là où rien n'est dessiné), pour la composition des exports.
#[allow(clippy::too_many_arguments)]
pub fn rendre_hors_ecran_prof(device: &wgpu::Device, queue: &wgpu::Queue, mt: &mut Moteur, dessins: &[Dessin], prise: &Prise, taille: [u32; 2], ciel: bool, qualite: crate::decor3dbudget::Qualite, fois: usize, avec_prof: bool) -> Option<(Vec<u8>, f32, Option<crate::profondeur::ProfDecor>)> {
    rendre_hors_ecran_tout(device, queue, mt, dessins, prise, taille, ciel, qualite, fois, avec_prof, false, true).map(|(c, t, p, _)| (c, t, p))
}

/// Ce que l'export garde du soleil d'un rendu du décor, pour les ombres des cartes (partie 185, `ombres.rs`) : la part du soleil
/// et le mélange de l'hémisphère à chaque pixel de la feuille (`SOLEIL`, deux octets par pixel), la carte d'ombres du décor (le
/// côté², profondeurs de 0 à 1) et l'éclairage de la passe.
#[derive(Clone, Debug)]
pub struct SortieOmbres {
    pub soleil: Vec<u8>,
    pub carte: Vec<f32>,
    pub cote: u32,
    pub eclairage: Eclairage,
}

/// Comme `rendre_hors_ecran_prof`, et, si `avec_ombres` et que le décor a ses ombres, dessine aussi la part du soleil (la troisième
/// cible de l'écran) et, avec `avec_prof`, relit ce qu'il faut aux ombres des cartes (`SortieOmbres`).
#[allow(clippy::too_many_arguments)]
///
/// `quatre` (partie 193) : la profondeur en quatre échantillons (les bords doux), sinon en une valeur (le coût d'avant).
pub fn rendre_hors_ecran_tout(device: &wgpu::Device, queue: &wgpu::Queue, mt: &mut Moteur, dessins: &[Dessin], prise: &Prise, taille: [u32; 2], ciel: bool, qualite: crate::decor3dbudget::Qualite, fois: usize, avec_prof: bool, avec_ombres: bool, quatre: bool) -> Option<(Vec<u8>, f32, Option<crate::profondeur::ProfDecor>, Option<SortieOmbres>)> {
    let max = device.limits().max_texture_dimension_2d;
    if taille[0] > max || taille[1] > max || taille[0] == 0 || taille[1] == 0 {
        return None;
    }
    let cible = |format: wgpu::TextureFormat, label: &str| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: taille[0], height: taille[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    };
    let tex = cible(COULEUR, "décor 3D hors écran");
    let tex_prof = cible(PROF1, "décor 3D hors écran : profondeur");
    let tex_prof4 = quatre.then(|| cible(PROF, "décor 3D hors écran : profondeur en quatre échantillons"));
    let view_prof4 = tex_prof4.as_ref().map(|t| t.create_view(&Default::default()));
    let view = tex.create_view(&Default::default());
    let view_prof = tex_prof.create_view(&Default::default());
    // (La part du soleil, partie 185 : seulement si le décor a ses ombres et qu'on la veut.)
    let aspect = dessins.first().map(|d| d.aspect).unwrap_or_default();
    let ec = eclairage(dessins, prise.oeil, &aspect, qualite);
    let tex_soleil = (avec_ombres && ec.ombres).then(|| cible(SOLEIL, "décor 3D hors écran : soleil"));
    let view_soleil = tex_soleil.as_ref().map(|t| t.create_view(&Default::default()));
    // Une première image : les maillages et les textures partent à la carte (une fois), rien n'est compté.
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("décor 3D hors écran") });
    mt.dessiner_feuille(device, queue, &mut enc, u64::MAX - 7, dessins, prise, taille, &view, &view_prof, view_prof4.as_ref(), view_soleil.as_ref(), ciel, qualite);
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
                mt.dessiner_feuille(device, queue, &mut enc, u64::MAX - 7, dessins, prise, taille, &view, &view_prof, view_prof4.as_ref(), view_soleil.as_ref(), ciel, qualite);
            }
            queue.submit([enc.finish()]);
            let _ = device.poll(wgpu::Maintain::Wait);
            meilleur = meilleur.min(debut.elapsed().as_secs_f32() * 1000.0 / fois as f32);
        }
    }
    let total = if meilleur.is_finite() { meilleur } else { 0.0 };
    let out = relire(device, queue, &tex, taille, 4)?;
    let (w, h) = (taille[0] as usize, taille[1] as usize);
    let prof = if avec_prof {
        let un: Vec<u16> = relire(device, queue, &tex_prof, taille, 2)?.chunks_exact(2).map(|b| u16::from_ne_bytes([b[0], b[1]])).collect();
        let quatre = match &tex_prof4 {
            Some(t) => Some(relire(device, queue, t, taille, 8)?.chunks_exact(8).map(|b| [0, 2, 4, 6].map(|k| u16::from_ne_bytes([b[k], b[k + 1]]))).collect()),
            None => None,
        };
        Some(crate::profondeur::ProfDecor::avec(w, h, un, quatre, crate::profondeur::MOTIF_CARTE))
    } else {
        None
    };
    let ombres = match (&tex_soleil, mt.prises.get(&(u64::MAX - 7)).filter(|p| p.ombre_faite).and_then(|p| p.ombre_tex.as_ref())) {
        (Some(ts), Some(carte)) if avec_prof => {
            let soleil = relire(device, queue, ts, taille, 2)?;
            let cote = ec.cote;
            let carte: Vec<f32> = relire(device, queue, carte, [cote, cote], 4)?.chunks_exact(4).map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]])).collect();
            Some(SortieOmbres { soleil, carte, cote, eclairage: ec })
        }
        _ => None,
    };
    Some((out, total, prof, ombres))
}

/// Les pixels d'une texture de `octets` octets par pixel (`COULEUR` : 4, `PROF` : 8, `PROF1` : 2), ligne par ligne.
fn relire(device: &wgpu::Device, queue: &wgpu::Queue, tex: &wgpu::Texture, taille: [u32; 2], octets: u32) -> Option<Vec<u8>> {
    let ligne = (taille[0] * octets).div_ceil(256) * 256;
    let lecture = device.create_buffer(&wgpu::BufferDescriptor { label: Some("décor 3D relu"), size: ligne as u64 * taille[1] as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("décor 3D relu") });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: if tex.format().is_depth_stencil_format() { wgpu::TextureAspect::DepthOnly } else { wgpu::TextureAspect::All } },
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
    let mut out = Vec::with_capacity((taille[0] * taille[1] * octets) as usize);
    for y in 0..taille[1] as usize {
        out.extend_from_slice(&data[y * ligne as usize..y * ligne as usize + (taille[0] * octets) as usize]);
    }
    drop(data);
    lecture.unmap();
    Some(out)
}
