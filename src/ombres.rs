//! **Les ombres des cartes et du décor 3D** (partie 185, `docs/INTERFACE.md` ; `docs/ESPACE3D.md`, partie 16).
//!
//! Jusqu'ici le décor 3D d'une séquence (parties 180 à 184) ne portait d'ombres que sur lui-même. Maintenant, sous le même
//! soleil :
//! 1. **une carte porte une ombre sur le décor** (le sol, les dalles, les objets) : l'ombre de son encre (son alpha), pas de son
//!    rectangle ; un personnage debout sur le sol y a son ombre ;
//! 2. **le décor ombre les cartes** : là où un objet est entre le soleil et une carte, la carte s'assombrit, comme une face du
//!    décor à l'ombre ;
//! 3. **une carte ombre les autres cartes** : la même carte d'ombres des cartes sert aux deux (2 et 3 ne font qu'une lecture de
//!    plus), sans rien à régler.
//!
//! **Comment.** Le décor garde son rendu gardé (`DecorCle` : il n'est redessiné que quand la caméra, un objet ou le soleil
//! bouge) ; les cartes, qui bougent à chaque image d'une animation, n'y entrent pas.
//! - **La carte d'ombres des cartes** : les cartes qui portent une ombre sont dessinées vues du soleil (la même matrice que la
//!   carte d'ombres du décor, `decor3dgpu::eclairage`), seulement là où leur encre est (le test d'alpha sur leur dessin), dans une
//!   carte de profondeur à part, du côté que la qualité des ombres de cet ordinateur donne. Seulement quand le plan a un décor
//!   avec ses ombres et au moins une carte qui en porte une.
//! - **Sur le décor** : à la composition, chaque pixel du décor retrouve son point du monde (sa profondeur de la partie 183 et le
//!   regard de l'œil), le projette dans la vue du soleil et le compare à la carte d'ombres des cartes (le filtre du décor). Pour
//!   assombrir comme le décor assombrit ses propres faces, son rendu garde à chaque pixel **la part du soleil** qu'il y a mise
//!   (`max(n·soleil, 0)` × ses propres ombres) et le mélange de la lumière hémisphère (`decor3dgpu::SOLEIL`) : la couleur à
//!   l'ombre d'une carte est alors exactement celle que le décor aurait calculée sans ce soleil-là, brume comprise
//!   (`assombrir_decor`).
//! - **Sur une carte** : son point du monde vient de son plan (le même qu'en 183, `OmbreCarte::plan`) ; on lit la carte d'ombres du
//!   décor et celle des cartes, avec un biais qui suit le plan de la carte (`Lumiere::gradient` : une carte ne s'ombre jamais
//!   elle-même), et la carte s'assombrit de ce que le décor perd à l'ombre pour une face tournée vers le soleil, sous la lumière
//!   hémisphère d'une face verticale (`Lumiere::carte`).
//!
//! Sur la carte graphique, tout se fait à la composition (`gpu.rs`). Pour l'export, le décor rendu par la carte graphique prêtée
//! (`decor3d::calculer`) rend aussi ce qu'il faut (`decor3dgpu::SortieOmbres`), et la composition par le processeur fait le même
//! calcul (`Contexte`) ; le rendu de secours sans carte graphique n'a pas d'ombres du tout, donc pas d'ombres de cartes.
//!
//! **Ce qui ne porte ni ne reçoit** : un dossier, un calque d'effet, un calque d'annotation, un calque accroché à la caméra (il est
//! dans le cadre, pas dans le monde), la couleur unie d'un calque, les pelures d'oignon. « Toujours devant le décor 3D » (partie
//! 183) ne change que ce qui cache quoi : la carte reste au soleil du monde, elle porte et reçoit comme les autres.
//!
//! **Sans décor, ou sans ses ombres** (l'aspect « Ombres », ou une qualité « sans ombres » sur cet ordinateur) : aucun élément ne
//! porte d'ombre, la composition est celle d'avant, par les mêmes chemins, au même coût.

use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use eframe::egui::Color32;

use crate::decor3dgpu::Eclairage;
use crate::decor3drendu::{Aspect, Prise};
use crate::document::{CameraSeen, Document, Layer};
use crate::espace3d::Monde;
use crate::espace3dmath::{self as m, M4, V3};
use crate::profondeur::PlanCarte;
use crate::render::{FondPart, Plan};

