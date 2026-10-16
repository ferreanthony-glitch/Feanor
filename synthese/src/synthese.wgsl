// Synthèse par l'exemple sur la carte graphique (étape d), d'après la version CUDA d'ebsynth
// (ebsynth_cuda.cu) : propagation de Jacobi (voisins à 4, 2 puis 1 pixel), recherches aléatoires
// dans des rayons croissants, occupation du style comptée par additions atomiques, masque des
// pixels qui ne bougent plus.
//
// Chaque image est rangée en mots de 4 octets par pixel : d'abord les mots du style (4 canaux par
// mot, complétés par des zéros), puis ceux des guides. Le vote n'écrit que les mots du style.

struct Params {
    tw: u32,           // taille du niveau de la cible (ou de l'image produite par `resample`)
    th: u32,
    sw: u32,           // taille du niveau du style
    sh: u32,
    pw: u32,           // taille du niveau précédent (`upscale`) ou de l'image lue (`resample`)
    ph: u32,
    words: u32,        // mots par pixel
    style_words: u32,  // dont mots du style
    patch_size: u32,
    radius: i32,       // décalage des voisins (`propagate`), rayon (`random_search`)
    seed: u32,
    stop: u32,         // seuil du masque des pixels immobiles
    lambda: f32,       // poids de l'uniformité
    omega_best: f32,   // occupation idéale d'un pixel du style
    pad0: u32,
    pad1: u32,
}

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<uniform> W: array<vec4<f32>, 16>;
@group(0) @binding(2) var<storage, read> src: array<u32>;
@group(0) @binding(3) var<storage, read_write> tgt: array<u32>;
@group(0) @binding(4) var<storage, read_write> nnf: array<vec2<i32>>;
@group(0) @binding(5) var<storage, read_write> nnf2: array<vec2<i32>>;
@group(0) @binding(6) var<storage, read_write> err: array<f32>;
@group(0) @binding(7) var<storage, read_write> omega: array<atomic<i32>>;
@group(0) @binding(8) var<storage, read_write> mask: array<u32>;
@group(0) @binding(9) var<storage, read_write> old: array<u32>;

fn unpack(w: u32) -> vec4<f32> {
    return vec4<f32>(f32(w & 0xffu), f32((w >> 8u) & 0xffu), f32((w >> 16u) & 0xffu), f32(w >> 24u));
}

fn unpack_u(w: u32) -> vec4<u32> {
    return vec4<u32>(w & 0xffu, (w >> 8u) & 0xffu, (w >> 16u) & 0xffu, w >> 24u);
}

// Valeurs tronquées, comme le portage sur le processeur.
fn pack_trunc(v: vec4<f32>) -> u32 {
    let u = vec4<u32>(clamp(v, vec4<f32>(0.0), vec4<f32>(255.0)));
    return u.x | (u.y << 8u) | (u.z << 16u) | (u.w << 24u);
}

