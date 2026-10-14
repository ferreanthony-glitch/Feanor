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

    pub fn get(&self, x: usize, y: usize) -> Color32 {
        match &self.tiles[self.tile_index_of(x, y)] {
            Some(t) => t[(y % TILE) * TILE + x % TILE],
            None => Color32::TRANSPARENT,
        }
    }

    pub fn tile_mut(&mut self, idx: usize) -> &mut [Color32; TILE_PIXELS] {
        self.tiles[idx].get_or_insert_with(empty_tile)
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
    pub fn to_rgba(&self) -> Vec<u8> {
        self.to_flat().into_iter().flat_map(unpremultiply).collect()
    }

    /// Construit l'image depuis des octets RGBA non prémultipliés ; les zones vides restent sans tuile.
    pub fn from_rgba(width: usize, height: usize, data: &[u8]) -> Self {
        let mut raster = Self::new(width, height);
        for idx in 0..raster.tiles.len() {
            let (ox, oy, tw, th) = raster.tile_bounds(idx);
            let mut tile: Option<Tile> = None;
            for y in 0..th {
                for x in 0..tw {
                    let i = ((oy + y) * width + ox + x) * 4;
                    if data[i + 3] == 0 {
                        continue;
                    }
                    let t = tile.get_or_insert_with(empty_tile);
                    t[y * TILE + x] = premultiply(data[i], data[i + 1], data[i + 2], data[i + 3]);
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