pub(crate) const NOM_PORTE: &str = "Porte une ombre";
pub(crate) const PHRASE_PORTE: &str = "L'encre de la carte de ce calque cache le soleil du décor 3D de la séquence : son ombre tombe sur le sol, les objets et les autres cartes. Décoché : la carte ne fait pas d'ombre (un fond peint, un ciel, une lueur).";
pub(crate) const ACTION_PORTE: &str = "calque.porte_ombre";
pub(crate) const NOM_RECOIT: &str = "Reçoit les ombres";
pub(crate) const PHRASE_RECOIT: &str = "La carte de ce calque s'assombrit là où un objet du décor 3D, ou une autre carte, lui cache le soleil, comme une face du décor à l'ombre. Décoché : elle reste telle qu'elle est dessinée.";
pub(crate) const ACTION_RECOIT: &str = "calque.recoit_ombres";

/// Le biais des comparaisons (en profondeur de la carte d'ombres, de 0 à 1) : celui des ombres du décor sur lui-même
/// (`decor3dgpu.rs`, `shadow`).
pub const BIAIS: f32 = 0.0015;
/// L'écart entre deux lectures du filtre, en texels de la carte d'ombres (celui du décor).
pub const PAS: f32 = 1.5;
/// L'encre qui porte une ombre : au moins la moitié d'opaque (le dessin à l'opacité de son calque).
pub const SEUIL_ALPHA: f32 = 0.5;

/// Les ombres de la carte d'un élément du plan (`render::PlanItem::ombre`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OmbreCarte {
    /// Son plan dans l'image de la feuille (`profondeur::plan_de`).
    pub plan: PlanCarte,
    /// Un point de la carte et sa normale, dans le repère où l'on regarde (celui du décor).
    pub point: V3,
    pub normale: V3,
    /// Porte une ombre, reçoit les ombres (`Layer::porte_ombre`, `Layer::recoit_ombres`).
    pub porte: bool,
    pub recoit: bool,
}

/// La carte de ce calque peut-elle porter ou recevoir des ombres ? Non pour un dossier, un calque d'effet, un calque d'annotation,
/// un calque accroché à la caméra.
pub fn concerne(layer: &Layer) -> bool {
    crate::espace3d::is_card(layer) && !layer.accroche && !layer.annotation
}

/// Les ombres de la carte du calque `layer` à l'image `frame`, posée comme l'image du plan la pose (comme
/// `profondeur::plan_du_calque`, sans la case « Toujours devant le décor 3D », qui ne touche pas au soleil). Rien si elle ne porte
/// ni ne reçoit, ou est vue par la tranche.
pub fn du_calque(world: &Monde, layer: &Layer, frame: usize, camera: &CameraSeen, repere: Option<&M4>) -> Option<OmbreCarte> {
    if !concerne(layer) || !(layer.porte_ombre || layer.recoit_ombres) {
        return None;
    }
    let (mx, eye) = match repere {
        Some(r) => (m::mul(r, &world.card_xf_de(layer, &layer.pose(frame), layer.depth_libre_at(frame)).matrix()), world.eye_reference()),
        None => (world.card_xf_de(layer, &layer.pose(frame), layer.depth_at(frame)).matrix(), world.eye_seen(camera)),
    };
    let plan = crate::profondeur::plan_de(world, &mx, eye)?;
    let n = m::cross(m::dir(&mx, [1.0, 0.0, 0.0]), m::dir(&mx, [0.0, 1.0, 0.0]));
    let len = m::length(n);
    if !(len.is_finite() && len > 1e-12) {
        return None;
    }
    Some(OmbreCarte { plan, point: m::point(&mx, [0.0; 3]), normale: m::scale(n, 1.0 / len), porte: layer.porte_ombre, recoit: layer.recoit_ombres })
}

// ------------------------------------------------------------------------------------------- la lumière du décor

/// Ce que les ombres des cartes savent du soleil d'un rendu du décor : la carte d'ombres (sa matrice, son côté, son filtre), l'œil
/// de l'image (`Prise`), les couleurs de la lumière et la brume. Les mêmes nombres sur la carte graphique (`octets`) et sur le
/// processeur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lumiere {
    pub light_vp: M4,
    pub inv_light: M4,
    pub prise: Prise,
    pub taps: u32,
    pub cote: u32,
    pub ciel: [f32; 3],
    pub sol: [f32; 3],
    pub soleil: [f32; 3],
    pub horizon: [f32; 3],
    /// Début, fin et force de la brume (force 0 : sans brume).
    pub brume: [f32; 3],
    /// Ce qui reste d'une carte à l'ombre (linéaire, par canal) : la lumière hémisphère d'une face verticale, sur la même lumière
    /// avec le soleil d'une face tournée vers lui.
    pub carte: [f32; 3],
}

