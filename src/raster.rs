use eframe::egui::Color32;

/// Côté d'une tuile, en pixels.
pub const TILE: usize = 64;
pub const TILE_PIXELS: usize = TILE * TILE;

/// Une tuile de pixels, **partagée à la copie** : cloner une tuile (ou un `Raster`, un `Cel`) ne
/// recopie rien, les deux se partagent les 16 Ko ; la première écriture (`DerefMut`) en fait une
/// copie à soi si la tuile est partagée (`Arc::make_mut`). Les valeurs se comportent comme celles
/// d'un `Box` : ce que l'un écrit, l'autre ne le voit jamais. Ce qui change : le cliché d'un
/// enregistrement (`enregistrement.rs`) et les étapes d'annulation ne coûtent plus une copie des
/// pixels, seulement l'écriture qui suit sur une tuile encore partagée.
#[derive(Clone)]
pub struct Tile(std::sync::Arc<[Color32; TILE_PIXELS]>);

impl Tile {
    /// Une tuile des pixels `pixels`.
    pub fn new(pixels: [Color32; TILE_PIXELS]) -> Self {
        Tile(std::sync::Arc::new(pixels))
    }

    /// Les deux tuiles sont la même mémoire (rien n'a été écrit depuis la copie).
    pub fn same_as(&self, other: &Tile) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for Tile {
    type Target = [Color32; TILE_PIXELS];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Tile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        std::sync::Arc::make_mut(&mut self.0)
    }
}

impl PartialEq for Tile {
    fn eq(&self, other: &Self) -> bool {
        self.same_as(other) || self.0[..] == other.0[..]
    }
}

impl std::fmt::Debug for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tile")
    }
}

pub fn empty_tile() -> Tile {
    Tile::new([Color32::TRANSPARENT; TILE_PIXELS])
}

/// Nombre de tuiles en largeur pour une image de `width` pixels.
pub fn tiles_across(width: usize) -> usize {
    width.div_ceil(TILE)
}

/// Origine et taille utile de la tuile `idx` (les tuiles du bord peuvent déborder de l'image).
pub fn tile_bounds(width: usize, height: usize, idx: usize) -> (usize, usize, usize, usize) {
    let tx = tiles_across(width);
    let ox = (idx % tx) * TILE;
    let oy = (idx / tx) * TILE;
    (ox, oy, TILE.min(width - ox), TILE.min(height - oy))
}

/// Toile infinie : on peut dessiner n'importe où autour de la feuille, jusqu'à `WORLD` pixels de
/// l'origine (de chaque côté, coordonnées négatives comprises). Bornée pour que les numéros de
/// tuiles tiennent dans un entier ; 32 768 px, c'est dix feuilles de 3 840 px.
pub const WORLD: i64 = 1 << 15;

/// Les numéros de tuile (`usize`) qui circulent (annulation, tuiles touchées, textures) sont des rangs
/// de `Raster::tiles` pour les tuiles de la fenêtre (la feuille), et, à partir de `OUTER`, le
/// codage des coordonnées signées d'une tuile hors de la feuille (`outer_id`).
pub const OUTER: usize = 1 << 40;
const BIAS: i64 = 1 << 19;

/// Numéro de la tuile (`tx`, `ty`) hors de la fenêtre d'un dessin (coordonnées de tuile signées).
pub fn outer_id(tx: i32, ty: i32) -> usize {
    OUTER | ((((ty as i64 + BIAS) as usize) & 0xF_FFFF) << 20) | (((tx as i64 + BIAS) as usize) & 0xF_FFFF)
}

/// Le numéro est celui d'une tuile hors de la fenêtre (`outer_id`).
pub fn is_outer_id(id: usize) -> bool {
    id >= OUTER
}

/// Coordonnées de tuile d'un numéro `outer_id`.
pub fn outer_key(id: usize) -> (i32, i32) {
    let v = id & (OUTER - 1);
    (((v & 0xF_FFFF) as i64 - BIAS) as i32, ((v >> 20) as i64 - BIAS) as i32)
}

