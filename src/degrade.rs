//! Dégradé (outil « Dégradé », Anthony, 2026-10-24 : « un outil dégradé, vecto et bitmap »).
//!
//! Ce module est le noyau, sans interface : le dégradé lui-même (`Degrade` : deux points, deux couleurs, deux
//! opacités, linéaire ou radial), ses réglages (`Reglages`) et son écriture dans des pixels (`peindre`).
//!
//! * Le point `depart` a la couleur de départ (la couleur d'avant-plan), le point `arrivee` la couleur d'arrivée (la
//!   couleur d'arrière-plan, ou la même devenue transparente). Au-delà des deux points, la couleur reste celle du bout.
//!   Linéaire : la couleur ne dépend que de la position le long de l'axe. Radial : `depart` est le centre,
//!   `arrivee` un point du cercle d'arrivée.
//! * Sur un calque vectoriel, le dégradé n'est jamais écrit en pixels dans les données : `Element::Fill` le garde
//!   (`gradient`), il se déplace avec la forme (`Degrade::transformed`) et se rend avec elle (`vector.rs`).
//! * Sur un calque bitmap, `peindre` écrit les pixels, par tuile, sans le moteur de pinceaux : un dégradé à 1920 × 1080 ne
//!   coûte que quelques millisecondes réparties sur les cœurs.

use eframe::egui::{Color32, Pos2};
use serde::{Deserialize, Serialize};

use crate::pixsel::PixelMask;
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS};
use crate::stencil::Stencil;
use crate::vector::Affine;

/// La forme du dégradé.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Genre {
    #[default]
    #[serde(rename = "lineaire")]
    Lineaire,
    #[serde(rename = "radial")]
    Radial,
}

/// Un dégradé : ses deux points (en pixels du dessin), ses deux couleurs et leurs opacités (0 à 1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Degrade {
    #[serde(rename = "genre")]
    pub genre: Genre,
    pub depart: [f32; 2],
    pub arrivee: [f32; 2],
    #[serde(rename = "couleur_depart")]
    pub c0: [u8; 3],
    #[serde(rename = "couleur_arrivee")]
    pub c1: [u8; 3],
    #[serde(rename = "opacite_depart")]
    pub a0: f32,
    #[serde(rename = "opacite_arrivee")]
    pub a1: f32,
}

impl Degrade {
    /// Tout est fini et les opacités sont des opacités : un dégradé lu d'un fichier abîmé ne passe pas le contrôle.
    pub fn is_sane(&self) -> bool {
        self.depart.iter().chain(self.arrivee.iter()).all(|v| v.is_finite()) && (0.0..=1.0).contains(&self.a0) && (0.0..=1.0).contains(&self.a1)
    }

