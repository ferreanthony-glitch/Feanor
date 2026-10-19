use eframe::egui::Color32;

/// Côté d'une tuile, en pixels.
pub const TILE: usize = 64;
pub const TILE_PIXELS: usize = TILE * TILE;

pub type Tile = Box<[Color32; TILE_PIXELS]>;

pub fn empty_tile() -> Tile {
    Box::new([Color32::TRANSPARENT; TILE_PIXELS])
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

/// Image RGBA prémultipliée, découpée en tuiles allouées seulement là où l'on a dessiné.
/// Un dessin au trait n'occupe ainsi qu'une petite partie de la mémoire d'une image pleine.
#[derive(Clone)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    pub tiles_x: usize,
    pub tiles: Vec<Option<Tile>>,
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

    pub fn tile_mut(&mut self, idx: usize) -> &mut [Color32; TILE_PIXELS] {
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
                let t = tile.get_or_insert_with(empty_tile);
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
