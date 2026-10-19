// Effets de calque sur la carte graphique (gpufx.rs). Chaque point d'entrée « fs_… » est UNE passe :
// elle lit la texture A (et parfois la B), écrit un rectangle de la feuille, et reproduit au
// niveau près le calcul sur processeur correspondant (fx.rs, fxfilters.rs, fxcine.rs) : mêmes
// formules, mêmes arrondis (les textures de travail sont en 8 bits, comme les dessins), même
// hasard. Les couleurs sont des valeurs gamma prémultipliées, comme les dessins.
//
// Géométrie : chaque texture montre un rectangle (x, y, largeur, hauteur) de son espace (la
// feuille, ou la feuille réduite pour la lueur diffuse). Hors de ce rectangle, une lecture rend du
// transparent : c'est ce que le calcul sur processeur fait hors de la zone dessinée.

struct P {
    // Rectangles de A, de B et de la cible.
    a: vec4<i32>,
    b: vec4<i32>,
    d: vec4<i32>,
    // Taille de la feuille (x, y), puis deux entiers libres.
    s: vec4<i32>,
    // Entiers de l'effet.
    n: vec4<i32>,
    // Réels de l'effet.
    f: array<vec4<f32>, 8>,
};
@group(0) @binding(0) var<uniform> p: P;
@group(1) @binding(0) var ta: texture_2d<f32>;
@group(2) @binding(0) var tb: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var pts = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(pts[i], 0.0, 1.0);
}

// Le pixel de la cible, dans l'espace des textures.
fn dst_px(pos: vec4<f32>) -> vec2<i32> {
    return vec2<i32>(floor(pos.xy)) + p.d.xy;
}

fn lda(sp: vec2<i32>) -> vec4<f32> {
    let q = sp - p.a.xy;
    if (q.x < 0 || q.y < 0 || q.x >= p.a.z || q.y >= p.a.w) {
        return vec4<f32>(0.0);
    }
    return textureLoad(ta, q, 0);
}

fn ldb(sp: vec2<i32>) -> vec4<f32> {
    let q = sp - p.b.xy;
    if (q.x < 0 || q.y < 0 || q.x >= p.b.z || q.y >= p.b.w) {
        return vec4<f32>(0.0);
    }
    return textureLoad(tb, q, 0);
}

// Lecture bilinéaire (comme `fxcine::Flat::sample`) du point (x, y), en pixels dont les centres sont
// à +0,5 ; transparent hors de la texture. Rend des valeurs de 0 à 255.
fn bil_a(x: f32, y: f32) -> vec4<f32> {
    let fx = x - 0.5;
    let fy = y - 0.5;
    let ix = floor(fx);
    let iy = floor(fy);
    let tx = fx - ix;
    let ty = fy - iy;
    let i = vec2<i32>(i32(ix), i32(iy));
    let c00 = lda(i);
    let c10 = lda(i + vec2<i32>(1, 0));
    let c01 = lda(i + vec2<i32>(0, 1));
    let c11 = lda(i + vec2<i32>(1, 1));
    let top = c00 + (c10 - c00) * tx;
    let bottom = c01 + (c11 - c01) * tx;
    return (top + (bottom - top) * ty) * 255.0;
}

fn bil_b(x: f32, y: f32) -> vec4<f32> {
    let fx = x - 0.5;
    let fy = y - 0.5;
    let ix = floor(fx);
    let iy = floor(fy);
    let tx = fx - ix;
    let ty = fy - iy;
    let i = vec2<i32>(i32(ix), i32(iy));
    let c00 = ldb(i);
    let c10 = ldb(i + vec2<i32>(1, 0));
    let c01 = ldb(i + vec2<i32>(0, 1));
    let c11 = ldb(i + vec2<i32>(1, 1));
    let top = c00 + (c10 - c00) * tx;
    let bottom = c01 + (c11 - c01) * tx;
    return (top + (bottom - top) * ty) * 255.0;
}

