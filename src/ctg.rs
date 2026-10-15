//! Calque CTG (13j), celui de TVPaint 12 (textes 011085106–204, 111020100–107) ; principe
//! confirmé par Anthony le 2026-10-16. On griffonne des couleurs (les « graffitis ») sur le
//! calque CTG ; chaque zone fermée par les traits des calques sources, touchée par un graffiti,
//! est remplie de sa couleur, sous les traits. Le remplissage est vivant (choix d'Anthony) :
//! recalculé à l'affichage et à l'export depuis les graffitis et les traits (dessin dérivé, voir
//! `fx::shown`), il suit toute retouche. « Charger la structure à partir des sources » donne au
//! calque CTG un dessin par dessin des sources.
//!
//! Remplissage : un trait est un pixel des sources d'opacité au moins `LINE_ALPHA` ; chaque zone
//! (pixels voisins hors trait) touchée par des graffitis prend la couleur la plus présente parmi
//! eux ; puis la couleur glisse sous les traits voisins (`UNDER` pixels), pour ne pas laisser de
//! liseré entre le trait et la couleur.

use std::collections::HashMap;

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::document::{CelId, Layer};
use crate::raster::{Raster, TILE};

/// Réglages d'un calque CTG : ses calques sources (id de calque ; aucun : tous les autres
/// calques de dessin du plan), et la fermeture automatique des trous (demandée par Anthony le
/// 2026-10-16) : un trou des traits jusqu'à `2 × gap` pixels ne laisse pas fuir la couleur
/// (0 : pas de fermeture).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ctg {
    pub sources: Vec<u64>,
    #[serde(default = "default_gap")]
    pub gap: u8,
}

/// Fermeture des trous d'un calque CTG neuf, en pixels de chaque côté du trou.
pub const DEFAULT_GAP: u8 = 4;

fn default_gap() -> u8 {
    DEFAULT_GAP
}

impl Default for Ctg {
    fn default() -> Self {
        Ctg { sources: Vec::new(), gap: DEFAULT_GAP }
    }
}

/// Ce que le remplissage d'un calque CTG lit à une image : les dessins de ses sources et sa
/// fermeture des trous.
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct CtgInput {
    pub sources: Vec<CelId>,
    pub gap: u8,
}

/// Opacité à partir de laquelle un pixel des sources est un trait (il ferme les zones).
pub const LINE_ALPHA: u8 = 128;
/// Pixels de trait que la couleur gagne sous le trait, de chaque côté.
pub const UNDER: usize = 2;

/// Les calques sources du calque CTG `li` de `layers` : les choisis, sinon tous les autres
/// calques de dessin (ni dossier, ni CTG).
pub fn source_layers(layers: &[Layer], li: usize) -> Vec<usize> {
    let Some(ctg) = layers.get(li).and_then(|l| l.ctg.as_ref()) else { return Vec::new() };
    (0..layers.len())
        .filter(|&k| k != li && !layers[k].folder && layers[k].ctg.is_none())
        .filter(|&k| ctg.sources.is_empty() || ctg.sources.contains(&layers[k].id))
        .collect()
}

/// Les dessins des sources du calque CTG `li` à l'image `frame` (ce que le remplissage lit).
pub fn source_cels(layers: &[Layer], li: usize, frame: usize) -> Vec<CelId> {
    source_layers(layers, li).into_iter().filter_map(|k| layers[k].shown_at(frame)).collect()
}

/// Ce que lit le remplissage du calque CTG `li` à l'image `frame` ; rien pour un calque ordinaire.
pub fn input(layers: &[Layer], li: usize, frame: usize) -> Option<CtgInput> {
    let ctg = layers.get(li)?.ctg.as_ref()?;
    Some(CtgInput { sources: source_cels(layers, li, frame), gap: ctg.gap })
}

/// Les traits épaissis de `r` pixels (carré) : les pixels à moins de `r` d'un trait, en lignes
/// puis en colonnes (deux balayages par sens).
fn thicken(line: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let pass = |src: &[bool], n: usize, len: usize, at: &dyn Fn(usize, usize) -> usize| -> Vec<bool> {
        let mut out = vec![false; src.len()];
        for k in 0..n {
            // Distance au dernier trait vu, de gauche à droite puis de droite à gauche.
            let mut d = usize::MAX;
            for i in 0..len {
                d = if src[at(k, i)] { 0 } else { d.saturating_add(1) };
                if d <= r {
                    out[at(k, i)] = true;
                }
            }
            d = usize::MAX;
            for i in (0..len).rev() {
                d = if src[at(k, i)] { 0 } else { d.saturating_add(1) };
                if d <= r {
                    out[at(k, i)] = true;
                }
            }
        }
        out
    };
    let rows = pass(line, h, w, &|y, x| y * w + x);
    pass(&rows, w, h, &|x, y| y * w + x)
}

