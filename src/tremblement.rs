//! Tremblement du trait (Anthony, 2026-10-19 : « une option uniquement pour le vectoriel de
//! tremblement du trait, avec des paramètres possibles pour modifier l'intensité du tremblement,
//! comme dans WigglyPaint »).
//!
//! Le trait « bout » : le dessin est montré en quelques variantes légèrement déplacées, qui se
//! suivent à basse cadence (comme une animation redessinée à la main), même sur une image fixe.
//!
//! # Choix de conception
//!
//! - **Un effet de la pile du calque** (`fx.rs`, `EffectKind::Boil`, réservé aux calques
//!   vectoriels) : il hérite de tout ce que les effets ont déjà — réglages enregistrés dans le
//!   projet, clés (tout se règle image par image), calcul en arrière-plan avec la règle « sans
//!   latence », cache des dessins dérivés par jeu de paramètres, aperçu réduit pendant la lecture,
//!   envoi d'avance à la carte graphique des images qui viennent, export et vignettes.
//! - **Un champ de déplacement sur les pixels déjà rendus du dessin**, pas un rejeu du moteur de
//!   brosses : chaque variante est le dessin rendu (les pixels exacts des traits en cache,
//!   `vector.rs`) relu à travers un champ de bruit lisse. Le moteur n'est donc jamais rejoué —
//!   ni sur le fil de l'interface ni en arrière-plan — et une variante coûte une relecture du
//!   dessin (quelques millisecondes à une ou deux dizaines pour une feuille de 1920 × 1080, sur
//!   plusieurs cœurs, en ne visitant que les tuiles proches de l'encre). Toutes les brosses,
//!   texturées ou non, en profitent pareil.
//! - **Le champ** est un bruit de valeurs en deux dimensions, sur un réseau dont la maille vaut
//!   la moitié de la longueur d'onde voulue (`wavelength`) ; chaque nœud porte un vecteur de
//!   norme au plus 1 (tiré dans un disque) et une valeur d'épaisseur ; l'interpolation est une
//!   combinaison convexe (lissée en 3t² − 2t³) : le déplacement est donc continu, de classe C¹,
//!   et ne dépasse jamais l'intensité demandée. Le hasard vient d'un hachage entier de
//!   (graine, variante, nœud) : une même image donne toujours les mêmes pixels (lecture, export,
//!   défilement, d'une séance à l'autre).
//! - **La variante** d'une image est `(image / vitesse) % variantes`, `vitesse` étant le nombre
//!   d'images du projet par variante (2 à 24 ips : 12 changements par seconde).
//! - **Animer à l'arrêt** : réglage du même effet. Éteint (par défaut : une image fixe ne coûte
//!   rien au repos, une passe de performance est en cours), l'image fixe montre la variante de
//!   l'image courante. Allumé, le dessin « bout » à la cadence du réglage tant qu'on ne lit pas
//!   (`rest_frame`, `rest_wake`) : seul cela redessine l'écran, à la cadence du tremblement (12 Hz
//!   par défaut), jamais à 60 Hz.
//! - **Édition** : sur le calque courant, tant qu'un geste de dessin, de gomme, de sélection ou de
//!   transformation est en cours (`App::boil_editing`), le dessin est montré sans tremblement, tel
//!   qu'il est : on dessine et on prend ce qu'on voit. Les dessins ne changent jamais (les prises,
//!   la sélection et l'export SBP lisent la vraie géométrie) ; l'export d'images et les vignettes
//!   montrent la variante de chaque image.

use std::time::Duration;

use eframe::egui::Color32;

use crate::raster::{empty_tile, tile_bounds, Raster, TILE};

/// Rang des réglages dans les valeurs de l'effet (`fx::BOIL`), puis de la variante, que
/// `fx::steps_at` ajoute à la fin (elle dépend de l'image, pas du réglage).
pub const INTENSITY: usize = 0;
pub const FREQUENCY: usize = 1;
pub const VARIANTS: usize = 2;
pub const SPEED: usize = 3;
pub const THICKNESS: usize = 4;
pub const SEED: usize = 5;
pub const REST: usize = 6;
pub const VARIANT: usize = 7;