// Couleur sans prémultiplication (rgb) et couverture (a) d'un pixel ; rien si transparent.
fn unpm(t: vec4<f32>) -> vec4<f32> {
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(t.rgb / t.a, t.a);
}

// Le pixel de couleur c (0 à 1) et de couverture a, prémultiplié.
fn prem(c: vec3<f32>, a: f32) -> vec4<f32> {
    let aa = clamp(a, 0.0, 1.0);
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)) * aa, aa);
}

fn luma(c: vec3<f32>) -> f32 {
    return 0.299 * c.r + 0.587 * c.g + 0.114 * c.b;
}

fn smooth_step(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / max(e1 - e0, 1e-6), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn round255(v: vec4<f32>) -> vec4<f32> {
    return floor(v * 255.0 + vec4<f32>(0.5));
}

// ---------------------------------------------------------------------- réglages de couleur

// Luminosité / contraste : f[0] = (k, b).
@fragment
fn fs_bc(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let t = lda(dst_px(pos));
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let c = t.rgb / t.a;
    return prem((c - vec3<f32>(0.5)) * p.f[0].x + vec3<f32>(0.5 + p.f[0].y), t.a);
}

// Niveaux : f[0] = (noir sur 255, étendue, exposant).
@fragment
fn fs_levels(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let t = lda(dst_px(pos));
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let c = t.rgb / t.a;
    let v = clamp((c * 255.0 - vec3<f32>(p.f[0].x)) / p.f[0].y, vec3<f32>(0.0), vec3<f32>(1.0));
    return prem(pow(v, vec3<f32>(p.f[0].z)), t.a);
}

fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let mx = max(c.r, max(c.g, c.b));
    let mn = min(c.r, min(c.g, c.b));
    let l = (mx + mn) / 2.0;
    if (mx - mn < 1e-6) {
        return vec3<f32>(0.0, 0.0, l);
    }
    let d = mx - mn;
    var s = d / (mx + mn);
    if (l > 0.5) {
        s = d / (2.0 - mx - mn);
    }
    var h = 0.0;
    if (mx == c.r) {
        h = (c.g - c.b) / d;
        if (c.g < c.b) {
            h += 6.0;
        }
    } else if (mx == c.g) {
        h = (c.b - c.r) / d + 2.0;
    } else {
        h = (c.r - c.g) / d + 4.0;
    }
    return vec3<f32>(h / 6.0, s, l);
}