/// Image RGBA prémultipliée, découpée en tuiles allouées seulement là où l'on a dessiné.
/// Un dessin au trait n'occupe ainsi qu'une petite partie de la mémoire d'une image pleine.
///
/// Deux espaces de tuiles : la **fenêtre** (`tiles`, la feuille, tuiles entières : rang = ligne ×
/// `tiles_x` + colonne, comme toujours) et le **hors-feuille** (`outer`, toute tuile hors de la
/// fenêtre, par coordonnées de tuile signées : la toile infinie). Le code qui ne connaît que la
/// feuille ne voit que `tiles` ; le dessin, l'affichage, l'export et l'enregistrement voient les deux
/// par `slot`, `tile_at`, `get_i`…
#[derive(Clone)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    pub tiles_x: usize,
    pub tiles: Vec<Option<Tile>>,
    pub outer: std::collections::BTreeMap<(i32, i32), Tile>,
}

impl Raster {
    pub fn new(width: usize, height: usize) -> Self {
        let tiles_x = tiles_across(width);
        let tiles_y = height.div_ceil(TILE);
        Self {
            width,
            height,
            tiles_x,
            tiles: vec![None; tiles_x * tiles_y],
            outer: Default::default(),
        }
    }

    /// Nombre de rangées de tuiles de la fenêtre.
    pub fn tiles_y(&self) -> usize {
        self.tiles.len().checked_div(self.tiles_x).unwrap_or(0)
    }

    /// La tuile (`tx`, `ty`) est dans la fenêtre (la feuille).
    pub fn in_window(&self, tx: i32, ty: i32) -> bool {
        tx >= 0 && ty >= 0 && (tx as usize) < self.tiles_x && (ty as usize) < self.tiles_y()
    }

    /// Numéro de la tuile (`tx`, `ty`) : son rang dans la fenêtre, sinon son codage hors-feuille.
    pub fn id_of(&self, tx: i32, ty: i32) -> usize {
        if self.in_window(tx, ty) {
            ty as usize * self.tiles_x + tx as usize
        } else {
            outer_id(tx, ty)
        }
    }

    /// Coordonnées de tuile du numéro `id`.
    pub fn key_of(&self, id: usize) -> (i32, i32) {
        if is_outer_id(id) {
            outer_key(id)
        } else {
            let tx = self.tiles_x.max(1);
            ((id % tx) as i32, (id / tx) as i32)
        }
    }

    /// Coin haut gauche, en pixels, de la tuile `id`.
    pub fn tile_origin(&self, id: usize) -> (i64, i64) {
        let (tx, ty) = self.key_of(id);
        (tx as i64 * TILE as i64, ty as i64 * TILE as i64)
    }

    /// La tuile `id`, si elle est dessinée.
    pub fn slot(&self, id: usize) -> Option<&Tile> {
        if is_outer_id(id) {
            self.outer.get(&outer_key(id))
        } else {
            self.tiles.get(id)?.as_ref()
        }
    }

    /// La tuile (`tx`, `ty`), si elle est dessinée.
    pub fn tile_at(&self, tx: i32, ty: i32) -> Option<&Tile> {
        self.slot(self.id_of(tx, ty))
    }

    /// Remplace la tuile `id` (rien : la vide) et rend l'ancienne. Un numéro de fenêtre hors de la
    /// fenêtre est ignoré.
    pub fn replace_slot(&mut self, id: usize, tile: Option<Tile>) -> Option<Tile> {
        if is_outer_id(id) {
            let key = outer_key(id);
            match tile {
                Some(t) => self.outer.insert(key, t),
                None => self.outer.remove(&key),
            }
        } else {
            match self.tiles.get_mut(id) {
                Some(slot) => std::mem::replace(slot, tile),
                None => None,
            }
        }
    }

    /// Le pixel (`x`, `y`) du monde (négatif ou hors de la feuille compris) ; transparent hors d'une
    /// tuile dessinée.
    pub fn get_i(&self, x: i64, y: i64) -> Color32 {
        let (tx, ty) = (x.div_euclid(TILE as i64), y.div_euclid(TILE as i64));
        if tx.abs() > WORLD || ty.abs() > WORLD {
            return Color32::TRANSPARENT;
        }
        match self.tile_at(tx as i32, ty as i32) {
            Some(t) => t[y.rem_euclid(TILE as i64) as usize * TILE + x.rem_euclid(TILE as i64) as usize],
            None => Color32::TRANSPARENT,
        }
    }

