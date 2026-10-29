//! Dessin vectoriel façon Storyboard Pro : des traits à épaisseur variable et des zones remplies
//! qui restent modifiables (sélection, déplacement, rotation, couleur, gomme qui coupe les traits).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock, Weak};

use eframe::egui::{pos2, vec2, Color32, Pos2, Rect, Vec2};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::brush::{coverage, paint, BrushSettings, Curve, StrokeMode, StrokeState, Toucher};
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VPoint {
    pub x: f32,
    pub y: f32,
    /// Rayon du trait en ce point.
    pub r: f32,
}

impl VPoint {
    pub fn pos(&self) -> Pos2 {
        pos2(self.x, self.y)
    }

    fn lerp(a: VPoint, b: VPoint, t: f32) -> VPoint {
        VPoint {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            r: a.r + (b.r - a.r) * t,
        }
    }
}

/// La trace d'un trait vectoriel (rendu des brosses, étape 5, `docs/VECTORIEL-COMME-BITMAP.md`) :
/// ce qu'il faut, avec le chemin et le réglage complet, pour rejouer ses empreintes par le moteur
/// bitmap exactement comme pendant le geste — et ses pixels, gardés en mémoire une fois rendus.
/// Un trait d'avant l'étape 5 (ou recoupé, interpolé) n'a pas d'appuis : ils se retrouvent
/// d'après les rayons (`pressure_for_radius`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Trace {
    /// Appui de chaque point du chemin, après la courbe de pression du toucher ; vide : inconnu.
    /// Enregistré en entiers sur 16 bits (1/65535, invisible au rendu).
    #[serde(rename = "appui", skip_serializing_if = "Vec::is_empty", serialize_with = "ser_pressure", deserialize_with = "de_pressure")]
    pub pressure: Vec<f32>,
    /// Instant de chaque point (secondes ; −1 : inconnu) ; vide : inconnus (vitesse nulle).
    /// Enregistré en dixièmes de milliseconde depuis le premier instant connu (seuls les écarts
    /// comptent pour la vitesse).
    #[serde(rename = "instants", skip_serializing_if = "Vec::is_empty", serialize_with = "ser_time", deserialize_with = "de_time")]
    pub time: Vec<f64>,
    /// Le toucher du stylet au moment du trait (plancher de taille, trait fin, bord doux).
    #[serde(rename = "toucher", skip_serializing_if = "Option::is_none")]
    pub feel: Option<Toucher>,
    /// La couleur B (connexion « couleur »).
    #[serde(rename = "couleur_b", skip_serializing_if = "Option::is_none")]
    pub secondary: Option<[u8; 3]>,
    /// Les pixels du trait rendu seul, et la transformation appliquée depuis (jamais enregistrés).
    #[serde(skip)]
    pub cache: StrokeCache,
}

/// Instant inconnu, dans le fichier.
const TIME_UNKNOWN: u32 = u32::MAX;

fn ser_pressure<S: Serializer>(v: &Vec<f32>, s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(v.iter().map(|p| (p.clamp(0.0, 1.0) * 65535.0).round() as u16))
}

/// Un nombre du fichier, entier (quantifié) ou flottant : les deux formes ont été écrites sous la
/// même version de fichier (le commit d01fffe écrivait des flottants), les deux se lisent.
enum Nombre {
    Entier(u64),
    Flottant(f64),
}

impl<'de> Deserialize<'de> for Nombre {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = Nombre;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("un nombre")
            }
            fn visit_u64<E>(self, v: u64) -> Result<Nombre, E> {
                Ok(Nombre::Entier(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Nombre, E> {
                // (Un instant inconnu s'écrivait −1.)
                Ok(if v < 0 { Nombre::Flottant(v as f64) } else { Nombre::Entier(v as u64) })
            }
            fn visit_f64<E>(self, v: f64) -> Result<Nombre, E> {
                Ok(Nombre::Flottant(v))
            }
        }
        d.deserialize_any(V)
    }
}

fn de_pressure<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f32>, D::Error> {
    let q: Vec<Nombre> = Vec::deserialize(d)?;
    Ok(q
        .into_iter()
        .map(|v| match v {
            Nombre::Entier(v) => v.min(65535) as f32 / 65535.0,
            Nombre::Flottant(v) => v as f32,
        })
        .collect())
}

fn ser_time<S: Serializer>(v: &Vec<f64>, s: S) -> Result<S::Ok, S::Error> {
    let t0 = v.iter().copied().find(|t| *t >= 0.0).unwrap_or(0.0);
    s.collect_seq(v.iter().map(|&t| if t < 0.0 { TIME_UNKNOWN } else { (((t - t0) * 10_000.0).round().max(0.0) as u64).min(TIME_UNKNOWN as u64 - 1) as u32 }))
}

fn de_time<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
    let q: Vec<Nombre> = Vec::deserialize(d)?;
    Ok(q
        .into_iter()
        .map(|v| match v {
            Nombre::Entier(v) if v == TIME_UNKNOWN as u64 => -1.0,
            Nombre::Entier(v) => v as f64 / 10_000.0,
            Nombre::Flottant(v) => v,
        })
        .collect())
}

impl Trace {
    /// Rien à enregistrer (les formes, les vieux traits).
    pub fn is_empty(&self) -> bool {
        self.pressure.is_empty() && self.time.is_empty() && self.feel.is_none() && self.secondary.is_none()
    }

    /// Les appuis (et les instants) correspondent aux `n` points du chemin.
    pub fn aligned(&self, n: usize) -> bool {
        self.pressure.len() == n
    }

    /// L'appui et l'instant entre les points `i` et `i + 1`, à `t` ; aucun si les appuis ne
    /// correspondent pas au chemin.
    pub fn sample(&self, n: usize, i: usize, t: f32) -> Option<(f32, f64)> {
        if !self.aligned(n) {
            return None;
        }
        let j = (i + 1).min(n.saturating_sub(1));
        let i = i.min(n.saturating_sub(1));
        let p = self.pressure[i] + (self.pressure[j] - self.pressure[i]) * t;
        let time = match (self.time.get(i), self.time.get(j)) {
            (Some(&a), Some(&b)) if self.time.len() == n && a >= 0.0 && b >= 0.0 => a + (b - a) * t as f64,
            (Some(&a), _) if self.time.len() == n && t <= 0.0 => a,
            _ => -1.0,
        };
        Some((p, time))
    }

    /// Une trace de `samples` (appui, instant) refaits point à point : le toucher et la couleur B
    /// de `self`.
    pub fn with_samples(&self, samples: &[(f32, f64)]) -> Trace {
        Trace {
            pressure: samples.iter().map(|s| s.0).collect(),
            time: samples.iter().map(|s| s.1).collect(),
            feel: self.feel,
            secondary: self.secondary,
            cache: StrokeCache::default(),
        }
    }

    /// La trace d'un trait dont les points ont été refaits sans pouvoir suivre les appuis : le
    /// toucher et la couleur B restent, l'appui se retrouvera d'après les rayons.
    pub fn without_samples(&self) -> Trace {
        Trace { pressure: Vec::new(), time: Vec::new(), feel: self.feel, secondary: self.secondary, cache: StrokeCache::default() }
    }

    /// La même trace, dont les pixels sont à refaire (points déplacés, rayons changés).
    pub fn without_cache(&self) -> Trace {
        Trace { cache: StrokeCache::default(), ..self.clone() }
    }

    /// La trace d'un trait transformé pour un aperçu (glissé en cours) : les pixels en cache
    /// suivent la transformation, en attendant le rendu exact du lâcher.
    pub fn transformed_preview(&self, a: &Affine) -> Trace {
        Trace { cache: self.cache.transformed(a), ..self.clone() }
    }
}

/// Les pixels d'un trait rendu seul (prémultipliés), en tuiles creuses de 64 × 64 sur la grille de
/// la feuille : un trait fin en diagonale ne coûte que les tuiles qu'il touche, pas sa boîte.
/// `base` : la transformation que le trait avait quand ils ont été rendus.
#[derive(Clone, Debug)]
pub struct CachedPixels {
    /// Grille des tuiles : première tuile (en tuiles), nombre de tuiles.
    tx0: i64,
    ty0: i64,
    tw: usize,
    th: usize,
    /// Pour chaque case de la grille, le numéro de sa tuile dans `tiles` (`u32::MAX` : vide).
    index: Vec<u32>,
    tiles: Vec<Tile>,
    base: Affine,
}

/// Au-delà (1024 tuiles, 16 Mo), un trait n'est pas gardé en cache : il est rejoué à chaque
/// rendu.
pub const MAX_CACHE_TILES: usize = 1024;

/// Ce qu'un trait garde en pixels au-delà de la feuille, de chaque côté (en cache, et sur la feuille
/// flottante d'une sélection) : déplacé vers l'intérieur, il se montre en entier.
pub const MAX_OVERHANG: usize = 1024;

/// Plus grand côté d'une image de dessin (limite des textures de la carte graphique).
pub const MAX_SIDE: usize = 8192;

/// Ce que le rectangle `b` déborde d'une feuille `w` × `h` : (gauche, haut, droite, bas), en pixels
/// entiers, bornés par `MAX_OVERHANG` et par `MAX_SIDE`.
pub fn overhang(b: Rect, w: usize, h: usize) -> (usize, usize, usize, usize) {
    if !b.is_finite() || b.max.x < b.min.x || b.max.y < b.min.y {
        return (0, 0, 0, 0);
    }
    let amount = |v: f32| (v.max(0.0).ceil() as usize).min(MAX_OVERHANG);
    let fit = |a: usize, b: usize, side: usize| {
        let room = MAX_SIDE.saturating_sub(side);
        let a = a.min(room);
        (a, b.min(room - a))
    };
    let (l, r) = fit(amount(-b.min.x), amount(b.max.x - w as f32), w);
    let (t, d) = fit(amount(-b.min.y), amount(b.max.y - h as f32), h);
    (l, t, r, d)
}

/// Où poser un élément sur une image : le pixel (x, y) de la feuille (de taille `sheet`) va en
/// (x + shift.0, y + shift.1) de l'image. Une feuille flottante élargie garde ce qui dépasse de la
/// feuille : elle est décalée de sa marge.
#[derive(Clone, Copy)]
pub struct Place {
    pub shift: (i64, i64),
    pub sheet: (usize, usize),
    /// L'image reçoit aussi ce qui sort de ses bornes (toile infinie : le dessin lui-même, avec ses
    /// tuiles hors de la feuille) ; faux : tout est coupé à l'image, comme pour les feuilles
    /// temporaires (feuille flottante, aperçus).
    pub open: bool,
}

/// Un cadre ne dépasse pas autant de pixels (le trait lointain est rendu dans son seul rectangle).
const MAX_FRAME_PIXELS: usize = 1 << 26;

