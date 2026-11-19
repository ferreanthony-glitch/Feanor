//! Superposition des calques par la carte graphique (wgpu).
//!
//! Deux temps, comme le calcul sur processeur qu'il remplace à l'écran :
//! 1. **composer** : le papier puis les calques (opacité, teinte de la pelure d'oignon) sont
//!    empilés dans une image à la taille du projet, pixel pour pixel ;
//! 2. **afficher** : cette image est posée à l'écran avec la vue (zoom, rotation, miroir). Réduite,
//!    chaque pixel d'écran est la moyenne de tout ce qu'il couvre de l'image (`fs_display`) ; à
//!    100 % et au-delà, ses pixels tels quels (ou lissés : `GpuCanvas::lisser`, `pose_nette`).
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

use eframe::egui::{self, pos2, Pos2, Rect};
use eframe::egui_wgpu::{self, wgpu, CallbackResources, CallbackTrait, RenderState};

use crate::document::{CelId, Document};
use crate::raster::{tile_bounds, Raster, TILE};
use crate::render::Plan;

/// Réglages d'un dessin dans le tampon : 4 coins (position, coordonnées de texture), une couleur,
/// des paramètres = 6 vec4 = 96 octets.
// (Partie 177 : un vec4 de plus, `extra`, pour l'iris et le volet en horloge.)
/// (Coins, couleur, réglages, réglage de plus, et le plan de la carte pour la profondeur partagée avec le décor 3D, partie 183.)
const ITEM_BYTES: u64 = 128;
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
    extra: vec4<f32>,
    // La profondeur partagée avec le décor 3D (partie 183, `profondeur.rs`) : le plan de la carte, l'inverse de sa distance à l'œil
    // au pixel (x, y) de la feuille étant prof.x · x + prof.y · y + prof.z. Lu seulement par `fs_compose_p` et `fs_blend_p`.
    prof: vec4<f32>,
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
    return fusionner(v, item.params.x);
}

// La profondeur du décor 3D posé dessous (partie 183) : l'inverse de la distance à l'œil de ce qu'il montre à chaque pixel de la
// feuille, 0 là où il n'y a rien (le ciel ne cache rien).
@group(3) @binding(0) var decor_prof: texture_2d<f32>;

// Les tolérances de `profondeur.rs` (`TOLERANCE_RELATIVE`, `TOLERANCE_METRES`) : le décor ne cache une carte que s'il est plus
// près qu'elle d'au moins cela.
const TOLERANCE_RELATIVE: f32 = 0.004;
const TOLERANCE_METRES: f32 = 0.01;

// Les drapeaux d'une carte sur un décor (prof.w) : 1, le décor la cache selon sa distance (partie 183) ; 2, elle reçoit les ombres
// (partie 185).
fn drapeau(b: u32) -> bool {
    return (u32(item.prof.w + 0.5) & b) != 0u;
}

// Le pixel de la carte se voit-il (1) ou le décor, plus près, le cache-t-il (0) ? Comme `profondeur::cache`.
fn visible(pos: vec4<f32>) -> f32 {
    if (!drapeau(1u)) {
        return 1.0;
    }
    let taille = vec2<i32>(textureDimensions(decor_prof));
    let p = vec2<i32>(floor(pos.xy));
    if (p.x < 0 || p.y < 0 || p.x >= taille.x || p.y >= taille.y) {
        return 1.0;
    }
    let decor = textureLoad(decor_prof, p, 0).r;
    if (decor <= 0.0) {
        return 1.0;
    }
    let carte = item.prof.x * pos.x + item.prof.y * pos.y + item.prof.z;
    if (carte <= 0.0) {
        return 1.0;
    }
    // (1 / décor < (1 − r) / carte − m, multiplié par décor · carte, tous deux positifs : sans division.)
    if (carte * (1.0 + TOLERANCE_METRES * decor) < decor * (1.0 - TOLERANCE_RELATIVE)) {
        return 0.0;
    }
    return 1.0;
}

// Une carte qui partage la profondeur du décor (partie 183) : composée comme `fs_compose` ou `fs_blend`, cachée là où le décor
// est plus près (le décor, dessous, se voit alors).
// (La profondeur n'est lue que là où la carte a de l'encre : une carte est surtout transparente.)
// (Partie 185 : et, si elle reçoit les ombres, assombrie là où le décor ou une autre carte lui cache le soleil, `ombres.rs`.)
@fragment
fn fs_compose_p(v: VOut) -> @location(0) vec4<f32> {
    let c = composer(v);
    if (c.a <= 0.0) {
        return c;
    }
    let o = c * visible(v.pos);
    if (drapeau(2u) && o.a > 0.0) {
        return assombrir_carte(o, ombre_recue(v.pos.xy));
    }
    return o;
}

@fragment
fn fs_blend_p(v: VOut) -> @location(0) vec4<f32> {
    let encre = textureSample(tex, samp, v.uv).a;
    var vis = 1.0;
    var teinte = vec3<f32>(1.0);
    if (encre > 0.0) {
        vis = visible(v.pos);
        if (drapeau(2u) && vis > 0.0) {
            teinte = vec3<f32>(1.0) - ombre_recue(v.pos.xy) * (vec3<f32>(1.0) - eclair.carte.rgb);
        }
    }
    return fusionner_t(v, item.params.x * vis, teinte);
}

fn fusionner(v: VOut, opacite: f32) -> vec4<f32> {
    return fusionner_t(v, opacite, vec3<f32>(1.0));
}

// `teinte` : ce qui reste de la couleur de la carte, en lumière linéaire, à l'ombre (partie 185 ; 1 : rien).
fn fusionner_t(v: VOut, opacite: f32, teinte: vec3<f32>) -> vec4<f32> {
    let raw = textureSample(tex, samp, v.uv);
    let b = textureLoad(back_tex, vec2<i32>(floor(v.pos.xy)), 0);
    let sa = raw.a * opacite;
    if (sa <= 0.0) {
        return b;
    }
    var cs = min(raw.rgb / raw.a, vec3<f32>(1.0));
    if (any(teinte < vec3<f32>(1.0))) {
        cs = vers_srgb(vers_lin(cs) * teinte);
    }
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
    return composer(v);
}

fn composer(v: VOut) -> vec4<f32> {
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

// Le plus de lectures par axe et par pixel d'écran, en réduisant.
const LECTURES_MAX: i32 = 12;

// En réduisant (la vue en dessous de 100 %, une vignette), un pixel d'écran couvre plusieurs pixels de
// l'image : **tous doivent compter**, sinon un trait fin se casse en pointillés et son épaisseur change
// d'une ligne à l'autre (partie 166.9). L'image est alors lue lissée (l'échantillonneur bilinéaire,
// choisi par `pose_nette` : une lecture pèse deux pixels de l'image par axe) :
// - vue droite : la moyenne exacte de ce que couvre le pixel d'écran. Chaque lecture, posée entre deux
//   pixels de l'image, plus près de celui qui compte le plus, les pèse tous les deux comme il faut :
//   (e + 1) / 2 lectures par axe pour e pixels couverts ;
// - vue tournée ou en perspective : n × n lectures réparties sur ce que couvre le pixel, n = e.
// Sans réduction (100 % et au-delà) : une seule lecture, comme toujours.
// (Les lectures se font au niveau 0 demandé, `textureSampleLevel` : la carte y applique le filtre du
// **grossissement** de l'échantillonneur. Avec celui de l'affichage, qui ne lisse pas en grossissant,
// chaque lecture ne prenait qu'un point : à 37 %, 4 points lus sur plus de 7 couverts.)
// Transitions (`transitions.rs`) : params.x est l'opacité de l'image posée ; params.y le mode
// (1 : image, 0 : aplat de gris params.z, par exemple le noir d'un fondu au noir, 2 : volet, l'image
// ne se voit qu'à gauche de la colonne params.z de la feuille). Une image seule : x = 1, y = 1.
// Partie 175 : 3 : aplat de la couleur params.z (rouge × 65536 + vert × 256 + bleu, valeurs gamma) ; 4 : image floue le long
// de l'horizontale de l'écran, sur params.z pixels de part et d'autre (la traînée d'un raccord filé).
// Partie 177 : 5 : aplat de la couleur params.z hors du rond de centre extra.xy et de rayon extra.z (en pixels de l'écran, bord
// lissé sur un pixel : l'iris) ; 6 : image vue seulement dans l'angle extra.z (radians) balayé depuis midi, dans le sens des
// aiguilles, autour du point extra.xy de l'écran (le volet en horloge).
@fragment
fn fs_display(v: VOut) -> @location(0) vec4<f32> {
    if (item.params.y < 0.5) {
        return vec4<f32>(vec3<f32>(item.params.z), 1.0) * item.params.x;
    }
    if (item.params.y > 2.5 && item.params.y < 3.5) {
        let c = u32(item.params.z);
        let rgb = vec3<f32>(f32((c >> 16u) & 255u), f32((c >> 8u) & 255u), f32(c & 255u)) / 255.0;
        return vec4<f32>(rgb, 1.0) * item.params.x;
    }
    if (item.params.y > 4.5 && item.params.y < 5.5) {
        let c = u32(item.params.z);
        let rgb = vec3<f32>(f32((c >> 16u) & 255u), f32((c >> 8u) & 255u), f32(c & 255u)) / 255.0;
        let dehors = clamp(distance(v.pos.xy, item.extra.xy) - item.extra.z + 0.5, 0.0, 1.0);
        return vec4<f32>(rgb, 1.0) * item.params.x * dehors;
    }
    let dx = dpdx(v.uv);
    let dy = dpdy(v.uv);
    if (item.params.y > 3.5 && item.params.y < 4.5) {
        var acc = vec4<f32>(0.0);
        for (var k = 0; k < 32; k++) {
            let o = ((f32(k) + 0.5) / 32.0 * 2.0 - 1.0) * item.params.z;
            acc += textureSampleLevel(tex, samp, v.uv + dx * o, 0.0);
        }
        let flou = acc / 32.0;
        return vec4<f32>(gamma_from_linear_rgb(flou.rgb), flou.a) * item.params.x;
    }
    let size = vec2<f32>(textureDimensions(tex));
    // Ce que couvre un pixel d'écran, en pixels de l'image : ses deux côtés, et son étendue par axe de l'image.
    let cx = dx * size;
    let cy = dy * size;
    let texels = max(length(cx), length(cy));
    let e = abs(cx) + abs(cy);
    var lin = vec4<f32>(0.0);
    if (texels <= 1.01) {
        lin = textureSampleLevel(tex, samp, v.uv, 0.0);
    } else if (min(abs(cx.y) + abs(cy.x), abs(cx.x) + abs(cy.y)) <= 0.002 * texels && max(e.x, e.y) <= f32(2 * LECTURES_MAX - 1)) {
        // Vue droite (ou tournée d'un quart de tour, retournée) : les pixels de l'image de `lo` à `hi`, chacun pour la part
        // de lui que le pixel d'écran couvre, lus deux par deux.
        let c = v.uv * size;
        let lo = c - 0.5 * e;
        let hi = c + 0.5 * e;
        let premier = floor(lo);
        let nx = i32(ceil((hi.x - premier.x) * 0.5));
        let ny = i32(ceil((hi.y - premier.y) * 0.5));
        for (var j = 0; j < ny; j++) {
            let ty = premier.y + 2.0 * f32(j);
            let ya = clamp(min(hi.y, ty + 1.0) - max(lo.y, ty), 0.0, 1.0);
            let yb = clamp(min(hi.y, ty + 2.0) - max(lo.y, ty + 1.0), 0.0, 1.0);
            let py = ty + 0.5 + yb / max(ya + yb, 1e-6);
            for (var i = 0; i < nx; i++) {
                let tx = premier.x + 2.0 * f32(i);
                let xa = clamp(min(hi.x, tx + 1.0) - max(lo.x, tx), 0.0, 1.0);
                let xb = clamp(min(hi.x, tx + 2.0) - max(lo.x, tx + 1.0), 0.0, 1.0);
                let px = tx + 0.5 + xb / max(xa + xb, 1e-6);
                lin += textureSampleLevel(tex, samp, vec2<f32>(px, py) / size, 0.0) * ((xa + xb) * (ya + yb));
            }
        }
        lin = lin / (e.x * e.y);
    } else {
        let n = i32(clamp(ceil(texels), 1.0, f32(LECTURES_MAX)));
        for (var j = 0; j < n; j++) {
            for (var i = 0; i < n; i++) {
                let o = (vec2<f32>(f32(i), f32(j)) + 0.5) / f32(n) - 0.5;
                lin += textureSampleLevel(tex, samp, v.uv + dx * o.x + dy * o.y, 0.0);
            }
        }
        lin = lin / f32(n * n);
    }
    // (Le volet écarte les pixels après les dérivées : elles se calculent en flot uniforme.)
    if (item.params.y > 1.5 && item.params.y < 2.5 && v.uv.x > item.params.z) {
        discard;
    }
    if (item.params.y > 5.5 && item.params.y < 6.5) {
        let d = v.pos.xy - item.extra.xy;
        var angle = atan2(d.x, -d.y);
        if (angle < 0.0) {
            angle = angle + 6.28318530718;
        }
        if (angle > item.extra.z) {
            discard;
        }
    }
    return vec4<f32>(gamma_from_linear_rgb(lin.rgb), lin.a) * item.params.x;
}

// sRGB ↔ linéaire (de 0 à 1), pour les ombres (partie 185).
fn vers_lin(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}
fn vers_srgb(l: vec3<f32>) -> vec3<f32> {
    let c = clamp(l, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055), c * 12.92, c <= vec3<f32>(0.0031308));
}
"#;