    /// Où en est le point (`x`, `y`) du dégradé : 0 au départ, 1 à l'arrivée, borné.
    pub fn parametre(&self, x: f32, y: f32) -> f32 {
        let (dx, dy) = (self.arrivee[0] - self.depart[0], self.arrivee[1] - self.depart[1]);
        let (px, py) = (x - self.depart[0], y - self.depart[1]);
        let t = match self.genre {
            Genre::Lineaire => {
                let l2 = dx * dx + dy * dy;
                if l2 <= 1e-12 {
                    0.0
                } else {
                    (px * dx + py * dy) / l2
                }
            }
            Genre::Radial => {
                let r = (dx * dx + dy * dy).sqrt();
                if r <= 1e-6 {
                    0.0
                } else {
                    (px * px + py * py).sqrt() / r
                }
            }
        };
        if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// La couleur (non prémultipliée, 0 à 255) et l'opacité (0 à 1) au point (`x`, `y`).
    pub fn couleur(&self, x: f32, y: f32) -> ([f32; 3], f32) {
        let t = self.parametre(x, y);
        let m = |a: u8, b: u8| a as f32 + (b as f32 - a as f32) * t;
        ([m(self.c0[0], self.c1[0]), m(self.c0[1], self.c1[1]), m(self.c0[2], self.c1[2])], self.a0 + (self.a1 - self.a0) * t)
    }

    /// Le pixel prémultiplié au point (`x`, `y`) (le centre du pixel : `x + 0,5`), l'opacité multipliée par `k`.
    pub fn pixel(&self, x: f32, y: f32, k: f32) -> Color32 {
        let (rgb, a) = self.couleur(x, y);
        premultiplie(rgb, (a * k).clamp(0.0, 1.0))
    }

    /// Le dégradé à mi-chemin de `a` (`t` = 0) et `b` (`t` = 1), de même genre : axe, couleurs et opacités se mélangent
    /// (l'interpolation d'un plan à l'autre, `tween.rs`).
    pub fn entre(a: &Degrade, b: &Degrade, t: f32) -> Degrade {
        let l = |x: f32, y: f32| x + (y - x) * t;
        let c = |x: [u8; 3], y: [u8; 3]| [0, 1, 2].map(|k| l(x[k] as f32, y[k] as f32).round().clamp(0.0, 255.0) as u8);
        Degrade {
            genre: a.genre,
            depart: [l(a.depart[0], b.depart[0]), l(a.depart[1], b.depart[1])],
            arrivee: [l(a.arrivee[0], b.arrivee[0]), l(a.arrivee[1], b.arrivee[1])],
            c0: c(a.c0, b.c0),
            c1: c(a.c1, b.c1),
            a0: l(a.a0, b.a0).clamp(0.0, 1.0),
            a1: l(a.a1, b.a1).clamp(0.0, 1.0),
        }
    }

    /// Les mêmes bouts échangés : le départ prend la couleur d'arrivée et inversement.
    pub fn reversed(&self) -> Degrade {
        Degrade { c0: self.c1, c1: self.c0, a0: self.a1, a1: self.a0, ..*self }
    }

    /// Le dégradé dont les deux points passent par `f` (la forme se déplace, tourne, se déforme : son axe la suit).
    pub fn mapped(&self, f: impl Fn(Pos2) -> Pos2) -> Degrade {
        let (a, b) = (f(Pos2::new(self.depart[0], self.depart[1])), f(Pos2::new(self.arrivee[0], self.arrivee[1])));
        Degrade { depart: [a.x, a.y], arrivee: [b.x, b.y], ..*self }
    }

    /// Le dégradé transformé par `a` : son axe suit la forme (le rayon d'un dégradé radial suit l'échelle).
    pub fn transformed(&self, a: &Affine) -> Degrade {
        self.mapped(|p| a.apply(p))
    }

    /// Le rectangle qui contient les deux points.
    pub fn axe(&self) -> (Pos2, Pos2) {
        (Pos2::new(self.depart[0], self.depart[1]), Pos2::new(self.arrivee[0], self.arrivee[1]))
    }
}

/// Couleur `rgb` (0 à 255, non prémultipliée) d'opacité `a`, en pixel prémultiplié.
pub fn premultiplie(rgb: [f32; 3], a: f32) -> Color32 {
    let c = |v: f32| (v * a + 0.5).clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(c(rgb[0]), c(rgb[1]), c(rgb[2]), (a * 255.0 + 0.5).clamp(0.0, 255.0) as u8)
}

/// Comment le dégradé finit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fin {
    /// Vers la couleur de fond (celle d'arrière-plan, le second carré du panneau Couleur).
    #[default]
    VersFond,
    /// Vers le transparent : la couleur d'avant-plan, de plus en plus transparente.
    VersTransparent,
}

/// Les réglages de l'outil (Réglages, outil Dégradé).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reglages {
    pub genre: Genre,
    pub fin: Fin,
    /// Opacité du dégradé, 0 à 1.
    pub opacite: f32,
    /// Échanger les deux bouts.
    pub inverse: bool,
    /// Remplir tout le calque (Alt tenu : la même chose, le temps du geste).
    pub tout_le_calque: bool,
}