/// Le cadre où un trait de portée `reach` est rendu seul, sur une feuille `w` × `h` : (abscisse du
/// coin haut gauche, ordonnée, largeur, hauteur), en pixels du monde. La feuille entière tant que le
/// trait y reste (rien n'a changé), agrandie aux tuiles qu'il touche autour sinon ; si le cadre
/// ainsi agrandi devenait énorme (un trait très loin de la feuille), le seul rectangle du trait.
fn stroke_frame(reach: Rect, w: usize, h: usize) -> (i64, i64, usize, usize) {
    let sheet = (0, 0, w, h);
    if !reach.is_finite() || reach.max.x < reach.min.x || reach.max.y < reach.min.y {
        return sheet;
    }
    let t = TILE as i64;
    let world = crate::raster::WORLD as f32;
    let c = |v: f32| v.clamp(-world, world);
    let down = |v: f32| (c(v).floor() as i64).div_euclid(t) * t;
    let up = |v: f32| (c(v).ceil() as i64 + t - 1).div_euclid(t) * t;
    let (rx0, ry0, rx1, ry1) = (down(reach.min.x), down(reach.min.y), up(reach.max.x), up(reach.max.y));
    let (x0, y0) = (rx0.min(0), ry0.min(0));
    let (x1, y1) = (if reach.max.x > w as f32 { rx1 } else { w as i64 }, if reach.max.y > h as f32 { ry1 } else { h as i64 });
    let (x0, y0) = (if reach.min.x < 0.0 { x0 } else { 0 }, if reach.min.y < 0.0 { y0 } else { 0 });
    let union_px = (x1 - x0).max(0) as usize * (y1 - y0).max(0) as usize;
    if union_px <= MAX_FRAME_PIXELS && (x1 > x0) && (y1 > y0) {
        return (x0, y0, (x1 - x0) as usize, (y1 - y0) as usize);
    }
    // Très loin de la feuille : le rectangle du trait seul.
    let (fw, fh) = ((rx1 - rx0).max(t) as usize, (ry1 - ry0).max(t) as usize);
    (rx0, ry0, fw.min(MAX_SIDE * 2), fh.min(MAX_SIDE * 2))
}

impl CachedPixels {
    /// Les tuiles dessinées de `sheet` ; `None` s'il y en a plus de `MAX_CACHE_TILES`.
    fn from_sheet(sheet: &Raster, base: Affine) -> Option<CachedPixels> {
        let mut drawn: Vec<(usize, usize, usize)> = Vec::new();
        for (idx, tile) in sheet.tiles.iter().enumerate() {
            let Some(tile) = tile else { continue };
            if tile.iter().any(|c| c.a() != 0) {
                drawn.push((idx % sheet.tiles_x, idx / sheet.tiles_x, idx));
            }
        }
        if drawn.is_empty() {
            return Some(CachedPixels { tx0: 0, ty0: 0, tw: 0, th: 0, index: Vec::new(), tiles: Vec::new(), base });
        }
        if drawn.len() > MAX_CACHE_TILES {
            return None;
        }
        let (tx0, ty0) = (drawn.iter().map(|d| d.0).min().unwrap_or(0), drawn.iter().map(|d| d.1).min().unwrap_or(0));
        let (tx1, ty1) = (drawn.iter().map(|d| d.0).max().unwrap_or(0) + 1, drawn.iter().map(|d| d.1).max().unwrap_or(0) + 1);
        let (tw, th) = (tx1 - tx0, ty1 - ty0);
        let mut index = vec![u32::MAX; tw * th];
        let mut tiles = Vec::with_capacity(drawn.len());
        for (tx, ty, idx) in drawn {
            index[(ty - ty0) * tw + (tx - tx0)] = tiles.len() as u32;
            tiles.push(sheet.tiles[idx].clone().expect("tuile dessinée"));
        }
        Some(CachedPixels { tx0: tx0 as i64, ty0: ty0 as i64, tw, th, index, tiles, base })
    }

    pub fn bytes(&self) -> usize {
        self.tiles.len() * TILE_PIXELS * 4
    }

    fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Boîte des pixels (coordonnées de la feuille d'origine).
    fn rect(&self) -> Rect {
        Rect::from_min_size(pos2((self.tx0 * TILE as i64) as f32, (self.ty0 * TILE as i64) as f32), vec2((self.tw * TILE) as f32, (self.th * TILE) as f32))
    }

    /// La tuile qui couvre la case (`cx`, `cy`) de la grille, s'il y en a une.
    fn tile(&self, cx: i64, cy: i64) -> Option<&Tile> {
        if cx < 0 || cy < 0 || cx as usize >= self.tw || cy as usize >= self.th {
            return None;
        }
        let k = self.index[cy as usize * self.tw + cx as usize];
        (k != u32::MAX).then(|| &self.tiles[k as usize])
    }

    /// Pixel (`x`, `y`) de la feuille d'origine, transparent hors des tuiles gardées.
    fn at(&self, x: i64, y: i64) -> Color32 {
        let (cx, cy) = (x.div_euclid(TILE as i64) - self.tx0, y.div_euclid(TILE as i64) - self.ty0);
        match self.tile(cx, cy) {
            Some(t) => t[y.rem_euclid(TILE as i64) as usize * TILE + x.rem_euclid(TILE as i64) as usize],
            None => Color32::TRANSPARENT,
        }
    }

    /// Couleur interpolée (prémultipliée) au point (`fx`, `fy`) de la feuille d'origine (centres de
    /// pixels à 0,5).
    fn sample(&self, fx: f32, fy: f32) -> Color32 {
        let (gx, gy) = (fx - 0.5, fy - 0.5);
        let (ix, iy) = (gx.floor(), gy.floor());
        let (tx, ty) = (gx - ix, gy - iy);
        let (ix, iy) = (ix as i64, iy as i64);
        let mut acc = [0.0f32; 4];
        for (dx, dy, wgt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)] {
            if wgt <= 0.0 {
                continue;
            }
            let c = self.at(ix + dx, iy + dy);
            acc[0] += c.r() as f32 * wgt;
            acc[1] += c.g() as f32 * wgt;
            acc[2] += c.b() as f32 * wgt;
            acc[3] += c.a() as f32 * wgt;
        }
        Color32::from_rgba_premultiplied((acc[0] + 0.5) as u8, (acc[1] + 0.5) as u8, (acc[2] + 0.5) as u8, (acc[3] + 0.5) as u8)
    }
}

/// Ce qu'un rendu a laissé : la signature du trait tel qu'il était (couleur, opacité, dureté,
/// réglage, nombre de points), ses pixels (aucun : trop grand pour le cache), et son numéro dans
/// le registre du budget.
struct Rendered {
    id: u64,
    sig: u64,
    pixels: Option<CachedPixels>,
}

type Cell = Arc<RwLock<Option<Rendered>>>;

/// Le budget global des caches de traits : au-delà, les pixels les moins récemment posés sont
/// rendus (ils seront rejoués à la demande). 512 Mo par défaut.
pub const CACHE_BUDGET_BYTES: usize = 512 << 20;

/// Le registre des caches vivants : octets et dernier usage de chacun, pour tenir le budget.
struct Registry {
    entries: HashMap<u64, (Weak<RwLock<Option<Rendered>>>, usize, u64)>,
    total: usize,
    clock: u64,
    next_id: u64,
    budget: usize,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry { entries: HashMap::new(), total: 0, clock: 0, next_id: 1, budget: CACHE_BUDGET_BYTES }));

impl Registry {
    fn insert(&mut self, cell: &Cell, bytes: usize) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.clock += 1;
        self.entries.insert(id, (Arc::downgrade(cell), bytes, self.clock));
        self.total += bytes;
        id
    }

    fn touch(&mut self, id: u64) {
        self.clock += 1;
        if let Some(e) = self.entries.get_mut(&id) {
            e.2 = self.clock;
        }
    }

    fn remove(&mut self, id: u64) {
        if let Some((_, bytes, _)) = self.entries.remove(&id) {
            self.total = self.total.saturating_sub(bytes);
        }
    }

    /// Au-dessus du budget : oublie les caches morts (traits disparus), puis vide les moins récents
    /// jusqu'à 90 % du budget. Rend ce qu'il faut laisser tomber hors du verrou.
    fn enforce(&mut self) -> Vec<Rendered> {
        let mut dropped = Vec::new();
        if self.total <= self.budget {
            return dropped;
        }
        let dead: Vec<u64> = self.entries.iter().filter(|(_, e)| e.0.strong_count() == 0).map(|(id, _)| *id).collect();
        for id in dead {
            self.remove(id);
        }
        if self.total <= self.budget {
            return dropped;
        }
        let mut by_age: Vec<(u64, u64)> = self.entries.iter().map(|(id, e)| (e.2, *id)).collect();
        by_age.sort_unstable();
        let target = self.budget / 10 * 9;
        for (_, id) in by_age {
            if self.total <= target {
                break;
            }
            let Some((weak, bytes, _)) = self.entries.remove(&id) else { continue };
            self.total = self.total.saturating_sub(bytes);
            if let Some(cell) = weak.upgrade() {
                if let Ok(mut w) = cell.try_write() {
                    if let Some(r) = w.take() {
                        dropped.push(r);
                    }
                }
            }
        }
        dropped
    }
}

thread_local! {
    /// Nombre de traits rendus seuls (rejoués ou tracés) par ce fil : ce que le cache évite.
    static REPLAYS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Les rendus seuls faits sur ce fil (pour les essais : un rendu depuis le cache n'en fait pas).
#[allow(dead_code)]
pub fn replay_count() -> usize {
    REPLAYS.with(|r| r.get())
}

/// Le budget des caches (pour les essais : le baisser force les évictions).
#[allow(dead_code)]
pub fn set_cache_budget(bytes: usize) {
    if let Ok(mut r) = REGISTRY.lock() {
        r.budget = bytes;
        let dropped = r.enforce();
        drop(r);
        drop(dropped);
    }
}

/// Les octets de tous les caches vivants (compte des traits disparus non encore oubliés).
#[allow(dead_code)]
pub fn cache_total_bytes() -> usize {
    REGISTRY.lock().map(|r| r.total).unwrap_or(0)
}

/// Le cache des pixels d'un trait : partagé entre les copies du trait (annulation, aperçu d'un
/// glissé), rempli au premier rendu, refait si le trait a changé entre-temps (sa signature ne
/// correspond plus) ou vidé par le budget, jamais comparé ni enregistré. `xf` : la transformation
/// d'aperçu appliquée au trait depuis ce rendu (identité hors glissé : toute autre transformation
/// vide le cache).
#[derive(Clone, Default)]
pub struct StrokeCache {
    cell: Cell,
    xf: Affine,
}

impl PartialEq for StrokeCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for StrokeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.bytes();
        if bytes > 0 { write!(f, "cache {bytes} o") } else { write!(f, "cache vide") }
    }
}

impl StrokeCache {
    pub fn transformed(&self, a: &Affine) -> StrokeCache {
        StrokeCache { cell: self.cell.clone(), xf: self.xf.then(a) }
    }