impl Lumiere {
    /// La lumière d'un rendu du décor d'éclairage `ec`, d'aspect `aspect`, vu de `prise` ; rien sans ses ombres.
    pub fn de(ec: &Eclairage, aspect: &Aspect, prise: &Prise) -> Option<Lumiere> {
        if !ec.ombres || ec.taps == 0 {
            return None;
        }
        let (ciel, sol) = aspect.ambiances();
        let soleil = aspect.couleur_soleil();
        let carte = [0, 1, 2].map(|i| {
            let hemi = 0.5 * (ciel[i] + sol[i]);
            hemi / (hemi + soleil[i]).max(1e-4)
        });
        Some(Lumiere {
            light_vp: ec.light_vp,
            inv_light: m::inverse_affine(&ec.light_vp),
            prise: *prise,
            taps: ec.taps,
            cote: ec.cote.max(1),
            ciel,
            sol,
            soleil,
            horizon: aspect.horizon(),
            brume: [ec.brume.0, ec.brume.1, if aspect.brume { crate::decor3drendu::BRUME_MAX } else { 0.0 }],
            carte,
        })
    }

    /// Le rayon du filtre des ombres des cartes sur le décor : celui du décor sur lui-même (3 × 3 lectures, 5 × 5 en hautes).
    pub fn rayon_decor(&self) -> i32 {
        if self.taps > 4 {
            2
        } else {
            1
        }
    }

    /// Le rayon du filtre des ombres reçues par une carte : une lecture filtrée (quatre texels) dans chacune des deux cartes
    /// d'ombres, à toutes les qualités. (Mesuré, partie 185.3 : 3 × 3 lectures coûtent 1,3 ms de plus pour huit personnages sur
    /// une feuille 4K, 5 ms pour huit cartes pleines ; une seule, rien de mesurable et 2,5 ms.)
    pub fn rayon_cartes(&self) -> i32 {
        0
    }

    /// Le point du monde vu au pixel (`x`, `y`) de la feuille, à l'inverse de distance (le long de l'axe de l'œil) `inv`.
    #[inline]
    pub fn monde(&self, x: f32, y: f32, inv: f32) -> V3 {
        let p = &self.prise;
        let z = 1.0 / inv;
        let (dx, dy) = ((x - p.centre[0]) / p.px_par_unite, -(y - p.centre[1]) / p.px_par_unite);
        [0, 1, 2].map(|k| p.oeil[k] + z * (p.avant[k] + p.droite[k] * dx + p.haut[k] * dy))
    }

    /// Le point `p` du monde dans la carte d'ombres : (u, v) de 0 à 1, et sa profondeur de 0 à 1.
    #[inline]
    pub fn vers_lumiere(&self, p: V3) -> V3 {
        let q = m::point(&self.light_vp, p);
        [q[0] * 0.5 + 0.5, 0.5 - q[1] * 0.5, q[2]]
    }

    /// La pente de la profondeur de la carte d'ombres le long du plan de la carte `o` (par unité de u et de v) : une lecture du
    /// filtre décalée de (du, dv) compare la carte à la profondeur que son propre plan a là, pas à celle du centre. Une carte n'est
    /// donc jamais dans sa propre ombre, quelle que soit sa pente sous le soleil (sans biais qui décolle les ombres des pieds).
    pub fn gradient(&self, o: &OmbreCarte) -> [f32; 2] {
        self.gradient_de(o.normale)
    }