    /// Les numéros de toutes les tuiles dessinées : la fenêtre, puis le hors-feuille.
    pub fn drawn_ids(&self) -> Vec<usize> {
        let mut out: Vec<usize> = self.tiles.iter().enumerate().filter(|(_, t)| t.is_some()).map(|(i, _)| i).collect();
        out.extend(self.outer.keys().map(|&(tx, ty)| outer_id(tx, ty)));
        out
    }

    /// Le rectangle de tuiles qui contient tout ce qui est dessiné, fenêtre et hors-feuille :
    /// (tuile gauche, haute, droite, basse), bornes comprises.
    pub fn drawn_tile_range(&self) -> Option<(i32, i32, i32, i32)> {
        let mut found: Option<(i32, i32, i32, i32)> = None;
        let mut add = |tx: i32, ty: i32| {
            found = Some(match found {
                None => (tx, ty, tx, ty),
                Some((a, b, c, d)) => (a.min(tx), b.min(ty), c.max(tx), d.max(ty)),
            });
        };
        for (idx, t) in self.tiles.iter().enumerate() {
            if t.is_some() {
                add((idx % self.tiles_x) as i32, (idx / self.tiles_x) as i32);
            }
        }
        for &(tx, ty) in self.outer.keys() {
            add(tx, ty);
        }
        found
    }

    /// Aucune tuile dessinée, dans la fenêtre comme hors d'elle.
    pub fn is_blank(&self) -> bool {
        self.outer.is_empty() && self.tiles.iter().all(Option::is_none)
    }

    /// La tuile (`tx`, `ty`) de la fenêtre déborde de la feuille (dernière colonne ou dernière
    /// rangée, quand la feuille n'est pas un multiple de la tuile).
    fn straddles(&self, tx: usize, ty: usize) -> bool {
        (tx + 1) * TILE > self.width || (ty + 1) * TILE > self.height
    }

    /// Le pixel (`x`, `y`) de la tuile (`tx`, `ty`) est hors de la feuille.
    fn beyond(&self, tx: usize, ty: usize, x: usize, y: usize) -> bool {
        tx * TILE + x >= self.width || ty * TILE + y >= self.height
    }

    /// Ce qui est dessiné hors de la feuille dans la tuile (`tx`, `ty`) : une tuile hors de la
    /// fenêtre telle qu'elle est, ou, pour une tuile de bord de la fenêtre, la part qui dépasse de la
    /// feuille (le reste effacé) ; rien si la tuile n'a rien là.
    pub fn beyond_tile(&self, tx: i32, ty: i32) -> Option<Tile> {
        if !self.in_window(tx, ty) {
            return self.outer.get(&(tx, ty)).filter(|t| t.iter().any(|c| c.a() != 0)).cloned();
        }
        let (utx, uty) = (tx as usize, ty as usize);
        if !self.straddles(utx, uty) {
            return None;
        }
        let tile = self.tiles[uty * self.tiles_x + utx].as_ref()?;
        let mut masked = [Color32::TRANSPARENT; TILE_PIXELS];
        let mut any = false;
        for y in 0..TILE {
            for x in 0..TILE {
                if self.beyond(utx, uty, x, y) && tile[y * TILE + x].a() != 0 {
                    masked[y * TILE + x] = tile[y * TILE + x];
                    any = true;
                }
            }
        }
        any.then(|| Tile::new(masked))
    }

    /// Ce qui est dessiné hors de la feuille : les tuiles de `outer`, et, pour une tuile de bord, la
    /// part qui dépasse (la part de la feuille effacée), rangées par coordonnées de tuile. Ce que
    /// l'enregistrement garde en plus du PNG de la feuille.
    pub fn beyond_sheet_tiles(&self) -> Vec<((i32, i32), Tile)> {
        let mut out: Vec<((i32, i32), Tile)> = self.outer.keys().filter_map(|&(tx, ty)| Some(((tx, ty), self.beyond_tile(tx, ty)?))).collect();
        for idx in 0..self.tiles.len() {
            let (tx, ty) = ((idx % self.tiles_x) as i32, (idx / self.tiles_x) as i32);
            if self.tiles[idx].is_some() && self.straddles(tx as usize, ty as usize) {
                if let Some(t) = self.beyond_tile(tx, ty) {
                    out.push(((tx, ty), t));
                }
            }
        }
        out.sort_by_key(|(k, _)| *k);
        out
    }

