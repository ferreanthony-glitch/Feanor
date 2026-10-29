//! Fond sous le trait (Anthony, 2026-10-18 : « une tâche fastidieuse du storyboard consiste à
//! remplir les personnages d'un fond blanc pour éviter que le décor en dessous ne transparaisse » ;
//! ses choix : le fond vit dans le même calque, comme une strate sous le trait, et s'allume par une
//! option du pinceau).
//!
//! La forme du fond est déduite du trait, à chaque changement : tout ce que le trait enferme (les
//! trous du trait jusqu'à `2 × trous` pixels sont fermés), plus un débord de `debord` pixels autour
//! du trait ; une seule forme globale, au bord adouci. Ce qu'on peint dans le fond (strate Fond)
//! reste dans cette forme et passe sous le trait.

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::raster::{premultiply, Raster, TILE};

/// Réglages du fond d'un calque (son option « Fond sous le trait » allumée).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FondReglages {
    /// Couleur du fond (blanc d'office).
    pub couleur: [u8; 3],
    /// Pixels de fond au-delà du bord extérieur du trait.
    #[serde(default = "default_debord")]
    pub debord: u8,
    /// Les trous du trait jusqu'à `2 × trous` pixels sont fermés.
    #[serde(default = "default_trous")]
    pub trous: u8,
    /// Le bord de l'image ferme la forme : un personnage coupé par le cadre (plan poitrine) a son
    /// fond jusqu'au bord ; éteint, ce qui touche le bord reste dehors.
    #[serde(default = "yes")]
    pub bord: bool,
    /// « Aplat intelligent » (`aplatregions.rs`, `aplatintel.rs`) : l'aplat suit le trait en gardant la couleur de chaque
    /// zone fermée, par correspondance de forme et de position entre les clés. Éteint d'office ; n'est écrit dans le projet
    /// que s'il est allumé (un projet sans l'option reste tel qu'avant, à son numéro de format près).
    #[serde(default, skip_serializing_if = "is_false")]
    pub intelligent: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn yes() -> bool {
    true
}

fn default_debord() -> u8 {
    DEBORD
}

fn default_trous() -> u8 {
    TROUS
}

/// Débord et fermeture des trous d'un fond neuf.
pub const DEBORD: u8 = 3;
pub const TROUS: u8 = 6;

impl Default for FondReglages {
    fn default() -> Self {
        FondReglages { couleur: [255, 255, 255], debord: DEBORD, trous: TROUS, bord: true, intelligent: false }
    }
}

/// Opacité à partir de laquelle un pixel est du trait (il ferme la forme).
pub const LINE_ALPHA: u8 = 96;

/// Très loin (distance au carré).
const FAR: f64 = 1e18;