    /// Comme `gradient`, pour une carte de normale `n` (unitaire, dans le repère du décor) : la vue Espace 3D, qui a ses cartes dans
    /// le monde (partie 189).
    pub fn gradient_de(&self, n: V3) -> [f32; 2] {
        let t1 = m::normalize(m::cross(n, if n[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] }));
        let t2 = m::cross(n, t1);
        let (d1, d2) = (m::dir(&self.light_vp, t1), m::dir(&self.light_vp, t2));
        let (u1, v1, z1) = (d1[0] * 0.5, -d1[1] * 0.5, d1[2]);
        let (u2, v2, z2) = (d2[0] * 0.5, -d2[1] * 0.5, d2[2]);
        let det = u1 * v2 - u2 * v1;
        if !(det.abs() > 1e-12) {
            return [0.0; 2];
        }
        let g = [(z1 * v2 - z2 * v1) / det, (u1 * z2 - u2 * z1) / det];
        if g.iter().all(|v| v.is_finite()) {
            g
        } else {
            [0.0; 2]
        }
    }

    /// La part du soleil que voit le point de carte d'ombres `l` (de 0 à 1) : `carte` (`cote`²), le filtre de rayon `rayon` (`PAS`
    /// texels entre deux lectures, chacune bilinéaire sur quatre texels, comme `textureSampleCompareLevel`), la profondeur de
    /// chaque lecture suivant `g` (`gradient`), moins `BIAIS`. Hors de la carte d'ombres : au soleil.
    pub fn pcf(&self, carte: &[f32], l: V3, g: [f32; 2], rayon: i32) -> f32 {
        if !(l[0] >= 0.0 && l[0] <= 1.0 && l[1] >= 0.0 && l[1] <= 1.0 && l[2] <= 1.0) {
            return 1.0;
        }
        let cote = self.cote as usize;
        if carte.len() < cote * cote {
            return 1.0;
        }
        let texel = 1.0 / self.cote as f32;
        let (mut somme, mut n) = (0.0, 0.0);
        for y in -rayon..=rayon {
            for x in -rayon..=rayon {
                let o = [x as f32 * texel * PAS, y as f32 * texel * PAS];
                let r = l[2] + g[0] * o[0] + g[1] * o[1] - BIAIS;
                somme += comparer(carte, cote, l[0] + o[0], l[1] + o[1], r);
                n += 1.0;
            }
        }
        somme / n
    }

    /// La couleur `c` (prémultipliée, valeurs gamma) d'un pixel du décor dont le rendu a gardé la part du soleil `s[0]` et le
    /// mélange de l'hémisphère `s[1]` (0 à 255), à `d` mètres de l'œil, dont `occ` (0 à 1) du soleil est caché par une carte : le
    /// soleil retiré d'autant, la brume gardée (`decor3dgpu.rs`, `eclairer` : c = base × (hémisphère + soleil × part), puis la
    /// brume).
    pub fn assombrir_decor(&self, c: Color32, s: [u8; 2], d: f32, occ: f32) -> Color32 {
        let a = c.a();
        if a == 0 || occ <= 0.0 || s[0] == 0 {
            return c;
        }
        let (part, t) = (s[0] as f32 / 255.0, s[1] as f32 / 255.0);
        let mut f = 0.0;
        if self.brume[2] > 0.0 {
            let v = ((d - self.brume[0]) / (self.brume[1] - self.brume[0]).max(1e-4)).clamp(0.0, 1.0) * self.brume[2];
            f = v * v * (3.0 - 2.0 * v);
        }
        let k = a as f32 / 255.0;
        let rgb = [c.r(), c.g(), c.b()].map(|v| (v as f32 / 255.0 / k).min(1.0));
        let out: [f32; 3] = std::array::from_fn(|i| {
            let lin = vers_lineaire(rgb[i]);
            let hemi = self.sol[i] + (self.ciel[i] - self.sol[i]) * t;
            let kk = self.soleil[i] * part;
            let ratio = 1.0 - occ * kk / (hemi + kk).max(1e-4);
            let brume = self.horizon[i] * f;
            ((lin - brume) * ratio + brume).max(0.0)
        });
        let o = out.map(|v| (crate::decor3drendu::vers_srgb(v) * k * 255.0).round().clamp(0.0, 255.0) as u8);
        Color32::from_rgba_premultiplied(o[0], o[1], o[2], a)
    }

    /// La couleur `c` (prémultipliée, valeurs gamma) d'un pixel de carte dont `occ` (0 à 1) du soleil est caché.
    pub fn assombrir_carte(&self, c: Color32, occ: f32) -> Color32 {
        let a = c.a();
        if a == 0 || occ <= 0.0 {
            return c;
        }
        let k = a as f32 / 255.0;
        let o: [u8; 3] = std::array::from_fn(|i| {
            let v = ([c.r(), c.g(), c.b()][i] as f32 / 255.0 / k).min(1.0);
            let lin = vers_lineaire(v) * (1.0 - occ * (1.0 - self.carte[i]));
            (crate::decor3drendu::vers_srgb(lin) * k * 255.0).round().clamp(0.0, 255.0) as u8
        });
        Color32::from_rgba_premultiplied(o[0], o[1], o[2], a)
    }

    /// Les réglages de la composition par la carte graphique (`gpu.rs`, `Lumiere` du shader, `OCTETS`) : `taille`, celle de
    /// l'image composée (pixels) ; `boite`, la boîte (u0, v0, u1, v1) des cartes dans leur carte d'ombres (rien : pas de carte
    /// d'ombres des cartes) ; `decor`, la carte d'ombres du décor est là.
    pub fn octets(&self, taille: [f32; 2], boite: Option<[f32; 4]>, decor: bool) -> Vec<u8> {
        let mut b = vec![0u8; OCTETS as usize];
        let put = |b: &mut [u8], at: usize, v: &[f32]| {
            for (k, x) in v.iter().enumerate() {
                b[at + k * 4..at + k * 4 + 4].copy_from_slice(&x.to_ne_bytes());
            }
        };
        let p = &self.prise;
        put(&mut b, 0, &self.light_vp);
        put(&mut b, 64, &[p.oeil[0], p.oeil[1], p.oeil[2], p.px_par_unite]);
        put(&mut b, 80, &[p.droite[0], p.droite[1], p.droite[2], p.centre[0]]);
        put(&mut b, 96, &[p.haut[0], p.haut[1], p.haut[2], p.centre[1]]);
        put(&mut b, 112, &[p.avant[0], p.avant[1], p.avant[2], self.rayon_decor() as f32]);
        put(&mut b, 128, &[self.ciel[0], self.ciel[1], self.ciel[2], 1.0 / self.cote as f32]);
        put(&mut b, 144, &[self.sol[0], self.sol[1], self.sol[2], self.brume[2]]);
        put(&mut b, 160, &[self.soleil[0], self.soleil[1], self.soleil[2], if decor { 1.0 } else { 0.0 }]);
        put(&mut b, 176, &[self.horizon[0], self.horizon[1], self.horizon[2], self.rayon_cartes() as f32]);
        put(&mut b, 192, &[self.brume[0], self.brume[1], taille[0], taille[1]]);
        put(&mut b, 208, &[self.carte[0], self.carte[1], self.carte[2], if boite.is_some() { 1.0 } else { 0.0 }]);
        put(&mut b, 224, &boite.unwrap_or([0.0; 4]));
        b
    }
}