/// Le remplissage : les zones fermées par les traits de `sources`, touchées par `graffiti`,
/// remplies de leur couleur. `gap` : les trous des traits jusqu'à `2 × gap` pixels sont fermés.
pub fn fill(sources: &[&Raster], graffiti: &Raster, gap: usize) -> Raster {
    let (w, h) = (graffiti.width, graffiti.height);
    let mut out = Raster::new(w, h);
    if w == 0 || h == 0 {
        return out;
    }
    // Traits : l'opacité la plus forte des sources.
    let mut line = vec![0u8; w * h];
    for s in sources {
        for (idx, t) in s.tiles.iter().enumerate() {
            let Some(t) = t else { continue };
            let (ox, oy, tw, th) = s.tile_bounds(idx);
            for y in 0..th.min(h.saturating_sub(oy)) {
                for x in 0..tw.min(w.saturating_sub(ox)) {
                    let a = t[y * TILE + x].a();
                    let p = &mut line[(oy + y) * w + ox + x];
                    *p = (*p).max(a);
                }
            }
        }
    }
    // Murs des zones : les traits, épaissis de `gap` pixels pour fermer les trous.
    let solid: Vec<bool> = line.iter().map(|&a| a >= LINE_ALPHA).collect();
    let wall = if gap > 0 { thicken(&solid, w, h, gap) } else { solid.clone() };
    // Couleur de chaque pixel : 0 = rien, sinon rang + 1 dans `colors`.
    let mut colors: Vec<Color32> = Vec::new();
    let mut color_of = vec![0u32; w * h];
    let mut seen = vec![false; w * h];
    let mut stack: Vec<usize> = Vec::new();
    let mut region: Vec<usize> = Vec::new();
    for (idx, t) in graffiti.tiles.iter().enumerate() {
        let Some(t) = t else { continue };
        let (ox, oy, tw, th) = graffiti.tile_bounds(idx);
        for y in 0..th {
            for x in 0..tw {
                if t[y * TILE + x].a() == 0 {
                    continue;
                }
                let start = (oy + y) * w + ox + x;
                if seen[start] || wall[start] {
                    continue;
                }
                // La zone de ce graffiti, et les couleurs des graffitis qui la touchent.
                region.clear();
                stack.push(start);
                seen[start] = true;
                let mut votes: HashMap<[u8; 3], u32> = HashMap::new();
                while let Some(p) = stack.pop() {
                    region.push(p);
                    let g = graffiti.get(p % w, p / w);
                    if g.a() != 0 {
                        let [r, gg, b, _] = crate::raster::unpremultiply(g);
                        *votes.entry([r, gg, b]).or_insert(0) += 1;
                    }
                    let (px, py) = (p % w, p / w);
                    let mut visit = |q: usize| {
                        if !seen[q] && !wall[q] {
                            seen[q] = true;
                            stack.push(q);
                        }
                    };
                    if px > 0 {
                        visit(p - 1);
                    }
                    if px + 1 < w {
                        visit(p + 1);
                    }
                    if py > 0 {
                        visit(p - w);
                    }
                    if py + 1 < h {
                        visit(p + w);
                    }
                }
                let Some((rgb, _)) = votes.into_iter().max_by_key(|(c, n)| (*n, *c)) else { continue };
                colors.push(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                let k = colors.len() as u32;
                for &p in &region {
                    color_of[p] = k;
                }
            }
        }
    }
    if colors.is_empty() {
        return out;
    }
    // La couleur regagne la bande d'épaississement (hors des vrais traits), puis passe sous les
    // traits voisins (`UNDER` pixels).
    grow(&mut color_of, w, h, gap, |p| !solid[p]);
    grow(&mut color_of, w, h, UNDER, |p| line[p] != 0);
    for (p, &k) in color_of.iter().enumerate() {
        if k != 0 {
            let (x, y) = (p % w, p / w);
            let idx = out.tile_index_of(x, y);
            out.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = colors[k as usize - 1];
        }
    }
    out
}

/// `times` fois : chaque pixel sans couleur où `can` permet d'aller prend la couleur d'un voisin.
fn grow(color_of: &mut [u32], w: usize, h: usize, times: usize, can: impl Fn(usize) -> bool) {
    for _ in 0..times {
        let before = color_of.to_vec();
        let mut changed = false;
        for y in 0..h {
            for x in 0..w {
                let p = y * w + x;
                if before[p] != 0 || !can(p) {
                    continue;
                }
                let near = [(x > 0).then(|| p - 1), (x + 1 < w).then(|| p + 1), (y > 0).then(|| p - w), (y + 1 < h).then(|| p + w)];
                if let Some(k) = near.into_iter().flatten().map(|q| before[q]).find(|&k| k != 0) {
                    color_of[p] = k;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// « Charger la structure à partir des sources » : les expositions du calque CTG suivent celles
/// des sources (une suite d'images où les sources montrent les mêmes dessins = une exposition
/// du calque CTG, vide là où les sources sont vides). `old` : les expositions du calque CTG ;
/// sans `clear`, une exposition garde le dessin de graffitis qu'il avait à son début. `fresh`
/// fournit un dessin vide neuf. Renvoie les nouvelles expositions.
pub fn structure(len: usize, sources: &[&Layer], old: &[Option<CelId>], clear: bool, mut fresh: impl FnMut() -> CelId) -> Vec<Option<CelId>> {
    let key = |f: usize| -> Vec<Option<CelId>> { sources.iter().map(|l| l.cel_at(f)).collect() };
    let mut out = Vec::with_capacity(len);
    let mut f = 0;
    while f < len {
        let k = key(f);
        let mut g = f + 1;
        while g < len && key(g) == k {
            g += 1;
        }
        let cel = if k.iter().all(Option::is_none) {
            None
        } else if let (false, Some(Some(c))) = (clear, old.get(f)) {
            Some(*c)
        } else {
            Some(fresh())
        };
        out.extend(std::iter::repeat_n(cel, g - f));
        f = g;
    }
    while out.last() == Some(&None) {
        out.pop();
    }
    out
}