impl Default for Reglages {
    fn default() -> Self {
        Self { genre: Genre::Lineaire, fin: Fin::VersFond, opacite: 1.0, inverse: false, tout_le_calque: false }
    }
}

impl Reglages {
    /// Le dégradé du geste qui va de `depart` à `arrivee`, avec les couleurs `avant` (avant-plan) et `arriere`
    /// (arrière-plan).
    pub fn degrade(&self, avant: [u8; 3], arriere: [u8; 3], depart: Pos2, arrivee: Pos2) -> Degrade {
        let o = self.opacite.clamp(0.0, 1.0);
        let (c1, a1) = match self.fin {
            Fin::VersFond => (arriere, 1.0),
            Fin::VersTransparent => (avant, 0.0),
        };
        let g = Degrade { genre: self.genre, depart: [depart.x, depart.y], arrivee: [arrivee.x, arrivee.y], c0: avant, c1, a0: o, a1: a1 * o };
        if self.inverse {
            g.reversed()
        } else {
            g
        }
    }
}

/// Où `peindre` écrit.
pub enum Portee<'a> {
    /// Toute la feuille.
    Tout,
    /// Une zone trouvée comme le fait le pot de peinture (`fill.rs`) : `region` est la zone, `grown` la zone élargie du
    /// débord. Dans la zone, le dégradé remplace ce qui est là (de quoi recolorer un aplat) ; dans le débord, il passe
    /// *sous* l'existant, pour ne pas abîmer le bord anticrénelé du trait.
    Zone { region: &'a [bool], grown: &'a [bool] },
    /// Une sélection (couverture de 0 à 1 par pixel, bords anticrénelés) : le dégradé se pose par-dessus, dans la
    /// mesure de la couverture.
    Masque(&'a PixelMask),
}

/// `top` (prémultiplié) posé sur `under` (prémultiplié) : `under` passe dessous.
fn derriere(top: Color32, under: Color32) -> Color32 {
    let inv = 255 - top.a() as u32;
    let ch = |t: u8, u: u8| (t as u32 + (u as u32 * inv + 127) / 255).min(255) as u8;
    Color32::from_rgba_premultiplied(ch(top.r(), under.r()), ch(top.g(), under.g()), ch(top.b(), under.b()), ch(top.a(), under.a()))
}

/// Le pixel (`x`, `y`) de `dst` après la pose du dégradé avec la couverture `cover` (0 à 1) : par-dessus ; sur un calque
/// qui préserve la transparence (`garde_alpha`), la couleur remplace celle du pixel dans la mesure de la couverture et
/// de l'opacité du dégradé, et le vide reste vide (`remplirsel::filled_pixel`).
fn pose(dst: Color32, g: &Degrade, x: usize, y: usize, cover: f32, garde_alpha: bool) -> Color32 {
    let (rgb, a) = g.couleur(x as f32 + 0.5, y as f32 + 0.5);
    let c = (a * cover).clamp(0.0, 1.0);
    if c <= 0.0 {
        return dst;
    }
    if garde_alpha {
        return crate::remplirsel::filled_pixel(dst, [rgb[0].round() as u8, rgb[1].round() as u8, rgb[2].round() as u8], c, true);
    }
    crate::render::over(dst, premultiplie(rgb, c), 255, None)
}