/// Distances au carré au plus proche pixel où `f` vaut 0, ligne par ligne (Felzenszwalb et
/// Huttenlocher) : `f` contient 0 ou `FAR`, et reçoit les distances.
fn edt_1d(f: &mut [f64], v: &mut [usize], z: &mut [f64], d: &mut [f64]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        let fq = f[q] + (q * q) as f64;
        loop {
            let p = v[k];
            let s = (fq - (f[p] + (p * p) as f64)) / (2.0 * (q as f64 - p as f64));
            if s <= z[k] && k > 0 {
                k -= 1;
                continue;
            }
            if s <= z[k] {
                // k = 0 et la parabole de q passe sous celle de v[0] partout.
                v[0] = q;
                z[0] = f64::NEG_INFINITY;
                z[1] = f64::INFINITY;
                break;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate().take(n) {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        let dq = q as f64 - p as f64;
        *out = dq * dq + f[p];
    }
    f.copy_from_slice(&d[..n]);
}

/// Distance au carré de chaque pixel de la grille `w × h` au plus proche pixel de `feature`.
pub fn edt(feature: &[bool], w: usize, h: usize) -> Vec<f64> {
    let mut grid: Vec<f64> = feature.iter().map(|&f| if f { 0.0 } else { FAR }).collect();
    let n = w.max(h);
    let (mut f, mut v, mut z, mut d) = (vec![0.0; n], vec![0usize; n], vec![0.0; n + 1], vec![0.0; n]);
    // Colonnes, puis lignes.
    for x in 0..w {
        for y in 0..h {
            f[y] = grid[y * w + x];
        }
        edt_1d(&mut f[..h], &mut v, &mut z, &mut d);
        for y in 0..h {
            grid[y * w + x] = f[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&grid[y * w..(y + 1) * w]);
        edt_1d(&mut f[..w], &mut v, &mut z, &mut d);
        grid[y * w..(y + 1) * w].copy_from_slice(&f[..w]);
    }
    grid
}

/// Ce que le calcul de la forme (et des régions de l'aplat intelligent, `aplatregions.rs`) sait du trait : le rectangle de
/// travail (`bx0`, `by0`, `bw` × `bh`, coordonnées du dessin), et dans ce rectangle, pour chaque pixel : est-il
/// un mur (`wall` : le trait épaissi de `trous`, qui ferme les trous), est-il le dehors (`outside`, atteint depuis le bord sans
/// traverser de mur).
pub(crate) struct Geometrie {
    pub bx0: usize,
    pub by0: usize,
    pub bw: usize,
    pub bh: usize,
    pub wall: Vec<bool>,
    pub outside: Vec<bool>,
}

/// La géométrie du trait `line` pour les réglages `r` ; rien s'il n'y a pas de trait. Sur la feuille seulement (ce qui est
/// hors d'elle n'y compte pas) : ce que lisent l'aplat intelligent et la peinture dans l'aplat.
pub(crate) fn geometrie(line: &Raster, r: &FondReglages) -> Option<Geometrie> {
    let g = geometrie_dans(line, r, false)?;
    Some(Geometrie { bx0: g.x0 as usize, by0: g.y0 as usize, bw: g.w, bh: g.h, wall: g.wall, outside: g.outside })
}

/// Le plus grand rectangle de travail d'une forme sur la toile infinie, en pixels (une forme qui demanderait plus se
/// limite à la feuille : une grille de distances de 16 millions de pixels pèse déjà 130 Mo).
const MAX_BOITE: i64 = 16 << 20;

/// Comme `Geometrie`, en coordonnées du monde (la toile infinie : le rectangle de travail peut sortir de la feuille).
pub(crate) struct GeometrieMonde {
    pub x0: i64,
    pub y0: i64,
    pub w: usize,
    pub h: usize,
    pub wall: Vec<bool>,
    pub outside: Vec<bool>,
}

/// Le rectangle des pixels du trait (opacité d'un trait, `LINE_ALPHA`) : (gauche, haut, droite, bas), bornes comprises.
/// `monde` : tout le dessin, les tuiles hors de la feuille et la part des tuiles de bord qui la dépasse comprises ;
/// sinon la feuille seule.
fn rectangle_du_trait(line: &Raster, monde: bool) -> Option<(i64, i64, i64, i64)> {
    let mut found: Option<(i64, i64, i64, i64)> = None;
    let mut scan = |ox: i64, oy: i64, tw: usize, th: usize, t: &crate::raster::Tile| {
        for y in 0..th {
            for x in 0..tw {
                if t[y * TILE + x].a() >= LINE_ALPHA {
                    let (gx, gy) = (ox + x as i64, oy + y as i64);
                    found = Some(match found {
                        None => (gx, gy, gx, gy),
                        Some((a, b, c, d)) => (a.min(gx), b.min(gy), c.max(gx), d.max(gy)),
                    });
                }
            }
        }
    };
    for (idx, t) in line.tiles.iter().enumerate() {
        let Some(t) = t else { continue };
        if monde {
            let (tx, ty) = ((idx % line.tiles_x) as i64, (idx / line.tiles_x) as i64);
            scan(tx * TILE as i64, ty * TILE as i64, TILE, TILE, t);
        } else {
            let (ox, oy, tw, th) = line.tile_bounds(idx);
            scan(ox as i64, oy as i64, tw, th, t);
        }
    }
    if monde {
        for (&(tx, ty), t) in &line.outer {
            scan(tx as i64 * TILE as i64, ty as i64 * TILE as i64, TILE, TILE, t);
        }
    }
    found
}

/// La géométrie du trait `line` pour les réglages `r`, sur la toile infinie si `monde` : le trait qui sort de la feuille
/// compte, et le rectangle de travail déborde de la feuille de ce côté-là. Là où le trait ne sort pas, le rectangle
/// s'arrête au bord de la feuille, comme avant (« le bord de l'image ferme la forme » : un personnage coupé par le cadre
/// a son fond jusqu'au bord). Un trait si étalé que le rectangle dépasserait `MAX_BOITE` pixels se limite à la feuille.
pub(crate) fn geometrie_dans(line: &Raster, r: &FondReglages, monde: bool) -> Option<GeometrieMonde> {
    let (w, h) = (line.width as i64, line.height as i64);
    // Le rectangle du trait, agrandi de la fermeture et du débord : on ne calcule que dedans.
    let (x0, y0, x1, y1) = rectangle_du_trait(line, monde)?;
    let trous = r.trous as f64;
    let pad = (r.trous.max(r.debord) as i64) + 3;
    let world = crate::raster::WORLD;
    let (bx0, by0) = (if monde && x0 < 0 { (x0 - pad).max(-world) } else { (x0 - pad).max(0) }, if monde && y0 < 0 { (y0 - pad).max(-world) } else { (y0 - pad).max(0) });
    let (bx1, by1) = (if monde && x1 > w - 1 { (x1 + pad).min(world) } else { (x1 + pad).min(w - 1) }, if monde && y1 > h - 1 { (y1 + pad).min(world) } else { (y1 + pad).min(h - 1) });
    if monde && (bx1 - bx0 + 1) * (by1 - by0 + 1) > MAX_BOITE {
        return geometrie_dans(line, r, false);
    }
    let (bw, bh) = ((bx1 - bx0 + 1) as usize, (by1 - by0 + 1) as usize);
    // Le trait dans la boîte.
    let mut solid = vec![false; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            solid[y * bw + x] = line.get_i(bx0 + x as i64, by0 + y as i64).a() >= LINE_ALPHA;
        }
    }
    // Les murs : le trait épaissi de `trous` pixels (ferme les trous).
    let d_line = edt(&solid, bw, bh);
    let wall: Vec<bool> = d_line.iter().map(|&d| d <= trous * trous).collect();
    // Le dehors : ce qu'on atteint depuis le bord de la boîte (et le reste du dessin) sans
    // traverser de mur ; le bord de l'image n'y mène pas si `bord` (il ferme la forme).
    let mut outside = vec![false; bw * bh];
    let mut stack: Vec<usize> = Vec::new();
    let on_edge = |p: usize| {
        let (x, y) = (bx0 + (p % bw) as i64, by0 + (p / bw) as i64);
        // (Le bord de la feuille : ses pixels de bord, pas ce qui est hors d'elle.)
        x >= 0 && y >= 0 && x < w && y < h && (x == 0 || y == 0 || x + 1 == w || y + 1 == h)
    };
    let seed = |p: usize, outside: &mut Vec<bool>, stack: &mut Vec<usize>| {
        if !wall[p] && !outside[p] && !(r.bord && on_edge(p)) {
            outside[p] = true;
            stack.push(p);
        }
    };
    for x in 0..bw {
        seed(x, &mut outside, &mut stack);
        seed((bh - 1) * bw + x, &mut outside, &mut stack);
    }
    for y in 0..bh {
        seed(y * bw, &mut outside, &mut stack);
        seed(y * bw + bw - 1, &mut outside, &mut stack);
    }
    while let Some(p) = stack.pop() {
        let (x, y) = (p % bw, p / bw);
        let near = [(x > 0).then(|| p - 1), (x + 1 < bw).then(|| p + 1), (y > 0).then(|| p - bw), (y + 1 < bh).then(|| p + bw)];
        for q in near.into_iter().flatten() {
            if !wall[q] && !outside[q] {
                outside[q] = true;
                stack.push(q);
            }
        }
    }
    Some(GeometrieMonde { x0: bx0, y0: by0, w: bw, h: bh, wall, outside })
}

/// La couverture (0 à 255) de chaque pixel d'un rectangle de travail `bw × bh` dont `outside` dit le dehors.
fn couverture_de_la_boite(outside: &[bool], bw: usize, bh: usize, r: &FondReglages) -> Vec<u8> {
    let trous = r.trous as f64;
    let debord = r.debord as f64;
    // La forme fermée, rendue à sa vraie taille : rongée de `trous - debord` (ou agrandie si le
    // débord est plus grand), au bord adouci d'un pixel.
    let grow = debord - trous;
    let cover: Vec<f64> = if grow <= 0.0 {
        let d_out = edt(outside, bw, bh);
        d_out.iter().zip(outside).map(|(&d, &o)| if o { 0.0 } else { (d.sqrt() - (-grow)).clamp(0.0, 1.0) }).collect()
    } else {
        let inside: Vec<bool> = outside.iter().map(|&o| !o).collect();
        let d_in = edt(&inside, bw, bh);
        d_in.iter().map(|&d| (grow + 1.0 - d.sqrt()).clamp(0.0, 1.0)).collect()
    };
    cover.iter().map(|&c| (c * 255.0).round() as u8).collect()
}

/// La forme du fond d'après le trait `line` : une couverture de 0 à 255 par pixel du dessin
/// (`width × height`), vide s'il n'y a pas de trait. Sur la feuille seulement (le trait hors d'elle n'y compte pas) ;
/// ce que le dessin montre est `silhouette_monde`.
pub fn silhouette(line: &Raster, r: &FondReglages) -> Vec<u8> {
    let (w, h) = (line.width, line.height);
    let mut out = vec![0u8; w * h];
    let Some(Geometrie { bx0, by0, bw, bh, outside, .. }) = geometrie(line, r) else { return out };
    let cover = couverture_de_la_boite(&outside, bw, bh, r);
    for y in 0..bh {
        for x in 0..bw {
            out[(by0 + y) * w + bx0 + x] = cover[y * bw + x];
        }
    }
    out
}

/// La forme du fond sur la toile infinie : une couverture de 0 à 255 pour chaque pixel du rectangle de travail, qui
/// déborde de la feuille là où le trait en sort (toile infinie : l'aplat suit le trait partout où il est, et n'est pas
/// coupé au bord de la feuille). Pour un trait qui reste dans la feuille, le résultat est celui de `silhouette`, pixel
/// pour pixel.
pub fn silhouette_monde(line: &Raster, r: &FondReglages) -> Couverture {
    let Some(GeometrieMonde { x0, y0, w, h, outside, .. }) = geometrie_dans(line, r, true) else { return Couverture::default() };
    Couverture { x0, y0, w, h, data: couverture_de_la_boite(&outside, w, h, r) }
}

/// La forme d'un fond sur la toile infinie : la couverture (0 à 255) des pixels du rectangle `x0, y0, w × h` du monde ;
/// rien (0) partout ailleurs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Couverture {
    pub x0: i64,
    pub y0: i64,
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl Couverture {
    /// La forme `data` d'une feuille `w × h` (ce que rend `silhouette`).
    pub fn depuis_feuille(w: usize, h: usize, data: Vec<u8>) -> Couverture {
        Couverture { x0: 0, y0: 0, w, h, data }
    }

    /// La couverture du pixel (`x`, `y`) du monde.
    pub fn get(&self, x: i64, y: i64) -> u8 {
        let (dx, dy) = (x - self.x0, y - self.y0);
        if dx < 0 || dy < 0 || dx >= self.w as i64 || dy >= self.h as i64 {
            return 0;
        }
        self.data.get(dy as usize * self.w + dx as usize).copied().unwrap_or(0)
    }

    /// Rien n'est couvert.
    pub fn est_vide(&self) -> bool {
        self.data.iter().all(|&c| c == 0)
    }

    /// La part qui est dans la feuille `w × h`, un octet par pixel (la forme de `silhouette`).
    pub fn feuille(&self, w: usize, h: usize) -> Vec<u8> {
        let mut out = vec![0u8; w * h];
        let (x0, y0) = (self.x0.max(0) as usize, self.y0.max(0) as usize);
        let (x1, y1) = (((self.x0 + self.w as i64).max(0) as usize).min(w), ((self.y0 + self.h as i64).max(0) as usize).min(h));
        for y in y0..y1 {
            for x in x0..x1 {
                out[y * w + x] = self.data[(y as i64 - self.y0) as usize * self.w + (x as i64 - self.x0) as usize];
            }
        }
        out
    }
}

/// Pixels du trait que gagne une zone remplie ou coupée dans le fond (pas de liseré au bord du
/// trait).
pub const UNDER: usize = 2;

/// La zone fermée par le trait `line` où l'on a cliqué (`x`, `y`) : les pixels qu'on atteint sans
/// traverser le trait (ses trous jusqu'à `2 × trous` pixels fermés), jusqu'au trait, plus
/// `UNDER` pixels sous lui. Un clic hors du rectangle du trait vise le dehors. Rien s'il n'y a
/// pas de trait, ou si le clic tombe sur le trait lui-même. Le masque a la taille du dessin.
pub fn zone(line: &Raster, x: usize, y: usize, trous: u8) -> Option<Vec<bool>> {
    let (w, h) = (line.width, line.height);
    if x >= w || y >= h {
        return None;
    }
    let solid_at = |gx: usize, gy: usize| line.get(gx, gy).a() >= LINE_ALPHA;
    if solid_at(x, y) {
        return None;
    }
    // Le rectangle du trait, agrandi : on ne calcule que dedans (le dehors continue au-delà).
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for (idx, t) in line.tiles.iter().enumerate() {
        let Some(t) = t else { continue };
        let (ox, oy, tw, th) = line.tile_bounds(idx);
        for ty in 0..th {
            for tx in 0..tw {
                if t[ty * TILE + tx].a() >= LINE_ALPHA {
                    x0 = x0.min(ox + tx);
                    y0 = y0.min(oy + ty);
                    x1 = x1.max(ox + tx);
                    y1 = y1.max(oy + ty);
                }
            }
        }
    }
    if x0 == usize::MAX {
        return None;
    }
    let pad = trous as usize + UNDER + 3;
    let (bx0, by0) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
    let (bx1, by1) = ((x1 + pad).min(w - 1), (y1 + pad).min(h - 1));
    let (bw, bh) = (bx1 - bx0 + 1, by1 - by0 + 1);
    let mut solid = vec![false; bw * bh];
    for by in 0..bh {
        for bx in 0..bw {
            solid[by * bw + bx] = solid_at(bx0 + bx, by0 + by);
        }
    }
    let d_line = edt(&solid, bw, bh);
    let t2 = (trous as f64) * (trous as f64);
    let wall: Vec<bool> = d_line.iter().map(|&d| d <= t2).collect();
    let inside_box = (bx0..=bx1).contains(&x) && (by0..=by1).contains(&y);
    let mut region = vec![false; bw * bh];
    let mut stack: Vec<usize> = Vec::new();
    if inside_box {
        // Un clic tout près du trait (dans la fermeture des trous) : le pixel libre le plus proche.
        let (cx, cy) = (x - bx0, y - by0);
        let r = trous as usize + 1;
        let start = (cy.saturating_sub(r)..=(cy + r).min(bh - 1))
            .flat_map(|py| (cx.saturating_sub(r)..=(cx + r).min(bw - 1)).map(move |px| (px, py)))
            .filter(|&(px, py)| !wall[py * bw + px])
            .min_by_key(|&(px, py)| px.abs_diff(cx).pow(2) + py.abs_diff(cy).pow(2))?;
        stack.push(start.1 * bw + start.0);
    } else {
        // Le dehors : depuis le bord du rectangle.
        for bx in 0..bw {
            stack.extend([bx, (bh - 1) * bw + bx]);
        }
        for by in 0..bh {
            stack.extend([by * bw, by * bw + bw - 1]);
        }
        stack.retain(|&p| !wall[p]);
    }
    for &p in &stack {
        region[p] = true;
    }
    while let Some(p) = stack.pop() {
        let (px, py) = (p % bw, p / bw);
        let near = [(px > 0).then(|| p - 1), (px + 1 < bw).then(|| p + 1), (py > 0).then(|| p - bw), (py + 1 < bh).then(|| p + bw)];
        for q in near.into_iter().flatten() {
            if !wall[q] && !region[q] {
                region[q] = true;
                stack.push(q);
            }
        }
    }
    // La zone regagne la bande de fermeture jusqu'au trait, puis passe un peu sous lui.
    let grow = |region: &mut Vec<bool>, times: usize, can: &dyn Fn(usize) -> bool| {
        for _ in 0..times {
            let before = region.clone();
            for p in 0..bw * bh {
                if before[p] || !can(p) {
                    continue;
                }
                let (px, py) = (p % bw, p / bw);
                let near = [(px > 0).then(|| p - 1), (px + 1 < bw).then(|| p + 1), (py > 0).then(|| p - bw), (py + 1 < bh).then(|| p + bw)];
                if near.into_iter().flatten().any(|q| before[q]) {
                    region[p] = true;
                }
            }
        }
    };
    grow(&mut region, trous as usize, &|p| !solid[p]);
    grow(&mut region, UNDER, &|p| solid[p]);
    let mut out = vec![false; w * h];
    for by in 0..bh {
        for bx in 0..bw {
            out[(by0 + by) * w + bx0 + bx] = region[by * bw + bx];
        }
    }
    // Le dehors continue hors du rectangle (sans effet sur le fond, qui n'y est pas) : la zone
    // est le dehors si l'on a cliqué hors du rectangle, ou si elle en atteint le bord (elle fuit
    // par un trou trop grand).
    let on_border = |p: usize| {
        let (px, py) = (p % bw, p / bw);
        px == 0 || py == 0 || px + 1 == bw || py + 1 == bh
    };
    let outside = !inside_box || (0..bw * bh).any(|p| region[p] && on_border(p));
    if outside {
        for gy in 0..h {
            for gx in 0..w {
                if !(bx0..=bx1).contains(&gx) || !(by0..=by1).contains(&gy) {
                    out[gy * w + gx] = true;
                }
            }
        }
    }
    Some(out)
}

/// Pose `color` (ou efface, si rien) sur les pixels de `mask` dans `r` ; renvoie les tuiles
/// d'avant (annulation).
pub fn paint_mask(r: &mut Raster, mask: &[bool], color: Option<Color32>) -> Vec<(usize, Option<crate::raster::Tile>)> {
    let w = r.width;
    let mut old = Vec::new();
    for idx in 0..r.tiles.len() {
        let (ox, oy, tw, th) = r.tile_bounds(idx);
        let touched = (0..th).any(|ty| (0..tw).any(|tx| mask[(oy + ty) * w + ox + tx]));
        if !touched || (color.is_none() && r.tiles[idx].is_none()) {
            continue;
        }
        old.push((idx, r.tiles[idx].clone()));
        let tile = r.tile_mut(idx);
        for ty in 0..th {
            for tx in 0..tw {
                if mask[(oy + ty) * w + ox + tx] {
                    tile[ty * TILE + tx] = color.unwrap_or(Color32::TRANSPARENT);
                }
            }
        }
    }
    old
}

/// L'étape d'annulation d'un trait peint dans la strate Fond : ses tuiles sont celles de la
/// peinture du fond, ou de sa coupe (`coupe`) ; la création éventuelle du dessin, avant, reste
/// telle quelle.
pub fn to_fond_edit(edit: crate::history::Edit, coupe: bool) -> crate::history::Edit {
    use crate::history::Edit;
    match edit {
        Edit::Tiles { cel, tiles } => Edit::FondTiles { cel, tiles, coupe },
        // (Le trait est la dernière étape du groupe.)
        Edit::Group(mut edits) => {
            if let Some(last) = edits.pop() {
                edits.push(to_fond_edit(last, coupe));
            }
            Edit::Group(edits)
        }
        other => other,
    }
}

/// La forme `cover` en dessin (blanc, d'opacité la couverture) : le pochoir qui garde dans la
/// forme ce qu'on peint dans le fond.
pub fn mask(cover: &[u8], width: usize, height: usize) -> Raster {
    let mut out = Raster::new(width, height);
    for (p, &c) in cover.iter().enumerate() {
        if c != 0 {
            let (x, y) = (p % width, p / width);
            let idx = out.tile_index_of(x, y);
            out.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = Color32::from_rgba_premultiplied(c, c, c, c);
        }
    }
    out
}

/// Un pixel du fond : couverture `c` de la forme, coupée de `cut` (gomme dans la strate Fond),
/// de la couleur `couleur`, avec par-dessus, dans la forme seulement, la peinture `paint`.
fn compose_px(c: u8, cut: u8, couleur: [u8; 3], paint: Color32) -> Color32 {
    let c = ((c as u32 * (255 - cut as u32) + 127) / 255) as u8;
    if c == 0 {
        return Color32::TRANSPARENT;
    }
    let fill = premultiply(couleur[0], couleur[1], couleur[2], c);
    if paint.a() == 0 {
        return fill;
    }
    // Peinture « par-dessus, dans la forme » : couleur = peinture × couverture + fond ×
    // (1 − opacité de la peinture), opacité = celle du fond.
    let (fa, sa) = (c as f32 / 255.0, paint.a() as f32 / 255.0);
    let mix = |sc: u8, fc: u8| (sc as f32 * fa + fc as f32 * (1.0 - sa)).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(mix(paint.r(), fill.r()), mix(paint.g(), fill.g()), mix(paint.b(), fill.b()), c)
}

/// Le fond à afficher sous le trait : la forme `cover` de la couleur `couleur`, moins ce qu'on en
/// a coupé (`cut`), et par-dessus, dans la forme seulement, ce qu'on a peint dans le fond (`paint`).
pub fn compose(cover: &[u8], width: usize, height: usize, couleur: [u8; 3], paint: Option<&Raster>, cut: Option<&Raster>) -> Raster {
    let mut out = Raster::new(width, height);
    for idx in 0..out.tiles.len() {
        compose_tile(&mut out, idx, cover, couleur, paint, cut);
    }
    out
}

/// Comme `compose`, sur la toile infinie : la forme `cover` peut sortir de la feuille, le fond aussi (ses tuiles hors de la
/// feuille sont rangées à part dans le dessin, `Raster::outer`). Seules les tuiles que la forme touche sont refaites.
pub fn compose_monde(cover: &Couverture, width: usize, height: usize, couleur: [u8; 3], paint: Option<&Raster>, cut: Option<&Raster>) -> Raster {
    let mut out = Raster::new(width, height);
    if cover.w == 0 || cover.h == 0 {
        return out;
    }
    let t = TILE as i64;
    for ty in cover.y0.div_euclid(t)..=(cover.y0 + cover.h as i64 - 1).div_euclid(t) {
        for tx in cover.x0.div_euclid(t)..=(cover.x0 + cover.w as i64 - 1).div_euclid(t) {
            let id = out.id_of(tx as i32, ty as i32);
            compose_tuile_monde(&mut out, id, cover, couleur, paint, cut);
        }
    }
    out
}

/// `compose_monde` sur la seule tuile `id` de `out` (de la feuille ou hors d'elle) : suivi en direct pendant qu'on peint
/// dans le fond. La tuile entière est refaite (une tuile de bord garde sa part hors de la feuille).
pub fn compose_tuile_monde(out: &mut Raster, id: usize, cover: &Couverture, couleur: [u8; 3], paint: Option<&Raster>, cut: Option<&Raster>) {
    let (tx, ty) = out.key_of(id);
    let (ox, oy) = (tx as i64 * TILE as i64, ty as i64 * TILE as i64);
    let mut any = false;
    let mut tile = [Color32::TRANSPARENT; TILE * TILE];
    for y in 0..TILE {
        for x in 0..TILE {
            let (gx, gy) = (ox + x as i64, oy + y as i64);
            let c = cover.get(gx, gy);
            if c == 0 {
                continue;
            }
            let k = cut.map_or(0, |r| r.get_i(gx, gy).a());
            let s = paint.map_or(Color32::TRANSPARENT, |r| r.get_i(gx, gy));
            let px = compose_px(c, k, couleur, s);
            if px.a() != 0 {
                tile[y * TILE + x] = px;
                any = true;
            }
        }
    }
    out.replace_slot(id, any.then(|| crate::raster::Tile::new(tile)));
}

/// `compose` sur la seule tuile `idx` de `out` (suivi en direct pendant qu'on peint dans le fond).
pub fn compose_tile(out: &mut Raster, idx: usize, cover: &[u8], couleur: [u8; 3], paint: Option<&Raster>, cut: Option<&Raster>) {
    let width = out.width;
    let (ox, oy, tw, th) = out.tile_bounds(idx);
    let mut any = false;
    let mut tile = [Color32::TRANSPARENT; TILE * TILE];
    for y in 0..th {
        for x in 0..tw {
            let (gx, gy) = (ox + x, oy + y);
            let c = cover[gy * width + gx];
            if c == 0 {
                continue;
            }
            let k = cut.map_or(0, |r| r.get(gx, gy).a());
            let s = paint.map_or(Color32::TRANSPARENT, |r| r.get(gx, gy));
            let px = compose_px(c, k, couleur, s);
            if px.a() != 0 {
                tile[y * TILE + x] = px;
                any = true;
            }
        }
    }
    if any {
        out.tiles[idx] = Some(crate::raster::Tile::new(tile));
    } else {
        out.tiles[idx] = None;
    }
}