    /// Y a-t-il quelque chose de dessiné hors de la feuille ?
    pub fn has_beyond(&self) -> bool {
        if self.outer.values().any(|t| t.iter().any(|c| c.a() != 0)) {
            return true;
        }
        self.tiles.iter().enumerate().any(|(idx, slot)| {
            let Some(tile) = slot else { return false };
            let (tx, ty) = (idx % self.tiles_x, idx / self.tiles_x);
            self.straddles(tx, ty) && (0..TILE_PIXELS).any(|i| self.beyond(tx, ty, i % TILE, i / TILE) && tile[i].a() != 0)
        })
    }

    /// Remet une tuile lue de l'enregistrement (`beyond_sheet_tiles`) : hors de la fenêtre, elle va
    /// dans `outer` ; sur une tuile de bord, ses pixels hors de la feuille sont posés dans la tuile
    /// de la fenêtre.
    pub fn absorb_beyond(&mut self, tx: i32, ty: i32, tile: Tile) {
        if !self.in_window(tx, ty) {
            if (tx as i64).abs() <= WORLD / TILE as i64 && (ty as i64).abs() <= WORLD / TILE as i64 {
                self.outer.insert((tx, ty), tile);
            }
            return;
        }
        let (utx, uty) = (tx as usize, ty as usize);
        let mut wanted = Vec::new();
        for i in 0..TILE_PIXELS {
            if self.beyond(utx, uty, i % TILE, i / TILE) && tile[i].a() != 0 {
                wanted.push(i);
            }
        }
        if wanted.is_empty() {
            return;
        }
        let id = uty * self.tiles_x + utx;
        let target = self.tile_mut(id);
        for i in wanted {
            target[i] = tile[i];
        }
    }

    pub fn tile_bounds(&self, idx: usize) -> (usize, usize, usize, usize) {
        tile_bounds(self.width, self.height, idx)
    }

    pub fn tile_index_of(&self, x: usize, y: usize) -> usize {
        (y / TILE) * self.tiles_x + x / TILE
    }

    /// Le pixel (`x`, `y`) ; transparent hors du dessin. Un dessin n'a pas toujours la taille de la
    /// feuille (aperçu réduit d'un effet, feuille flottante élargie, image de vidéo, dessin d'un
    /// projet abîmé) : lire un point de la feuille dedans ne doit jamais fermer SHOT (le lire sans
    /// ce contrôle sortait des tuiles, `raster.rs:57`, 2026-10-18).
    pub fn get(&self, x: usize, y: usize) -> Color32 {
        if x >= self.width || y >= self.height {
            return Color32::TRANSPARENT;
        }
        match self.tiles.get(self.tile_index_of(x, y)) {
            Some(Some(t)) => t[(y % TILE) * TILE + x % TILE],
            _ => Color32::TRANSPARENT,
        }
    }

    /// Les numéros des tuiles où `self` ou `other` a quelque chose (fenêtre et hors-feuille) : ce qu'une
    /// image refaite peut avoir changé.
    pub fn ids_either(&self, other: &Raster) -> Vec<usize> {
        let mut out: Vec<usize> = (0..self.tiles.len().max(other.tiles.len()))
            .filter(|&i| self.tiles.get(i).is_some_and(Option::is_some) || other.tiles.get(i).is_some_and(Option::is_some))
            .collect();
        let keys: std::collections::BTreeSet<(i32, i32)> = self.outer.keys().chain(other.outer.keys()).copied().collect();
        out.extend(keys.into_iter().map(|(tx, ty)| outer_id(tx, ty)));
        out
    }