/// Les ombres des cartes et du décor 3D (partie 185, `ombres.rs`) : la lumière du décor (`ombres::Lumiere::octets`), et ce que la
/// composition et la carte d'ombres des cartes en font. Le même texte dans les deux modules (`eclair` y est déclarée à part).
const LUMIERE: &str = r#"
struct Lumiere {
    light_vp: mat4x4<f32>,
    oeil: vec4<f32>,       // xyz ; w : pixels par unité de la prise
    droite: vec4<f32>,     // xyz ; w : centre x
    haut: vec4<f32>,       // xyz ; w : centre y
    avant: vec4<f32>,      // xyz ; w : rayon du filtre sur le décor
    ciel: vec4<f32>,       // ambiance du ciel ; w : texel de la carte d'ombres
    sol: vec4<f32>,        // ambiance du sol ; w : brume la plus forte (0 : sans)
    soleil: vec4<f32>,     // couleur du soleil ; w : la carte d'ombres du décor est là
    horizon: vec4<f32>,    // couleur de la brume ; w : rayon du filtre sur une carte
    brume: vec4<f32>,      // début, fin de la brume ; taille de l'image composée
    carte: vec4<f32>,      // ce qui reste d'une carte à l'ombre ; w : la carte d'ombres des cartes est faite
    boite: vec4<f32>,      // où sont les cartes dans leur carte d'ombres (u0, v0, u1, v1)
};

// Le point du monde vu au pixel `px` de la feuille, à l'inverse de distance `inv` (`Lumiere::monde`).
fn lum_monde(px: vec2<f32>, inv: f32) -> vec3<f32> {
    let z = 1.0 / inv;
    let d = vec2<f32>((px.x - eclair.droite.w) / eclair.oeil.w, -(px.y - eclair.haut.w) / eclair.oeil.w);
    return eclair.oeil.xyz + z * (eclair.avant.xyz + eclair.droite.xyz * d.x + eclair.haut.xyz * d.y);
}

// Le point `p` du monde dans la carte d'ombres : (u, v) et sa profondeur (`Lumiere::vers_lumiere`).
fn lum_vers(p: vec3<f32>) -> vec3<f32> {
    let q = eclair.light_vp * vec4<f32>(p, 1.0);
    return vec3<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5, q.z);
}
"#;

/// La composition (partie 185) : le soleil du décor, sa lumière, sa carte d'ombres et celle des cartes (groupe 3, avec la profondeur
/// du décor de la partie 183).
const COMPOSER_OMBRES: &str = r#"
@group(3) @binding(1) var decor_soleil: texture_2d<f32>;
@group(3) @binding(2) var<uniform> eclair: Lumiere;
@group(3) @binding(3) var ombres_decor: texture_depth_2d;
@group(3) @binding(4) var ombres_cartes: texture_depth_2d;
@group(3) @binding(5) var s_cmp: sampler_comparison;

// `ombres::BIAIS`, `ombres::PAS`.
const BIAIS: f32 = 0.0015;
const PAS: f32 = 1.5;

// La part du soleil que voit le point `l` de la carte d'ombres `t` (`Lumiere::pcf`) : (2r + 1)² lectures comparées, chacune
// décalée sur le plan de la carte qui reçoit (`g`).
fn pcf(t: texture_depth_2d, l: vec3<f32>, g: vec2<f32>, r: i32) -> f32 {
    if (l.x < 0.0 || l.x > 1.0 || l.y < 0.0 || l.y > 1.0 || l.z > 1.0) {
        return 1.0;
    }
    let texel = eclair.ciel.w;
    var somme = 0.0;
    var n = 0.0;
    for (var y = -r; y <= r; y = y + 1) {
        for (var x = -r; x <= r; x = x + 1) {
            let o = vec2<f32>(f32(x), f32(y)) * texel * PAS;
            somme = somme + textureSampleCompareLevel(t, s_cmp, l.xy + o, l.z + dot(g, o) - BIAIS);
            n = n + 1.0;
        }
    }
    return somme / n;
}

fn dans_boite(l: vec3<f32>) -> bool {
    return l.x >= eclair.boite.x && l.x <= eclair.boite.z && l.y >= eclair.boite.y && l.y <= eclair.boite.w;
}

// Le décor sous une carte qui lui cache `occ` du soleil : le soleil retiré d'autant, la brume gardée (`Lumiere::assombrir_decor`).
fn assombrir_decor(c: vec4<f32>, s: vec2<f32>, d: f32, occ: f32) -> vec4<f32> {
    if (occ <= 0.0) {
        return c;
    }
    var f = 0.0;
    if (eclair.sol.w > 0.0) {
        let v = clamp((d - eclair.brume.x) / max(eclair.brume.y - eclair.brume.x, 0.0001), 0.0, 1.0) * eclair.sol.w;
        f = v * v * (3.0 - 2.0 * v);
    }
    let lin = vers_lin(min(c.rgb / c.a, vec3<f32>(1.0)));
    let hemi = mix(eclair.sol.rgb, eclair.ciel.rgb, s.y);
    let kk = eclair.soleil.rgb * s.x;
    let ratio = vec3<f32>(1.0) - occ * kk / max(hemi + kk, vec3<f32>(0.0001));
    let brume = eclair.horizon.rgb * f;
    let o = max((lin - brume) * ratio + brume, vec3<f32>(0.0));
    return vec4<f32>(vers_srgb(o) * c.a, c.a);
}

// Une carte qui reçoit les ombres, dont `occ` du soleil est caché (`Lumiere::assombrir_carte`).
fn assombrir_carte(c: vec4<f32>, occ: f32) -> vec4<f32> {
    if (occ <= 0.0) {
        return c;
    }
    let lin = vers_lin(min(c.rgb / c.a, vec3<f32>(1.0))) * (vec3<f32>(1.0) - occ * (vec3<f32>(1.0) - eclair.carte.rgb));
    return vec4<f32>(vers_srgb(lin) * c.a, c.a);
}

// Ce que le décor et les autres cartes cachent du soleil au pixel `px` de la carte (son plan : prof.xyz ; sa pente dans la carte
// d'ombres : extra.xy) (`Contexte::sur_la_carte`).
fn ombre_recue(px: vec2<f32>) -> f32 {
    let inv = item.prof.x * px.x + item.prof.y * px.y + item.prof.z;
    if (inv <= 0.0) {
        return 0.0;
    }
    let l = lum_vers(lum_monde(px, inv));
    let r = i32(eclair.horizon.w + 0.5);
    let g = item.extra.xy;
    var vis = 1.0;
    if (eclair.soleil.w > 0.5) {
        vis = pcf(ombres_decor, l, g, r);
    }
    if (vis > 0.0 && eclair.carte.w > 0.5 && dans_boite(l)) {
        vis = vis * pcf(ombres_cartes, l, g, r);
    }
    return 1.0 - vis;
}

// Le décor, assombri là où une carte lui cache le soleil (`Contexte::sur_le_decor`).
@fragment
fn fs_decor_o(v: VOut) -> @location(0) vec4<f32> {
    let c = composer(v);
    if (c.a <= 0.0 || eclair.carte.w < 0.5) {
        return c;
    }
    let taille = vec2<i32>(textureDimensions(decor_prof));
    let p = vec2<i32>(floor(v.pos.xy));
    if (p.x < 0 || p.y < 0 || p.x >= taille.x || p.y >= taille.y) {
        return c;
    }
    let inv = textureLoad(decor_prof, p, 0).r;
    let s = textureLoad(decor_soleil, p, 0).rg;
    if (inv <= 0.0 || s.x <= 0.0) {
        return c;
    }
    let w = lum_monde(v.pos.xy, inv);
    let l = lum_vers(w);
    if (!dans_boite(l)) {
        return c;
    }
    let occ = 1.0 - pcf(ombres_cartes, l, vec2<f32>(0.0), i32(eclair.avant.w + 0.5));
    return assombrir_decor(c, s, distance(w, eclair.oeil.xyz), occ);
}
"#;

/// La carte d'ombres des cartes (partie 185) : chaque carte qui porte une ombre, vue du soleil, là où son encre est.
const OMBRE_DES_CARTES: &str = r#"
struct Item {
    corners: array<vec4<f32>, 4>,
    color: vec4<f32>,
    params: vec4<f32>,
    extra: vec4<f32>,
    prof: vec4<f32>,
};
@group(0) @binding(0) var<uniform> item: Item;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
@group(2) @binding(0) var<uniform> eclair: Lumiere;

// `ombres::SEUIL_ALPHA`.
const SEUIL_ALPHA: f32 = 0.5;