pub const MAX_INTENSITY: f32 = 20.0;
pub const MIN_VARIANTS: usize = 2;
pub const MAX_VARIANTS: usize = 6;
pub const MAX_SPEED: usize = 12;

/// Longueur d'onde (pixels de la feuille) à fréquence 0 % (doux) et 100 % (nerveux).
const SMOOTH: f32 = 160.0;
const JITTERY: f32 = 4.0;

/// Les réglages d'un tremblement, lus dans les valeurs d'un effet (bornés, complétés : un projet
/// abîmé ou d'une autre version ne fait pas planter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Amplitude maximale du déplacement, en pixels de la feuille.
    pub intensity: f32,
    /// 0 : déplacement très doux le long du trait ; 100 : nerveux (vagues de quelques pixels).
    pub frequency: f32,
    pub variants: usize,
    /// Images du projet par variante.
    pub speed: usize,
    /// Variation d'épaisseur du trait, 0 à 100 %.
    pub thickness: f32,
    pub seed: u32,
    /// Animer à l'arrêt.
    pub rest: bool,
    /// La variante montrée (ajoutée par `fx::steps_at`, 0 si absente).
    pub variant: usize,
}

impl Settings {
    pub fn of(values: &[f32]) -> Settings {
        Self::of_with(values, !REST_OFF.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// Comme `of`; `rest_allowed` faux : « Animer à l'arrêt » est éteint quel que soit le réglage
    /// du calque (Aide > Diagnostic des performances).
    pub fn of_with(values: &[f32], rest_allowed: bool) -> Settings {
        let at = |k: usize, default: f32| values.get(k).copied().filter(|v| v.is_finite()).unwrap_or(default);
        let variants = (at(VARIANTS, 3.0).round() as i64).clamp(MIN_VARIANTS as i64, MAX_VARIANTS as i64) as usize;
        Settings {
            intensity: at(INTENSITY, 0.0).clamp(0.0, MAX_INTENSITY),
            frequency: at(FREQUENCY, 50.0).clamp(0.0, 100.0),
            variants,
            speed: (at(SPEED, 2.0).round() as i64).clamp(1, MAX_SPEED as i64) as usize,
            thickness: at(THICKNESS, 0.0).clamp(0.0, 100.0),
            seed: (at(SEED, 1.0).round() as i64).clamp(1, 999) as u32,
            rest: rest_allowed && at(REST, 0.0) >= 0.5,
            variant: (at(VARIANT, 0.0).round().max(0.0) as usize) % variants,
        }
    }

    /// Le tremblement change-t-il quelque chose ?
    pub fn changes(&self) -> bool {
        self.intensity >= 0.05 || self.thickness > 0.0
    }
}

/// « Animer à l'arrêt » forcé éteint (réglage du diagnostic des performances).
static REST_OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_rest_off(off: bool) {
    REST_OFF.store(off, std::sync::atomic::Ordering::Relaxed);
}

/// La variante de l'image `frame` : `(image / vitesse) % variantes`.
pub fn variant_at(frame: usize, speed: usize, variants: usize) -> usize {
    (frame / speed.max(1)) % variants.max(1)
}

/// La variante à ajouter aux valeurs d'un tremblement à l'image `frame`. `rest` : l'image virtuelle
/// du repos (`rest_frame`), prise à la place de `frame` par les calques qui animent à l'arrêt.
pub fn variant_for(values: &[f32], frame: usize, rest: Option<usize>) -> usize {
    let s = Settings::of(values);
    variant_at(rest.filter(|_| s.rest).unwrap_or(frame), s.speed, s.variants)
}

/// Longueur d'onde du déplacement (pixels), de 160 (fréquence 0) à 4 (fréquence 100), en échelle
/// exponentielle : le curseur change le caractère du trait de façon régulière.
pub fn wavelength(frequency: f32) -> f32 {
    SMOOTH * (JITTERY / SMOOTH).powf((frequency / 100.0).clamp(0.0, 1.0))
}

// ---------------------------------------------------------------------- le champ

fn mix(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^ (h >> 16)
}

/// Un nombre de [−1, 1] pour le nœud (i, j), la graine et le canal `k`.
fn unit(i: i32, j: i32, seed: u32, k: u32) -> f32 {
    let h = mix((i as u32).wrapping_mul(0x8DA6_B343) ^ mix((j as u32).wrapping_mul(0xD816_3841) ^ mix(seed ^ k.wrapping_mul(0x9E37_79B9))));
    (h >> 8) as f32 / (1u32 << 23) as f32 - 1.0
}

/// Le champ de déplacement d'une variante : ses nœuds se tirent à la demande (rien n'est gardé).
#[derive(Clone, Copy, Debug)]
pub struct Field {
    /// Amplitude maximale, en pixels de la feuille.
    pub amp: f32,
    /// Maille du réseau, en pixels de la feuille.
    pub cell: f32,
    seed: u32,
}

impl Field {
    pub fn new(s: &Settings) -> Field {
        Field { amp: s.intensity, cell: (wavelength(s.frequency) / 2.0).max(2.0), seed: mix(s.seed.wrapping_mul(0x9E37_79B1) ^ (s.variant as u32 + 1).wrapping_mul(0x85EB_CA6B)) }
    }

    /// Le nœud (i, j) : un vecteur de norme au plus 1 et une valeur d'épaisseur dans [−1, 1].
    fn node(&self, i: i32, j: i32) -> [f32; 3] {
        let (mut u, mut v) = (unit(i, j, self.seed, 0), unit(i, j, self.seed, 1));
        let n2 = u * u + v * v;
        if n2 > 1.0 {
            let k = n2.sqrt().recip();
            u *= k;
            v *= k;
        }
        [u, v, unit(i, j, self.seed, 2)]
    }

    /// Le déplacement (dx, dy, en pixels de la feuille) et la valeur d'épaisseur ([−1, 1]) au point
    /// (x, y) de la feuille. La référence des essais : le calcul (`boil`) tire les nœuds d'une
    /// tuile une fois (`Lattice`) et obtient les mêmes valeurs.
    #[cfg(test)]
    pub fn at(&self, x: f32, y: f32) -> [f32; 3] {
        let (gx, gy) = (x / self.cell, y / self.cell);
        let (ix, iy) = (gx.floor(), gy.floor());
        let (i, j) = (ix as i32, iy as i32);
        let nodes = [self.node(i, j), self.node(i + 1, j), self.node(i, j + 1), self.node(i + 1, j + 1)];
        blend(&nodes, gx - ix, gy - iy, self.amp)
    }
}

/// Interpolation lissée (3t² − 2t³) des quatre nœuds d'une maille, la norme multipliée par `amp`.
/// Une combinaison convexe : la norme du déplacement reste au plus `amp`.
fn blend(n: &[[f32; 3]; 4], tx: f32, ty: f32, amp: f32) -> [f32; 3] {
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let w = [(1.0 - sx) * (1.0 - sy), sx * (1.0 - sy), (1.0 - sx) * sy, sx * sy];
    let mut out = [0.0f32; 3];
    for (node, w) in n.iter().zip(w) {
        for c in 0..3 {
            out[c] += node[c] * w;
        }
    }
    [out[0] * amp, out[1] * amp, out[2]]
}

/// Les nœuds qui couvrent un rectangle de la feuille, tirés une fois pour toute une tuile.
struct Lattice {
    i0: i32,
    j0: i32,
    nx: usize,
    nodes: Vec<[f32; 3]>,
}

impl Lattice {
    /// Les nœuds du champ pour les points de [x0, x1] × [y0, y1].
    fn covering(f: &Field, x0: f32, x1: f32, y0: f32, y1: f32) -> Lattice {
        let (i0, j0) = ((x0 / f.cell).floor() as i32, (y0 / f.cell).floor() as i32);
        let (i1, j1) = ((x1 / f.cell).floor() as i32 + 1, (y1 / f.cell).floor() as i32 + 1);
        let (nx, ny) = ((i1 - i0 + 1) as usize, (j1 - j0 + 1) as usize);
        let mut nodes = Vec::with_capacity(nx * ny);
        for j in 0..ny as i32 {
            for i in 0..nx as i32 {
                nodes.push(f.node(i0 + i, j0 + j));
            }
        }
        Lattice { i0, j0, nx, nodes }
    }

    /// Comme `Field::at`, pour un point dans le rectangle couvert.
    fn at(&self, f: &Field, x: f32, y: f32) -> [f32; 3] {
        let (gx, gy) = (x / f.cell, y / f.cell);
        let (ix, iy) = (gx.floor(), gy.floor());
        let (i, j) = ((ix as i32 - self.i0) as usize, (iy as i32 - self.j0) as usize);
        let k = j * self.nx + i;
        blend(&[self.nodes[k], self.nodes[k + 1], self.nodes[k + self.nx], self.nodes[k + self.nx + 1]], gx - ix, gy - iy, f.amp)
    }
}

// ---------------------------------------------------------------------- le calcul

/// Le dessin `raster` (réduit `1 << level` fois pour l'aperçu) relu à travers le champ de la
/// variante donnée par `values` (les réglages, puis la variante : `fx::steps_at`). Les pixels vides
/// loin de l'encre ne sont jamais visités ; le travail est réparti sur les cœurs par rangées de
/// tuiles.
pub fn boil(raster: &Raster, values: &[f32], level: u8) -> Raster {
    let s = Settings::of(values);
    if !s.changes() || raster.tiles.iter().all(Option::is_none) {
        return raster.clone();
    }
    let field = Field::new(&s);
    // Pixels de l'image réduite ↔ pixels de la feuille.
    let k = (1u32 << level) as f32;
    // Jusqu'où l'encre d'une tuile peut venir lire (le déplacement, plus un pixel de lissage).
    let reach = (s.intensity / k).ceil() as usize + 2;
    let thick = s.thickness / 100.0;
    let (w, h) = (raster.width, raster.height);
    let mut out = Raster::new(w, h);
    let tiles_x = out.tiles_x.max(1);
    crate::render::par_chunks(&mut out.tiles, tiles_x, |ty, row| {
        for (tx, slot) in row.iter_mut().enumerate() {
            let (ox, oy, tw, th) = tile_bounds(w, h, ty * tiles_x + tx);
            if !ink_near(raster, ox, oy, tw, th, reach) {
                continue;
            }
            let lattice = Lattice::covering(&field, ox as f32 * k, (ox + tw) as f32 * k, oy as f32 * k, (oy + th) as f32 * k);
            let mut tile = empty_tile();
            // (Le tableau, pris une fois : écrire par la tuile passerait chaque fois par `make_mut`.)
            let pixels: &mut [Color32; crate::raster::TILE_PIXELS] = &mut tile;
            let mut any = false;
            for y in 0..th {
                for x in 0..tw {
                    let (px, py) = ((ox + x) as f32 + 0.5, (oy + y) as f32 + 0.5);
                    let [dx, dy, n] = lattice.at(&field, px * k, py * k);
                    let mut c = crate::fxwarp::sample(raster, px + dx / k, py + dy / k);
                    if thick > 0.0 && c[3] >= 1.0 && c[3] < 254.5 {
                        c = thickened(c, thick, n);
                    }
                    let a = c[3].round().clamp(0.0, 255.0);
                    if a > 0.0 {
                        let ch = |v: f32| v.round().clamp(0.0, a) as u8;
                        pixels[y * TILE + x] = Color32::from_rgba_premultiplied(ch(c[0]), ch(c[1]), ch(c[2]), a as u8);
                        any = true;
                    }
                }
            }
            if any {
                *slot = Some(tile);
            }
        }
    });
    out
}

/// Un pixel de bord (couverture partielle) dont la couverture grimpe ou descend selon `n` ([−1, 1])
/// et `thick` (0 à 1) : le trait s'épaissit ou s'amincit d'une fraction de pixel. La couleur suit
/// (prémultipliée).
fn thickened(c: [f32; 4], thick: f32, n: f32) -> [f32; 4] {
    let a = c[3] / 255.0;
    let a2 = a.powf((1.5 * thick * n).exp2().recip());
    let m = a2 / a;
    [c[0] * m, c[1] * m, c[2] * m, a2 * 255.0]
}

/// Une tuile du dessin, à moins de `reach` pixels du rectangle donné, a-t-elle de l'encre ?
fn ink_near(r: &Raster, ox: usize, oy: usize, tw: usize, th: usize, reach: usize) -> bool {
    let (x0, y0) = (ox.saturating_sub(reach) / TILE, oy.saturating_sub(reach) / TILE);
    let (x1, y1) = (((ox + tw + reach).min(r.width).max(1) - 1) / TILE, ((oy + th + reach).min(r.height).max(1) - 1) / TILE);
    (y0..=y1).any(|ty| (x0..=x1).any(|tx| r.tiles[ty * r.tiles_x + tx].is_some()))
}

// ---------------------------------------------------------------------- le repos

/// L'image virtuelle du repos : le temps de l'horloge converti en images du projet. Les calques
/// qui animent à l'arrêt prennent la variante de cette image, pas de l'image courante.
pub fn rest_frame(now_ms: u64, fps: u32) -> usize {
    (now_ms * fps.max(1) as u64 / 1000) as usize
}

/// Dans combien de temps la prochaine variante d'un calque qui anime à l'arrêt apparaît-elle ?
/// `None` : aucun des éléments du plan n'anime à l'arrêt (l'écran ne se redessine pas). La plus
/// courte attente de tous les calques ; juste après la bascule, jamais avant.
pub fn rest_wake(items: &[crate::render::PlanItem], fps: u32, now_ms: u64) -> Option<Duration> {
    let fps = fps.max(1) as u64;
    items
        .iter()
        .flat_map(|i| &i.fx)
        .filter(|s| s.kind == crate::fx::EffectKind::Boil)
        .map(|s| Settings::of(&s.values))
        .filter(|s| s.rest && s.changes())
        .map(|s| {
            // La variante change quand `now_ms * fps / 1000` atteint un multiple de `vitesse`.
            let period_ms = 1000.0 * s.speed as f64 / fps as f64;
            let next = ((now_ms as f64 / period_ms).floor() + 1.0) * period_ms;
            Duration::from_secs_f64(((next - now_ms as f64).max(0.0) + 1.0) / 1000.0)
        })
        .min()
}

/// Les mêmes éléments du plan montrant chacune des autres variantes, pour les demander d'avance au
/// fil de fond (celles du repos : l'écran bascule de l'une à l'autre à chaque tour d'horloge).
pub fn other_variants(items: &[crate::render::PlanItem]) -> Vec<crate::render::PlanItem> {
    let mut out = Vec::new();
    for item in items {
        for (si, s) in item.fx.iter().enumerate() {
            if s.kind != crate::fx::EffectKind::Boil {
                continue;
            }
            let set = Settings::of(&s.values);
            if !set.rest || !set.changes() {
                continue;
            }
            for v in (0..set.variants).filter(|&v| v != set.variant) {
                let mut copy = item.clone();
                let mut values = s.values.clone();
                values.resize(VARIANT + 1, 0.0);
                values[VARIANT] = v as f32;
                copy.fx[si].values = values;
                out.push(copy);
            }
        }
    }
    out
}