    /// Les numéros des tuiles qui recouvrent le rectangle `x0..x1` × `y0..y1` (pixels du monde,
    /// bornes ramenées dans le monde) : celles de la fenêtre par leur rang, les autres par leur
    /// codage hors-feuille.
    pub fn ids_covering(&self, x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<usize> {
        let (lo, hi) = (-WORLD, WORLD);
        let (x0, y0, x1, y1) = (x0.clamp(lo, hi), y0.clamp(lo, hi), x1.clamp(lo, hi), y1.clamp(lo, hi));
        if x0 >= x1 || y0 >= y1 {
            return Vec::new();
        }
        let t = TILE as i64;
        let mut out = Vec::new();
        for ty in y0.div_euclid(t)..=(y1 - 1).div_euclid(t) {
            for tx in x0.div_euclid(t)..=(x1 - 1).div_euclid(t) {
                out.push(self.id_of(tx as i32, ty as i32));
            }
        }
        out
    }

    /// La tuile `idx` (rang de la fenêtre, ou numéro hors-feuille), créée vide au besoin.
    pub fn tile_mut(&mut self, idx: usize) -> &mut [Color32; TILE_PIXELS] {
        if is_outer_id(idx) {
            return self.outer.entry(outer_key(idx)).or_insert_with(empty_tile);
        }
        self.tiles[idx].get_or_insert_with(empty_tile)
    }

    /// Pose `src` (même taille, prémultipliée) par-dessus cette image.
    pub fn composite_over(&mut self, src: &Raster) {
        for (idx, tile) in src.tiles.iter().enumerate() {
            let Some(tile) = tile else { continue };
            if idx >= self.tiles.len() {
                break;
            }
            let dst = self.tile_mut(idx);
            for (d, &s) in dst.iter_mut().zip(tile.iter()) {
                if s.a() != 0 {
                    *d = crate::render::over(*d, s, 255, None);
                }
            }
        }
    }

    /// Même image dans une feuille `width`×`height`, décalée de (`dx`, `dy`) pixels.
    pub fn placed_in(&self, width: usize, height: usize, dx: usize, dy: usize) -> Raster {
        let mut out = Raster::new(width, height);
        for (idx, tile) in self.tiles.iter().enumerate() {
            let Some(tile) = tile else { continue };
            let (ox, oy, tw, th) = self.tile_bounds(idx);
            for y in 0..th {
                for x in 0..tw {
                    let c = tile[y * TILE + x];
                    let (nx, ny) = (ox + x + dx, oy + y + dy);
                    if c.a() == 0 || nx >= width || ny >= height {
                        continue;
                    }
                    let i = out.tile_index_of(nx, ny);
                    out.tile_mut(i)[(ny % TILE) * TILE + nx % TILE] = c;
                }
            }
        }
        out
    }

    /// Même image, agrandie ou réduite `s` fois, dans une feuille `width`×`height` (le coin en
    /// haut à gauche reste en place). Seule la partie dessinée est recalculée.
    pub fn scaled(&self, width: usize, height: usize, s: f32) -> Raster {
        let mut out = Raster::new(width, height);
        // Rectangle des tuiles dessinées.
        let mut bbox: Option<(usize, usize, usize, usize)> = None;
        for (idx, t) in self.tiles.iter().enumerate() {
            if t.is_none() {
                continue;
            }
            let (ox, oy, tw, th) = self.tile_bounds(idx);
            bbox = Some(match bbox {
                None => (ox, oy, ox + tw, oy + th),
                Some((a, b, c, d)) => (a.min(ox), b.min(oy), c.max(ox + tw), d.max(oy + th)),
            });
        }
        let Some((x0, y0, x1, y1)) = bbox else { return out };
        let (bw, bh) = (x1 - x0, y1 - y0);
        let mut rgba = vec![0u8; bw * bh * 4];
        for y in 0..bh {
            for x in 0..bw {
                let p = unpremultiply(self.get(x0 + x, y0 + y));
                rgba[(y * bw + x) * 4..(y * bw + x) * 4 + 4].copy_from_slice(&p);
            }
        }
        let Some(img) = image::RgbaImage::from_raw(bw as u32, bh as u32, rgba) else { return out };
        let (nw, nh) = (((bw as f32 * s).round() as u32).max(1), ((bh as f32 * s).round() as u32).max(1));
        let filter = if s < 1.0 { image::imageops::FilterType::Triangle } else { image::imageops::FilterType::CatmullRom };
        let img = image::imageops::resize(&img, nw, nh, filter);
        let (dx, dy) = ((x0 as f32 * s).round() as usize, (y0 as f32 * s).round() as usize);
        for (y, row) in img.rows().enumerate() {
            let sy = dy + y;
            if sy >= height {
                break;
            }
            for (x, p) in row.enumerate() {
                let sx = dx + x;
                if sx >= width || p[3] == 0 {
                    continue;
                }
                let i = out.tile_index_of(sx, sy);
                out.tile_mut(i)[(sy % TILE) * TILE + sx % TILE] = premultiply(p[0], p[1], p[2], p[3]);
            }
        }
        out
    }

    /// Copie l'image dans un tableau plein, ligne par ligne.
    pub fn to_flat(&self) -> Vec<Color32> {
        let mut out = vec![Color32::TRANSPARENT; self.width * self.height];
        for (idx, tile) in self.tiles.iter().enumerate() {
            let Some(tile) = tile else { continue };
            let (ox, oy, tw, th) = self.tile_bounds(idx);
            for y in 0..th {
                let start = (oy + y) * self.width + ox;
                out[start..start + tw].copy_from_slice(&tile[y * TILE..y * TILE + tw]);
            }
        }
        out
    }

    /// Octets RGBA non prémultipliés (pour l'enregistrement en PNG).
    ///
    /// Seules les tuiles dessinées sont parcourues (le reste est transparent, `[0, 0, 0, 0]`) : un
    /// dessin au trait sur une grande feuille coûtait 15 ms, il en coûte moins de 2.
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.width * self.height * 4];
        for (idx, tile) in self.tiles.iter().enumerate() {
            let Some(tile) = tile else { continue };
            let (ox, oy, tw, th) = self.tile_bounds(idx);
            for y in 0..th {
                let start = ((oy + y) * self.width + ox) * 4;
                let dst = &mut out[start..start + tw * 4];
                for (d, &c) in dst.chunks_exact_mut(4).zip(&tile[y * TILE..y * TILE + tw]) {
                    d.copy_from_slice(&unpremultiply(c));
                }
            }
        }
        out
    }

    /// Construit l'image depuis des octets RGBA non prémultipliés ; les zones vides restent sans tuile.
    pub fn from_rgba(width: usize, height: usize, data: &[u8]) -> Self {
        let mut raster = Self::new(width, height);
        for idx in 0..raster.tiles.len() {
            let (ox, oy, tw, th) = raster.tile_bounds(idx);
            let mut tile: Option<Tile> = None;
            for y in 0..th {
                let start = ((oy + y) * width + ox) * 4;
                let src = &data[start..start + tw * 4];
                // Une ligne sans pixel visible : rien à poser (le cas de presque toutes).
                // (Un OU de tous les alphas, sans court-circuit : le compilateur le vectorise.)
                if src.chunks_exact(4).fold(0u8, |acc, p| acc | p[3]) == 0 {
                    continue;
                }
                // (Le tableau, pris une fois : une écriture par la tuile passerait par `make_mut`.)
                let t: &mut [Color32; TILE_PIXELS] = tile.get_or_insert_with(empty_tile);
                for (x, p) in src.chunks_exact(4).enumerate() {
                    if p[3] != 0 {
                        t[y * TILE + x] = premultiply(p[0], p[1], p[2], p[3]);
                    }
                }
            }
            raster.tiles[idx] = tile;
        }
        raster
    }
}

pub fn premultiply(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    let m = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
    Color32::from_rgba_premultiplied(m(r), m(g), m(b), a)
}

pub fn unpremultiply(c: Color32) -> [u8; 4] {
    let a = c.a() as u32;
    if a == 0 {
        return [0, 0, 0, 0];
    }
    let u = |v: u8| ((v as u32 * 255 + a / 2) / a).min(255) as u8;
    [u(c.r()), u(c.g()), u(c.b()), c.a()]
}