struct OOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Un sommet de la carte (sa place dans l'image composée) : relevé sur son plan, dans le monde, puis vu du soleil.
fn poser(ndc: vec2<f32>, uv: vec2<f32>) -> OOut {
    let px = vec2<f32>((ndc.x + 1.0) * 0.5 * eclair.brume.z, (1.0 - ndc.y) * 0.5 * eclair.brume.w);
    let inv = max(item.prof.x * px.x + item.prof.y * px.y + item.prof.z, 1e-6);
    let q = eclair.light_vp * vec4<f32>(lum_monde(px, inv), 1.0);
    var o: OOut;
    // (Ce qui est entre le soleil et la carte d'ombres cache tout : ramené au plus près.)
    o.pos = vec4<f32>(q.x, q.y, clamp(q.z, 0.0, 1.0), 1.0);
    o.uv = uv;
    return o;
}

@vertex
fn vs_ombre(@builtin(vertex_index) i: u32) -> OOut {
    var order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let c = item.corners[order[i]];
    return poser(c.xy, c.zw);
}

@vertex
fn vs_ombre_mesh(@location(0) v: vec4<f32>) -> OOut {
    return poser(v.xy, v.zw);
}

@fragment
fn fs_ombre(o: OOut) {
    if (textureSampleLevel(tex, samp, o.uv, 0.0).a * item.params.x < SEUIL_ALPHA) {
        discard;
    }
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
    /// Résultat des effets sur la carte graphique d'un calque (`gpufx.rs`) : la texture est dessinée
    /// par des passes, jamais remplie de tuiles, et ne sert pas à recycler celle d'un dessin.
    fx: Option<FxState>,
    /// Le décor 3D (partie 183) : la texture de sa profondeur, de même taille (`decor3dgpu::PROF`), dessinée avec lui, et ce que
    /// la composition des cartes en lit (groupe 3 de `fs_compose_p`, `fs_blend_p`).
    prof: Option<(wgpu::Texture, wgpu::BindGroup)>,
    /// Le décor 3D avec ses ombres (partie 185) : ce que les ombres des cartes en lisent.
    ombre: Option<Box<OmbreDecor>>,
}

/// Où en est la texture résultat des effets d'un calque.
#[derive(Clone, Copy, Debug, Default)]
struct FxState {
    /// Signature de ce qu'il faut montrer (effets, dessin d'origine) et de ce que la texture montre.
    wanted: u64,
    done: u64,
}

impl CelTexture {
    /// Poids de la texture sur la carte.
    fn bytes(&self) -> u64 {
        // (Le décor 3D : sa profondeur à côté, deux octets par pixel, partie 183.)
        self.rect[2] as u64 * self.rect[3] as u64 * if self.prof.is_some() { 6 } else { 4 } + self.ombre.as_ref().map_or(0, |o| o.bytes())
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
pub(crate) fn texture_axis(lo: usize, hi: usize, total: usize) -> (usize, usize) {
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
    /// La même lecture, lissée aussi en grossissant (`GpuCanvas::lisser`).
    display_group_lisse: wgpu::BindGroup,
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
    /// Les mêmes quatre, pour une carte qui partage la profondeur du décor 3D (partie 183, `profondeur.rs`) : ils lisent aussi la
    /// profondeur du décor (groupe 3) et cachent la carte là où il est plus près. Un plan sans décor ne s'en sert jamais.
    compose_p: wgpu::RenderPipeline,
    blend_p: wgpu::RenderPipeline,
    compose_mesh_p: wgpu::RenderPipeline,
    blend_mesh_p: wgpu::RenderPipeline,
    /// Le groupe 3 de ces composeurs : la profondeur du décor (partie 183), son soleil, sa lumière, sa carte d'ombres et celle des
    /// cartes (partie 185).
    prof_layout: wgpu::BindGroupLayout,
    /// Les ombres des cartes (partie 185, `ombres.rs`) : le décor assombri sous les cartes ; la carte d'ombres des cartes (quatre coins,
    /// ou un maillage) ; la lumière du décor qu'elle lit ; les textures vides des groupes sans ombres.
    decor_o: wgpu::RenderPipeline,
    ombre_quad: wgpu::RenderPipeline,
    ombre_mesh: wgpu::RenderPipeline,
    lumiere_layout: wgpu::BindGroupLayout,
    vides: Vides,
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
    /// Mémoire visée pour les textures des dessins (`CACHE_BYTES`, plus petite dans les essais), telle que réglée.
    cache_bytes: u64,
    /// Plafond posé par le budget de mémoire vidéo du système (`memoirevideo.rs`, avec ce qu'il faut pour le suivre avec
    /// retenue) ; sans plafond : pas de lecture (le poids réglé vaut seul). Le cache vise le plus petit des deux (`limit`).
    vram: crate::memoirevideo::Suivi,
    /// Numéro de la dernière lecture du budget prise en compte.
    vram_seq: u64,
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
    /// Effets de calque sur la carte graphique (`gpufx.rs`) ; rien si le shader n'a pas pu être
    /// compilé (le calcul de fond sur processeur sert alors).
    gpufx: Option<crate::gpufx::GpuFx>,
    /// Le réglage « Effets sur la carte graphique » (Préférences, diagnostic).
    fx_enabled: bool,
    /// Les chaînes de passes à exécuter à la prochaine composition (`sync` les pose).
    fx_jobs: Vec<FxJob>,
    /// L'encre autour de la feuille lue par le tremblement (`gpufx::Bordure`), par texture de résultat.
    bordures: HashMap<CelId, BordureGardee>,
    /// Passes d'effets dessinées, pour les mesures.
    fx_passes: u64,
    /// Les décors 3D à redessiner à la prochaine composition (`place_decor` les pose ; `decor3dgpu.rs`).
    decor_jobs: Vec<DecorJob>,
}

/// Un décor 3D à dessiner dans sa texture (la feuille vue de l'œil de la caméra) avant la composition.
struct DecorJob {
    out: CelId,
    /// Un dessin par objet du décor (partie 181) ; les objets d'un même modèle partagent ce qui est dans la carte.
    dessins: Vec<crate::decor3dgpu::Dessin>,
    ciel: bool,
    prise: crate::decor3drendu::Prise,
    taille: [u32; 2],
    qualite: crate::decor3dbudget::Qualite,
    sig: u64,
}

impl CanvasResources {
    /// Le poids que le cache des dessins vise : le réglage, ou moins si la mémoire vidéo manque (`memoirevideo.rs`).
    fn limit(&self) -> u64 {
        crate::memoirevideo::effectif(self.cache_bytes, self.vram.plafond)
    }

    /// Prend en compte une lecture du budget de mémoire vidéo (une seule fois par lecture), `own` étant le poids du cache
    /// des dessins maintenant. Une décision qui change le poids visé est notée pour la fenêtre de diagnostic.
    fn follow_vram(&mut self, reading: Option<crate::memoirevideo::Lecture>, own: u64) {
        let Some(l) = reading else {
            // (Plus de lecture — essais — : le poids réglé, comme avant.)
            self.vram = Default::default();
            self.vram_seq = 0;
            return;
        };
        if l.seq == self.vram_seq {
            return;
        }
        self.vram_seq = l.seq;
        // Ce que tient l'image affichée (la dernière composée) : le plafond ne descend pas plus bas.
        let tick = self.tick;
        let travail: u64 = self.textures.values().filter(|t| t.last_used + 1 >= tick).map(CelTexture::bytes).sum();
        let suivi = crate::memoirevideo::prochain_plafond(self.vram, l, own, self.cache_bytes, travail);
        if let Some(cap) = suivi.plafond.filter(|_| crate::memoirevideo::vaut_d_etre_note(self.vram.plafond, suivi.plafond, self.cache_bytes)) {
            crate::memoirevideo::noter(crate::memoirevideo::decision(l, own, self.cache_bytes, cap));
        }
        self.vram = suivi;
    }
}

/// Un calque dont la texture d'effets est à refaire : la texture du dessin d'origine, celle du
/// résultat, la chaîne, la signature à laquelle la texture sera à jour.
struct FxJob {
    src: CelId,
    out: CelId,
    chain: std::sync::Arc<crate::gpufx::Chain>,
    sig: u64,
    bordure: Option<wgpu::TextureView>,
}

/// Une bordure d'encre envoyée à la carte, pour tel dessin à telle version : refaite seulement quand il change.
struct BordureGardee {
    source: CelId,
    version: u64,
    /// L'adresse des tuiles du dessin : un même numéro et une même version peuvent être ceux d'un autre projet.
    tuiles: usize,
    m: i32,
    /// La préparation d'écran qui s'en est servie la dernière.
    vue: u64,
    /// La bordure et sa texture, et l'empreinte de ses pixels ; rien : pas d'encre autour de la feuille.
    faite: Option<(crate::gpufx::Bordure, wgpu::TextureView, u64)>,
}

/// Bordures gardées, au plus : les plus anciennes partent (une bordure se refait en une milliseconde, et tout un épisode de dessins qui débordent ne doit rien tenir en mémoire vidéo).
const BORDURES_MAX: usize = 32;

impl BordureGardee {
    /// Mémoire de la carte qu'elle tient (octets) : rien pour un dessin sans encre autour de la feuille.
    fn bytes(&self) -> u64 {
        self.faite.as_ref().map_or(0, |(b, ..)| b.w as u64 * b.h as u64 * 4)
    }
}

/// La bordure du dessin `cel` pour la texture de résultat `out`, sur `m` pixels : celle qu'on a, ou une nouvelle si le dessin a changé. `bytes` : le poids du cache, tenu à jour.
#[allow(clippy::too_many_arguments)]
fn bordure_de(rs: &RenderState, res: &mut CanvasResources, out: CelId, src: CelId, cel: &crate::document::Cel, m: i32, tick: u64, bytes: &mut u64) -> Option<(crate::gpufx::Bordure, wgpu::TextureView, u64)> {
    let tuiles = cel.raster.tiles.as_ptr() as usize;
    // (À chaque image, pour chaque calque qui tremble : d'abord ce qu'on sait déjà de cette version du dessin, avec ou sans encre autour.)
    if let Some(g) = res.bordures.get_mut(&out) {
        if g.source == src && g.version == cel.version && g.tuiles == tuiles && g.m == m {
            g.vue = tick;
            return g.faite.clone();
        }
    }
    if let Some(g) = res.bordures.remove(&out) {
        *bytes = bytes.saturating_sub(g.bytes());
    }
    if res.bordures.len() >= BORDURES_MAX {
        if let Some(k) = res.bordures.iter().min_by_key(|(_, g)| g.vue).map(|(&k, _)| k) {
            if let Some(g) = res.bordures.remove(&k) {
                *bytes = bytes.saturating_sub(g.bytes());
            }
        }
    }
    let bordure = if crate::fx::cel_deborde(cel) { crate::gpufx::Bordure::de(&cel.raster, m) } else { None };
    #[cfg(test)]
    BORDURES_FAITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let faite = bordure.map(|(b, px)| {
        let texture = rs.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("effets : bordure d'encre"),
            size: wgpu::Extent3d { width: b.w as u32, height: b.h as u32, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::gpufx::FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        rs.queue.write_texture(
            texture.as_image_copy(),
            &px,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(b.w as u32 * 4), rows_per_image: Some(b.h as u32) },
            wgpu::Extent3d { width: b.w as u32, height: b.h as u32, depth_or_array_layers: 1 },
        );
        let mut h = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&px, &mut h);
        (b, texture.create_view(&Default::default()), std::hash::Hasher::finish(&h))
    });
    let gardee = BordureGardee { source: src, version: cel.version, tuiles, m, vue: tick, faite: faite.clone() };
    *bytes += gardee.bytes();
    res.bordures.insert(out, gardee);
    faite
}

/// (Essais.) Combien de bordures ont été refaites depuis le début.
#[cfg(test)]
pub(crate) static BORDURES_FAITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Les textures de dessin d'un projet qui n'est pas affiché (un onglet inactif, `onglets.rs`).
pub(crate) struct TexturesDeProjet(HashMap<CelId, CelTexture>);

impl TexturesDeProjet {
    /// Mémoire de la carte graphique qu'elles tiennent (octets).
    pub(crate) fn bytes(&self) -> u64 {
        self.0.values().map(CelTexture::bytes).sum()
    }
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
    /// L'image composée se pose **lissée** quand la vue la grossit, au lieu de pixels nets : pendant que la vue nette (`vuenette.rs`)
    /// n'a pas encore l'image à la taille de l'écran (la vue bouge, un trait vient d'être tracé), on voit un dessin adouci plutôt
    /// qu'en marches (Anthony, 2026-11-17). Réglé à chaque image par l'écran ; faux : l'affichage de toujours.
    pub(crate) lisser: bool,
}

impl GpuCanvas {
    /// L'état de rendu partagé avec eframe : la vue Espace 3D y pose le moteur du décor 3D (`decor3dgpu.rs`, partie 180).
    pub fn render_state(&self) -> &RenderState {
        &self.render_state
    }

    /// Plus grand côté d'une texture de la carte (au-delà, l'image composée de la feuille ne se crée pas).
    pub fn max_side(&self) -> u32 {
        self.render_state.device.limits().max_texture_dimension_2d
    }

    pub fn new(rs: &RenderState) -> Self {
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("calques"),
            // (Partie 185 : la lumière du décor et les ombres, à la suite.)
            source: wgpu::ShaderSource::Wgsl(format!("{SHADER}{LUMIERE}{COMPOSER_OMBRES}").into()),
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
        // La profondeur partagée avec le décor 3D (partie 183) : un quatrième groupe, la profondeur du décor (le troisième est le
        // fond des modes de fusion, posé aussi pour les autres).
        // (Partie 185 : et les ombres des cartes, `ombres.rs` : le soleil du décor, sa lumière, sa carte d'ombres, celle des cartes et
        // leur échantillonneur ; des textures vides là où il n'y en a pas, `Vides`.)
        let texture_lue = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let carte_d_ombres = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let lumiere_entree = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: NonZeroU64::new(crate::ombres::OCTETS) },
            count: None,
        };
        let prof_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("calques : profondeur et ombres du décor 3D"),
            entries: &[
                texture_lue(0),
                texture_lue(1),
                lumiere_entree(2),
                carte_d_ombres(3),
                carte_d_ombres(4),
                wgpu::BindGroupLayoutEntry { binding: 5, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
            ],
        });
        let lumiere_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("calques : lumière du décor 3D"), entries: &[lumiere_entree(0)] });
        let p_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("calques : profondeur partagée"),
            bind_group_layouts: &[&uniform_layout, &texture_layout, &texture_layout, &prof_layout],
            push_constant_ranges: &[],
        });
        let quad_pipeline = |entry: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&p_layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
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
        let compose_p = quad_pipeline("fs_compose_p", Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        let blend_p = quad_pipeline("fs_blend_p", None);
        let compose_mesh_p = mesh_pipeline("fs_compose_p", &p_layout, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        let blend_mesh_p = mesh_pipeline("fs_blend_p", &p_layout, None);
        // Les ombres des cartes (partie 185) : le décor assombri sous elles, et la carte d'ombres des cartes (profondeur seule).
        let decor_o = quad_pipeline("fs_decor_o", Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        let ombre_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("calques : ombres des cartes"), source: wgpu::ShaderSource::Wgsl(format!("{LUMIERE}{OMBRE_DES_CARTES}").into()) });
        let ombre_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("calques : ombres des cartes"), bind_group_layouts: &[&uniform_layout, &texture_layout, &lumiere_layout], push_constant_ranges: &[] });
        let ombre_pipeline = |entry: &str, mesh: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&ombre_layout),
                vertex: wgpu::VertexState { module: &ombre_shader, entry_point: Some(entry), buffers: if mesh { &mesh_buffers } else { &[] }, compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: &ombre_shader, entry_point: Some("fs_ombre"), targets: &[], compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState { format: OMBRE_FORMAT, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: Default::default() }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let ombre_quad = ombre_pipeline("vs_ombre", false);
        let ombre_mesh = ombre_pipeline("vs_ombre_mesh", true);
        let vides = Vides::new(device);
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
            compose_p,
            blend_p,
            compose_mesh_p,
            blend_mesh_p,
            prof_layout,
            decor_o,
            ombre_quad,
            ombre_mesh,
            lumiere_layout,
            vides,
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
            vram: Default::default(),
            vram_seq: 0,
            whole_textures: false,
            staging: Vec::new(),
            fresh_staging: false,
            copies: 0,
            composes: 0,
            frame: None,
            frame2: None,
            tick: 0,
            gpufx: crate::gpufx::GpuFx::try_new(device, &rs.queue),
            fx_enabled: true,
            fx_jobs: Vec::new(),
            bordures: HashMap::new(),
            fx_passes: 0,
            decor_jobs: Vec::new(),
        });
        // Le budget de mémoire vidéo du système, lu sur un autre fil (`memoirevideo.rs`).
        crate::memoirevideo::suivre(&rs.adapter.get_info());
        Self {
            render_state: rs.clone(),
            lisser: false,
        }
    }

    /// La mémoire visée pour les textures des dessins (un Go d'ordinaire ; les Préférences et le
    /// diagnostic des performances la règlent). Les plus anciennes partent à la prochaine image si
    /// elle est dépassée.
    pub fn set_cache_bytes(&self, bytes: u64) {
        if let Some(res) = self.render_state.renderer.write().callback_resources.get_mut::<CanvasResources>() {
            res.cache_bytes = bytes;
        }
    }

    /// Les effets de calque passent-ils par la carte graphique (réglage « Effets sur la carte
    /// graphique » allumé, et shader compilé) ?
    pub fn effects_on(&self) -> bool {
        self.render_state.renderer.read().callback_resources.get::<CanvasResources>().is_some_and(|r| r.fx_enabled && r.gpufx.is_some())
    }

    /// Cette carte graphique sait-elle dessiner les effets (le shader a été accepté au démarrage) ?
    pub fn effects_supported(&self) -> bool {
        self.render_state.renderer.read().callback_resources.get::<CanvasResources>().is_some_and(|r| r.gpufx.is_some())
    }

    /// Allume ou éteint les effets sur la carte graphique (Préférences, diagnostic). Éteints, tout
    /// passe par le calcul de fond sur processeur, comme avant.
    pub fn set_effects_gpu(&self, on: bool) {
        if let Some(res) = self.render_state.renderer.write().callback_resources.get_mut::<CanvasResources>() {
            if res.fx_enabled != on {
                res.fx_enabled = on;
                // (Les textures de résultat n'ont plus lieu d'être ; l'image composée gardée non plus.)
                res.textures.retain(|_, t| t.fx.is_none());
                res.fx_jobs.clear();
                res.bordures.clear();
                res.frame = None;
                res.frame2 = None;
            }
        }
    }

    /// (Essais.) Les bordures d'encre gardées.
    #[cfg(test)]
    pub fn bordures(&self) -> usize {
        self.render_state.renderer.read().callback_resources.get::<CanvasResources>().map_or(0, |r| r.bordures.len())
    }

    /// Essais et mesures : mémoire vidéo prise par les effets sur la carte graphique, textures de travail
    /// et textures de résultat (octets).
    #[cfg(test)]
    pub fn effects_bytes(&self) -> u64 {
        let renderer = self.render_state.renderer.read();
        let Some(res) = renderer.callback_resources.get::<CanvasResources>() else { return 0 };
        res.gpufx.as_ref().map_or(0, crate::gpufx::GpuFx::bytes) + res.textures.values().filter(|t| t.fx.is_some()).map(CelTexture::bytes).sum::<u64>()
    }

    /// Essais et mesures : passes d'effets dessinées depuis le début.
    #[cfg(test)]
    pub fn effects_passes(&self) -> u64 {
        let renderer = self.render_state.renderer.read();
        renderer.callback_resources.get::<CanvasResources>().map_or(0, |r| r.fx_passes)
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

    /// Essais et mesures (partie 183) : l'image composée gardée est oubliée, la prochaine commande de dessin recompose tout.
    #[cfg(test)]
    pub fn oublier_composition(&self) {
        if let Some(f) = self.render_state.renderer.write().callback_resources.get_mut::<CanvasResources>().and_then(|r| r.frame.as_mut()) {
            f.composed = None;
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

    /// Essais : l'image composée telle qu'elle est en mémoire de la carte (valeurs gamma
    /// prémultipliées, comme les dessins), sans passer par l'affichage.
    #[cfg(test)]
    pub fn read_frame(&self) -> Vec<eframe::egui::Color32> {
        let rs = &self.render_state;
        let renderer = rs.renderer.read();
        let Some(frame) = renderer.callback_resources.get::<CanvasResources>().and_then(|r| r.frame.as_ref()) else { return Vec::new() };
        let (w, h) = frame.size;
        let row = (w * 4).div_ceil(256) * 256;
        let readback = rs.device.create_buffer(&wgpu::BufferDescriptor { label: Some("lecture de l'image composée"), size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut encoder = rs.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            frame.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            extent(w, h),
        );
        rs.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("lecture"));
        rs.device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((w * h) as usize);
        for y in 0..h as usize {
            for x in 0..w as usize {
                let i = y * row as usize + x * 4;
                out.push(eframe::egui::Color32::from_rgba_premultiplied(data[i], data[i + 1], data[i + 2], data[i + 3]));
            }
        }
        out
    }

    /// Le projet affiché s'en va (onglets de projets) : ses textures de dessin sont rangées dans ce qui est
    /// rendu — les numéros de dessin sont ceux d'un projet, l'autre ne doit pas les voir — et l'image composée
    /// gardée est abandonnée. L'onglet les reprend à son retour (`poser_textures`), sans renvoyer un pixel.
    pub(crate) fn prendre_textures(&self) -> TexturesDeProjet {
        let mut renderer = self.render_state.renderer.write();
        let Some(res) = renderer.callback_resources.get_mut::<CanvasResources>() else { return TexturesDeProjet(HashMap::new()) };
        res.fx_jobs.clear();
        // (Les bordures se refont en une milliseconde : elles ne partent pas avec l'onglet, elles sont libérées.)
        res.bordures.clear();
        if let Some(f) = res.frame.as_mut() {
            f.composed = None;
        }
        if let Some(f) = res.frame2.as_mut() {
            f.composed = None;
        }
        TexturesDeProjet(std::mem::take(&mut res.textures))
    }

    /// La mémoire visée pour les textures des dessins (octets) : les onglets cachés n'en gardent pas plus,
    /// tous ensemble.
    pub(crate) fn limite_octets(&self) -> u64 {
        self.render_state.renderer.read().callback_resources.get::<CanvasResources>().map_or(0, CanvasResources::limit)
    }

    /// Essais et mesures : le poids que le cache vise maintenant (le réglage, ou moins si la mémoire vidéo manque).
    #[cfg(test)]
    pub fn cache_limit(&self) -> u64 {
        self.limite_octets()
    }

    /// Le projet qui revient reprend ses textures (le cache est celui de l'onglet qui part, vidé par
    /// `prendre_textures`).
    pub(crate) fn poser_textures(&self, textures: TexturesDeProjet) {
        let mut renderer = self.render_state.renderer.write();
        let Some(res) = renderer.callback_resources.get_mut::<CanvasResources>() else { return };
        res.textures = textures.0;
        res.fx_jobs.clear();
        res.bordures.clear();
        if let Some(f) = res.frame.as_mut() {
            f.composed = None;
        }
        if let Some(f) = res.frame2.as_mut() {
            f.composed = None;
        }
    }

    /// Nombre de dessins qu'on peut envoyer d'avance sans chasser ceux qui sont affichés. Le poids
    /// d'un dessin est celui des textures que le cache garde (elles n'ont que la place du contenu) ;
    /// un cache encore presque vide compte un quart de feuille.
    pub fn ahead_capacity(&self, doc: &Document, shown: usize) -> usize {
        let sheet = (doc.width as u64 * doc.height as u64 * 4).max(1);
        let renderer = self.render_state.renderer.read();
        let Some(res) = renderer.callback_resources.get::<CanvasResources>() else { return 0 };
        // (Les textures de résultat des effets ne comptent pas : on n'en envoie pas d'avance.)
        let cels = || res.textures.values().filter(|t| t.fx.is_none());
        let n = cels().count() as u64;
        // (Une feuille de moins de 64 Ko — moins de 128 × 128 pixels — : le plancher ne dépasse pas la feuille, sinon `clamp` s'arrête net.)
        let per = if n >= 8 { (cels().map(CelTexture::bytes).sum::<u64>() / n).clamp(sheet.min(64 << 10), sheet) } else { sheet / 4 };
        ((res.limit() / per) as usize).saturating_sub(shown + 4) / 2
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
        let mut bytes: u64 = res.textures.values().map(CelTexture::bytes).sum::<u64>() + res.bordures.values().map(BordureGardee::bytes).sum::<u64>();
        // Une nouvelle lecture du budget de mémoire vidéo (toutes les 1,5 s, sur un autre fil) : le plafond du cache est
        // refait avec le poids du cache de maintenant, puis tenu jusqu'à la lecture suivante.
        res.follow_vram(crate::memoirevideo::lecture(), bytes);
        // Un dessin passé par des effets (13i) : son dessin dérivé, sous son propre numéro ; en
        // aperçu ou en attente du fil de fond, une texture plus petite ou plus ancienne. Cherché
        // une fois par élément.
        // Une pile dont tous les effets ont une version sur la carte graphique (`gpufx.rs`) n'est pas
        // calculée par le fil de fond : seul le dessin d'origine part à la carte, les effets se
        // dessinent à la composition, à pleine résolution, à chaque image.
        let fx_on = res.fx_enabled && res.gpufx.is_some();
        let on_gpu: Vec<bool> = plan.items.iter().map(|item| fx_on && gpu_fx_item(doc, item)).collect();
        // Une pile qui mêle des effets du trait (calcul de fond : ils lisent les traits du dessin vectoriel) et
        // des effets de la carte : le fil de fond calcule le dessin dérivé par les effets du trait seuls, la
        // carte passe les autres effets dessus, à pleine résolution (`fx::hybrid_split`).
        let hybrid: Vec<Option<(crate::render::PlanItem, Vec<crate::fx::FxStep>)>> = plan.items.iter().zip(&on_gpu).map(|(item, &gpu)| gpu.then(|| crate::fx::hybrid_split(item)).flatten()).collect();
        // Le décor 3D (partie 180) : dessiné par la carte, dans sa texture, avant la composition (`place_decor`) ; jamais rendu
        // par le processeur ici (la caméra qui bouge ferait un rendu par image).
        let max_side = rs.device.limits().max_texture_dimension_2d;
        let decor_gpu: Vec<bool> = plan.items.iter().map(|item| matches!(item.fond, Some(crate::render::FondPart::Decor(_))) && size.0 <= max_side && size.1 <= max_side).collect();
        let displays: Vec<Option<crate::fx::Display>> = plan
            .items
            .iter()
            .zip(&on_gpu)
            .enumerate()
            .map(|(i, (item, &gpu))| {
                if decor_gpu[i] {
                    return None;
                }
                // (Sur la carte en attendant le calcul de fond d'un dessin qui déborde : il part ici, son résultat prend la place à son arrivée.)
                if gpu {
                    crate::fx::demander_si_debord(doc, item);
                }
                if let Some((line, _)) = &hybrid[i] {
                    crate::fx::display(doc, line, level)
                } else if gpu {
                    doc.cels.get(&item.cel).map(|cel| crate::fx::Display { cel: crate::fx::Shown::Plain(cel), id: item.cel, ready: true })
                } else {
                    crate::fx::display(doc, item, level)
                }
            })
            .collect();
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
        for (i, item) in plan.items.iter().enumerate() {
            if let (true, Some(crate::render::FondPart::Decor(key))) = (decor_gpu[i], &item.fond) {
                resolved[i] = place_decor(rs, res, doc, key, size, tick, &mut bytes);
            }
        }
        // Les calques à effets sur la carte graphique : la texture du résultat de chaque pile (créée à
        // la taille de ce que les effets touchent), et la chaîne de passes à y dessiner si elle n'est
        // pas à jour (exécutée à la composition, `prepare`).
        let sheet = size;
        let mut seen: Vec<CelId> = Vec::new();
        for (i, item) in plan.items.iter().enumerate() {
            if !on_gpu[i] {
                continue;
            }
            // (Une pile mêlée : les effets de la carte partent du dessin dérivé par les effets du trait.)
            let chained;
            let item = match (&hybrid[i], &displays[i]) {
                (Some((_, rest)), Some(shown)) => {
                    chained = crate::render::PlanItem { cel: shown.id, fx: rest.clone(), ..item.clone() };
                    &chained
                }
                (Some(_), None) => continue,
                _ => item,
            };
            let nth = seen.iter().filter(|&&c| c == item.cel).count();
            seen.push(item.cel);
            // (Ce que le tremblement lit autour de la feuille : le dessin qui part à la carte.)
            let lu = displays[i].as_ref().map(|d| &*d.cel);
            if let Some(out) = place_fx(rs, res, item, lu, nth, sheet, tick, &mut bytes) {
                resolved[i] = Some(out);
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
        let budget = res.limit();
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
            clip: None,
            extra: None,
        };
        // (Partie 175.) Une image glissée de `d` points à l'écran, coupée à `clip` ; floue de `flou` points (mode 4 de
        // `fs_display`), sinon posée telle quelle.
        let glissee = |layer: &MixLayer, second: bool, d: egui::Vec2, flou: f32, clip: Rect| {
            let mut s = quad(layer, second, if flou > 0.05 { [1.0, 4.0, flou] } else { [1.0, 1.0, 0.0] });
            for c in &mut s.screen {
                c[0] += d.x;
                c[1] += d.y;
            }
            s.clip = Some(clip);
            s
        };
        // Un aplat de la couleur `c` (valeurs gamma), d'opacité `k`, sur les coins `coins` à l'écran (mode 3).
        let aplat = |coins: [Pos2; 4], c: [u8; 3], k: f32| Show {
            screen: std::array::from_fn(|i| [coins[i].x, coins[i].y, uv[i][0], uv[i][1]]),
            ws: None,
            params: [k.clamp(0.0, 1.0), 3.0, couleur_en_reglage(c)],
            second: false,
            clip: None,
            extra: None,
        };
        let loin = 1.0e6_f32;
        let mut shows = Vec::new();
        match (&second, mix) {
            (Some(s), Some(m)) if m.glisse => {
                // Poussée, raccord filé : A sort d'un côté, B arrive de l'autre (la droite d'origine), chacun coupé à la part du
                // cadre où il se voit encore (son propre cadre, glissé, dans le cadre filmé).
                let cadre = Rect::from_points(&m.cadre);
                let flou = m.blur * m.largeur().length();
                let coupe = |dec: f32| {
                    if m.vertical {
                        let h = cadre.height();
                        Rect::from_min_max(pos2(cadre.left(), cadre.top() + dec.max(0.0) * h), pos2(cadre.right(), cadre.bottom() + dec.min(0.0) * h))
                    } else {
                        let w = cadre.width();
                        Rect::from_min_max(pos2(cadre.left() + dec.max(0.0) * w, cadre.top()), pos2(cadre.right() + dec.min(0.0) * w, cadre.bottom()))
                    }
                };
                let pas = if m.vertical { m.hauteur() } else { m.largeur() };
                shows.push(glissee(&first, false, pas * m.shift_a, flou, coupe(m.shift_a)));
                shows.push(glissee(s, true, pas * m.shift_b, flou, coupe(m.shift_b)));
            }
            (Some(s), Some(m)) if m.zoom.is_some() => {
                // (Partie 177.) Le zoom traversant : chaque image grossie autour du centre du zoom, coupée au cadre ; B par-dessus,
                // de poids `m`.
                let [za, zb] = m.zoom.unwrap_or([1.0, 1.0]);
                let c = m.dans_le_cadre(m.zoom_centre[0], m.zoom_centre[1]);
                let cadre = Rect::from_points(&m.cadre);
                let grossie = |layer: &MixLayer, second: bool, z: f32, k: f32| {
                    let mut sh = quad(layer, second, [k.clamp(0.0, 1.0), 1.0, 0.0]);
                    for p in &mut sh.screen {
                        p[0] = c.x + (p[0] - c.x) * z;
                        p[1] = c.y + (p[1] - c.y) * z;
                    }
                    sh.clip = Some(cadre);
                    sh
                };
                shows.push(grossie(&first, false, za, 1.0));
                shows.push(grossie(s, true, zb, m.m));
            }
            (Some(s), Some(m)) if m.horloge.is_some() => {
                // Le volet en horloge : A, puis B seulement dans l'angle balayé (mode 6), autour du milieu du cadre.
                shows.push(quad(&first, false, [1.0, 1.0, 0.0]));
                let c = m.dans_le_cadre(0.5, 0.5);
                let mut sh = quad(s, true, [1.0, 6.0, 0.0]);
                sh.extra = Some([c.x, c.y, m.horloge.unwrap_or(0.0), 0.0]);
                shows.push(sh);
            }
            (Some(s), Some(m)) if m.wipe && m.wipe_depuis != crate::transitions::Cote::Gauche => {
                // Le volet depuis la droite, le haut ou le bas : B coupé à la part du cadre qu'il a recouverte.
                shows.push(quad(&first, false, [1.0, 1.0, 0.0]));
                let cadre = Rect::from_points(&m.cadre);
                let k = m.m.clamp(0.0, 1.0);
                let part = match m.wipe_depuis {
                    crate::transitions::Cote::Droite => Rect::from_min_max(pos2(cadre.right() - k * cadre.width(), cadre.top()), cadre.max),
                    crate::transitions::Cote::Haut => Rect::from_min_max(cadre.min, pos2(cadre.right(), cadre.top() + k * cadre.height())),
                    _ => Rect::from_min_max(pos2(cadre.left(), cadre.bottom() - k * cadre.height()), cadre.max),
                };
                let mut sh = quad(s, true, [1.0, 1.0, 0.0]);
                sh.clip = Some(part);
                shows.push(sh);
            }
            (Some(s), Some(m)) if m.objet.is_some() => {
                // Volet par un objet : A à gauche du milieu de l'objet, B à droite, puis l'objet par-dessus.
                let c = m.objet.unwrap_or(0.5);
                let x = m.dans_le_cadre(c, 0.5).x;
                shows.push(glissee(&first, false, egui::Vec2::ZERO, 0.0, Rect::from_min_max(pos2(-loin, -loin), pos2(x, loin))));
                shows.push(glissee(s, true, egui::Vec2::ZERO, 0.0, Rect::from_min_max(pos2(x, -loin), pos2(loin, loin))));
                let coins = crate::transitions::objet_coins(c).map(|p| m.dans_le_cadre(p[0], p[1]));
                shows.push(aplat(coins, crate::transitions::OBJET_COULEUR, 1.0));
            }
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
            // La couche de couleur : un aplat sur la feuille, d'opacité `k`, de la couleur de la transition (partie 175).
            shows.push(quad(&first, false, [m.k.clamp(0.0, 1.0), 3.0, couleur_en_reglage(m.couleur)]));
        }
        if let Some((m, [cx, cy, r])) = mix.and_then(|m| m.iris.map(|i| (m, i))) {
            // (Partie 177.) L'iris : la couleur hors du rond, sur le cadre (mode 5).
            let c = m.dans_le_cadre(cx, cy);
            let rayon = r * m.largeur().length();
            let mut sh = aplat(m.cadre, m.couleur, 1.0);
            sh.params[1] = 5.0;
            sh.extra = Some([c.x, c.y, rayon, 0.0]);
            shows.push(sh);
        }
        egui_wgpu::Callback::new_paint_callback(rect, CanvasPaint { items, split, shows, lisser: self.lisser })
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
        // (Et, pour le décor 3D, s'il a sa profondeur : partie 183.)
        // (Et sa lumière, s'il a les ombres des cartes : partie 185.)
        let places: Vec<Option<([u32; 4], (u32, u32), bool, Option<crate::ombres::Lumiere>)>> = {
            let renderer = self.render_state.renderer.read();
            let textures = renderer.callback_resources.get::<CanvasResources>().map(|res| &res.textures);
            resolved.iter().map(|id| id.and_then(|id| textures?.get(&id)).map(|t| (t.rect, t.dims, t.prof.is_some(), t.ombre.as_ref().map(|o| o.lumiere)))).collect()
        };
        // Le décor 3D posé dessous, s'il a sa profondeur sur la carte : les cartes qui portent leur plan s'y cachent (partie 183).
        let mut decor: Option<CelId> = None;
        // Sa lumière, s'il a les ombres des cartes, et son rang parmi les éléments composés (partie 185).
        let mut lumiere: Option<(CelId, crate::ombres::Lumiere, usize)> = None;
        let mut items = Vec::with_capacity(plan.items.len() + 1);
        if let Some(p) = plan.paper {
            items.push(GpuItem {
                cel: None,
                corners: full,
                color: [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, 0.0],
                params: [1.0, 0.0, 0.0, 0.0],
                blend: 0,
                mesh: None,
                adjust: None,
                prof: None,
                ombre: None,
                decor_ombre: false,
            });
        }
        for ((item, id), place) in plan.items.iter().zip(resolved).zip(&places) {
            // Le repère d'un calque d'effet d'ajustement : ses effets passent sur l'image composée jusqu'ici
            // (`gpufx.rs`), le résultat est posé dessus avec l'opacité et le mode du calque. Un repère
            // qui n'a pas pu être résolu par la carte (calque dans un dossier…) ne montre rien : il est
            // résolu sur le processeur avant (`canvas.rs`, `gpufx::adjustments_supported`).
            if let Some(crate::render::FondPart::Reglage(r)) = &item.fond {
                if r.root && crate::gpufx::steps_supported(&r.steps) {
                    items.push(GpuItem {
                        cel: None,
                        corners: full,
                        color: [0.0; 4],
                        params: [r.opacity as f32 / 255.0, 1.0, r.blend.index() as f32, 0.0],
                        blend: r.blend.index(),
                        mesh: None,
                        adjust: Some(r.steps.clone()),
                        prof: None,
                        ombre: None,
                        decor_ombre: false,
                    });
                }
                continue;
            }
            // Un élément sans dessin n'est pas composé.
            let Some(id) = *id else { continue };
            if let Some(crate::render::FondPart::Decor(_)) = &item.fond {
                decor = place.is_some_and(|p| p.2).then_some(id);
                lumiere = place.and_then(|p| p.3).map(|l| (id, l, items.len()));
            }
            let ombre = match (lumiere, item.ombre) {
                (Some((d, lum, _)), Some(o)) if item.tint.is_none() && (o.porte || o.recoit) => Some(OmbreGpu { decor: d, porte: o.porte, recoit: o.recoit, plan: o.plan, gradient: lum.gradient(&o) }),
                _ => None,
            };
            // (Sans texture, rien à poser : la passe de composition saute l'élément.)
            let (rect, rdims) = place.map_or(([0, 0, stage[0] as u32, stage[1] as u32], (stage[0] as u32, stage[1] as u32)), |p| (p.0, p.1));
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
                adjust: None,
                prof: decor.zip(item.profondeur),
                ombre,
                decor_ombre: false,
            });
        }
        // Le décor s'assombrit sous les cartes qui portent une ombre (partie 185).
        if let Some((d, _, rang)) = lumiere {
            if items.iter().any(|i| i.ombre.is_some_and(|o| o.porte && o.decor == d)) {
                if let Some(i) = items.get_mut(rang) {
                    i.decor_ombre = i.cel == Some(d);
                }
            }
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
    /// Couche de couleur : sa couleur (valeurs gamma, 0 à 255) et son opacité.
    pub couleur: [u8; 3],
    pub k: f32,
    /// Colonnes de la feuille (0 à 1) où le cadre filmé commence et finit : le volet y avance.
    pub wipe_u: [f32; 2],
    /// Poussée, raccord filé (partie 175, `transitions::Mix`) : les deux plans glissent de `shift_a` et `shift_b` largeurs du
    /// cadre, flous de `blur` largeurs, chacun coupé à son cadre.
    pub glisse: bool,
    pub shift_a: f32,
    pub shift_b: f32,
    pub blur: f32,
    /// Volet par un objet : le milieu de l'objet, en parts du cadre.
    pub objet: Option<f32>,
    /// (Partie 177.) Le glissement est vertical ; le côté d'où le volet amène B ; l'iris (centre en parts du cadre, rayon en
    /// largeurs du cadre) ; l'angle balayé du volet en horloge ; le zoom traversant (grossissements de A et B, et son centre).
    pub vertical: bool,
    pub wipe_depuis: crate::transitions::Cote,
    pub iris: Option<[f32; 3]>,
    pub horloge: Option<f32>,
    pub zoom: Option<[f32; 2]>,
    pub zoom_centre: [f32; 2],
    /// Le cadre filmé à l'écran, en points (haut-gauche, haut-droite, bas-droite, bas-gauche) : les glissements, les coupes et
    /// l'objet s'y mesurent.
    pub cadre: [Pos2; 4],
}

impl DisplayMix {
    /// Le point (`x`, `y`) du cadre (en parts de sa largeur et de sa hauteur) à l'écran.
    pub(crate) fn dans_le_cadre(&self, x: f32, y: f32) -> Pos2 {
        let [hg, hd, bd, bg] = self.cadre;
        let haut = hg + (hd - hg) * x;
        let bas = bg + (bd - bg) * x;
        haut + (bas - haut) * y
    }

    /// La largeur du cadre à l'écran, en points, le long de son haut.
    fn largeur(&self) -> egui::Vec2 {
        self.cadre[1] - self.cadre[0]
    }

    /// La hauteur du cadre à l'écran, en points, le long de son bord gauche.
    fn hauteur(&self) -> egui::Vec2 {
        self.cadre[3] - self.cadre[0]
    }
}

/// La couleur `c` (valeurs gamma) dans le réglage d'un aplat de `fs_display` (mode 3) : un seul nombre, exact en flottant.
fn couleur_en_reglage(c: [u8; 3]) -> f32 {
    (c[0] as u32 * 65536 + c[1] as u32 * 256 + c[2] as u32) as f32
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
    /// Calque d'effet d'ajustement (`calqueffet.rs`) : ses effets s'appliquent à l'image composée
    /// jusque-là, le résultat est posé dessus avec l'opacité et le mode de l'élément.
    adjust: Option<Vec<crate::fx::FxStep>>,
    /// La profondeur partagée avec le décor 3D (partie 183) : la texture du décor posé dessous (sa profondeur est à côté d'elle)
    /// et le plan de la carte (`PlanItem::profondeur`).
    prof: Option<(CelId, crate::profondeur::PlanCarte)>,
    /// Les ombres de la carte (partie 185, `ombres.rs`) : le décor posé dessous (ses ombres de cartes sont à côté de lui), si elle
    /// porte une ombre, si elle la reçoit, son plan et sa pente dans la carte d'ombres.
    ombre: Option<OmbreGpu>,
    /// Le décor, assombri sous les cartes qui portent une ombre (partie 185).
    decor_ombre: bool,
}

/// Les ombres d'une carte sur la carte graphique (`GpuItem::ombre`).
#[derive(Clone, Copy, PartialEq, Debug)]
struct OmbreGpu {
    decor: CelId,
    porte: bool,
    recoit: bool,
    plan: crate::profondeur::PlanCarte,
    gradient: [f32; 2],
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
    /// Empreinte des effets d'un calque d'ajustement.
    adjust: Option<u64>,
    /// La profondeur partagée avec le décor 3D : le décor et le plan de la carte (partie 183).
    prof: Option<(CelId, [u32; 3])>,
    /// Les ombres de la carte, et le décor assombri sous les cartes (partie 185).
    ombre: Option<(CelId, bool, bool, [u32; 3], [u32; 2])>,
    decor_ombre: bool,
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
        let adjust = item.adjust.as_ref().map(|steps| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for s in steps {
                std::hash::Hash::hash(&s.kind, &mut h);
                for v in &s.values {
                    std::hash::Hash::hash(&v.to_bits(), &mut h);
                }
            }
            std::hash::Hasher::finish(&h)
        });
        let prof = item.prof.map(|(decor, plan)| (decor, plan.map(f32::to_bits)));
        let ombre = item.ombre.map(|o| (o.decor, o.porte, o.recoit, o.plan.map(f32::to_bits), o.gradient.map(f32::to_bits)));
        ItemKey { cel: item.cel, generation, numbers, blend: item.blend, mesh, adjust, prof, ombre, decor_ombre: item.decor_ombre }
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

/// Le format de la carte d'ombres des cartes (celui du décor).
const OMBRE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Ce que lit le groupe 3 d'une carte sur un décor sans ombres de cartes (partie 185) : rien d'utile, des textures d'un pixel, une
/// lumière à zéro (ses drapeaux disent qu'il n'y a ni soleil ni carte d'ombres), l'échantillonneur des ombres.
struct Vides {
    soleil: wgpu::TextureView,
    lumiere: wgpu::Buffer,
    profondeur: wgpu::TextureView,
    cmp: wgpu::Sampler,
}

impl Vides {
    fn new(device: &wgpu::Device) -> Vides {
        let tex = |format: wgpu::TextureFormat, label: &str| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: extent(1, 1),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        Vides {
            soleil: tex(crate::decor3dgpu::SOLEIL, "ombres : soleil vide"),
            lumiere: device.create_buffer(&wgpu::BufferDescriptor { label: Some("ombres : lumière vide"), size: crate::ombres::OCTETS, usage: wgpu::BufferUsages::UNIFORM, mapped_at_creation: false }),
            profondeur: tex(OMBRE_FORMAT, "ombres : carte vide"),
            cmp: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("ombres"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            }),
        }
    }
}

/// Le groupe 3 des composeurs d'une carte sur un décor (`prof_layout`) : la profondeur du décor `prof`, et, s'il a ses ombres de
/// cartes, son soleil, sa lumière et ses deux cartes d'ombres (rien : des vides).
fn groupe_decor(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, vides: &Vides, prof: &wgpu::TextureView, ombres: Option<(&wgpu::TextureView, &wgpu::Buffer, &wgpu::TextureView, &wgpu::TextureView)>) -> wgpu::BindGroup {
    let (soleil, lumiere, decor, cartes) = ombres.unwrap_or((&vides.soleil, &vides.lumiere, &vides.profondeur, &vides.profondeur));
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("décor 3D : profondeur et ombres"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(prof) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(soleil) },
            wgpu::BindGroupEntry { binding: 2, resource: lumiere.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(decor) },
            wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(cartes) },
            wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&vides.cmp) },
        ],
    })
}