/// Taille des réglages de la lumière sur la carte graphique (`Lumiere::octets`).
pub const OCTETS: u64 = 256;

/// sRGB (0 à 1) vers linéaire.
#[inline]
pub fn vers_lineaire(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Une lecture comparée bilinéaire, comme la carte graphique (`textureSampleCompareLevel`, `LessEqual`, bords répétés) : la part
/// des quatre texels autour de (u, v) dont la profondeur est au moins `r`.
fn comparer(carte: &[f32], cote: usize, u: f32, v: f32, r: f32) -> f32 {
    let (x, y) = (u * cote as f32 - 0.5, v * cote as f32 - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let at = |i: i64, j: i64| -> f32 {
        let (i, j) = (i.clamp(0, cote as i64 - 1) as usize, j.clamp(0, cote as i64 - 1) as usize);
        if r <= carte[j * cote + i] {
            1.0
        } else {
            0.0
        }
    };
    let (i, j) = (x0 as i64, y0 as i64);
    let haut = at(i, j) * (1.0 - fx) + at(i + 1, j) * fx;
    let bas = at(i, j + 1) * (1.0 - fx) + at(i + 1, j + 1) * fx;
    haut * (1.0 - fy) + bas * fy
}

/// La boîte (u0, v0, u1, v1) de la carte d'ombres où tombent ces points du monde, agrandie de la portée du filtre ; rien si l'un
/// d'eux n'est pas fini.
pub fn boite_de(lum: &Lumiere, points: impl IntoIterator<Item = V3>) -> Option<[f32; 4]> {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for p in points {
        let l = lum.vers_lumiere(p);
        if !(l[0].is_finite() && l[1].is_finite()) {
            return None;
        }
        b = [b[0].min(l[0]), b[1].min(l[1]), b[2].max(l[0]), b[3].max(l[1])];
    }
    let marge = (lum.rayon_decor() as f32 * PAS + 2.0) / lum.cote as f32;
    (b[0] <= b[2]).then_some([b[0] - marge, b[1] - marge, b[2] + marge, b[3] + marge])
}

#[inline]
fn dans(b: &[f32; 4], l: V3) -> bool {
    l[0] >= b[0] && l[0] <= b[2] && l[1] >= b[1] && l[1] <= b[3]
}

// ------------------------------------------------------------------------------------------- la composition par le processeur

/// Ce que l'export garde du soleil d'un rendu du décor fait par la carte graphique (`decor3d::calculer`) : sa lumière, la part du
/// soleil à chaque pixel (deux octets, `decor3dgpu::SOLEIL`) et sa carte d'ombres.
pub struct Soleil {
    pub lum: Lumiere,
    pub soleil: Vec<u8>,
    pub carte: Vec<f32>,
}

/// Ce qu'il faut à la composition par le processeur d'un plan pour les ombres de ses cartes : le décor (sa profondeur et son
/// soleil), la carte d'ombres des cartes (si l'une en porte), et, pour chaque élément qui reçoit les ombres, son plan et sa pente.
pub struct Contexte {
    pub lum: Lumiere,
    soleil: Arc<Soleil>,
    prof: crate::profondeur::Profondeurs,
    w: usize,
    h: usize,
    cartes: Option<(Vec<f32>, [f32; 4])>,
    recepteurs: Vec<Option<(PlanCarte, [f32; 2])>>,
}

impl Contexte {
    /// Le pixel (`x`, `y`) du décor, de couleur `c` : assombri là où une carte lui cache le soleil.
    #[inline]
    pub fn sur_le_decor(&self, x: i64, y: i64, c: Color32) -> Color32 {
        let Some((cartes, boite)) = &self.cartes else { return c };
        if c.a() == 0 || x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return c;
        }
        let i = y as usize * self.w + x as usize;
        let inv = self.prof[i];
        let s = [self.soleil.soleil[2 * i], self.soleil.soleil[2 * i + 1]];
        if inv <= 0.0 || s[0] == 0 {
            return c;
        }
        let p = self.lum.monde(x as f32 + 0.5, y as f32 + 0.5, inv);
        let l = self.lum.vers_lumiere(p);
        if !dans(boite, l) {
            return c;
        }
        let occ = 1.0 - self.lum.pcf(cartes, l, [0.0; 2], self.lum.rayon_decor());
        self.lum.assombrir_decor(c, s, m::length(m::sub(p, self.lum.prise.oeil)), occ)
    }

    /// Le pixel (`x`, `y`) du monde de l'élément `idx` du plan, de couleur `c` : assombri là où le décor ou une carte cache le
    /// soleil à sa carte, s'il reçoit les ombres.
    #[inline]
    pub fn sur_la_carte(&self, idx: usize, x: i64, y: i64, c: Color32) -> Color32 {
        let Some(Some((plan, g))) = self.recepteurs.get(idx) else { return c };
        if c.a() == 0 {
            return c;
        }
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        let inv = crate::profondeur::inverse_distance(plan, fx, fy);
        if !(inv > 0.0) {
            return c;
        }
        let l = self.lum.vers_lumiere(self.lum.monde(fx, fy, inv));
        let r = self.lum.rayon_cartes();
        let mut vis = self.lum.pcf(&self.soleil.carte, l, *g, r);
        if let Some((cartes, boite)) = &self.cartes {
            if vis > 0.0 && dans(boite, l) {
                vis *= self.lum.pcf(cartes, l, *g, r);
            }
        }
        self.lum.assombrir_carte(c, 1.0 - vis)
    }

    /// L'élément `idx` reçoit-il les ombres ?
    #[inline]
    pub fn recoit(&self, idx: usize) -> bool {
        self.recepteurs.get(idx).is_some_and(Option::is_some)
    }
}

type Entree = Arc<OnceLock<Option<Arc<Contexte>>>>;

/// Les contextes faits récemment, par empreinte du plan (les tuiles d'une même image, sur plusieurs fils, n'en font qu'un).
static GARDES: Mutex<Vec<(u64, Entree)>> = Mutex::new(Vec::new());
const GARDES_MAX: usize = 6;

/// Le contexte des ombres des cartes du plan `plan` pour la composition par le processeur ; rien : pas de décor avec son soleil
/// rendu par la carte graphique, ou aucune carte qui porte ou reçoit (la composition d'avant, sans rien lire de plus).
pub fn contexte(doc: &Document, plan: &Plan) -> Option<Arc<Contexte>> {
    let di = plan.items.iter().position(|i| matches!(i.fond, Some(FondPart::Decor(_))))?;
    if !plan.items[di + 1..].iter().any(|i| i.ombre.is_some_and(|o| o.porte || o.recoit) && i.tint.is_none()) {
        return None;
    }
    let decor = crate::fx::shown(doc, &plan.items[di])?;
    let soleil = crate::decor3d::soleil_du_rendu(decor.version)?;
    let (w, h) = (doc.width, doc.height);
    let prof = crate::profondeur::du_decor(&decor, w, h)?;
    if soleil.soleil.len() != w * h * 2 {
        return None;
    }
    // L'empreinte : le rendu du décor, et pour chaque carte ce qui fait son ombre (son dessin montré, sa place, son plan).
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    decor.version.hash(&mut hs);
    let pivot = doc.stage_center();
    for (i, item) in plan.items.iter().enumerate().skip(di + 1) {
        let Some(o) = item.ombre else { continue };
        (i, item.cel, item.opacity, o.porte, o.recoit, item.tint.is_some()).hash(&mut hs);
        o.plan.map(f32::to_bits).hash(&mut hs);
        o.normale.map(f32::to_bits).hash(&mut hs);
        if o.porte {
            crate::fx::shown(doc, item).map(|c| c.version).hash(&mut hs);
            item.affine(pivot).map(|a| [a.a, a.b, a.c, a.d, a.tx, a.ty].map(f32::to_bits)).hash(&mut hs);
            item.pre.as_ref().and_then(|p| p.warp.as_ref()).map(|w| Arc::as_ptr(w) as usize).hash(&mut hs);
            item.fx.len().hash(&mut hs);
        }
    }
    let cle = hs.finish();
    let entree = {
        let mut g = GARDES.lock().unwrap_or_else(|e| e.into_inner());
        match g.iter().find(|(k, _)| *k == cle) {
            Some((_, e)) => e.clone(),
            None => {
                let e: Entree = Arc::new(OnceLock::new());
                g.push((cle, e.clone()));
                if g.len() > GARDES_MAX {
                    g.remove(0);
                }
                e
            }
        }
    };
    entree.get_or_init(|| faire(doc, plan, di, soleil, prof)).clone()
}

/// Oublie les contextes gardés (un autre projet).
pub fn oublier() {
    GARDES.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

fn faire(doc: &Document, plan: &Plan, di: usize, soleil: Arc<Soleil>, prof: crate::profondeur::Profondeurs) -> Option<Arc<Contexte>> {
    let lum = soleil.lum;
    let (w, h) = (doc.width, doc.height);
    let recepteurs: Vec<Option<(PlanCarte, [f32; 2])>> =
        plan.items.iter().enumerate().map(|(i, item)| item.ombre.filter(|o| i > di && o.recoit && item.tint.is_none()).map(|o| (o.plan, lum.gradient(&o)))).collect();
    let portees: Vec<&crate::render::PlanItem> = plan.items.iter().skip(di + 1).filter(|i| i.ombre.is_some_and(|o| o.porte) && i.tint.is_none()).collect();
    let cartes = if portees.is_empty() { None } else { carte_des_cartes(doc, &lum, &portees) };
    if cartes.is_none() && recepteurs.iter().all(Option::is_none) {
        return None;
    }
    Some(Arc::new(Contexte { lum, soleil, prof, w, h, cartes, recepteurs }))
}

/// La carte d'ombres des cartes `portees`, vue du soleil `lum`, par le processeur : pour chaque texel, le rayon du soleil coupe le
/// plan de la carte ; ce point, vu de l'œil, tombe sur un pixel de la feuille ; s'il y a là de l'encre de la carte (elle composée
/// seule, à l'opacité de son calque), la profondeur la plus proche du soleil est gardée. Et la boîte des cartes dans la carte.
fn carte_des_cartes(doc: &Document, lum: &Lumiere, portees: &[&crate::render::PlanItem]) -> Option<(Vec<f32>, [f32; 4])> {
    let cote = lum.cote as usize;
    let mut carte = vec![1.0f32; cote * cote];
    let mut boite: Option<[f32; 4]> = None;
    let feuille = crate::render::Area::sheet(doc);
    // (Ce qui dépasse beaucoup de la feuille n'est pas lu : une moitié de feuille de chaque côté.)
    let (mx, my) = (doc.width as i64 / 2, doc.height as i64 / 2);
    let large = crate::render::Area { x0: feuille.x0 - mx, y0: feuille.y0 - my, x1: feuille.x1 + mx, y1: feuille.y1 + my };
    let p = &lum.prise;
    for item in portees {
        let Some(o) = item.ombre else { continue };
        let seul = Plan { paper: None, items: vec![crate::render::PlanItem { profondeur: None, ombre: None, ..(*item).clone() }] };
        let Some(encre) = crate::render::plan_ink_bounds(doc, &seul) else { continue };
        let zone = crate::render::Area { x0: encre.x0.max(large.x0), y0: encre.y0.max(large.y0), x1: encre.x1.min(large.x1), y1: encre.y1.min(large.y1) };
        if zone.x1 <= zone.x0 || zone.y1 <= zone.y0 {
            continue;
        }
        let alpha: Vec<u8> = crate::render::composite_area(doc, &seul, zone).iter().map(|c| c.a()).collect();
        let zw = zone.width();
        // La boîte de la carte dans la carte d'ombres : ses coins posés sur son plan (toute la carte si l'un ne l'est pas).
        let coins = [(zone.x0, zone.y0), (zone.x1, zone.y0), (zone.x1, zone.y1), (zone.x0, zone.y1)].map(|(x, y)| {
            let inv = crate::profondeur::inverse_distance(&o.plan, x as f32, y as f32);
            (inv > 0.0).then(|| lum.monde(x as f32, y as f32, inv))
        });
        let b = if coins.iter().all(Option::is_some) { boite_de(lum, coins.iter().flatten().copied()).unwrap_or([0.0, 0.0, 1.0, 1.0]) } else { [0.0, 0.0, 1.0, 1.0] };
        let b = [b[0].max(0.0), b[1].max(0.0), b[2].min(1.0), b[3].min(1.0)];
        if b[0] > b[2] || b[1] > b[3] {
            continue;
        }
        boite = Some(boite.map_or(b, |a| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]));
        let (i0, i1) = (((b[0] * cote as f32) as usize).min(cote - 1), ((b[2] * cote as f32).ceil() as usize).min(cote));
        let (j0, j1) = (((b[1] * cote as f32) as usize).min(cote - 1), ((b[3] * cote as f32).ceil() as usize).min(cote));
        let (n, pt) = (o.normale, o.point);
        let seuil = (SEUIL_ALPHA * 255.0).round() as u8;
        crate::render::par_chunks(&mut carte, cote, |j, ligne| {
            if j < j0 || j >= j1 {
                return;
            }
            let qy = 1.0 - 2.0 * (j as f32 + 0.5) / cote as f32;
            for i in i0..i1 {
                let qx = 2.0 * (i as f32 + 0.5) / cote as f32 - 1.0;
                let a = m::point(&lum.inv_light, [qx, qy, 0.0]);
                let d = m::sub(m::point(&lum.inv_light, [qx, qy, 1.0]), a);
                let den = m::dot(n, d);
                if den.abs() < 1e-12 {
                    continue;
                }
                let t = m::dot(n, m::sub(pt, a)) / den;
                if !(t <= 1.0) {
                    continue;
                }
                let t = t.max(0.0);
                if t >= ligne[i] {
                    continue;
                }
                let q = m::add(a, m::scale(d, t));
                let rel = m::sub(q, p.oeil);
                let z = m::dot(rel, p.avant);
                if !(z > 1e-6) {
                    continue;
                }
                let x = p.centre[0] + p.px_par_unite * m::dot(rel, p.droite) / z;
                let y = p.centre[1] - p.px_par_unite * m::dot(rel, p.haut) / z;
                let (ax, ay) = (x.floor() as i64 - zone.x0, y.floor() as i64 - zone.y0);
                if ax < 0 || ay < 0 || ax >= zw as i64 || ay >= zone.height() as i64 {
                    continue;
                }
                if alpha[ay as usize * zw + ax as usize] >= seuil {
                    ligne[i] = t;
                }
            }
        });
    }
    let boite = boite?;
    let marge = (lum.rayon_decor() as f32 * PAS + 2.0) / cote as f32;
    Some((carte, [boite[0] - marge, boite[1] - marge, boite[2] + marge, boite[3] + marge]))
}

// ------------------------------------------------------------------------------------------- les cases du calque

impl crate::app::App {
    /// Coche ou décoche « Porte une ombre » sur le calque `li` (une étape d'annulation).
    pub(crate) fn basculer_porte_ombre(&mut self, li: usize) {
        self.basculer_ombre(li, true);
    }

    /// Coche ou décoche « Reçoit les ombres » sur le calque `li` (une étape d'annulation).
    pub(crate) fn basculer_recoit_ombres(&mut self, li: usize) {
        self.basculer_ombre(li, false);
    }

    fn basculer_ombre(&mut self, li: usize, porte: bool) {
        let Some(l) = self.layers().get(li) else { return };
        if !crate::espace3d::is_card(l) || l.annotation {
            self.status = "Un dossier, un calque d'effet ou d'annotation n'a pas de carte dans le décor : il n'a pas d'ombre.".into();
            return;
        }
        let on = if porte { !l.porte_ombre } else { !l.recoit_ombres };
        self.end_stroke();
        let snap = self.snapshot();
        if porte {
            self.layers_mut()[li].porte_ombre = on;
        } else {
            self.layers_mut()[li].recoit_ombres = on;
        }
        let nom = match (porte, on) {
            (true, true) => "Porte une ombre",
            (true, false) => "Ne porte pas d'ombre",
            (false, true) => "Reçoit les ombres",
            (false, false) => "Ne reçoit pas les ombres",
        };
        self.commit_named(snap, nom);
        self.renderer.invalidate_all();
        let accroche = self.layers()[li].accroche;
        self.status = match (porte, on) {
            _ if accroche => "Un calque accroché à la caméra est dans le cadre, pas dans le décor : il n'a pas d'ombre (la case vaudra s'il est décroché).".into(),
            (true, true) => "Porte une ombre : l'encre de la carte cache le soleil du décor 3D.".into(),
            (true, false) => "La carte ne fait plus d'ombre.".into(),
            (false, true) => "Reçoit les ombres : la carte s'assombrit là où on lui cache le soleil.".into(),
            (false, false) => "La carte ne reçoit plus les ombres : elle reste telle qu'elle est dessinée.".into(),
        };
    }
}