fn hsl_ch(q0: f32, q1: f32, t0: f32) -> f32 {
    let t = t0 - floor(t0);
    if (t < 1.0 / 6.0) {
        return q0 + (q1 - q0) * 6.0 * t;
    }
    if (t < 0.5) {
        return q1;
    }
    if (t < 2.0 / 3.0) {
        return q0 + (q1 - q0) * (2.0 / 3.0 - t) * 6.0;
    }
    return q0;
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> vec3<f32> {
    if (s <= 0.0) {
        return vec3<f32>(l);
    }
    var q1 = l + s - l * s;
    if (l < 0.5) {
        q1 = l * (1.0 + s);
    }
    let q0 = 2.0 * l - q1;
    return vec3<f32>(hsl_ch(q0, q1, h + 1.0 / 3.0), hsl_ch(q0, q1, h), hsl_ch(q0, q1, h - 1.0 / 3.0));
}

// Teinte / saturation / luminosité : f[0] = (teinte en tours, facteur de saturation, luminosité).
@fragment
fn fs_hue(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let t = lda(dst_px(pos));
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let hsl = rgb_to_hsl(t.rgb / t.a);
    var h = hsl.x + p.f[0].x;
    h = h - floor(h);
    let s = clamp(hsl.y * p.f[0].y, 0.0, 1.0);
    var l = hsl.z * (1.0 + p.f[0].z);
    if (p.f[0].z >= 0.0) {
        l = hsl.z + (1.0 - hsl.z) * p.f[0].z;
    }
    return prem(hsl_to_rgb(h, s, l), t.a);
}

// Noir et blanc : f[0].x = part de luminance.
@fragment
fn fs_bw(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let t = lda(dst_px(pos));
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let c = t.rgb / t.a;
    let y = luma(c);
    return prem(c + (vec3<f32>(y) - c) * p.f[0].x, t.a);
}

// Négatif : f[0].x = part.
@fragment
fn fs_invert(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let t = lda(dst_px(pos));
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let c = t.rgb / t.a;
    return prem(c + (vec3<f32>(1.0) - 2.0 * c) * p.f[0].x, t.a);
}

// Postérisation : f[0].x = nombre de niveaux moins un.
@fragment
fn fs_poster(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let t = lda(dst_px(pos));
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let c = t.rgb / t.a;
    let n = p.f[0].x;
    return prem(floor(c * n + vec3<f32>(0.5)) / n, t.a);
}

// Seuil : f[0].x = niveau (0 à 255 de luminance).
@fragment
fn fs_thresh(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let t = lda(dst_px(pos));
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let c = t.rgb / t.a;
    var v = 0.0;
    if (luma(c) * 255.0 >= p.f[0].x) {
        v = 1.0;
    }
    return prem(vec3<f32>(v), t.a);
}

// ---------------------------------------------------------------------- hasard

// Le même hasard que `fxfilters::noise_at`.
fn noise_at(x: u32, y: u32) -> f32 {
    var h = (x * 0x9E3779B1u) ^ (y * 0x85EBCA77u);
    h = h ^ (h >> 15u);
    h = h * 0x2C1B3C6Du;
    h = h ^ (h >> 12u);
    return f32(h & 0xFFFFu) / 32767.5 - 1.0;
}

// Le même hasard que `fxcine::hash01`.
fn hash01(a: u32, b: u32, c: u32) -> f32 {
    var h = (a * 0x9E3779B1u) ^ (b * 0x85EBCA77u) ^ (c * 0xC2B2AE3Du);
    h = h ^ (h >> 15u);
    h = h * 0x2C1B3C6Du;
    h = h ^ (h >> 12u);
    h = h * 0x297A2D39u;
    h = h ^ (h >> 15u);
    return f32(h >> 8u) / 16777216.0;
}

// Bruit : f[0].x = amplitude (quantité / 100 × 0,5).
@fragment
fn fs_noise(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let t = lda(sp);
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let c = t.rgb / t.a;
    let n = noise_at(u32(sp.x), u32(sp.y));
    return prem(c + vec3<f32>(n * p.f[0].x), t.a);
}

// Grain de film : f[0].x = amplitude ; n = (grain en pixels, graine, image).
@fragment
fn fs_grain(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let t = lda(sp);
    if (t.a <= 0.0) {
        return t;
    }
    let cell = u32(p.n.x);
    let seed = u32(p.n.y);
    let frame = u32(p.n.z);
    let n = hash01((u32(sp.x) / cell) ^ (seed << 16u), u32(sp.y) / cell, frame * 7919u + seed) * 2.0 - 1.0;
    let c = t.rgb / t.a;
    let u = abs(2.0 * luma(c) - 1.0);
    let mid = 1.0 - u * u * 0.6;
    return prem(c + vec3<f32>(n * p.f[0].x * mid), t.a);
}

// ---------------------------------------------------------------------- effets « Cinéma »

// Flash : f[0] = (quantité, couleur rouge, vert, bleu), n.x = 1 en écran.
@fragment
fn fs_flash(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let t = lda(dst_px(pos));
    let a = p.f[0].x;
    let col = p.f[0].yzw;
    if (p.n.x != 0) {
        let u = unpm(t);
        let out_c = vec3<f32>(1.0) - (vec3<f32>(1.0) - u.rgb) * (vec3<f32>(1.0) - col * a);
        let cov2 = u.a + a * (1.0 - u.a);
        var base = col;
        if (u.a > 0.0) {
            base = out_c;
        }
        return prem(base, cov2);
    }
    let k = 1.0 - a;
    let t8 = round255(t);
    let ch = floor(t8.rgb * k + col * 255.0 * a + vec3<f32>(0.5));
    let alpha = floor(clamp(t8.a * k + 255.0 * a + 0.5, 0.0, 255.0));
    let out8 = min(clamp(ch, vec3<f32>(0.0), vec3<f32>(255.0)), vec3<f32>(alpha));
    return vec4<f32>(out8, alpha) / 255.0;
}

fn burst_mask(px: f32, py: f32, dist: f32, half_side: f32, seed: u32) -> f32 {
    let rays = 36.0;
    let a = atan2(py, px) / 6.2831853 + 1.0;
    let turn = a - floor(a);
    let k = turn * rays;
    let ray = u32(floor(k));
    let f = k - floor(k);
    let width = 0.25 + 0.65 * hash01(seed, ray, 11u);
    let edge = clamp((0.5 - abs(f - 0.5)) / max(0.5 * width, 1e-3), 0.0, 1.0);
    var alive = 0.0;
    if (hash01(seed, ray, 29u) < 0.72) {
        alive = 1.0;
    }
    return alive * smooth_step(0.0, 1.0, edge) * smooth_step(0.18, 0.55, dist / half_side);
}

fn lines_mask(y: i32, w: f32, h: f32, px: f32, seed: u32) -> f32 {
    let bands = 70.0;
    let yb = f32(y) / max(h, 1.0) * bands;
    let band = u32(floor(yb));
    let fy = yb - floor(yb);
    if (hash01(seed, band, 53u) > 0.45) {
        return 0.0;
    }
    let thick = 0.15 + 0.5 * hash01(seed, band, 71u);
    let across = clamp(1.0 - abs(fy - 0.5) / max(0.5 * thick, 1e-3), 0.0, 1.0);
    let start = 0.15 + 0.35 * hash01(seed, band, 97u);
    let along = smooth_step(start, start + 0.12, abs(px) / max(w / 2.0, 1.0));
    return across * along;
}

// Image d'impact : f[0] = (quantité, seuil, force de la teinte, éclat), f[1] = (teinte rgb, lignes),
// f[2] = (centre x, centre y, largeur, hauteur de la feuille), f[3].x = demi-côté ; n = (mode, graine).
@fragment
fn fs_impact(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let t = lda(sp);
    let amount = p.f[0].x;
    let thr = p.f[0].y;
    let tint_force = p.f[0].z;
    let burst = p.f[0].w;
    let tint = p.f[1].xyz;
    let lines = p.f[1].w;
    let cx = p.f[2].x;
    let cy = p.f[2].y;
    let w = p.f[2].z;
    let h = p.f[2].w;
    let half_side = p.f[3].x;
    let mode = p.n.x;
    let seed = u32(p.n.y);
    let u = unpm(t);
    let base = u.rgb * u.a + vec3<f32>(1.0 - u.a);
    let l = luma(base);
    var out_c = vec3<f32>(0.0);
    if (mode == 0) {
        out_c = vec3<f32>(1.0) - base;
    } else if (mode == 1) {
        if (l >= thr) { out_c = vec3<f32>(1.0); }
    } else if (mode == 2) {
        if (l < thr) { out_c = vec3<f32>(1.0); }
    } else {
        out_c = clamp(floor(base * 2.0 + vec3<f32>(0.5)) / 2.0, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    if (tint_force > 0.0) {
        let lo = luma(out_c);
        out_c = out_c + (tint * lo - out_c) * tint_force;
    }
    let px = f32(sp.x) + 0.5 - cx;
    let py = f32(sp.y) + 0.5 - cy;
    let dist = sqrt(px * px + py * py);
    var mark = 0.0;
    if (burst > 0.0) {
        mark = max(mark, burst * burst_mask(px, py, dist, half_side, seed));
    }
    if (lines > 0.0) {
        mark = max(mark, lines * lines_mask(sp.y, w, h, px, seed));
    }
    if (mark > 0.0) {
        var ink = 1.0;
        if (luma(out_c) > 0.5) { ink = 0.0; }
        out_c = out_c + (vec3<f32>(ink) - out_c) * clamp(mark, 0.0, 1.0);
    }
    let mixed = base + (out_c - base) * amount;
    return prem(mixed, 1.0);
}

// Vignettage : f[0] = (intensité, rayon intérieur, rayon extérieur), f[1] = (centre x, y, demi-axes x, y).
@fragment
fn fs_vignette(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let t = lda(sp);
    let u = (f32(sp.x) + 0.5 - p.f[1].x) / p.f[1].z;
    let v = (f32(sp.y) + 0.5 - p.f[1].y) / p.f[1].w;
    let d = sqrt(u * u + v * v);
    let k = 1.0 - p.f[0].x * smooth_step(p.f[0].y, p.f[0].z, d);
    if (k >= 0.999) {
        return t;
    }
    let t8 = round255(t);
    let ch = floor(t8.rgb * k + vec3<f32>(0.5));
    return vec4<f32>(ch, t8.a) / 255.0;
}

// Aberration chromatique : f[0] = (force / portée, centre x, centre y).
@fragment
fn fs_aberr(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let px = f32(sp.x) + 0.5;
    let py = f32(sp.y) + 0.5;
    let dx = px - p.f[0].y;
    let dy = py - p.f[0].z;
    let k = p.f[0].x;
    let r = bil_a(px - dx * k, py - dy * k);
    let g = bil_a(px, py);
    let b = bil_a(px + dx * k, py + dy * k);
    let a = clamp(floor(max(r.a, max(g.a, b.a)) + 0.5), 0.0, 255.0);
    let ch = clamp(floor(vec3<f32>(r.r, g.g, b.b) + vec3<f32>(0.5)), vec3<f32>(0.0), vec3<f32>(a));
    return vec4<f32>(ch, a) / 255.0;
}

// Secousse de l'image : f[0] = (centre x, y, zoom, décalage x), f[1] = (décalage y, sin, cos, flou) ;
// n.x = nombre de lectures.
@fragment
fn fs_shake(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let cx = p.f[0].x;
    let cy = p.f[0].y;
    let z = p.f[0].z;
    let dx = p.f[0].w;
    let dy = p.f[1].x;
    let s = p.f[1].y;
    let c = p.f[1].z;
    let spread = p.f[1].w;
    let n = p.n.x;
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < n; i++) {
        var m = 1.0;
        if (n > 1) {
            m = 1.0 + spread * (f32(i) / f32(n - 1) - 0.5);
        }
        let qx = f32(sp.x) + 0.5 - cx - dx * m;
        let qy = f32(sp.y) + 0.5 - cy - dy * m;
        let rx = (qx * c - qy * s) / z;
        let ry = (qx * s + qy * c) / z;
        acc += bil_a(rx + cx, ry + cy);
    }
    let inv = 1.0 / f32(n);
    let a = clamp(floor(acc.a * inv + 0.5), 0.0, 255.0);
    let ch = clamp(floor(acc.rgb * inv + vec3<f32>(0.5)), vec3<f32>(0.0), vec3<f32>(a));
    return vec4<f32>(ch, a) / 255.0;
}

// ---------------------------------------------------------------------- déformations et rendu

// Déformation (`fxwarp::warp`) : chaque pixel de la feuille va chercher sa couleur ailleurs dans le dessin,
// en lecture bilinéaire. n.x = 0 onde (f[0] = amplitude, k, sin, cos), 1 ondulation (amplitude, k, centre),
// 2 tourbillon (centre, rayon, angle), 3 pincement (centre, rayon, exposant).
@fragment
fn fs_warp(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let x = f32(sp.x) + 0.5;
    let y = f32(sp.y) + 0.5;
    var sx = x;
    var sy = y;
    let a = p.f[0];
    switch p.n.x {
        case 0: {
            let d = a.x * sin((x * a.w + y * a.z) * a.y);
            sx = x - a.z * d;
            sy = y + a.w * d;
        }
        case 1: {
            let dx = x - a.z;
            let dy = y - a.w;
            let d = sqrt(dx * dx + dy * dy);
            if (d >= 1e-3) {
                let m = a.x * sin(d * a.y) / d;
                sx = x + dx * m;
                sy = y + dy * m;
            }
        }
        case 2: {
            let dx = x - a.x;
            let dy = y - a.y;
            let d = sqrt(dx * dx + dy * dy);
            if (d < a.z) {
                let t = -a.w * (1.0 - d / a.z) * (1.0 - d / a.z);
                let s = sin(t);
                let c = cos(t);
                sx = a.x + dx * c - dy * s;
                sy = a.y + dx * s + dy * c;
            }
        }
        default: {
            let dx = x - a.x;
            let dy = y - a.y;
            let d = sqrt(dx * dx + dy * dy);
            if (d < a.z && d >= 1e-3) {
                let f = pow(d / a.z, a.w) * a.z / d;
                sx = a.x + dx * f;
                sy = a.y + dy * f;
            }
        }
    }
    let c = bil_a(sx, sy);
    let al = clamp(floor(c.a + 0.5), 0.0, 255.0);
    let ch = clamp(floor(c.rgb + vec3<f32>(0.5)), vec3<f32>(0.0), vec3<f32>(al));
    return vec4<f32>(ch, al) / 255.0;
}

// Bruit lissé de `fxwarp::value_noise` (les nuages).
fn vhash(i: i32, j: i32, seed: u32) -> f32 {
    var h = (bitcast<u32>(i) * 0x8DA6B343u) ^ (bitcast<u32>(j) * 0xD8163841u) ^ (seed * 0xCB1AB31Fu);
    h = h ^ (h >> 13u);
    h = h * 0x5BD1E995u;
    h = h ^ (h >> 15u);
    return f32(h & 0xFFFFu) / 65535.0;
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let ix = floor(x);
    let iy = floor(y);
    let tx = x - ix;
    let ty = y - iy;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let i = i32(ix);
    let j = i32(iy);
    let top = vhash(i, j, seed) + (vhash(i + 1, j, seed) - vhash(i, j, seed)) * sx;
    let bottom = vhash(i, j + 1, seed) + (vhash(i + 1, j + 1, seed) - vhash(i, j + 1, seed)) * sx;
    return top + (bottom - top) * sy;
}

// Rendu (`fxwarp::shade`) : la couleur des pixels dessinés multipliée par un facteur ; la couverture reste.
// n.x = 0 nuages (f[0] = 1 / échelle, quantité ; n.y = graine), 1 éclairage (f[0] = centre, rayon,
// intensité), 2 dégradé (f[0] = centre, cos, sin ; f[1] = portée, force).
@fragment
fn fs_shade(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let t = lda(sp);
    if (t.a <= 0.0) {
        return t;
    }
    let x = f32(sp.x) + 0.5;
    let y = f32(sp.y) + 0.5;
    let a = p.f[0];
    var k = 1.0;
    switch p.n.x {
        case 0: {
            var n = 0.0;
            var w = 1.0;
            var f = a.x;
            var total = 0.0;
            for (var o = 0; o < 4; o++) {
                n += value_noise(x * f, y * f, u32(p.n.y) + u32(o)) * w;
                total += w;
                w *= 0.5;
                f *= 2.0;
            }
            k = 1.0 - a.y + a.y * (0.4 + 1.2 * n / total);
        }
        case 1: {
            let d = sqrt((x - a.x) * (x - a.x) + (y - a.y) * (y - a.y)) / a.z;
            var fall = clamp(1.0 - d, 0.0, 1.0);
            fall = fall * fall * (3.0 - 2.0 * fall);
            k = 1.0 + a.w * (2.0 * fall - 1.0);
        }
        default: {
            let b = p.f[1];
            var u = (((x - a.x) * a.z + (y - a.y) * a.w) / b.x + 1.0) / 2.0;
            u = clamp(u, 0.0, 1.0);
            k = 1.0 - b.y * u * u * (3.0 - 2.0 * u);
        }
    }
    k = max(k, 0.0);
    let t8 = round255(t);
    let ch = clamp(floor(t8.rgb * k + vec3<f32>(0.5)), vec3<f32>(0.0), vec3<f32>(t8.a));
    return vec4<f32>(ch, t8.a) / 255.0;
}

// Mosaïque, première passe : une valeur par carré de n.x pixels de la feuille (aligné sur la feuille,
// borné par elle), la moyenne prémultipliée arrondie ; le pixel de la cible est le carré.
@fragment
fn fs_mosaic_avg(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let cell = dst_px(pos);
    let s = p.n.x;
    let x0 = cell.x * s;
    let y0 = cell.y * s;
    let x1 = min(x0 + s, p.s.x);
    let y1 = min(y0 + s, p.s.y);
    var sum = vec4<u32>(0u);
    for (var y = y0; y < y1; y++) {
        for (var x = x0; x < x1; x++) {
            sum += bytes4(lda(vec2<i32>(x, y)));
        }
    }
    let n = u32(max((x1 - x0) * (y1 - y0), 0));
    if (n == 0u || sum.a == 0u) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>((sum + vec4<u32>(n / 2u)) / vec4<u32>(n)) / 255.0;
}

// Mosaïque, seconde passe : chaque pixel de la feuille prend la valeur de son carré.
@fragment
fn fs_mosaic_fill(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    return lda(vec2<i32>(sp.x / p.n.x, sp.y / p.n.x));
}

// ---------------------------------------------------------------------- flous

// Un octet de 0 à 255 : la valeur de la texture, en entier.
fn bytes4(t: vec4<f32>) -> vec4<u32> {
    return vec4<u32>(floor(t * 255.0 + vec4<f32>(0.5)));
}

// Flou en boîte en lignes, de rayon n.x : la moyenne de 2r + 1 pixels, arrondie au plus proche, en
// entiers comme `fx::box_rows` (les trois passes du flou gaussien sont trois de ces passes, puis trois
// en colonnes ; ce qui sort de la zone est transparent).
@fragment
fn fs_box_h(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let r = p.n.x;
    var acc = vec4<u32>(0u);
    for (var i = -r; i <= r; i++) {
        acc += bytes4(lda(sp + vec2<i32>(i, 0)));
    }
    let size = u32(2 * r + 1);
    return vec4<f32>((acc + vec4<u32>(size / 2u)) / vec4<u32>(size)) / 255.0;
}

@fragment
fn fs_box_v(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let r = p.n.x;
    var acc = vec4<u32>(0u);
    for (var i = -r; i <= r; i++) {
        acc += bytes4(lda(sp + vec2<i32>(0, i)));
    }
    let size = u32(2 * r + 1);
    return vec4<f32>((acc + vec4<u32>(size / 2u)) / vec4<u32>(size)) / 255.0;
}

// Flou de mouvement : f[0] = (cos, sin, longueur) ; n.x = nombre de lectures. Lecture bilinéaire
// entre pixels voisins (indices), comme `fx::motion`.
@fragment
fn fs_motion(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let n = p.n.x;
    let c = p.f[0].x;
    let s = p.f[0].y;
    let length = p.f[0].z;
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < n; i++) {
        let t = (f32(i) / f32(n - 1) - 0.5) * length;
        acc += bil_a(f32(sp.x) + t * c + 0.5, f32(sp.y) + t * s + 0.5);
    }
    let m = clamp(floor(acc / f32(n) + vec4<f32>(0.5)), vec4<f32>(0.0), vec4<f32>(255.0));
    return m / 255.0;
}

// Le premier pixel de l'ombre portée : la silhouette (noire), décalée de n = (dx, dy), à f[0].x.
@fragment
fn fs_shadow_sil(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let src = lda(sp - vec2<i32>(p.n.x, p.n.y));
    let a = floor(src.a * 255.0 * p.f[0].x + 0.5);
    return vec4<f32>(0.0, 0.0, 0.0, a / 255.0);
}

// `top` (en 0 à 255, prémultiplié) posé sur `bottom` : `brushfill::over_premul`.
fn over255(top: vec4<f32>, bottom: vec4<f32>) -> vec4<f32> {
    let k = 1.0 - top.a / 255.0;
    return floor(min(top + bottom * k + vec4<f32>(0.5), vec4<f32>(255.0)));
}

// Ombre portée : le dessin (A) posé sur l'ombre floutée (B).
@fragment
fn fs_shadow_over(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    return over255(round255(lda(sp)), round255(ldb(sp))) / 255.0;
}

// Lueur : le dessin (A) posé sur son halo (B) renforcé de f[0].x.
@fragment
fn fs_glow_over(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let h8 = round255(ldb(sp));
    let k = p.f[0].x;
    let a = min(h8.a * k, 255.0);
    let c = min(min(h8.rgb * k, vec3<f32>(255.0)), vec3<f32>(a));
    let halo = vec4<f32>(floor(c), floor(a));
    return over255(round255(lda(sp)), halo) / 255.0;
}

// Netteté : le dessin (A) accentué par la différence avec son flou (B), f[0].x = gain.
@fragment
fn fs_sharpen(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let t = lda(sp);
    if (t.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let o = t.rgb / t.a;
    let bl = unpm(ldb(sp));
    var b = o;
    if (bl.a > 0.0) {
        b = bl.rgb;
    }
    return prem(o + (o - b) * p.f[0].x, t.a);
}

// Lueur diffuse, première passe : la feuille réduite n.x fois (moyenne des pixels prémultipliés
// arrondie, comme `fxproxy::downscale`), puis ce qui dépasse le seuil f[0].x.
@fragment
fn fs_bloom_down(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let f = p.n.x;
    var sum = vec4<u32>(0u);
    for (var j = 0; j < f; j++) {
        for (var i = 0; i < f; i++) {
            sum += bytes4(lda(sp * f + vec2<i32>(i, j)));
        }
    }
    let nn = u32(f * f);
    let m = vec4<f32>((sum + vec4<u32>(nn / 2u)) / vec4<u32>(nn)) / 255.0;
    let u = unpm(m);
    if (u.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let thr = p.f[0].x;
    let k = clamp((luma(u.rgb) - thr) / (1.0 - thr), 0.0, 1.0);
    return prem(u.rgb * k, u.a * k);
}

// Lueur diffuse, dernière passe : l'image (A) et le halo réduit (B, lu en bilinéaire) ajoutés en écran ;
// n.x = facteur de réduction, f[0].x = intensité.
@fragment
fn fs_bloom_final(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = dst_px(pos);
    let t = lda(sp);
    let f = f32(p.n.x);
    let g = bil_b((f32(sp.x) + 0.5) / f, (f32(sp.y) + 0.5) / f);
    if (g.a < 0.5) {
        return t;
    }
    let s8 = round255(t);
    let s = s8 / 255.0;
    let gg = min(g / 255.0 * p.f[0].x, vec4<f32>(1.0));
    let o = floor((vec4<f32>(1.0) - (vec4<f32>(1.0) - s) * (vec4<f32>(1.0) - gg)) * 255.0 + vec4<f32>(0.5));
    let a = o.a;
    return vec4<f32>(min(o.rgb, vec3<f32>(a)), a) / 255.0;
}