/// Les ombres des cartes d'un décor 3D sur la carte graphique (partie 185, `ombres.rs`) : gardées avec la texture du décor.
struct OmbreDecor {
    /// La part du soleil et le mélange de l'hémisphère à chaque pixel de la feuille (`decor3dgpu::SOLEIL`), dessinés avec le décor.
    soleil: wgpu::Texture,
    /// La lumière du rendu du décor (sa carte d'ombres, l'œil, les couleurs).
    lumiere: crate::ombres::Lumiere,
    /// Ses réglages sur la carte, et le groupe qui les donne à la carte d'ombres des cartes.
    uniform: wgpu::Buffer,
    groupe_lumiere: wgpu::BindGroup,
    /// La carte d'ombres du décor, telle que sa dernière passe l'a dessinée (`decor3dgpu::Moteur::carte_ombres`).
    carte_decor: Option<wgpu::TextureView>,
    /// La carte d'ombres des cartes (son côté), faite à la première carte qui porte une ombre.
    cartes: Option<(u32, wgpu::TextureView)>,
    /// Le groupe 3 des composeurs, refait à chaque composition qui s'en sert.
    groupe: Option<wgpu::BindGroup>,
}

impl OmbreDecor {
    fn bytes(&self) -> u64 {
        let s = self.soleil.size();
        s.width as u64 * s.height as u64 * 2 + self.cartes.as_ref().map_or(0, |(c, _)| *c as u64 * *c as u64 * 4)
    }
}