// Hachage PCG (Jarzynski et Olano 2020) : le hasard sans état.
fn hash(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// Coût du patch de la cible centré en (tx, ty) contre celui du style centré en (sx, sy), qui est
// entier ; arrêt dès qu'une ligne dépasse `ebest`.
fn cost(tx: i32, ty: i32, sx: i32, sy: i32, ebest: f32) -> f32 {
    let r = i32(P.patch_size / 2u);
    let tw = i32(P.tw);
    let th = i32(P.th);
    var e = 0.0;
    for (var py = -r; py <= r; py++) {
        for (var px = -r; px <= r; px++) {
            let x = clamp(tx + px, 0, tw - 1);
            let y = clamp(ty + py, 0, th - 1);
            let ti = u32(y * tw + x) * P.words;
            let si = u32((sy + py) * i32(P.sw) + sx + px) * P.words;
            for (var k = 0u; k < P.words; k++) {
                let d = unpack(tgt[ti + k]) - unpack(src[si + k]);
                e += dot(W[k] * d, d);
            }
        }
        if (e > ebest) {
            return e;
        }
    }
    return e;
}

fn patch_omega(b: vec2<i32>) -> i32 {
    let r = i32(P.patch_size / 2u);
    var sum = 0;
    for (var py = -r; py <= r; py++) {
        let row = (b.y + py) * i32(P.sw) + b.x;
        for (var px = -r; px <= r; px++) {
            sum += atomicLoad(&omega[u32(row + px)]);
        }
    }
    return sum;
}

fn add_omega(b: vec2<i32>, inc: i32) {
    let r = i32(P.patch_size / 2u);
    for (var py = -r; py <= r; py++) {
        let row = (b.y + py) * i32(P.sw) + b.x;
        for (var px = -r; px <= r; px++) {
            atomicAdd(&omega[u32(row + px)], inc);
        }
    }
}

// Garde `b` pour le pixel (x, y) s'il fait mieux (coût + uniformité × occupation).
fn try_patch(x: i32, y: i32, b: vec2<i32>, best: ptr<function, vec2<i32>>, ebest: ptr<function, f32>) {
    if (P.lambda == 0.0) {
        let e = cost(x, y, b.x, b.y, *ebest);
        if (e < *ebest) {
            *best = b;
            *ebest = e;
        }
        return;
    }
    let area = f32(P.patch_size * P.patch_size);
    let cur_occ = f32(patch_omega(*best)) / area / P.omega_best;
    let new_occ = f32(patch_omega(b)) / area / P.omega_best;
    let cur_total = *ebest + P.lambda * cur_occ;
    let e = cost(x, y, b.x, b.y, cur_total);
    if (e + P.lambda * new_occ < cur_total) {
        add_omega(b, 1);
        add_omega(*best, -1);
        *best = b;
        *ebest = e;
    }
}

fn inside_source(n: vec2<i32>) -> bool {
    let hp = i32(P.patch_size / 2u);
    return n.x >= hp && n.x < i32(P.sw) - hp && n.y >= hp && n.y < i32(P.sh) - hp;
}

// Rééchantillonnage bilinéaire de `src` (pw × ph) en `tgt` (tw × th), comme ebsynth : l'échelle
// de la largeur sert aussi pour la hauteur, valeurs tronquées.
@compute @workgroup_size(16, 16)
fn resample(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    let s = f32(P.pw) / f32(P.tw);
    let fx = s * f32(id.x);
    let fy = s * f32(id.y);
    let ix = i32(fx);
    let iy = i32(fy);
    let a = fx - f32(ix);
    let b = fy - f32(iy);
    let w = i32(P.pw);
    let h = i32(P.ph);
    let x0 = clamp(ix, 0, w - 1);
    let x1 = clamp(ix + 1, 0, w - 1);
    let y0 = clamp(iy, 0, h - 1);
    let y1 = clamp(iy + 1, 0, h - 1);
    let p00 = u32(y0 * w + x0) * P.words;
    let p10 = u32(y0 * w + x1) * P.words;
    let p01 = u32(y1 * w + x0) * P.words;
    let p11 = u32(y1 * w + x1) * P.words;
    let o = (id.y * P.tw + id.x) * P.words;
    for (var k = 0u; k < P.words; k++) {
        let v = (1.0 - a) * (1.0 - b) * unpack(src[p00 + k]) + a * (1.0 - b) * unpack(src[p10 + k])
            + (1.0 - a) * b * unpack(src[p01 + k]) + a * b * unpack(src[p11 + k]);
        tgt[o + k] = pack_trunc(v);
    }
}

// Correspondances tirées au hasard, centres de patch entièrement dans le style.
@compute @workgroup_size(16, 16)
fn init_random(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    let i = id.y * P.tw + id.x;
    let hp = P.patch_size / 2u;
    let h1 = hash(i ^ hash(P.seed));
    let h2 = hash(h1);
    nnf[i] = vec2<i32>(i32(hp + h1 % (P.sw - 2u * hp)), i32(hp + h2 % (P.sh - 2u * hp)));
}

// Passage au niveau plus fin : `nnf2` (pw × ph) doublé dans `nnf`, à `patch` pixels des bords.
@compute @workgroup_size(16, 16)
fn upscale(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    let n = nnf2[u32(clamp(y / 2, 0, i32(P.ph) - 1)) * P.pw + u32(clamp(x / 2, 0, i32(P.pw) - 1))];
    let p = i32(P.patch_size);
    nnf[id.y * P.tw + id.x] = vec2<i32>(clamp(n.x * 2 + x % 2, p, i32(P.sw) - p - 1), clamp(n.y * 2 + y % 2, p, i32(P.sh) - p - 1));
}

@compute @workgroup_size(16, 16)
fn omega_clear(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.sw || id.y >= P.sh) {
        return;
    }
    atomicStore(&omega[id.y * P.sw + id.x], 0);
}

@compute @workgroup_size(16, 16)
fn omega_init(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    add_omega(nnf[id.y * P.tw + id.x], 1);
}

// Tous les pixels actifs (bit 0) ; le bit 1 marque ceux qui ont changé au dernier vote.
@compute @workgroup_size(16, 16)
fn mask_all(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    mask[id.y * P.tw + id.x] = 3u;
}