    /// Le cache est celui d'un aperçu transformé (glissé en cours).
    pub fn is_preview(&self) -> bool {
        self.xf != Affine::IDENTITY
    }

    /// Le trait a été rendu (même si ses pixels étaient trop grands pour être gardés) : un nouveau
    /// rendu ne le rejouerait pas pour rien.
    pub fn is_rendered(&self) -> bool {
        self.cell.read().is_ok_and(|r| r.is_some())
    }

    /// Les pixels gardés, en octets.
    pub fn bytes(&self) -> usize {
        self.cell.read().ok().and_then(|r| r.as_ref().and_then(|r| r.pixels.as_ref().map(CachedPixels::bytes))).unwrap_or(0)
    }
}

/// La signature d'un trait pour son cache : tout ce qui change ses pixels, sauf la place des points
/// (les transformations passent par `xf`).
fn stroke_signature(points: &[VPoint], color: [u8; 3], opacity: f32, hardness: f32, brush: Option<&BrushSettings>, trace: &Trace) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: &[u8]| {
        for &x in b {
            h ^= x as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(&color);
    eat(&opacity.to_bits().to_le_bytes());
    eat(&hardness.to_bits().to_le_bytes());
    eat(&(points.len() as u64).to_le_bytes());
    eat(&(trace.pressure.len() as u64).to_le_bytes());
    eat(&(trace.time.len() as u64).to_le_bytes());
    if let Some(s) = trace.secondary {
        eat(&s);
    }
    if let Some(b) = brush {
        if let Ok(bytes) = serde_json::to_vec(b) {
            eat(&bytes);
        }
    } else {
        eat(b"lisse");
    }
    if let Some(f) = &trace.feel {
        if let Ok(bytes) = serde_json::to_vec(f) {
            eat(&bytes);
        }
    }
    h
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Element {
    /// Trait à épaisseur variable le long d'une ligne médiane.
    Stroke {
        points: Vec<VPoint>,
        color: [u8; 3],
        opacity: f32,
        hardness: f32,
        /// Le réglage complet du pinceau (pointe et papier désignés par leur clé de contenu, voir
        /// `brushtex::content_key`) : le trait est rejoué par le moteur bitmap, empreinte par
        /// empreinte, et reste modifiable. Aucun (traits d'avant 7b, formes) : trait lisse.
        #[serde(default, rename = "pinceau", skip_serializing_if = "Option::is_none")]
        brush: Option<Box<crate::brush::BrushSettings>>,
        /// Appuis, instants, toucher, couleur B et pixels en cache (étape 5).
        #[serde(default, skip_serializing_if = "Trace::is_empty")]
        trace: Trace,
    },
    /// Zone remplie, délimitée par un ou plusieurs contours (pair-impair : les trous restent vides).
    Fill {
        contours: Vec<Vec<[f32; 2]>>,
        color: [u8; 3],
        opacity: f32,
        /// Un dégradé à la place de la couleur unie (outil Dégradé, `degrade.rs`, format 38) : il reste un
        /// réglage de la zone (jamais des pixels écrits dans le dessin), suit la forme quand elle se déplace
        /// ou se déforme, et se rend avec elle. `color` reste la couleur de départ (sélection par couleur,
        /// pastilles) ; `opacity` multiplie celle du dégradé. Absent : couleur unie, comme avant.
        #[serde(default, rename = "degrade", skip_serializing_if = "Option::is_none")]
        gradient: Option<crate::degrade::Degrade>,
    },
    /// Texte (5d), modifiable avec l'outil Texte : dessiné par les contours des lettres de sa
    /// police (voir `text.rs`).
    Text {
        text: String,
        #[serde(rename = "police")]
        font: String,
        /// Pixels par em.
        #[serde(rename = "taille")]
        size: f32,
        color: [u8; 3],
        opacity: f32,
        /// Placement (a, b, c, d, tx, ty) du texte mis en page en (0, 0).
        #[serde(rename = "placement")]
        place: [f32; 6],
    },
}

impl Affine {
    pub fn to_array(&self) -> [f32; 6] {
        [self.a, self.b, self.c, self.d, self.tx, self.ty]
    }

    pub fn from_array(v: [f32; 6]) -> Affine {
        Affine { a: v[0], b: v[1], c: v[2], d: v[3], tx: v[4], ty: v[5] }
    }
}

impl Element {
    /// Un texte posé en `place`.
    pub fn text(text: String, font: String, size: f32, color: [u8; 3], place: Affine) -> Element {
        Element::Text { text, font, size, color, opacity: 1.0, place: place.to_array() }
    }

    /// Texte : sa boîte (coins dans l'ordre) et les contours de chaque lettre, placés.
    pub fn text_shape(&self) -> Option<([Pos2; 4], Vec<Vec<Vec<[f32; 2]>>>)> {
        let Element::Text { text, font, size, place, .. } = self else { return None };
        let a = Affine::from_array(*place);
        let laid = crate::text::layout(text, font, *size);
        let b = laid.bounds;
        let corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].map(|p| a.apply(p));
        let glyphs = laid
            .glyphs
            .iter()
            .map(|g| {
                g.iter()
                    .map(|c| {
                        c.iter()
                            .map(|p| {
                                let q = a.apply(pos2(p[0], p[1]));
                                [q.x, q.y]
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
        Some((corners, glyphs))
    }

    pub fn bounds(&self) -> Rect {
        let mut r = Rect::NOTHING;
        match self {
            Element::Stroke { points, brush, .. } => {
                // Un pinceau texturé peut grossir ses empreintes au hasard, les disperser, et une
                // pointe en image tournée déborde du cercle jusqu'à √2 fois le rayon.
                let grow = 1.0
                    + brush.as_ref().map_or(0.0, |b| {
                        b.size_jitter.max(0.0) * 0.5 + 2.0 * (b.scatter.max(0.0) + b.scatter_along.max(0.0)) + if b.tip.is_some() { std::f32::consts::SQRT_2 - 1.0 } else { 0.0 }
                    });
                for p in points {
                    r = r.union(Rect::from_center_size(p.pos(), Vec2::splat(p.r * 2.0 * grow)));
                }
            }
            Element::Fill { contours, .. } => {
                for p in contours.iter().flatten() {
                    r.extend_with(pos2(p[0], p[1]));
                }
            }
            Element::Text { .. } => {
                if let Some((corners, glyphs)) = self.text_shape() {
                    r = Rect::from_points(&corners);
                    for p in glyphs.iter().flatten().flatten() {
                        r.extend_with(pos2(p[0], p[1]));
                    }
                }
            }
        }
        r
    }

    /// Couleur de l'élément (trait, zone ou texte).
    pub fn color(&self) -> [u8; 3] {
        match self {
            Element::Stroke { color, .. } | Element::Fill { color, .. } | Element::Text { color, .. } => *color,
        }
    }

    /// Une couleur choisie pour l'élément. Une zone en dégradé devient unie (le dégradé se refait avec l'outil Dégradé ;
    /// Ctrl+Z le rend).
    pub fn set_color(&mut self, c: [u8; 3]) {
        match self {
            Element::Stroke { color, .. } | Element::Text { color, .. } => *color = c,
            Element::Fill { color, gradient, .. } => {
                *color = c;
                *gradient = None;
            }
        }
        self.invalidate_cache();
    }

    /// Le dégradé de la zone, s'il en a un.
    pub fn gradient(&self) -> Option<&crate::degrade::Degrade> {
        match self {
            Element::Fill { gradient, .. } => gradient.as_ref(),
            _ => None,
        }
    }

    pub fn scale_width(&mut self, k: f32) {
        if let Element::Stroke { points, brush, trace, .. } = self {
            for p in points {
                p.r = (p.r * k).clamp(0.2, 200.0);
            }
            // La taille du pinceau suit : la pression se retrouve d'après le rayon (voir le rendu).
            if let Some(b) = brush {
                b.size *= k;
            }
            trace.cache = StrokeCache::default();
        }
    }

    /// Les pixels en cache du trait sont à refaire (après une transformation, au lâcher : le
    /// prochain rendu rejoue le moteur exactement).
    pub fn invalidate_cache(&mut self) {
        if let Element::Stroke { trace, .. } = self {
            trace.cache = StrokeCache::default();
        }
    }

    /// Le trait a ses pixels exacts en cache (ou ceux de son aperçu transformé) : le poser ne le
    /// rejoue pas, de quelque fil que ce soit. Les autres éléments n'ont pas de cache.
    pub fn cache_ready(&self) -> bool {
        let Element::Stroke { points, color, opacity, hardness, brush, trace } = self else { return true };
        let Ok(cell) = trace.cache.cell.read() else { return false };
        let Some(r) = cell.as_ref() else { return false };
        r.pixels.is_some() && (trace.cache.is_preview() || r.sig == stroke_signature(points, *color, *opacity, *hardness, brush.as_deref(), trace))
    }

    /// Un trait au réglage complet qui n'a pas (ou plus) de rendu en cache : le montrer le rejoue.
    pub fn cache_missing(&self) -> bool {
        matches!(self, Element::Stroke { brush: Some(_), trace, .. } if !trace.cache.is_rendered())
    }

    /// Les pixels en cache du trait suivent une transformation d'aperçu (glissé, flèches) : ils sont
    /// déformés, pas exacts.
    pub fn cache_is_preview(&self) -> bool {
        matches!(self, Element::Stroke { trace, .. } if trace.cache.is_preview())
    }

    /// Même géométrie et même aspect que `other` (seul le cache peut différer).
    pub fn same_as(&self, other: &Element) -> bool {
        self == other
    }

    /// Prend les pixels en cache de `from`, le même trait rendu exactement ailleurs (le rejeu en
    /// arrière-plan, `rejeu.rs`) : l'aperçu transformé laisse la place au rendu exact.
    pub fn adopt_cache(&mut self, from: &Element) {
        if let (Element::Stroke { trace, .. }, Element::Stroke { trace: src, .. }) = (self, from) {
            trace.cache = src.cache.clone();
        }
    }

    /// Octets des pixels en cache de l'élément.
    pub fn cache_bytes(&self) -> usize {
        match self {
            Element::Stroke { trace, .. } => trace.cache.bytes(),
            _ => 0,
        }
    }

    /// L'élément transformé. Un trait oublie ses pixels en cache (le prochain rendu le rejoue
    /// exactement à sa nouvelle place) ; pour l'aperçu d'un glissé, voir `transformed_preview`.
    pub fn transformed(&self, a: &Affine) -> Element {
        self.transformed_with(a, false)
    }

    /// L'élément transformé pour l'aperçu d'un glissé : un trait garde ses pixels en cache et les
    /// pose transformés (une translation est arrondie au pixel : copie nette et rapide ; échelle
    /// et rotation sont rééchantillonnées), jusqu'au rendu exact du lâcher.
    pub fn transformed_preview(&self, a: &Affine) -> Element {
        let a = match a.integer_translation() {
            Some(_) => *a,
            None if a.is_translation() => Affine::translate(vec2(a.tx.round(), a.ty.round())),
            None => *a,
        };
        self.transformed_with(&a, true)
    }

    fn transformed_with(&self, a: &Affine, preview: bool) -> Element {
        match self {
            Element::Stroke {
                points,
                color,
                opacity,
                hardness,
                brush,
                trace,
            } => {
                let k = a.width_factor();
                Element::Stroke {
                    points: points
                        .iter()
                        .map(|p| {
                            let q = a.apply(p.pos());
                            VPoint { x: q.x, y: q.y, r: p.r * k }
                        })
                        .collect(),
                    color: *color,
                    opacity: *opacity,
                    hardness: *hardness,
                    brush: brush.as_ref().map(|b| Box::new(crate::brush::BrushSettings { size: b.size * k, ..**b })),
                    trace: if preview { trace.transformed_preview(a) } else { trace.without_cache() },
                }
            }
            Element::Fill {
                contours,
                color,
                opacity,
                gradient,
            } => Element::Fill {
                contours: contours
                    .iter()
                    .map(|c| {
                        c.iter()
                            .map(|p| {
                                let q = a.apply(pos2(p[0], p[1]));
                                [q.x, q.y]
                            })
                            .collect()
                    })
                    .collect(),
                color: *color,
                opacity: *opacity,
                // L'axe du dégradé suit la forme.
                gradient: gradient.map(|g| g.transformed(a)),
            },
            Element::Text { text, font, size, color, opacity, place } => Element::Text {
                text: text.clone(),
                font: font.clone(),
                size: *size,
                color: *color,
                opacity: *opacity,
                place: Affine::from_array(*place).then(a).to_array(),
            },
        }
    }

    /// Le point `p` touche-t-il l'élément (à `tol` près) ?
    pub fn hit(&self, p: Pos2, tol: f32) -> bool {
        if !self.bounds().expand(tol).contains(p) {
            return false;
        }
        match self {
            Element::Stroke { points, .. } => {
                if points.len() == 1 {
                    return points[0].pos().distance(p) <= points[0].r + tol;
                }
                points.windows(2).any(|s| {
                    let (d, t) = segment_distance(p, s[0].pos(), s[1].pos());
                    d <= s[0].r + (s[1].r - s[0].r) * t + tol
                })
            }
            Element::Fill { contours, .. } => inside_contours(p, contours),
            // Un texte se prend n'importe où dans sa boîte.
            Element::Text { .. } => self.text_shape().is_some_and(|(c, _)| point_in_polygon(p, &c)),
        }
    }

    /// L'élément a-t-il au moins un point dans le lasso ?
    pub fn touches(&self, lasso: &[Pos2]) -> bool {
        match self {
            Element::Stroke { points, .. } => points.iter().any(|q| point_in_polygon(q.pos(), lasso)),
            Element::Fill { contours, .. } => contours
                .iter()
                .flatten()
                .any(|q| point_in_polygon(pos2(q[0], q[1]), lasso)),
            Element::Text { .. } => self.text_shape().is_some_and(|(c, glyphs)| {
                c.iter().any(|q| point_in_polygon(*q, lasso))
                    || glyphs.iter().flatten().flatten().any(|q| point_in_polygon(pos2(q[0], q[1]), lasso))
            }),
        }
    }

    /// Lignes qui dessinent l'élément sélectionné à l'écran.
    pub fn outline(&self) -> Vec<Vec<Pos2>> {
        match self {
            Element::Stroke { points, .. } => vec![points.iter().map(VPoint::pos).collect()],
            Element::Fill { contours, .. } => contours
                .iter()
                .map(|c| {
                    let mut line: Vec<Pos2> = c.iter().map(|p| pos2(p[0], p[1])).collect();
                    if let Some(&first) = line.first() {
                        line.push(first);
                    }
                    line
                })
                .collect(),
            Element::Text { .. } => match self.text_shape() {
                Some((c, _)) => vec![vec![c[0], c[1], c[2], c[3], c[0]]],
                None => Vec::new(),
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VectorDrawing {
    pub elements: Vec<Element>,
}

impl VectorDrawing {
    pub fn render(&self, width: usize, height: usize) -> Raster {
        let mut raster = Raster::new(width, height);
        for e in &self.elements {
            render_element(&mut raster, e);
        }
        raster
    }

    /// L'image du dessin en deux feuilles : le reste (`base`) et les éléments `items` (tous si
    /// aucun) — posés depuis leurs pixels en cache. Pour l'aperçu d'une déformation (enveloppe,
    /// perspective) : la feuille des éléments choisis est déformée à la volée par-dessus le reste.
    pub fn render_split(&self, items: Option<&[usize]>, width: usize, height: usize) -> (Raster, Raster) {
        let (mut base, mut sheet) = (Raster::new(width, height), Raster::new(width, height));
        for (i, e) in self.elements.iter().enumerate() {
            let chosen = items.is_none_or(|list| list.contains(&i));
            render_element(if chosen { &mut sheet } else { &mut base }, e);
        }
        (base, sheet)
    }

    /// Comme `render_split`, mais la feuille des éléments `items` garde ce qui dépasse de la feuille
    /// (jusqu'à `MAX_OVERHANG` de chaque côté) : un trait déplacé vers l'intérieur se montre en
    /// entier, au lieu d'être coupé en ligne droite là où la feuille s'arrêtait. Rend le reste
    /// (à la taille de la feuille), les éléments choisis (plus grande) et la marge de cette
    /// seconde à gauche et en haut : son pixel (x, y) est le point (x − marge.0, y − marge.1) de la
    /// feuille. Posés depuis les pixels en cache (qui gardent aussi ce qui dépasse).
    pub fn render_split_extended(&self, items: &[usize], width: usize, height: usize) -> (Raster, Raster, (usize, usize)) {
        let reach = items.iter().filter_map(|&i| self.elements.get(i)).fold(Rect::NOTHING, |r, e| r.union(e.bounds().expand(1.5)));
        let (l, t, r, d) = overhang(reach, width, height);
        let mut chosen = vec![false; self.elements.len()];
        for &i in items {
            if let Some(c) = chosen.get_mut(i) {
                *c = true;
            }
        }
        let (picked, rest): (Vec<&Element>, Vec<&Element>) = {
            let (a, b): (Vec<_>, Vec<_>) = self.elements.iter().zip(&chosen).partition(|(_, &c)| c);
            (a.into_iter().map(|(e, _)| e).collect(), b.into_iter().map(|(e, _)| e).collect())
        };
        // Le reste du dessin garde ce qu'il a hors de la feuille (toile infinie : sinon il disparaîtrait pendant le geste) ;
        // les bandes de rendu (en parallèle) ne posent que la feuille, elles servent quand tout y est.
        let sort = |e: &&Element| {
            let b = e.bounds().expand(1.5);
            !b.is_finite() || b.min.x < 0.0 || b.min.y < 0.0 || b.max.x > width as f32 || b.max.y > height as f32
        };
        let base = if rest.iter().any(sort) {
            let mut base = Raster::new(width, height);
            for e in &rest {
                render_element(&mut base, e);
            }
            base
        } else {
            render_in_bands(&rest, width, height, (0, 0), (width, height))
        };
        let sheet = render_in_bands(&picked, width + l + r, height + t + d, (l as i64, t as i64), (width, height));
        (base, sheet, (l, t))
    }

    /// L'image approchée du dessin, tout de suite : les traits qui ont déjà leurs pixels en cache
    /// les posent ; les autres sont tracés lisses (sans leur réglage, sans rejeu du moteur). Pour
    /// afficher un projet qui s'ouvre, le temps que le rejeu exact arrive en arrière-plan.
    pub fn render_quick(&self, width: usize, height: usize) -> Raster {
        let mut raster = Raster::new(width, height);
        for e in &self.elements {
            match e {
                Element::Stroke { points, color, opacity, hardness, brush: Some(_), trace } if e.cache_bytes() == 0 => {
                    // (Toile infinie : le trait approché montre aussi ce qui sort de la feuille, comme le rendu exact qui le
                    // remplacera.)
                    let (sheet, off) = render_stroke_alone(width, height, points, *color, *opacity, *hardness, None, trace, e.bounds().expand(1.5), true);
                    blit_sheet(&mut raster, &sheet, Rect::EVERYTHING, (-off.0, -off.1), true);
                }
                _ => render_element(&mut raster, e),
            }
        }
        raster
    }

    /// Combien de traits de ce dessin seraient rejoués par le moteur si on les montrait (aucun
    /// rendu en cache : jamais rendus, ou évincés du budget).
    pub fn missing_caches(&self) -> usize {
        self.elements.iter().filter(|e| e.cache_missing()).count()
    }

    /// Élément le plus haut sous le point.
    pub fn hit_test(&self, p: Pos2, tol: f32) -> Option<usize> {
        (0..self.elements.len())
            .rev()
            .find(|&i| self.elements[i].hit(p, tol))
    }

    /// Les remplissages au pot passent sous les traits, après les remplissages déjà là.
    pub fn fill_insert_index(&self) -> usize {
        self.elements
            .iter()
            .take_while(|e| matches!(e, Element::Fill { .. }))
            .count()
    }
}

/// Transformation affine : p' = (a·x + b·y + tx, c·x + d·y + ty).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Default for Affine {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    /// Une translation pure (ni échelle, ni rotation).
    pub fn is_translation(&self) -> bool {
        let eps = 1e-4;
        (self.a - 1.0).abs() < eps && self.b.abs() < eps && self.c.abs() < eps && (self.d - 1.0).abs() < eps
    }

    /// Une translation d'un nombre entier de pixels (ou rien) : (dx, dy).
    pub fn integer_translation(&self) -> Option<(i32, i32)> {
        let (dx, dy) = (self.tx.round(), self.ty.round());
        (self.is_translation() && (self.tx - dx).abs() < 1e-3 && (self.ty - dy).abs() < 1e-3).then_some((dx as i32, dy as i32))
    }

    pub fn translate(v: Vec2) -> Self {
        Self {
            tx: v.x,
            ty: v.y,
            ..Self::IDENTITY
        }
    }

    pub fn scale_about(o: Pos2, sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            tx: o.x - sx * o.x,
            ty: o.y - sy * o.y,
        }
    }

    pub fn rotate_about(o: Pos2, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            a: c,
            b: -s,
            c: s,
            d: c,
            tx: o.x - (c * o.x - s * o.y),
            ty: o.y - (s * o.x + c * o.y),
        }
    }

    pub fn apply(&self, p: Pos2) -> Pos2 {
        pos2(
            self.a * p.x + self.b * p.y + self.tx,
            self.c * p.x + self.d * p.y + self.ty,
        )
    }

    /// Facteur appliqué aux épaisseurs (moyenne géométrique des échelles).
    pub fn width_factor(&self) -> f32 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    /// `self` puis `next`.
    pub fn then(&self, next: &Affine) -> Affine {
        Affine {
            a: next.a * self.a + next.b * self.c,
            b: next.a * self.b + next.b * self.d,
            c: next.c * self.a + next.d * self.c,
            d: next.c * self.b + next.d * self.d,
            tx: next.a * self.tx + next.b * self.ty + next.tx,
            ty: next.c * self.tx + next.d * self.ty + next.ty,
        }
    }

    /// Transformation inverse (`None` si elle écrase tout sur une ligne).
    pub fn inverse(&self) -> Option<Affine> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-8 {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Affine {
            a,
            b,
            c,
            d,
            tx: -(a * self.tx + b * self.ty),
            ty: -(c * self.tx + d * self.ty),
        })
    }

    /// Rectangle englobant de `r` transformé.
    pub fn apply_rect(&self, r: Rect) -> Rect {
        Rect::from_points(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].map(|p| self.apply(p)))
    }
}

/// Couverture (0 à 1) d'un ou plusieurs polygones (règle pair-impair), anticrénelée, sur la partie
/// de l'image qu'ils recouvrent : (x0, y0, largeur, hauteur, couvertures ligne par ligne).
pub fn polygon_coverage(contours: &[Vec<[f32; 2]>], width: usize, height: usize) -> Option<(usize, usize, usize, usize, Vec<f32>)> {
    let pts: Vec<Pos2> = contours.iter().flatten().map(|p| pos2(p[0], p[1])).collect();
    let mut mask = Mask::new(Rect::from_points(&pts).expand(1.0), width, height)?;
    fill_even_odd(&mut mask, contours);
    for v in &mut mask.data {
        *v = v.min(1.0);
    }
    Some((mask.x0 as usize, mask.y0 as usize, mask.w, mask.h, mask.data))
}

// ---------------------------------------------------------------------------------- rendu

/// Couverture (0 à 1) d'un élément sur une zone de l'image, avant de la peindre.
struct Mask {
    x0: i64,
    y0: i64,
    w: usize,
    h: usize,
    data: Vec<f32>,
}

impl Mask {
    fn new(bounds: Rect, width: usize, height: usize) -> Option<Self> {
        if !bounds.is_finite() || bounds.max.x < 0.0 || bounds.max.y < 0.0 {
            return None;
        }
        let x0 = bounds.min.x.floor().max(0.0) as usize;
        let y0 = bounds.min.y.floor().max(0.0) as usize;
        let x1 = (bounds.max.x.ceil() as usize).min(width);
        let y1 = (bounds.max.y.ceil() as usize).min(height);
        if x0 >= x1 || y0 >= y1 {
            return None;
        }
        Some(Self {
            x0: x0 as i64,
            y0: y0 as i64,
            w: x1 - x0,
            h: y1 - y0,
            data: vec![0.0; (x1 - x0) * (y1 - y0)],
        })
    }

    /// Comme `new`, sans les bornes de l'image : toute la toile infinie (négative comprise), jusqu'aux
    /// limites du monde et d'une couverture de `MAX_FRAME_PIXELS` pixels.
    fn open(bounds: Rect) -> Option<Self> {
        if !bounds.is_finite() {
            return None;
        }
        let world = crate::raster::WORLD as f32;
        let c = |v: f32| v.clamp(-world, world);
        let (x0, y0) = (c(bounds.min.x).floor() as i64, c(bounds.min.y).floor() as i64);
        let (x1, y1) = (c(bounds.max.x).ceil() as i64, c(bounds.max.y).ceil() as i64);
        if x0 >= x1 || y0 >= y1 {
            return None;
        }
        let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
        if w.saturating_mul(h) > MAX_FRAME_PIXELS {
            return None;
        }
        Some(Self { x0, y0, w, h, data: vec![0.0; w * h] })
    }

    fn stamp(&mut self, cx: f32, cy: f32, radius: f32, hardness: f32) {
        let (mut radius, mut flow) = (radius, 1.0);
        if radius < 0.75 {
            flow = (radius / 0.75).max(0.0);
            radius = 0.75;
        }
        let inner = (radius * hardness.clamp(0.0, 1.0)).min(radius - 0.5).max(0.0);
        let lx0 = ((cx - radius - 1.0).floor() - self.x0 as f32).max(0.0) as usize;
        let ly0 = ((cy - radius - 1.0).floor() - self.y0 as f32).max(0.0) as usize;
        let lx1 = (((cx + radius + 1.0).ceil() - self.x0 as f32).max(0.0) as usize).min(self.w);
        let ly1 = (((cy + radius + 1.0).ceil() - self.y0 as f32).max(0.0) as usize).min(self.h);
        for ly in ly0..ly1 {
            let dy = (self.y0 + ly as i64) as f32 + 0.5 - cy;
            for lx in lx0..lx1 {
                let dx = (self.x0 + lx as i64) as f32 + 0.5 - cx;
                let v = coverage((dx * dx + dy * dy).sqrt(), radius, inner, false) * flow;
                let m = &mut self.data[ly * self.w + lx];
                if v > *m {
                    *m = v;
                }
            }
        }
    }

    fn composite(&self, raster: &mut Raster, rgb: [u8; 3], opacity: f32) {
        for ly in 0..self.h {
            for lx in 0..self.w {
                let m = self.data[ly * self.w + lx];
                if m <= 0.0 {
                    continue;
                }
                let (x, y) = (self.x0 + lx as i64, self.y0 + ly as i64);
                let t = TILE as i64;
                let idx = raster.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
                let i = y.rem_euclid(t) as usize * TILE + x.rem_euclid(t) as usize;
                let tile = raster.tile_mut(idx);
                tile[i] = paint(tile[i], rgb, m.min(1.0) * opacity);
            }
        }
    }

    /// Comme `composite`, avec la couleur et l'opacité du dégradé `g` au centre de chaque pixel.
    fn composite_gradient(&self, raster: &mut Raster, g: &crate::degrade::Degrade, opacity: f32) {
        for ly in 0..self.h {
            for lx in 0..self.w {
                let m = self.data[ly * self.w + lx];
                if m <= 0.0 {
                    continue;
                }
                let (x, y) = (self.x0 + lx as i64, self.y0 + ly as i64);
                let (rgb, a) = g.couleur(x as f32 + 0.5, y as f32 + 0.5);
                let t = TILE as i64;
                let idx = raster.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
                let i = y.rem_euclid(t) as usize * TILE + x.rem_euclid(t) as usize;
                let tile = raster.tile_mut(idx);
                tile[i] = paint(tile[i], [rgb[0].round() as u8, rgb[1].round() as u8, rgb[2].round() as u8], m.min(1.0) * opacity * a);
            }
        }
    }
}

pub fn render_element(raster: &mut Raster, e: &Element) {
    render_element_clipped(raster, e, Rect::EVERYTHING);
}

/// Hauteur d'une bande de rendu, en pixels (deux rangées de tuiles).
const BAND: usize = 2 * TILE;

/// Les éléments `elems` (dans cet ordre) posés sur une image neuve `w` × `h`, `shift` les décalant
/// (voir `Place`). Quand tous ont leurs pixels exacts en cache (rien à rejouer), l'image est faite
/// par bandes de lignes réparties entre les cœurs : chaque bande pose chaque élément dans son
/// rectangle seulement, et les bandes (des tuiles entières) se rejoignent sans rien copier. Le
/// résultat est le même pixel pour pixel. Saisir 300 longs traits : 112 ms d'un seul fil.
fn render_in_bands(elems: &[&Element], w: usize, h: usize, shift: (i64, i64), sheet: (usize, usize)) -> Raster {
    let bands = h.div_ceil(BAND);
    let threads = crate::parallele::coeurs().min(8);
    if elems.len() < 8 || bands < 2 || threads < 2 || !elems.iter().all(|e| e.cache_ready()) {
        let mut raster = Raster::new(w, h);
        for e in elems {
            render_element_in(&mut raster, e, Rect::EVERYTHING, Place { shift, sheet, open: false });
        }
        return raster;
    }
    let results: Vec<Vec<(usize, Raster)>> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    (t..bands)
                        .step_by(threads)
                        .map(|b| {
                            let y0 = b * BAND;
                            let mut band = Raster::new(w, BAND.min(h - y0));
                            let place = Place { shift: (shift.0, shift.1 - y0 as i64), sheet, open: false };
                            for e in elems {
                                render_element_in(&mut band, e, Rect::EVERYTHING, place);
                            }
                            (b, band)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap_or_else(|e| std::panic::resume_unwind(e))).collect()
    });
    let mut raster = Raster::new(w, h);
    let per_row = raster.tiles_x;
    for (b, band) in results.into_iter().flatten() {
        let first = b * (BAND / TILE) * per_row;
        for (k, tile) in band.tiles.into_iter().enumerate() {
            raster.tiles[first + k] = tile;
        }
    }
    raster
}

/// Dessine l'élément dans `clip` seulement (bords en pixels entiers) : chaque pixel du rectangle
/// reçoit exactement ce qu'un rendu complet lui donnerait. Un trait est rendu seul une fois (rejoué
/// par le moteur bitmap avec sa trace, ou tracé lisse s'il n'a pas de réglage), ses pixels sont
/// gardés dans sa trace, et les rendus suivants les posent tels quels — transformés si le trait l'a
/// été depuis (déplacement : copie ; échelle, rotation : rééchantillonnage, exact au lâcher quand
/// le cache est refait).
pub fn render_element_clipped(raster: &mut Raster, e: &Element, clip: Rect) {
    render_element_in(raster, e, clip, Place { shift: (0, 0), sheet: (raster.width, raster.height), open: true });
}

/// Comme `render_element_clipped`, sur une image qui n'est pas la feuille elle-même (`place`) :
/// la feuille flottante élargie d'une sélection. `clip` est un rectangle de l'image.
pub fn render_element_in(raster: &mut Raster, e: &Element, clip: Rect, place: Place) {
    let shift = vec2(place.shift.0 as f32, place.shift.1 as f32);
    if let Element::Stroke { points, color, opacity, hardness, brush, trace } = e {
        // Jamais hors de la boîte de l'élément (+ 1,5) : refaire une zone donne la même image
        // qu'un rendu complet.
        let reach = e.bounds().expand(1.5);
        let clip = clip.intersect(reach.translate(shift));
        if clip.width() <= 0.0 || clip.height() <= 0.0 {
            return;
        }
        // Un aperçu transformé pose les pixels de son cache tels quels (il est, par construction,
        // la copie transformée du trait rendu) ; sinon la signature doit correspondre.
        let preview = trace.cache.is_preview();
        let sig = if preview { 0 } else { stroke_signature(points, *color, *opacity, *hardness, brush.as_deref(), trace) };
        if let Ok(cell) = trace.cache.cell.read() {
            if let Some(r) = cell.as_ref().filter(|r| preview || r.sig == sig) {
                if let Ok(mut reg) = REGISTRY.lock() {
                    reg.touch(r.id);
                }
                match &r.pixels {
                    Some(px) => {
                        let delta = px.base.inverse().unwrap_or(Affine::IDENTITY).then(&trace.cache.xf).then(&Affine::translate(shift));
                        blit_cache(raster, px, &delta, clip, place.open);
                    }
                    None => {
                        // Trop grand pour le cache : rejoué à chaque fois, posé dans la zone.
                        let (sheet, off) = render_stroke_alone(place.sheet.0, place.sheet.1, points, *color, *opacity, *hardness, brush.as_deref(), trace, reach, place.open);
                        blit_sheet(raster, &sheet, clip, (place.shift.0 - off.0, place.shift.1 - off.1), place.open);
                    }
                }
                return;
            }
        }
        // Premier rendu, ou le trait a changé depuis : rendu seul, posé, et gardé. Depuis un aperçu
        // (cache perdu en plein geste) : gardé aussi, avec la transformation du moment pour base,
        // sans signature (seul un autre aperçu du même trait le reprendra). Les pixels gardés sont
        // ceux de l'image du trait, marge comprise (ce qui dépasse de la feuille) : `base` en tient
        // compte.
        let (sheet, off) = render_stroke_alone(place.sheet.0, place.sheet.1, points, *color, *opacity, *hardness, brush.as_deref(), trace, reach, place.open);
        blit_sheet(raster, &sheet, clip, (place.shift.0 - off.0, place.shift.1 - off.1), place.open);
        let kept = CachedPixels::from_sheet(&sheet, trace.cache.xf.then(&Affine::translate(vec2(off.0 as f32, off.1 as f32))));
        let bytes = kept.as_ref().map_or(0, CachedPixels::bytes);
        let Ok(mut reg) = REGISTRY.lock() else { return };
        let id = reg.insert(&trace.cache.cell, bytes);
        drop(reg);
        let old = match trace.cache.cell.write() {
            Ok(mut cell) => cell.replace(Rendered { id, sig, pixels: kept }),
            Err(_) => None,
        };
        let dropped = match REGISTRY.lock() {
            Ok(mut reg) => {
                if let Some(old) = &old {
                    reg.remove(old.id);
                }
                reg.enforce()
            }
            Err(_) => Vec::new(),
        };
        drop(old);
        drop(dropped);
        return;
    }
    // Une zone ou un texte sur une image décalée : sa géométrie décalée, posée comme à l'ordinaire.
    if place.shift != (0, 0) {
        render_element_in(raster, &e.transformed(&Affine::translate(shift)), clip, Place { shift: (0, 0), ..place });
        return;
    }
    let bounds = e.bounds().expand(1.5).intersect(clip);
    let mask = if place.open { Mask::open(bounds) } else { Mask::new(bounds, raster.width, raster.height) };
    let Some(mut mask) = mask else {
        return;
    };
    match e {
        Element::Stroke { .. } => unreachable!("rendu plus haut"),
        Element::Fill {
            contours,
            color,
            opacity,
            gradient,
        } => {
            fill_even_odd(&mut mask, contours);
            match gradient {
                Some(g) => mask.composite_gradient(raster, g, *opacity),
                None => mask.composite(raster, *color, *opacity),
            }
        }
        Element::Text { color, opacity, .. } => {
            // Lettre par lettre (règle non nulle des polices) ; les couvertures s'ajoutent, deux
            // lettres qui se touchent ne se creusent pas.
            if let Some((_, glyphs)) = e.text_shape() {
                for g in &glyphs {
                    fill_nonzero(&mut mask, g);
                }
            }
            mask.composite(raster, *color, *opacity);
        }
    }
}

/// L'appui qui a donné le rayon `r` à un trait de réglage `s` : l'inverse de la connexion de la
/// taille (la pression simple d'avant, ou une connexion à courbe, cherchée par dichotomie — la
/// réponse est croissante ou décroissante selon la courbe). Sans connexion : appui plein. (Rendu
/// des brosses, étape 3 : avant, seule la pression simple était retrouvée, une brosse importée à
/// connexion gardait un appui plein au lâcher sur un calque vectoriel.)
pub fn pressure_for_radius(s: &BrushSettings, r: f32) -> f32 {
    let Some(c) = s.size_connection() else { return 1.0 };
    let target = r / (s.size * 0.5).max(1e-3);
    let (at0, at1) = (c.response(0.0), c.response(1.0));
    if (at1 - at0).abs() < 1e-6 {
        return 1.0;
    }
    let rising = at1 > at0;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if (c.response(mid) < target) == rising {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (0.5 * (lo + hi)).clamp(0.0, 1.0)
}

/// Le trait rendu seul sur une feuille `w`×`h` transparente. Avec un réglage : rejoué par le
/// moteur des pinceaux bitmap, point par point, avec l'appui et l'instant de la trace (sinon
/// l'appui d'après le rayon, `pressure_for_radius`) et le toucher du moment (sa courbe de pression
/// et son seuil ont déjà joué : l'appui gardé est celui d'après la courbe). Le chemin gardé est la
/// suite des points lissés que le moteur a reçus pendant le geste : rejoué sans lissage, il donne
/// les mêmes empreintes, dans le même ordre, avec le même hasard — la même image que l'aperçu.
/// Sans réglage (traits d'avant 7b, formes) : le trait lisse d'avant.
///
/// `reach` : la boîte du trait. Ce qui en dépasse de la feuille est gardé (jusqu'à `MAX_OVERHANG`
/// de chaque côté) : le trait est rendu sur une image plus grande, décalée de la marge rendue
/// (gauche, haut), le grain du papier lu comme sur la feuille. Les pixels de la feuille sont
/// ceux qu'on aurait sans marge ; ceux de la marge servent quand le trait est déplacé vers
/// l'intérieur. Aucune marge (trait dans la feuille) : une image à la taille de la feuille.
#[allow(clippy::too_many_arguments)]
fn render_stroke_alone(w: usize, h: usize, points: &[VPoint], color: [u8; 3], opacity: f32, hardness: f32, brush: Option<&BrushSettings>, trace: &Trace, reach: Rect, open: bool) -> (Raster, (i64, i64)) {
    REPLAYS.with(|r| r.set(r.get() + 1));
    // Le cadre du trait : sur la toile infinie (`open`), la feuille agrandie aux tuiles qu'il touche,
    // où qu'elles soient ; sinon, la marge d'avant (jusqu'à `MAX_OVERHANG` de chaque côté).
    let (l, t, sw, sh) = if open {
        let (x0, y0, fw, fh) = stroke_frame(reach, w, h);
        (-x0, -y0, fw, fh)
    } else {
        let (l, t, r, d) = overhang(reach, w, h);
        (l as i64, t as i64, w + l + r, h + t + d)
    };
    let shifted: Vec<VPoint>;
    let points = if (l, t) == (0, 0) {
        points
    } else {
        shifted = points.iter().map(|p| VPoint { x: p.x + l as f32, y: p.y + t as f32, r: p.r }).collect();
        &shifted
    };
    let mut sheet = Raster::new(sw, sh);
    let Some(b) = brush else {
        if let Some(mut mask) = Mask::new(path_bounds(points), sw, sh) {
            stamp_path(&mut mask, points, hardness);
            mask.composite(&mut sheet, color, opacity);
        }
        return (sheet, (l, t));
    };
    let settings = BrushSettings { opacity, smoothing: 0.0, smooth_kind: crate::brush::SmoothKind::Follow, smooth_deferred: false, aliased: false, ..*b };
    let textures = crate::brushtex::shared_textures(&settings);
    let feel = trace.feel.unwrap_or_default();
    let feel = Toucher { pressure_curve: Curve::linear(), threshold: 0.0, ..feel };
    let mut stroke = StrokeState::new(0, StrokeMode::Paint(color), settings, None, true).with_textures(textures).with_feel(feel).with_grain_offset((l, t));
    if let Some(b) = trace.secondary {
        stroke = stroke.with_secondary(b);
    }
    let sampled = trace.pressure.len() == points.len();
    let timed = trace.time.len() == points.len();
    let mut dirty = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let pressure = match trace.pressure.get(i).filter(|_| sampled) {
            Some(&v) if v >= 0.0 => v,
            _ => pressure_for_radius(&settings, p.r),
        };
        match trace.time.get(i).filter(|_| timed) {
            Some(&t) if t >= 0.0 => stroke.add_point_at(&mut sheet, p.pos(), pressure, t, &mut dirty),
            _ => stroke.add_point(&mut sheet, p.pos(), pressure, &mut dirty),
        }
    }
    stroke.finish(&mut sheet, &mut dirty);
    (sheet, (l, t))
}

/// Bornes entières de `clip` dans une image `w`×`h` : (x0, y0, x1, y1), vides si rien. `open` : l'image
/// reçoit aussi ce qui sort de ses bornes (toile infinie), seulement bornée au monde.
fn clip_bounds(clip: Rect, w: usize, h: usize, open: bool) -> (i64, i64, i64, i64) {
    let world = crate::raster::WORLD as f32;
    if open {
        let c = |v: f32| v.clamp(-world, world);
        return (c(clip.min.x).floor() as i64, c(clip.min.y).floor() as i64, c(clip.max.x).ceil() as i64, c(clip.max.y).ceil() as i64);
    }
    let x0 = clip.min.x.max(0.0).floor() as usize;
    let y0 = clip.min.y.max(0.0).floor() as usize;
    let x1 = (clip.max.x.max(0.0).ceil() as usize).min(w);
    let y1 = (clip.max.y.max(0.0).ceil() as usize).min(h);
    (x0 as i64, y0 as i64, x1 as i64, y1 as i64)
}

/// Pose `s` par-dessus le pixel (`x`, `y`) du monde de `raster` (hors de la feuille aussi : la tuile
/// est créée là où il faut).
fn put_over(raster: &mut Raster, x: i64, y: i64, s: Color32) {
    let t = TILE as i64;
    let id = raster.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
    let dst = raster.tile_mut(id);
    let i = y.rem_euclid(t) as usize * TILE + x.rem_euclid(t) as usize;
    dst[i] = crate::render::over(dst[i], s, 255, None);
}

/// Pose la feuille `sheet` (le trait seul) sur `raster`, dans `clip` (un rectangle de `raster`) :
/// le pixel (x, y) de `sheet` va en (x + d.0, y + d.1).
fn blit_sheet(raster: &mut Raster, sheet: &Raster, clip: Rect, d: (i64, i64), open: bool) {
    let (x0, y0, x1, y1) = clip_bounds(clip, raster.width, raster.height, open);
    for idx in 0..sheet.tiles.len() {
        let Some(tile) = &sheet.tiles[idx] else { continue };
        let (tx, ty, tw, th) = sheet.tile_bounds(idx);
        let (from_x, to_x) = ((tx as i64 + d.0).max(x0), ((tx + tw) as i64 + d.0).min(x1));
        let (from_y, to_y) = ((ty as i64 + d.1).max(y0), ((ty + th) as i64 + d.1).min(y1));
        for y in from_y..to_y {
            let sy = (y - d.1) as usize;
            for x in from_x..to_x {
                let sx = (x - d.0) as usize;
                let s = tile[(sy % TILE) * TILE + sx % TILE];
                if s.a() == 0 {
                    continue;
                }
                put_over(raster, x, y, s);
            }
        }
    }
}

/// Pose les pixels en cache d'un trait, transformés par `delta` (rien : tels quels ; une
/// translation entière : copiés décalés ; sinon rééchantillonnés), dans `clip`.
fn blit_cache(raster: &mut Raster, px: &CachedPixels, delta: &Affine, clip: Rect, open: bool) {
    if px.is_empty() {
        return;
    }
    let (cx0, cy0, cx1, cy1) = clip_bounds(clip, raster.width, raster.height, open);
    if cx0 >= cx1 || cy0 >= cy1 {
        return;
    }
    if let Some((dx, dy)) = delta.integer_translation() {
        let (dx, dy) = (dx as i64, dy as i64);
        for cy in 0..px.th {
            for cx in 0..px.tw {
                let Some(tile) = px.tile(cx as i64, cy as i64) else { continue };
                // Coin de la tuile sur la feuille de destination.
                let (ox, oy) = ((px.tx0 + cx as i64) * TILE as i64 + dx, (px.ty0 + cy as i64) * TILE as i64 + dy);
                let y_from = cy0.max(oy);
                let y_to = cy1.min(oy + TILE as i64);
                let x_from = cx0.max(ox);
                let x_to = cx1.min(ox + TILE as i64);
                for y in y_from..y_to {
                    for x in x_from..x_to {
                        let s = tile[(y - oy) as usize * TILE + (x - ox) as usize];
                        if s.a() == 0 {
                            continue;
                        }
                        put_over(raster, x, y, s);
                    }
                }
            }
        }
        return;
    }
    let Some(inv) = delta.inverse() else { return };
    let dest = delta.apply_rect(px.rect()).expand(1.0);
    let (x0, y0, x1, y1) = clip_bounds(dest.intersect(Rect::from_min_max(pos2(cx0 as f32, cy0 as f32), pos2(cx1 as f32, cy1 as f32))), raster.width, raster.height, open);
    for y in y0..y1 {
        for x in x0..x1 {
            let s = inv.apply(pos2(x as f32 + 0.5, y as f32 + 0.5));
            let c = px.sample(s.x, s.y);
            if c.a() == 0 {
                continue;
            }
            put_over(raster, x, y, c);
        }
    }
}

/// Remplissage à règle non nulle (celle des polices : un contour dans le même sens que celui qui
/// l'entoure ne fait pas de trou), anticrénelé comme `fill_even_odd`.
fn fill_nonzero(mask: &mut Mask, contours: &[Vec<[f32; 2]>]) {
    const SUB: usize = 4;
    let mut edges = Vec::new();
    for c in contours.iter().filter(|c| c.len() >= 3) {
        for i in 0..c.len() {
            let (a, b) = (c[i], c[(i + 1) % c.len()]);
            if a[1] != b[1] {
                edges.push((a[0], a[1], b[0], b[1], if b[1] > a[1] { 1 } else { -1 }));
            }
        }
    }
    let ox = mask.x0 as f32;
    let mut xs: Vec<(f32, i32)> = Vec::new();
    for ly in 0..mask.h {
        let row_y = (mask.y0 + ly as i64) as f32;
        let row = &mut mask.data[ly * mask.w..(ly + 1) * mask.w];
        for s in 0..SUB {
            let sy = row_y + (s as f32 + 0.5) / SUB as f32;
            xs.clear();
            for &(x0, y0, x1, y1, dir) in &edges {
                if (y0 <= sy) != (y1 <= sy) {
                    xs.push((x0 + (sy - y0) * (x1 - x0) / (y1 - y0) - ox, dir));
                }
            }
            xs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            let mut start = 0.0;
            for &(x, dir) in &xs {
                let was = winding;
                winding += dir;
                if was == 0 && winding != 0 {
                    start = x;
                } else if was != 0 && winding == 0 {
                    add_span(row, start, x, 1.0 / SUB as f32);
                }
            }
        }
    }
}

fn stamp_path(mask: &mut Mask, points: &[VPoint], hardness: f32) {
    let Some(first) = points.first() else { return };
    mask.stamp(first.x, first.y, first.r, hardness);
    for s in points.windows(2) {
        let (a, b) = (s[0], s[1]);
        let len = a.pos().distance(b.pos());
        let step = (a.r.min(b.r) * 0.25).clamp(0.3, 3.0);
        let n = (len / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            let p = VPoint::lerp(a, b, k as f32 / n as f32);
            mask.stamp(p.x, p.y, p.r, hardness);
        }
    }
}

/// Remplissage pair-impair, anticrénelé : 4 lignes de balayage par pixel et couverture
/// horizontale exacte.
fn fill_even_odd(mask: &mut Mask, contours: &[Vec<[f32; 2]>]) {
    const SUB: usize = 4;
    let mut edges = Vec::new();
    for c in contours.iter().filter(|c| c.len() >= 3) {
        for i in 0..c.len() {
            let (a, b) = (c[i], c[(i + 1) % c.len()]);
            if a[1] != b[1] {
                edges.push((a[0], a[1], b[0], b[1]));
            }
        }
    }
    let ox = mask.x0 as f32;
    let mut xs: Vec<f32> = Vec::new();
    for ly in 0..mask.h {
        let row_y = (mask.y0 + ly as i64) as f32;
        let row = &mut mask.data[ly * mask.w..(ly + 1) * mask.w];
        for s in 0..SUB {
            let sy = row_y + (s as f32 + 0.5) / SUB as f32;
            xs.clear();
            for &(x0, y0, x1, y1) in &edges {
                if (y0 <= sy) != (y1 <= sy) {
                    xs.push(x0 + (sy - y0) * (x1 - x0) / (y1 - y0) - ox);
                }
            }
            xs.sort_by(|a, b| a.total_cmp(b));
            for pair in xs.chunks_exact(2) {
                add_span(row, pair[0], pair[1], 1.0 / SUB as f32);
            }
        }
    }
}

fn add_span(row: &mut [f32], a: f32, b: f32, weight: f32) {
    let a = a.max(0.0);
    let b = b.min(row.len() as f32);
    if b <= a {
        return;
    }
    let first = a.floor() as usize;
    let last = (b.ceil() as usize).min(row.len());
    for (i, v) in row.iter_mut().enumerate().take(last).skip(first) {
        let lo = (i as f32).max(a);
        let hi = ((i + 1) as f32).min(b);
        if hi > lo {
            *v += (hi - lo) * weight;
        }
    }
}

/// Rectangle couvert par un trait de points `path` (cercles de rayon `r`), anticrénelage compris.
pub fn path_bounds(path: &[VPoint]) -> Rect {
    path.iter()
        .fold(Rect::NOTHING, |r, p| r.union(Rect::from_center_size(p.pos(), Vec2::splat(p.r * 2.0 + 3.0))))
}

/// Tuiles de l'image recouvertes par `rect`.
pub fn tiles_in(raster: &Raster, rect: Rect) -> Vec<usize> {
    if !rect.is_finite()
        || rect.max.x < 0.0
        || rect.max.y < 0.0
        || rect.min.x >= raster.width as f32
        || rect.min.y >= raster.height as f32
    {
        return Vec::new();
    }
    let x0 = rect.min.x.max(0.0) as usize / TILE;
    let y0 = rect.min.y.max(0.0) as usize / TILE;
    let x1 = (rect.max.x as usize).min(raster.width - 1) / TILE;
    let y1 = (rect.max.y as usize).min(raster.height - 1) / TILE;
    let tiles_x = raster.tiles_x;
    (y0..=y1)
        .flat_map(|ty| (x0..=x1).map(move |tx| ty * tiles_x + tx))
        .collect()
}

/// Comme `tiles_in`, avec toute la toile infinie : les tuiles hors de la feuille aussi (numéros
/// hors-feuille, `Raster::id_of`). Pour le code qui sait lire et écrire par numéro de tuile
/// (`Raster::slot`, `replace_slot`) ; le code d'avant, qui indexe `raster.tiles`, garde `tiles_in`.
pub fn tiles_in_world(raster: &Raster, rect: Rect) -> Vec<usize> {
    if !rect.is_finite() {
        return Vec::new();
    }
    let w = crate::raster::WORLD as f32;
    let c = |v: f32| v.clamp(-w, w);
    raster.ids_covering(c(rect.min.x).floor() as i64, c(rect.min.y).floor() as i64, c(rect.max.x).floor() as i64 + 1, c(rect.max.y).floor() as i64 + 1)
}

// ---------------------------------------------------------------------------------- géométrie

fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> (f32, f32) {
    let ab = b - a;
    let len2 = ab.length_sq();
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((a + ab * t).distance(p), t)
}

pub fn point_in_polygon(p: Pos2, poly: &[Pos2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            inside = !inside;
        }
    }
    inside
}

fn inside_contours(p: Pos2, contours: &[Vec<[f32; 2]>]) -> bool {
    let mut inside = false;
    for c in contours {
        let n = c.len();
        for i in 0..n {
            let (a, b) = (c[i], c[(i + 1) % n]);
            if (a[1] > p.y) != (b[1] > p.y) && p.x < a[0] + (p.y - a[1]) * (b[0] - a[0]) / (b[1] - a[1]) {
                inside = !inside;
            }
        }
    }
    inside
}

/// Douglas-Peucker : indique quels points garder.
fn rdp_keep(n: usize, tol: f32, dist: impl Fn(usize, usize, usize) -> f32) -> Vec<bool> {
    let mut keep = vec![false; n];
    if n == 0 {
        return keep;
    }
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0, n - 1)];
    while let Some((s, e)) = stack.pop() {
        if e <= s + 1 {
            continue;
        }
        let mut best = (0.0f32, s);
        for i in s + 1..e {
            let d = dist(i, s, e);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > tol {
            keep[best.1] = true;
            stack.push((s, best.1));
            stack.push((best.1, e));
        }
    }
    keep
}

fn dist3(p: VPoint, a: VPoint, b: VPoint) -> f32 {
    let ab = [b.x - a.x, b.y - a.y, b.r - a.r];
    let ap = [p.x - a.x, p.y - a.y, p.r - a.r];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if len2 > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let d = [ap[0] - ab[0] * t, ap[1] - ab[1] * t, ap[2] - ab[2] * t];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Un morceau du trait entre les abscisses curvilignes `from` et `to` (indice du segment +
/// fraction), avec ses appuis et instants interpolés si la trace suit le chemin (pour le cutter).
pub fn piece_traced(points: &[VPoint], trace: &Trace, from: f32, to: f32) -> (Vec<VPoint>, Option<Vec<(f32, f64)>>) {
    let n = points.len();
    let at = |a: f32| -> (VPoint, Option<(f32, f64)>) {
        let i = (a.floor().max(0.0) as usize).min(n.saturating_sub(2));
        let t = (a - i as f32).clamp(0.0, 1.0);
        (VPoint::lerp(points[i], points[(i + 1).min(n - 1)], t), trace.sample(n, i, t))
    };
    let mut out = Vec::new();
    let mut samples = Vec::new();
    let mut push = |p: VPoint, s: Option<(f32, f64)>| {
        out.push(p);
        if let Some(s) = s {
            samples.push(s);
        }
    };
    let (p, s) = at(from);
    push(p, s);
    let first = from.floor() as usize + 1;
    for k in first..n {
        if k as f32 >= to {
            break;
        }
        push(points[k], trace.sample(n, k, 0.0));
    }
    let (p, s) = at(to);
    push(p, s);
    let traced = trace.aligned(n) && samples.len() == out.len();
    (out, traced.then_some(samples))
}

/// Un morceau de trait prêt à poser (le cutter) : voir `piece_element`.
pub fn piece_stroke(points: Vec<VPoint>, samples: Option<Vec<(f32, f64)>>, color: [u8; 3], opacity: f32, hardness: f32, brush: &Option<Box<BrushSettings>>, trace: &Trace) -> Element {
    piece_element(points, samples.as_deref(), color, opacity, hardness, brush, trace)
}

/// Allège un trait sans le déformer de plus de `tol` pixels (épaisseur comprise).
pub fn simplify(points: &[VPoint], tol: f32) -> Vec<VPoint> {
    let mut pts: Vec<VPoint> = Vec::with_capacity(points.len());
    for p in points {
        if pts
            .last()
            .is_none_or(|q| (q.x - p.x).abs() + (q.y - p.y).abs() > 1e-3)
        {
            pts.push(*p);
        }
    }
    if pts.len() <= 2 {
        return pts;
    }
    let keep = rdp_keep(pts.len(), tol, |i, s, e| dist3(pts[i], pts[s], pts[e]));
    pts.into_iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(p))
        .collect()
}

fn simplify_open(pts: &[[f32; 2]], tol: f32) -> Vec<[f32; 2]> {
    let p = |i: usize| pos2(pts[i][0], pts[i][1]);
    let keep = rdp_keep(pts.len(), tol, |i, s, e| segment_distance(p(i), p(s), p(e)).0);
    pts.iter()
        .zip(keep)
        .filter_map(|(q, k)| k.then_some(*q))
        .collect()
}

fn simplify_closed(pts: &[[f32; 2]], tol: f32) -> Vec<[f32; 2]> {
    let n = pts.len();
    if n < 4 {
        return pts.to_vec();
    }
    let d2 = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
    let far = (1..n)
        .max_by(|&i, &j| d2(pts[i], pts[0]).total_cmp(&d2(pts[j], pts[0])))
        .unwrap_or(n / 2);
    let mut out = simplify_open(&pts[..=far], tol);
    let mut back = pts[far..].to_vec();
    back.push(pts[0]);
    let back = simplify_open(&back, tol);
    out.pop();
    out.extend_from_slice(&back[..back.len() - 1]);
    out
}

/// Contours (bords de pixels) d'une zone, simplifiés : la forme vectorielle d'un remplissage.
pub fn trace_contours(mask: &[bool], w: usize, h: usize) -> Vec<Vec<[f32; 2]>> {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if x0 == usize::MAX {
        return Vec::new();
    }
    let inside = |x: i64, y: i64| {
        x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && mask[y as usize * w + x as usize]
    };
    // Chaque bord entre un pixel de la zone et un pixel extérieur devient une arête orientée.
    let mut next: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if !mask[y * w + x] {
                continue;
            }
            let (x, y) = (x as i64, y as i64);
            if !inside(x, y - 1) {
                next.entry((x, y)).or_default().push((x + 1, y));
            }
            if !inside(x + 1, y) {
                next.entry((x + 1, y)).or_default().push((x + 1, y + 1));
            }
            if !inside(x, y + 1) {
                next.entry((x + 1, y + 1)).or_default().push((x, y + 1));
            }
            if !inside(x - 1, y) {
                next.entry((x, y + 1)).or_default().push((x, y));
            }
        }
    }
    let mut loops = Vec::new();
    let starts: Vec<(i64, i64)> = next.keys().copied().collect();
    for s in starts {
        while let Some(first) = next.get_mut(&s).and_then(|v| v.pop()) {
            let mut path = vec![s];
            let mut cur = first;
            while cur != s {
                path.push(cur);
                match next.get_mut(&cur).and_then(|v| v.pop()) {
                    Some(n) => cur = n,
                    None => break,
                }
            }
            loops.push(path);
        }
    }
    loops
        .into_iter()
        .filter_map(|lp| {
            let n = lp.len();
            let corners: Vec<[f32; 2]> = (0..n)
                .filter(|&i| {
                    let (a, b, c) = (lp[(i + n - 1) % n], lp[i], lp[(i + 1) % n]);
                    (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0) != 0
                })
                .map(|i| [lp[i].0 as f32, lp[i].1 as f32])
                .collect();
            if corners.len() < 3 {
                return None;
            }
            let simple = simplify_closed(&corners, 0.7);
            (simple.len() >= 3).then_some(simple)
        })
        .collect()
}

/// Le chemin densifié (un point tous les `step` pixels au plus), avec, pour chaque point, l'appui
/// et l'instant interpolés de la trace quand elle suit le chemin (sinon rien).
pub fn densify_traced(points: &[VPoint], trace: &Trace, step: f32) -> (Vec<VPoint>, Vec<(f32, f64)>) {
    let n = points.len();
    let aligned = trace.aligned(n);
    let mut out = Vec::with_capacity(n);
    let mut samples = Vec::with_capacity(if aligned { n } else { 0 });
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            let a = points[i - 1];
            let m = (a.pos().distance(p.pos()) / step).ceil() as usize;
            for k in 1..m {
                let t = k as f32 / m as f32;
                out.push(VPoint::lerp(a, *p, t));
                if aligned {
                    samples.extend(trace.sample(n, i - 1, t));
                }
            }
        }
        out.push(*p);
        if aligned {
            samples.extend(trace.sample(n, i, 0.0));
        }
    }
    (out, samples)
}