/// Une image composée posée à l'écran (une, ou deux avec une couche pour une transition).
struct Show {
    /// Coins de l'image à l'écran, en points.
    screen: [[f32; 4]; 4],
    /// Leur w, en perspective.
    ws: Option<[f32; 4]>,
    /// Opacité, mode et réglage de `fs_display` (voir le shader). Mode 4 (flou) : le réglage est donné en points, et passe en
    /// pixels au moment de l'envoi.
    params: [f32; 3],
    /// L'image du second plan d'une transition, au lieu de la première.
    second: bool,
    /// Ce qui se voit de l'image à l'écran, en points (partie 175 : poussée, raccord filé, volet par un objet) ; rien : toute.
    clip: Option<Rect>,
    /// Le réglage de plus des modes 5 et 6 (partie 177) : un point de l'écran et un rayon (en points, passés en pixels à
    /// l'envoi) ou un angle.
    extra: Option<[f32; 4]>,
}

struct CanvasPaint {
    /// Ce qu'on compose, du fond vers le dessus : les éléments du premier plan, puis ceux du second
    /// (à partir de `split`).
    items: Vec<GpuItem>,
    split: usize,
    /// Ce qu'on pose à l'écran, dans l'ordre.
    shows: Vec<Show>,
    /// Poser l'image lissée quand la vue la grossit (`GpuCanvas::lisser`). Réduite, tournée à 100 % ou en perspective, elle se pose
    /// lissée de toute façon (`pose_nette`).
    lisser: bool,
}