// Vote simple : la moyenne (tronquée) des pixels que donnent les patchs qui recouvrent le pixel.
// L'ancien style est gardé dans `old` pour le masque.
@compute @workgroup_size(16, 16)
fn vote(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    let i = id.y * P.tw + id.x;
    let r = i32(P.patch_size / 2u);
    var sum: array<vec4<u32>, 16>;
    for (var k = 0u; k < P.style_words; k++) {
        sum[k] = vec4<u32>(0u);
    }
    var count = 0u;
    for (var py = -r; py <= r; py++) {
        for (var px = -r; px <= r; px++) {
            let qx = x + px;
            let qy = y + py;
            if (qx < 0 || qx >= i32(P.tw) || qy < 0 || qy >= i32(P.th)) {
                continue;
            }
            let n = nnf[u32(qy) * P.tw + u32(qx)] - vec2<i32>(px, py);
            if (n.x < 0 || n.x >= i32(P.sw) || n.y < 0 || n.y >= i32(P.sh)) {
                continue;
            }
            let si = (u32(n.y) * P.sw + u32(n.x)) * P.words;
            for (var k = 0u; k < P.style_words; k++) {
                sum[k] += unpack_u(src[si + k]);
            }
            count += 1u;
        }
    }
    for (var k = 0u; k < P.style_words; k++) {
        let v = sum[k] / max(count, 1u);
        old[i * P.style_words + k] = tgt[i * P.words + k];
        tgt[i * P.words + k] = v.x | (v.y << 8u) | (v.z << 16u) | (v.w << 24u);
    }
}

@compute @workgroup_size(16, 16)
fn eval_error(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    let i = id.y * P.tw + id.x;
    let n = nnf[i];
    err[i] = cost(i32(id.x), i32(id.y), n.x, n.y, 3.4e38);
}

// Propagation de Jacobi : chaque pixel essaie ses quatre voisins à `radius` pixels (lus dans
// `nnf`, écrits dans `nnf2`).
@compute @workgroup_size(16, 16)
fn propagate(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    let i = id.y * P.tw + id.x;
    var best = nnf[i];
    var e = err[i];
    if ((mask[i] & 1u) != 0u) {
        let r = P.radius;
        var offsets = array<vec2<i32>, 4>(vec2<i32>(-r, 0), vec2<i32>(r, 0), vec2<i32>(0, -r), vec2<i32>(0, r));
        for (var k = 0; k < 4; k++) {
            let o = offsets[k];
            let qx = x + o.x;
            let qy = y + o.y;
            if (qx < 0 || qx >= i32(P.tw) || qy < 0 || qy >= i32(P.th)) {
                continue;
            }
            let n = nnf[u32(qy) * P.tw + u32(qx)] - o;
            if (inside_source(n)) {
                try_patch(x, y, n, &best, &e);
            }
        }
    }
    err[i] = e;
    nnf2[i] = best;
}

// Recherche aléatoire dans un carré de rayon `radius` autour de la correspondance.
@compute @workgroup_size(16, 16)
fn random_search(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    let i = id.y * P.tw + id.x;
    if ((mask[i] & 1u) == 0u) {
        return;
    }
    var best = nnf[i];
    var e = err[i];
    let hp = i32(P.patch_size / 2u);
    let r = P.radius;
    let xmin = max(best.x - r, hp);
    let xmax = min(best.x + r, i32(P.sw) - 1 - hp);
    let ymin = max(best.y - r, hp);
    let ymax = min(best.y + r, i32(P.sh) - 1 - hp);
    let h1 = hash(i ^ hash(P.seed));
    let h2 = hash(h1);
    let n = vec2<i32>(xmin + i32(h1 % u32(xmax - xmin + 1)), ymin + i32(h2 % u32(ymax - ymin + 1)));
    try_patch(i32(id.x), i32(id.y), n, &best, &e);
    err[i] = e;
    nnf[i] = best;
}

// Masque : un pixel dont aucun canal du style n'a bougé d'au moins `stop` au dernier vote est
// immobile (bit 1 à 0).
@compute @workgroup_size(16, 16)
fn eval_mask(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    let i = id.y * P.tw + id.x;
    var moved = false;
    for (var k = 0u; k < P.style_words; k++) {
        let a = vec4<i32>(unpack_u(tgt[i * P.words + k]));
        let b = vec4<i32>(unpack_u(old[i * P.style_words + k]));
        let d = abs(a - b);
        if (max(max(d.x, d.y), max(d.z, d.w)) >= i32(P.stop)) {
            moved = true;
        }
    }
    mask[i] = select(0u, 2u, moved);
}

// Un pixel reste actif (bit 0) si un pixel de son patch a bougé.
@compute @workgroup_size(16, 16)
fn dilate(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= P.tw || id.y >= P.th) {
        return;
    }
    let x = i32(id.x);
    let y = i32(id.y);
    let r = i32(P.patch_size / 2u);
    var near = false;
    for (var py = -r; py <= r; py++) {
        for (var px = -r; px <= r; px++) {
            let qx = x + px;
            let qy = y + py;
            if (qx >= 0 && qx < i32(P.tw) && qy >= 0 && qy < i32(P.th) && (mask[u32(qy) * P.tw + u32(qx)] & 2u) != 0u) {
                near = true;
            }
        }
    }
    let i = id.y * P.tw + id.x;
    mask[i] = (mask[i] & 2u) | select(0u, 1u, near);
}