/// Un morceau d'un trait, avec ses appuis s'ils suivaient le chemin ; un trait lisse (sans
/// réglage) est simplifié comme avant.
fn piece_element(points: Vec<VPoint>, samples: Option<&[(f32, f64)]>, color: [u8; 3], opacity: f32, hardness: f32, brush: &Option<Box<BrushSettings>>, trace: &Trace) -> Element {
    let (points, trace) = match (brush, samples) {
        (None, _) => (simplify(&points, 0.25), trace.without_samples()),
        (Some(_), Some(s)) if s.len() == points.len() => (points, trace.with_samples(s)),
        (Some(_), _) => (points, trace.without_samples()),
    };
    Element::Stroke { points, color, opacity, hardness, brush: brush.clone(), trace }
}

/// Gomme vectorielle : coupe les traits sous le cercle (ou les retire entiers si `whole`).
/// Les zones remplies et les textes ne sont retirés qu'en mode `whole`. Les morceaux d'un trait
/// rejoué gardent ses appuis et ses instants (interpolés) ; les traits non touchés ne bougent pas.
pub fn erase_circle(d: &mut VectorDrawing, c: Pos2, radius: f32, whole: bool) -> bool {
    let mut changed = false;
    let mut out = Vec::with_capacity(d.elements.len());
    for e in std::mem::take(&mut d.elements) {
        if !e.bounds().expand(radius).contains(c) {
            out.push(e);
            continue;
        }
        let cut: Option<Vec<Element>> = match &e {
            Element::Stroke {
                points,
                color,
                opacity,
                hardness,
                brush,
                trace,
            } => {
                if whole {
                    e.hit(c, radius).then(Vec::new)
                } else {
                    let (dense, samples) = densify_traced(points, trace, (radius * 0.3).max(0.4));
                    let traced = samples.len() == dense.len();
                    let mut pieces: Vec<(Vec<VPoint>, Vec<(f32, f64)>)> = Vec::new();
                    let mut cur: (Vec<VPoint>, Vec<(f32, f64)>) = (Vec::new(), Vec::new());
                    let mut removed = false;
                    for (k, p) in dense.iter().enumerate() {
                        if p.pos().distance(c) <= radius {
                            removed = true;
                            if !cur.0.is_empty() {
                                pieces.push(std::mem::take(&mut cur));
                            }
                        } else {
                            cur.0.push(*p);
                            if traced {
                                cur.1.push(samples[k]);
                            }
                        }
                    }
                    if !cur.0.is_empty() {
                        pieces.push(cur);
                    }
                    removed.then(|| {
                        pieces
                            .into_iter()
                            .filter(|p| p.0.len() >= 2)
                            .map(|(points, s)| piece_element(points, traced.then_some(s.as_slice()), *color, *opacity, *hardness, brush, trace))
                            .collect()
                    })
                }
            }
            Element::Fill { .. } | Element::Text { .. } => (whole && e.hit(c, 0.0)).then(Vec::new),
        };
        match cut {
            Some(parts) => {
                changed = true;
                out.extend(parts);
            }
            None => out.push(e),
        }
    }
    d.elements = out;
    changed
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    /// Le budget des caches : au-dessus, les pixels posés le moins récemment partent les premiers
    /// (jusqu'à 90 % du budget), après les caches des traits disparus.
    #[test]
    fn the_cache_budget_drops_the_least_recently_used_pixels_first() {
        let mut reg = Registry { entries: HashMap::new(), total: 0, clock: 0, next_id: 1, budget: 1000 };
        let mut cells: Vec<Cell> = (0..5).map(|_| Cell::default()).collect();
        let ids: Vec<u64> = cells
            .iter()
            .map(|c| {
                let id = reg.insert(c, 300);
                *c.write().unwrap() = Some(Rendered { id, sig: 0, pixels: None });
                id
            })
            .collect();
        assert_eq!(reg.total, 1500);
        // Le premier vient d'être utilisé : il reste ; les deux plus vieux partent (1500 → 900).
        reg.touch(ids[0]);
        let dropped = reg.enforce();
        assert_eq!(dropped.len(), 2);
        assert_eq!(reg.total, 900);
        assert!(cells[0].read().unwrap().is_some());
        assert!(cells[1].read().unwrap().is_none() && cells[2].read().unwrap().is_none());
        assert!(cells[3].read().unwrap().is_some() && cells[4].read().unwrap().is_some());
        // Un trait disparu (plus personne ne tient sa cellule) : son cache est oublié d'abord, sans
        // toucher aux vivants.
        drop(cells.remove(3));
        let fresh = Cell::default();
        let id = reg.insert(&fresh, 300);
        *fresh.write().unwrap() = Some(Rendered { id, sig: 0, pixels: None });
        assert_eq!(reg.total, 1200);
        let dropped = reg.enforce();
        assert!(dropped.is_empty());
        assert_eq!(reg.total, 900);
        assert!(fresh.read().unwrap().is_some());
        assert!(cells[0].read().unwrap().is_some() && cells[3].read().unwrap().is_some(), "les vivants restent");
    }
}