/// Les ombres des cartes du plan `span` de `items` (partie 185, `ombres.rs`) : la carte d'ombres de ses cartes qui en portent une,
/// vues du soleil de son décor, là où leur encre est ; les réglages de la lumière ; le groupe 3 de ses composeurs. Rien sans décor
/// qui ait ses ombres de cartes.
fn ombres_des_cartes(encoder: &mut wgpu::CommandEncoder, res: &mut CanvasResources, items: &[GpuItem], ranges: &[Option<std::ops::Range<u32>>], span: std::ops::Range<usize>, device: &wgpu::Device, queue: &wgpu::Queue) {
    let Some(decor) = items[span.clone()].iter().find_map(|i| i.ombre.map(|o| o.decor)) else { return };
    let Some(mut od) = res.textures.get_mut(&decor).and_then(|t| t.ombre.take()) else { return };
    let taille = res.frame.as_ref().map_or([1.0; 2], |f| [f.size.0 as f32, f.size.1 as f32]);
    let lum = od.lumiere;
    let porteurs: Vec<usize> = span.filter(|&i| items[i].ombre.is_some_and(|o| o.porte && o.decor == decor) && items[i].cel.is_some()).collect();
    // La boîte des cartes dans leur carte d'ombres : leurs coins (ou les sommets de leur maillage), relevés sur leur plan.
    let mut points = Vec::new();
    for &i in &porteurs {
        let (item, o) = (&items[i], items[i].ombre.expect("ombre"));
        let ndc: Vec<[f32; 2]> = match &item.mesh {
            Some(m) => m.iter().map(|v| [v[0], v[1]]).collect(),
            None => item.corners.iter().map(|c| [c[0], c[1]]).collect(),
        };
        for p in ndc {
            let (x, y) = ((p[0] + 1.0) * 0.5 * taille[0], (1.0 - p[1]) * 0.5 * taille[1]);
            let inv = crate::profondeur::inverse_distance(&o.plan, x, y).max(1e-6);
            points.push(lum.monde(x, y, inv));
        }
    }
    let boite = if porteurs.is_empty() { None } else { Some(crate::ombres::boite_de(&lum, points).unwrap_or([0.0, 0.0, 1.0, 1.0])) };
    if boite.is_some() {
        let cote = lum.cote.max(1);
        if od.cartes.as_ref().is_none_or(|(c, _)| *c != cote) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("décor 3D : ombres des cartes"),
                size: extent(cote, cote),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: OMBRE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            od.cartes = Some((cote, tex.create_view(&Default::default())));
            od.groupe = None;
        }
    }
    queue.write_buffer(&od.uniform, 0, &lum.octets(taille, boite, od.carte_decor.is_some()));
    if let (Some(_), Some((_, vue))) = (boite, od.cartes.as_ref()) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ombres des cartes"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: vue, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(2, &od.groupe_lumiere, &[]);
        for &i in &porteurs {
            let Some(t) = items[i].cel.and_then(|id| res.textures.get(&id)) else { continue };
            pass.set_bind_group(0, &res.uniform_group, &[(i as u64 * res.stride) as u32]);
            pass.set_bind_group(1, &t.bind_group, &[]);
            match (&items[i].mesh, ranges[i].clone()) {
                (Some(_), Some(range)) => {
                    let Some((buffer, _)) = &res.mesh_vertices else { continue };
                    pass.set_pipeline(&res.ombre_mesh);
                    pass.set_vertex_buffer(0, buffer.slice(..));
                    pass.draw(range, 0..1);
                }
                (Some(_), None) => continue,
                (None, _) => {
                    pass.set_pipeline(&res.ombre_quad);
                    pass.draw(0..6, 0..1);
                }
            }
        }
    }
    if od.groupe.is_none() {
        if let Some((prof, _)) = res.textures.get(&decor).and_then(|t| t.prof.as_ref()) {
            let prof = prof.create_view(&Default::default());
            let soleil = od.soleil.create_view(&Default::default());
            let carte_decor = od.carte_decor.clone().unwrap_or_else(|| res.vides.profondeur.clone());
            let cartes = od.cartes.as_ref().map_or_else(|| res.vides.profondeur.clone(), |c| c.1.clone());
            od.groupe = Some(groupe_decor(device, &res.prof_layout, &res.vides, &prof, Some((&soleil, &od.uniform, &carte_decor, &cartes))));
        }
    }
    if let Some(t) = res.textures.get_mut(&decor) {
        t.ombre = Some(od);
    }
}