/// Les pixels de la tuile `idx` après le dégradé ; `None` si rien n'y change.
fn tuile_peinte(raster: &Raster, idx: usize, portee: &Portee, g: &Degrade, garde_alpha: bool, pochoir: Option<&Stencil>) -> Option<Tile> {
    let (ox, oy, tw, th) = raster.tile_bounds(idx);
    let w = raster.width;
    let ancienne = raster.tiles[idx].as_ref();
    let mut px: [Color32; TILE_PIXELS] = match ancienne {
        Some(t) => **t,
        None => [Color32::TRANSPARENT; TILE_PIXELS],
    };
    let mut change = false;
    for y in 0..th {
        for x in 0..tw {
            let (gx, gy) = (ox + x, oy + y);
            let i = y * TILE + x;
            let facteur = pochoir.map_or(1.0, |st| st.at(idx, i));
            if facteur <= 0.0 {
                continue;
            }
            let dst = px[i];
            let out = match portee {
                Portee::Tout => pose(dst, g, gx, gy, facteur, garde_alpha),
                Portee::Masque(m) => {
                    let cover = m.at(gx, gy) * facteur;
                    if cover <= 0.0 {
                        continue;
                    }
                    pose(dst, g, gx, gy, cover, garde_alpha)
                }
                Portee::Zone { region, grown } => {
                    let k = gy * w + gx;
                    if !grown[k] {
                        continue;
                    }
                    let src = g.pixel(gx as f32 + 0.5, gy as f32 + 0.5, 1.0);
                    if region[k] {
                        src
                    } else {
                        derriere(dst, src)
                    }
                }
            };
            if out != dst {
                px[i] = out;
                change = true;
            }
        }
    }
    change.then(|| Tile::new(px))
}

/// Écrit le dégradé `g` dans `raster` (la feuille), dans `portee` : rend les tuiles d'avant (pour l'annulation, une seule
/// étape). Les tuiles se calculent en parallèle ; rien n'est écrit dans une tuile que le dégradé ne change pas.
pub fn peindre(raster: &mut Raster, portee: &Portee, g: &Degrade, garde_alpha: bool, pochoir: Option<&Stencil>) -> Vec<(usize, Option<Tile>)> {
    let w = raster.width;
    // Les tuiles que la portée peut toucher.
    let candidates: Vec<usize> = (0..raster.tiles.len())
        .filter(|&idx| {
            if pochoir.is_some_and(|st| st.blocks(idx)) {
                return false;
            }
            let (ox, oy, tw, th) = raster.tile_bounds(idx);
            match portee {
                Portee::Tout => true,
                Portee::Masque(m) => {
                    let b = m.bounds();
                    (ox as f32) < b.max.x && ((ox + tw) as f32) > b.min.x && (oy as f32) < b.max.y && ((oy + th) as f32) > b.min.y
                }
                Portee::Zone { grown, .. } => (0..th).any(|y| {
                    let s = (oy + y) * w + ox;
                    s + tw <= grown.len() && grown[s..s + tw].iter().any(|&g| g)
                }),
            }
        })
        .collect();
    let calculees: Vec<Option<Tile>> = if candidates.len() >= 8 {
        crate::parallele::par_map(&candidates, 16, |_, &idx| tuile_peinte(raster, idx, portee, g, garde_alpha, pochoir))
    } else {
        candidates.iter().map(|&idx| tuile_peinte(raster, idx, portee, g, garde_alpha, pochoir)).collect()
    };
    let mut avant = Vec::new();
    for (idx, neuve) in candidates.into_iter().zip(calculees) {
        if let Some(t) = neuve {
            avant.push((idx, raster.tiles[idx].clone()));
            raster.tiles[idx] = Some(t);
        }
    }
    avant
}

/// Le dégradé en couleurs d'une grille de `n` × `n` cases sur le rectangle `de`..`a` : de quoi peindre l'aperçu d'un geste
/// avec un maillage de sommets colorés (un sommet par angle de case, prémultiplié).
pub fn sommets(g: &Degrade, de: Pos2, a: Pos2, nx: usize, ny: usize, k: f32) -> Vec<Color32> {
    let mut out = Vec::with_capacity((nx + 1) * (ny + 1));
    for j in 0..=ny {
        for i in 0..=nx {
            let p = Pos2::new(de.x + (a.x - de.x) * i as f32 / nx.max(1) as f32, de.y + (a.y - de.y) * j as f32 / ny.max(1) as f32);
            out.push(g.pixel(p.x, p.y, k));
        }
    }
    out
}