/// Compose les éléments `span` de `items` dans `frame` (effacée d'abord). Un calque en mode de
/// fusion lit ce qui est déjà composé : on le copie à part avant (fin de passe, copie, nouvelle
/// passe).
fn compose_span(
    encoder: &mut wgpu::CommandEncoder,
    res: &CanvasResources,
    frame: &Frame,
    items: &[GpuItem],
    ranges: &[Option<std::ops::Range<u32>>],
    span: std::ops::Range<usize>,
    fx: &mut Option<crate::gpufx::GpuFx>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> u64 {
    let stride = res.stride as usize;
    let mut passes = 0u64;
    let mut pass = compose_pass(encoder, &frame.target, true);
    for i in span {
        let item = &items[i];
        // Calque d'effet d'ajustement : ses effets passent sur l'image composée jusqu'ici (il faut finir
        // la passe de composition pour la lire), le résultat est posé comme un dessin plein cadre.
        let adjusted;
        let mut leased = None;
        let group = if let Some(steps) = &item.adjust {
            let Some(fx) = fx.as_mut() else { continue };
            let sheet = frame.size;
            let input = crate::gpufx::Geo { x: 0, y: 0, w: sheet.0 as i32, h: sheet.1 as i32 };
            let Some(chain) = crate::gpufx::plan_chain(steps, input, sheet) else { continue };
            drop(pass);
            let source = frame.texture.create_view(&Default::default());
            leased = fx.run(device, queue, encoder, &chain, &source, None, None);
            passes += chain.passes.len() as u64;
            pass = compose_pass(encoder, &frame.target, false);
            let Some(l) = leased else { continue };
            adjusted = texture_group(device, &res.texture_layout, fx.view(l), &res.nearest);
            &adjusted
        } else {
            match item.cel {
                None => &res.white,
                Some(id) => match res.textures.get(&id) {
                    Some(t) => &t.bind_group,
                    None => continue,
                },
            }
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
        // Une carte qui partage la profondeur du décor 3D (partie 183) : les mêmes, qui lisent aussi la profondeur du décor ; et une
        // carte qui reçoit les ombres, ou le décor sous les cartes qui en portent (partie 185) : le groupe de ses ombres.
        let decor_de = item.prof.map(|(d, _)| d).or(item.ombre.filter(|o| o.recoit).map(|o| o.decor)).or(if item.decor_ombre { item.cel } else { None });
        let groupe3 = decor_de.and_then(|d| res.textures.get(&d)).and_then(|t| t.ombre.as_ref().and_then(|o| o.groupe.as_ref()).or(t.prof.as_ref().map(|p| &p.1)));
        let assombri = item.decor_ombre && groupe3.is_some() && !meshed && item.blend == 0;
        let prof = groupe3.filter(|_| !item.decor_ombre || assombri);
        if let Some(group) = prof {
            if item.blend == 0 {
                pass.set_bind_group(2, &frame.backdrop_group, &[]);
            }
            pass.set_bind_group(3, group, &[]);
        }
        pass.set_pipeline(match (item.blend != 0, meshed, prof.is_some()) {
            _ if assombri => &res.decor_o,
            (false, false, false) => &res.compose,
            (false, true, false) => &res.compose_mesh,
            (true, false, false) => &res.blend,
            (true, true, false) => &res.blend_mesh,
            (false, false, true) => &res.compose_p,
            (false, true, true) => &res.compose_mesh_p,
            (true, false, true) => &res.blend_p,
            (true, true, true) => &res.blend_mesh_p,
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
        // (La texture de travail du résultat est rendue : la passe ci-dessus l'a déjà notée, un calque
        // d'ajustement plus haut peut la réécrire dans une passe suivante.)
        if let (Some(l), Some(fx)) = (leased, fx.as_mut()) {
            fx.release(l);
        }
    }
    passes
}

fn put_item(bytes: &mut [u8], corners: &[[f32; 4]; 4], color: &[f32; 4], params: &[f32; 4]) {
    put_item_extra(bytes, corners, color, params, &[0.0; 4]);
}

fn put_floats(bytes: &mut [u8], values: &[f32]) {
    for (k, v) in values.iter().enumerate() {
        bytes[k * 4..k * 4 + 4].copy_from_slice(&v.to_ne_bytes());
    }
}

fn put_item_extra(bytes: &mut [u8], corners: &[[f32; 4]; 4], color: &[f32; 4], params: &[f32; 4], extra: &[f32; 4]) {
    for (k, v) in corners.iter().flatten().chain(color).chain(params).chain(extra).enumerate() {
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
        // Le décor 3D (partie 180) : redessiné dans sa texture, de l'œil de la caméra, avant la composition qui la lit.
        let jobs = resources.get_mut::<CanvasResources>().map(|r| std::mem::take(&mut r.decor_jobs)).unwrap_or_default();
        if !jobs.is_empty() {
            let mut moteur = resources.remove::<crate::decor3dgpu::Moteur>().unwrap_or_else(|| crate::decor3dgpu::Moteur::new(device, queue));
            if let Some(res) = resources.get_mut::<CanvasResources>() {
                for j in jobs {
                    let Some((view, prof, soleil)) = res.textures.get(&j.out).and_then(|t| Some((t.texture.create_view(&Default::default()), t.prof.as_ref()?.0.create_view(&Default::default()), t.ombre.as_ref().map(|o| o.soleil.create_view(&Default::default()))))) else { continue };
                    moteur.dessiner_feuille(device, queue, encoder, j.out, &j.dessins, &j.prise, j.taille, &view, &prof, soleil.as_ref(), j.ciel, j.qualite);
                    if let Some(t) = res.textures.get_mut(&j.out) {
                        if let Some(state) = t.fx.as_mut() {
                            state.done = j.sig;
                        }
                        // (Les ombres des cartes lisent la carte d'ombres que le décor vient de dessiner, partie 185.)
                        if let Some(o) = t.ombre.as_mut() {
                            o.carte_decor = moteur.carte_ombres(j.out).map(|(_, v)| v);
                            o.groupe = None;
                        }
                    }
                }
            }
            resources.insert(moteur);
        }
        let Some(res) = resources.get_mut::<CanvasResources>() else { return Vec::new() };
        // Effets de calque sur la carte graphique (`gpufx.rs`) : les textures de résultat dont la
        // signature a changé sont redessinées, avant la composition qui les lit.
        if let Some(mut fx) = res.gpufx.take() {
            fx.begin_frame();
            for job in std::mem::take(&mut res.fx_jobs) {
                let (Some(src), Some(out)) = (res.textures.get(&job.src), res.textures.get(&job.out)) else { continue };
                let (src_view, out_view) = (src.texture.create_view(&Default::default()), out.texture.create_view(&Default::default()));
                fx.run(device, queue, encoder, &job.chain, &src_view, Some(&out_view), job.bordure.as_ref());
                res.fx_passes += job.chain.passes.len() as u64;
                if let Some(state) = res.textures.get_mut(&job.out).and_then(|t| t.fx.as_mut()) {
                    state.done = job.sig;
                }
            }
            res.gpufx = Some(fx);
        }
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
            // (Le plan de la carte, après le réglage de plus : la profondeur partagée avec le décor 3D, partie 183 ; et ses drapeaux :
            // 1, cachée par le décor ; 2, elle reçoit les ombres, sa pente dans la carte d'ombres au réglage de plus, partie 185.)
            let drapeaux = if item.prof.is_some() { 1.0 } else { 0.0 } + if item.ombre.is_some_and(|o| o.recoit) { 2.0 } else { 0.0 };
            if let Some(plan) = item.prof.map(|p| p.1).or(item.ombre.map(|o| o.plan)) {
                put_floats(&mut bytes[i * stride + 112..], &[plan[0], plan[1], plan[2], drapeaux]);
            }
            if let Some(o) = item.ombre {
                put_floats(&mut bytes[i * stride + 96..], &[o.gradient[0], o.gradient[1], 0.0, 0.0]);
            }
        }
        let [w, h] = screen.size_in_pixels.map(|v| v.max(1) as f32);
        let ppp = screen.pixels_per_point;
        for (k, show) in self.shows.iter().enumerate() {
            let on_screen = show.screen.map(|c| [c[0] * ppp / w * 2.0 - 1.0, 1.0 - c[1] * ppp / h * 2.0, c[2], c[3]]);
            let (ws, persp) = match show.ws {
                Some(w) => (w, 1.0),
                None => ([0.0; 4], 0.0),
            };
            // (Le flou d'un raccord filé, en points, passe en pixels d'écran : le shader le lit le long de ses pixels.)
            let reglage = if show.params[1] > 3.5 { show.params[2] * ppp } else { show.params[2] };
            // (Le point et le rayon des modes 5 et 6, en points, passent en pixels de l'écran ; l'angle du mode 6 reste.)
            let extra = match show.extra {
                Some(e) if show.params[1] > 5.5 => [e[0] * ppp, e[1] * ppp, e[2], e[3]],
                Some(e) => [e[0] * ppp, e[1] * ppp, e[2] * ppp, e[3]],
                None => [0.0; 4],
            };
            put_item_extra(&mut bytes[(self.items.len() + k) * stride..], &on_screen, &ws, &[show.params[0], show.params[1], reglage, persp], &extra);
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
        // (Les calques d'ajustement ont besoin des effets : on les sort le temps de la composition.)
        let mut fx = res.gpufx.take();
        let mut passes = 0;
        // Les ombres des cartes (partie 185) : la carte d'ombres des cartes de chaque plan, avant sa composition.
        if fresh1 {
            ombres_des_cartes(encoder, res, &self.items, &ranges, 0..split, device, queue);
        }
        if fresh2 {
            ombres_des_cartes(encoder, res, &self.items, &ranges, split..self.items.len(), device, queue);
        }
        if fresh1 {
            if let Some(frame) = &res.frame {
                passes += compose_span(encoder, res, frame, &self.items, &ranges, 0..split, &mut fx, device, queue);
                res.composes += 1;
            }
        }
        if fresh2 {
            if let Some(frame) = &res.frame2 {
                passes += compose_span(encoder, res, frame, &self.items, &ranges, split..self.items.len(), &mut fx, device, queue);
                res.composes += 1;
            }
        }
        res.gpufx = fx;
        res.fx_passes += passes;
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
        // La zone d'affichage, en pixels : la coupe d'une image (partie 175) ne la dépasse jamais, et elle revient après.
        let base = info.clip_rect_in_pixels();
        let (bx0, by0) = (base.left_px.max(0), base.top_px.max(0));
        let (bx1, by1) = ((base.left_px + base.width_px).min(w as i32), (base.top_px + base.height_px).min(h as i32));
        for (k, show) in self.shows.iter().enumerate() {
            let Some(frame) = (if show.second { &res.frame2 } else { &res.frame }) else { continue };
            if let Some(c) = show.clip {
                let ppp = info.pixels_per_point;
                let x0 = ((c.left() * ppp).round().clamp(-1.0e9, 1.0e9) as i32).max(bx0);
                let y0 = ((c.top() * ppp).round().clamp(-1.0e9, 1.0e9) as i32).max(by0);
                let x1 = ((c.right() * ppp).round().clamp(-1.0e9, 1.0e9) as i32).min(bx1);
                let y1 = ((c.bottom() * ppp).round().clamp(-1.0e9, 1.0e9) as i32).min(by1);
                if x1 <= x0 || y1 <= y0 {
                    continue;
                }
                pass.set_scissor_rect(x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32);
            }
            pass.set_bind_group(0, &res.uniform_group, &[((self.items.len() + k) as u64 * res.stride) as u32]);
            let lisse = self.lisser || !pose_nette(&show.screen, show.ws.is_some(), info.pixels_per_point, frame.size);
            pass.set_bind_group(1, if lisse { &frame.display_group_lisse } else { &frame.display_group }, &[]);
            pass.draw(0..6, 0..1);
            if show.clip.is_some() && bx1 > bx0 && by1 > by0 {
                pass.set_scissor_rect(bx0 as u32, by0 as u32, (bx1 - bx0) as u32, (by1 - by0) as u32);
            }
        }
    }
}

/// L'image composée se pose-t-elle en **pixels nets** (l'échantillonneur de l'affichage, qui ne lisse pas en grossissant) ? Seulement
/// si la vue ne la réduit pas, et si elle est droite ou la grossit. Réduite, `fs_display` moyenne ce que couvre chaque pixel d'écran,
/// et ses lectures doivent être lissées pour que tous les pixels de l'image comptent ; tournée à 100 %, ou en perspective, elle se pose
/// lissée plutôt qu'en marches (partie 166.9). `coins` : les coins de l'image à l'écran, en points (haut-gauche, haut-droite,
/// bas-droite, bas-gauche) ; `taille` : celle de l'image, en pixels.
pub(crate) fn pose_nette(coins: &[[f32; 4]; 4], perspective: bool, ppp: f32, taille: (u32, u32)) -> bool {
    if perspective {
        return false;
    }
    let (haut, gauche) = ([coins[1][0] - coins[0][0], coins[1][1] - coins[0][1]], [coins[3][0] - coins[0][0], coins[3][1] - coins[0][1]]);
    let longueur = |v: [f32; 2]| v[0].hypot(v[1]);
    let (lh, lg) = (longueur(haut), longueur(gauche));
    // Pixels d'écran par pixel de l'image.
    let echelle = (lh * ppp / taille.0.max(1) as f32).min(lg * ppp / taille.1.max(1) as f32);
    let droite = (haut[1].abs() <= 1e-3 * lh && gauche[0].abs() <= 1e-3 * lg) || (haut[0].abs() <= 1e-3 * lh && gauche[1].abs() <= 1e-3 * lg);
    echelle >= 0.99 && (droite || echelle > 1.02)
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
    // (L'échantillonneur « composer » est bilinéaire dans les deux sens : c'est celui qu'il faut pour lisser en grossissant, et pour
    // les lectures de `fs_display` quand la vue réduit l'image.)
    let display_group_lisse = texture_group(device, texture_layout, &srgb, nearest);
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
    Frame { size, texture, target, display_group, display_group_lisse, backdrop, backdrop_group, composed: None }
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

/// Les effets de cet élément du plan se dessinent-ils sur la carte graphique ? Oui si toute sa pile y
/// passe (`gpufx::steps_supported`), ou si elle mêle des effets du trait (calcul de fond) à des effets de
/// la carte (`fx::hybrid_split` : la carte passe ceux-là sur le dessin dérivé par ceux-ci), et s'il
/// s'agit d'un dessin ordinaire du projet, à la taille de la feuille (ni remplissage au griffonnage, ni fond sous
/// le trait, ni feuille flottante, ni image de vidéo).
pub(crate) fn gpu_fx_item(doc: &Document, item: &crate::render::PlanItem) -> bool {
    item.ctg.is_none()
        && item.fond.is_none()
        && item.pre.is_none()
        && (crate::gpufx::steps_supported(&item.fx) || crate::fx::hybrid_split(item).is_some())
        // Un dessin qui passe le bord de la feuille, avec un effet qui va lire ses voisins : la carte ne voit pas ce qui arrive de l'autre côté, le calcul de fond si (partie 121).
        // Mais tant que ce calcul n'a pas répondu, c'est la carte qui dessine : l'effet suit le trait sans délai, seule la bande près du bord n'a pas encore tout lu.
        && doc.cels.get(&item.cel).is_some_and(|c| {
            c.raster.width == doc.width && c.raster.height == doc.height && (crate::fx::pile_juste_au_bord(&item.fx) || !crate::fx::cel_deborde(c) || crate::fx::resultat_pret(doc, item).is_none())
        })
}

/// La texture résultat des effets de l'élément `item` (dessin d'origine déjà envoyé sous son numéro) :
/// créée ou reprise, à la taille de ce que les effets touchent ; la chaîne qui la remplit est mise dans
/// les travaux de la prochaine composition si elle n'est pas à jour. Rend son numéro, ou rien (pile sans
/// passe : le dessin d'origine est montré tel quel).
#[allow(clippy::too_many_arguments)]
fn place_fx(rs: &RenderState, res: &mut CanvasResources, item: &crate::render::PlanItem, lu: Option<&crate::document::Cel>, nth: usize, sheet: (u32, u32), tick: u64, bytes: &mut u64) -> Option<CelId> {
    let src = item.cel;
    let (rect, generation) = res.textures.get(&src).filter(|t| t.dims == sheet).map(|t| (t.rect, t.generation))?;
    let out = crate::gpufx::out_id(src, nth, &item.fx);
    // L'encre autour de la feuille, pour un tremblement en tête de pile.
    let bord = match (crate::gpufx::Bordure::largeur_voulue(&item.fx), lu) {
        (Some(m), Some(cel)) => bordure_de(rs, res, out, src, cel, m, tick, bytes),
        _ => None,
    };
    let chain = crate::gpufx::plan_chain_bord(&item.fx, crate::gpufx::Geo::from_rect(rect), sheet, bord.as_ref().map(|b| b.0))?;
    // (Une encre qui ne passe que hors de la feuille ne change pas la texture du dessin : son empreinte compte à part.)
    let sig = crate::gpufx::signature(&item.fx, rect, generation, sheet) ^ bord.as_ref().map_or(0, |b| b.2 << 1);
    let want = chain.out.rect();
    if !res.textures.get(&out).is_some_and(|t| t.fx.is_some() && t.rect == want) {
        if let Some(old) = res.textures.remove(&out) {
            *bytes -= old.bytes();
        }
        let texture = crate::gpufx::target_texture(&rs.device, want[2], want[3]);
        let view = texture.create_view(&Default::default());
        let bind_group = texture_group(&rs.device, &res.texture_layout, &view, &res.nearest);
        let entry = CelTexture {
            bind_group,
            texture,
            rect: want,
            dims: sheet,
            version: 0,
            last_used: tick,
            generation: tick,
            filled: Vec::new(),
            partial: None,
            fx: Some(FxState::default()),
            prof: None,
            ombre: None,
        };
        *bytes += entry.bytes();
        res.textures.insert(out, entry);
    }
    let entry = res.textures.get_mut(&out).expect("texture d'effets présente");
    entry.last_used = tick;
    let state = entry.fx.as_mut().expect("texture d'effets");
    if state.wanted != sig {
        state.wanted = sig;
        // La texture va changer : l'image composée gardée n'est plus bonne.
        entry.generation = tick;
    }
    if entry.fx.is_some_and(|s| s.done != sig) {
        res.fx_jobs.retain(|j| j.out != out);
        res.fx_jobs.push(FxJob { src, out, chain: std::sync::Arc::new(chain), sig, bordure: bord.map(|b| b.1) });
    }
    Some(out)
}

/// La texture du décor 3D de la clé `key` (`decor3d.rs`) : une par décor, de la taille de la feuille, redessinée par la carte
/// quand la clé change (la caméra ou un objet a bougé) — jamais remplie par le processeur. Rien : le décor a changé depuis la
/// clé, ou n'est plus dans le projet.
#[allow(clippy::too_many_arguments)]
fn place_decor(rs: &RenderState, res: &mut CanvasResources, doc: &Document, key: &crate::decor3d::DecorCle, sheet: (u32, u32), tick: u64, bytes: &mut u64) -> Option<CelId> {
    let objets = crate::decor3d::objets_de(doc, key)?;
    let aspect = key.aspect;
    let prise = key.prise()?;
    let out = crate::decor3d::id_texture(key.decor);
    let qualite = if aspect.ombres { crate::decor3d::qualite(crate::decor3d::triangles_de(&objets)) } else { crate::decor3dbudget::Qualite::Sans };
    let sig = crate::decor3d::id_of(key) ^ (qualite as u64) << 1;
    let want = [0, 0, sheet.0, sheet.1];
    if !res.textures.get(&out).is_some_and(|t| t.fx.is_some() && t.prof.is_some() && t.rect == want) {
        if let Some(old) = res.textures.remove(&out) {
            *bytes -= old.bytes();
        }
        let texture = crate::gpufx::target_texture(&rs.device, sheet.0, sheet.1);
        let view = texture.create_view(&Default::default());
        let bind_group = texture_group(&rs.device, &res.texture_layout, &view, &res.nearest);
        // (Sa profondeur, dessinée avec lui : les cartes s'y cachent, partie 183.)
        let prof = prof_texture(&rs.device, &res.prof_layout, &res.vides, sheet);
        let entry = CelTexture { bind_group, texture, rect: want, dims: sheet, version: 0, last_used: tick, generation: tick, filled: Vec::new(), partial: None, fx: Some(FxState::default()), prof: Some(prof), ombre: None };
        *bytes += entry.bytes();
        res.textures.insert(out, entry);
    }
    // Les ombres des cartes (partie 185, `ombres.rs`) : seulement si le décor a ses ombres sur cet ordinateur ; sa lumière, la part du
    // soleil dessinée avec lui (une troisième cible), et plus tard la carte d'ombres des cartes.
    let lumiere = if aspect.ombres && qualite != crate::decor3dbudget::Qualite::Sans {
        let ec = crate::decor3dgpu::eclairage(&crate::decor3d::dessins_gpu(&objets, aspect), prise.oeil, &aspect, qualite);
        crate::ombres::Lumiere::de(&ec, &aspect, &prise)
    } else {
        None
    };
    let lumiere_layout = res.lumiere_layout.clone();
    let entry = res.textures.get_mut(&out).expect("texture du décor présente");
    let avant = entry.bytes();
    match lumiere {
        Some(lum) => match entry.ombre.as_mut() {
            Some(o) if o.soleil.size().width == sheet.0 && o.soleil.size().height == sheet.1 => {
                if o.lumiere != lum {
                    o.lumiere = lum;
                    o.groupe = None;
                }
            }
            _ => {
                let soleil = rs.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("décor 3D : soleil"),
                    size: extent(sheet.0, sheet.1),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: crate::decor3dgpu::SOLEIL,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let uniform = rs.device.create_buffer(&wgpu::BufferDescriptor { label: Some("décor 3D : lumière"), size: crate::ombres::OCTETS, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
                let groupe_lumiere = rs.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("décor 3D : lumière"), layout: &lumiere_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }] });
                entry.ombre = Some(Box::new(OmbreDecor { soleil, lumiere: lum, uniform, groupe_lumiere, carte_decor: None, cartes: None, groupe: None }));
                // (La part du soleil est à dessiner avec le décor : le rendu d'avant ne l'a pas.)
                if let Some(state) = entry.fx.as_mut() {
                    state.done = 0;
                }
            }
        },
        None => entry.ombre = None,
    }
    *bytes = *bytes - avant + entry.bytes();
    entry.last_used = tick;
    let state = entry.fx.as_mut().expect("texture du décor");
    if state.wanted != sig {
        state.wanted = sig;
        // La texture va changer : l'image composée gardée n'est plus bonne.
        entry.generation = tick;
    }
    if entry.fx.is_some_and(|s| s.done != sig) {
        res.decor_jobs.retain(|j| j.out != out);
        let dessins = crate::decor3d::dessins_gpu(&objets, aspect);
        res.decor_jobs.push(DecorJob { out, dessins, ciel: aspect.ciel, prise, taille: [sheet.0, sheet.1], qualite, sig });
    }
    Some(out)
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
    if *bytes + new_bytes > res.limit() {
        let oldest = res
            .textures
            .iter()
            .filter(|(_, t)| t.rect[2] == rect[2] && t.rect[3] == rect[3] && t.last_used < tick && t.fx.is_none())
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
        CelTexture { bind_group, texture, rect, dims, version: u64::MAX, last_used: tick, generation: tick, filled: vec![false; tiles], partial: None, fx: None, prof: None, ombre: None }
    });
    *bytes += entry.bytes();
    res.textures.insert(id, entry);
}

/// La texture de la profondeur du décor 3D (partie 183, `decor3dgpu::PROF`), de la taille de la feuille, et ce que la composition
/// des cartes en lit.
fn prof_texture(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, vides: &Vides, size: (u32, u32)) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("décor 3D : profondeur"),
        size: extent(size.0, size.1),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::decor3dgpu::PROF,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    // (Sans ombres de cartes : le reste du groupe est vide, partie 185.)
    let group = groupe_decor(device, layout, vides, &view, None);
    (texture, group)
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
