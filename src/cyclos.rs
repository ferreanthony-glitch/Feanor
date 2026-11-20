//! Partie 202 (`docs/INTERFACE.md` partie 202 ; Anthony : « est-ce que les cartes ne prennent pas la lumière parce qu'il n'y a pas de
//! cyclo autour de l'environnement ? On devrait pouvoir en créer avec des choix de cyclo de jour, nuageux, nuit, montagne, ville
//! etc. ») : **les cyclos**, l'environnement d'une séquence autour de son décor 3D.
//!
//! - **Un cyclo** (`Cyclo`) : un ciel à l'infini (son dégradé, ses nuages, ses silhouettes à l'horizon), le sol au loin, et la lumière
//!   qu'il donne (le soleil et ses ombres de la partie 185, l'ambiance du ciel et du sol, la lumière des cartes). Un par séquence
//!   (`Decor3d::cyclo`, dans `decors3d.json`), même sans objet 3D : les cartes dessinées ont leur ciel et sa lumière.
//! - **Neuf choix** (`Preset`), tout dessiné par le code (aucune photo, rien de téléchargé) : Jour clair, Nuageux, Coucher de soleil,
//!   Nuit (étoiles, lune), Brouillard, Montagne (crêtes en couches, perspective aérienne), Ville (silhouettes d'immeubles, fenêtres
//!   allumées la nuit), Mer (horizon plat), Studio (le cyclo des photographes : un gris neutre, le sol et le fond d'un seul tenant).
//!   Puis : l'heure, les nuages, la couleur dominante, la graine, la rotation, la brume. Lisible pour un storyboard, pas
//!   photoréaliste.
//! - **Son image** (`Equirect`) : une fois, de ses réglages, hors du fil de l'interface, en équirectangulaire (2048 × 1024), gardée par
//!   l'empreinte de ses réglages (`cle`) ; la rotation ne la refait pas (elle tourne à la lecture). Rien ne se refait au repos ni quand
//!   la caméra bouge : le ciel ne dépend que de la direction du regard.
//! - **Sa lumière** (`CycloVu`, dans `Aspect::cyclo`) : le décor la prend (le ciel, la brume, l'ambiance, le soleil), et les cartes
//!   qui ont « Prend la lumière du cyclo » : `c × ambiante` en lumière linéaire, l'ambiante réglée pour que Jour clair laisse les cartes
//!   à leurs couleurs dessinées (1), que la Nuit les assombrisse et les bleuisse, que le Coucher de soleil les réchauffe.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::espace3dmath::{self as m, V3};

/// La taille de l'image d'un cyclo (équirectangulaire : 360° sur 180°).
pub const LARGEUR: usize = 2048;
pub const HAUTEUR: usize = 1024;
/// Celle des vignettes de la galerie.
pub const VIGNETTE: [usize; 2] = [192, 96];

/// Les choix de cyclo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Preset {
    #[default]
    #[serde(rename = "jour")]
    Jour,
    #[serde(rename = "nuageux")]
    Nuageux,
    #[serde(rename = "coucher")]
    Coucher,
    #[serde(rename = "nuit")]
    Nuit,
    #[serde(rename = "brouillard")]
    Brouillard,
    #[serde(rename = "montagne")]
    Montagne,
    #[serde(rename = "ville")]
    Ville,
    #[serde(rename = "mer")]
    Mer,
    #[serde(rename = "studio")]
    Studio,
}

impl Preset {
    pub const TOUS: [Preset; 9] = [Preset::Jour, Preset::Nuageux, Preset::Coucher, Preset::Nuit, Preset::Brouillard, Preset::Montagne, Preset::Ville, Preset::Mer, Preset::Studio];

    pub fn nom(self) -> &'static str {
        match self {
            Preset::Jour => "Jour clair",
            Preset::Nuageux => "Nuageux",
            Preset::Coucher => "Coucher de soleil",
            Preset::Nuit => "Nuit",
            Preset::Brouillard => "Brouillard",
            Preset::Montagne => "Montagne",
            Preset::Ville => "Ville",
            Preset::Mer => "Mer",
            Preset::Studio => "Studio",
        }
    }

    /// Le cyclo de ce choix, à ses réglages d'origine.
    pub fn cyclo(self) -> Cyclo {
        let (heure, nuages, brume) = match self {
            Preset::Jour => (13.0, 0.15, 0.25),
            Preset::Nuageux => (13.0, 0.85, 0.45),
            Preset::Coucher => (19.2, 0.35, 0.35),
            Preset::Nuit => (23.5, 0.1, 0.2),
            Preset::Brouillard => (11.0, 0.3, 0.95),
            Preset::Montagne => (10.0, 0.25, 0.35),
            Preset::Ville => (15.0, 0.2, 0.35),
            Preset::Mer => (12.0, 0.2, 0.3),
            Preset::Studio => (13.0, 0.0, 0.0),
        };
        Cyclo { preset: self, heure, nuages, brume, couleur: None, graine: 1, rotation: 0.0 }
    }
}

/// Le cyclo d'une séquence et ses réglages.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cyclo {
    pub preset: Preset,
    /// L'heure (0 à 24) : la hauteur et la couleur du soleil.
    #[serde(default = "midi")]
    pub heure: f32,
    /// La couverture des nuages (0 à 1).
    #[serde(default)]
    pub nuages: f32,
    /// La brume (0 à 1) : vers l'horizon, sur le décor aussi.
    #[serde(default)]
    pub brume: f32,
    /// Une couleur dominante (sRGB) qui teinte le ciel ; rien : celles du choix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub couleur: Option<[u8; 3]>,
    /// La graine : d'autres nuages, d'autres crêtes, une autre ville.
    #[serde(default = "un")]
    pub graine: u32,
    /// La rotation autour de la verticale (degrés) : à la lecture, l'image n'est pas refaite.
    #[serde(default)]
    pub rotation: f32,
}

fn midi() -> f32 {
    13.0
}
fn un() -> u32 {
    1
}

impl Cyclo {
    /// Ses réglages dans leurs bornes (un fichier abîmé ne fait rien d'étrange).
    pub fn borne(mut self) -> Cyclo {
        let fin = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.heure = fin(self.heure, 0.0, 24.0, 13.0);
        self.nuages = fin(self.nuages, 0.0, 1.0, 0.0);
        self.brume = fin(self.brume, 0.0, 1.0, 0.0);
        self.rotation = fin(self.rotation, -360.0, 360.0, 0.0);
        self
    }

    /// L'empreinte de ce qui fait son image (pas la rotation : elle tourne à la lecture).
    pub fn cle(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        "cyclo".hash(&mut h);
        self.preset.hash(&mut h);
        self.heure.to_bits().hash(&mut h);
        self.nuages.to_bits().hash(&mut h);
        self.brume.to_bits().hash(&mut h);
        self.couleur.hash(&mut h);
        self.graine.hash(&mut h);
        h.finish()
    }

    /// La hauteur (degrés) du soleil (de la lune, la nuit) à son heure : au plus haut vers 13 h ; sous l'horizon de 20 h à 6 h, la lune
    /// prend sa place, basse.
    pub fn hauteur_soleil(&self) -> f32 {
        let t = (self.heure - 7.0) / 12.0 * std::f32::consts::PI;
        let h = t.sin() * 62.0;
        if h < 4.0 {
            // (La nuit : la lune, à mi-hauteur.)
            if self.nuit() > 0.5 {
                35.0
            } else {
                4.0
            }
        } else {
            h
        }
    }

    /// L'azimut (degrés, la convention de `Aspect::soleil`) du soleil : le matin d'un côté, le soir de l'autre ; plus la rotation.
    pub fn azimut_soleil(&self) -> f32 {
        -35.0 + (self.heure - 13.0) * 12.0 + self.rotation
    }

    /// La part de nuit (0 le jour, 1 la nuit), d'après l'heure ; la Nuit l'est toujours.
    pub fn nuit(&self) -> f32 {
        if self.preset == Preset::Nuit {
            return 1.0;
        }
        let d = (self.heure - 13.0).abs();
        smooth(6.5, 8.0, d)
    }

    /// La part de soleil bas (le coucher, l'aube) : 0 au milieu du jour.
    pub fn chaleur(&self) -> f32 {
        if self.preset == Preset::Coucher {
            return 1.0;
        }
        let d = (self.heure - 13.0).abs();
        smooth(4.0, 6.0, d) * (1.0 - smooth(7.0, 8.5, d))
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn lin(c: [u8; 3]) -> [f32; 3] {
    crate::couleurs3d::lineaire(c)
}

/// Les couleurs et la lumière d'un cyclo (linéaires), prêtes pour le rendu et pour les cartes. `Copy` : elle va dans `Aspect`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CycloVu {
    /// La séquence a un cyclo.
    pub actif: bool,
    /// L'empreinte de son image (`Cyclo::cle`) ; son image est prête (`pret`).
    pub cle: u64,
    pub pret: bool,
    /// La rotation (radians), à la lecture de l'image.
    pub rotation: f32,
    pub zenith: [f32; 3],
    pub horizon: [f32; 3],
    pub sol: [f32; 3],
    /// L'ambiance du ciel et du sol (la lumière hémisphère du décor).
    pub ciel_amb: [f32; 3],
    pub sol_amb: [f32; 3],
    /// La couleur fois la force du soleil (de la lune).
    pub soleil: [f32; 3],
    /// Vers le soleil (la lune, la nuit ; longueur 1) : il suit l'heure et la rotation du cyclo (`Aspect::soleil`).
    pub soleil_dir: [f32; 3],
    /// La force de la brume (0 à 1).
    pub brume: f32,
    /// La lumière d'une carte qui prend celle du cyclo : `ambiante`, plus `ambiante_soleil` fois sa face au soleil (`max(n·soleil, 0)`),
    /// en lumière linéaire. Jour clair : 1 et 0.
    pub ambiante: [f32; 3],
    pub ambiante_soleil: [f32; 3],
}

/// Les couleurs d'un choix au milieu du jour : zénith, horizon, sol (sRGB), la lumière des cartes.
fn palette(p: Preset) -> ([u8; 3], [u8; 3], [u8; 3], [f32; 3]) {
    match p {
        Preset::Jour => ([0x5c, 0x8f, 0xd8], [0xe2, 0xd8, 0xd2], [0x8c, 0x86, 0x78], [1.0, 1.0, 1.0]),
        Preset::Nuageux => ([0x8d, 0x97, 0xa6], [0xc9, 0xcc, 0xcf], [0x7a, 0x7a, 0x74], [0.86, 0.88, 0.91]),
        Preset::Coucher => ([0x3d, 0x4f, 0x8f], [0xf2, 0x9e, 0x5c], [0x5a, 0x48, 0x44], [1.0, 0.8, 0.6]),
        Preset::Nuit => ([0x0c, 0x13, 0x2b], [0x2a, 0x34, 0x5a], [0x18, 0x1a, 0x24], [0.11, 0.14, 0.32]),
        Preset::Brouillard => ([0xb4, 0xb8, 0xbc], [0xd4, 0xd5, 0xd4], [0x9a, 0x9b, 0x98], [0.9, 0.92, 0.94]),
        Preset::Montagne => ([0x4e, 0x83, 0xcf], [0xd8, 0xdd, 0xe2], [0x6b, 0x77, 0x5e], [0.98, 0.98, 1.0]),
        Preset::Ville => ([0x6b, 0x91, 0xc8], [0xdb, 0xd1, 0xc6], [0x6e, 0x6a, 0x66], [0.96, 0.95, 0.96]),
        Preset::Mer => ([0x3f, 0x86, 0xd6], [0xd5, 0xe3, 0xec], [0x2f, 0x5f, 0x84], [1.0, 1.0, 1.02]),
        Preset::Studio => ([0xb8, 0xb8, 0xb8], [0xc8, 0xc8, 0xc8], [0xc0, 0xc0, 0xc0], [1.0, 1.0, 1.0]),
    }
}

impl Cyclo {
    /// Ses couleurs et sa lumière.
    pub fn vu(&self) -> CycloVu {
        let c = self.borne();
        let (z, h, s, carte) = palette(c.preset);
        let (mut zenith, mut horizon, mut sol) = (lin(z), lin(h), lin(s));
        let (nuit, chaleur) = (c.nuit(), c.chaleur());
        // L'heure : le soir réchauffe l'horizon, la nuit assombrit et bleuit (sauf le Studio, qui n'a pas d'heure).
        let studio = c.preset == Preset::Studio;
        if !studio && c.preset != Preset::Nuit {
            let (cz, ch, cs, _) = palette(Preset::Coucher);
            zenith = mix3(zenith, lin(cz), chaleur * 0.7);
            horizon = mix3(horizon, lin(ch), chaleur * 0.8);
            sol = mix3(sol, lin(cs), chaleur * 0.5);
            let (nz, nh, ns, _) = palette(Preset::Nuit);
            zenith = mix3(zenith, lin(nz), nuit);
            horizon = mix3(horizon, lin(nh), nuit);
            sol = mix3(sol, lin(ns), nuit);
        }
        if let Some(t) = c.couleur {
            let t = lin(t);
            zenith = mix3(zenith, t, 0.35);
            horizon = mix3(horizon, t, 0.25);
        }
        // Les nuages voilent : le ciel grisaille un peu.
        let gris = [0.55, 0.57, 0.6].map(|v: f32| v * (1.0 - nuit * 0.85));
        zenith = mix3(zenith, gris, c.nuages * 0.4);
        // La lumière : le ciel et le sol (les ambiances d'aujourd'hui au Jour clair), le soleil.
        let (ac, asol) = (crate::decor3drendu::AMBIANCE_CIEL, crate::decor3drendu::AMBIANCE_SOL);
        let ref_z = lin(palette(Preset::Jour).0);
        let k = |v: [f32; 3], r: [f32; 3], a: [f32; 3]| [0, 1, 2].map(|i| a[i] * (v[i] / r[i].max(1e-3)).clamp(0.1, 1.6));
        let ciel_amb = k(zenith, ref_z, ac);
        let sol_amb = k(sol, lin(palette(Preset::Jour).2), asol);
        let force = crate::decor3drendu::SOLEIL_FORCE * (1.0 - c.nuages * 0.6) * (1.0 - nuit * 0.8) * if studio { 0.7 } else { 1.0 };
        let teinte = mix3(crate::decor3drendu::SOLEIL_COULEUR, [1.0, 0.62, 0.38], chaleur);
        let teinte = mix3(teinte, [0.55, 0.65, 1.0], nuit);
        let soleil = teinte.map(|v| v * force);
        // Les cartes : la lumière du choix, déplacée par l'heure (le soir chaud, la nuit sombre et bleue) ; un peu de soleil.
        let mut ambiante = carte;
        if !studio && c.preset != Preset::Nuit {
            ambiante = mix3(ambiante, palette(Preset::Coucher).3, chaleur);
            ambiante = mix3(ambiante, palette(Preset::Nuit).3, nuit);
        }
        if let Some(t) = c.couleur {
            ambiante = mix3(ambiante, lin(t).map(|v| v * 1.6), 0.15);
        }
        let jour_clair = c.preset == Preset::Jour && chaleur <= 0.0 && nuit <= 0.0 && c.couleur.is_none();
        let ambiante_soleil = if jour_clair || studio { [0.0; 3] } else { soleil.map(|v| v * 0.08) };
        CycloVu {
            actif: true,
            cle: c.cle(),
            pret: false,
            rotation: c.rotation.to_radians(),
            zenith,
            horizon,
            sol,
            ciel_amb,
            sol_amb,
            soleil,
            soleil_dir: {
                let (a, e) = (c.azimut_soleil().to_radians(), c.hauteur_soleil().abs().clamp(8.0, 89.0).to_radians());
                m::normalize([e.cos() * a.sin(), e.sin(), e.cos() * a.cos()])
            },
            brume: c.brume,
            ambiante,
            ambiante_soleil,
        }
    }
}

// ------------------------------------------------------------------------------------------------ l'image

/// L'image d'un cyclo : équirectangulaire, sRGB (RGBA8), le haut au zénith ; la colonne du milieu regarde vers +z (derrière la
/// caméra de référence, comme l'azimut 0 de `Aspect::soleil`).
#[derive(Clone, Debug, PartialEq)]
pub struct Equirect {
    pub largeur: usize,
    pub hauteur: usize,
    pub rgba: Vec<u8>,
}

impl Equirect {
    /// La couleur (linéaire) vue dans la direction `dir` (longueur 1), tournée de `rotation` (radians), lue en bilinéaire.
    pub fn vers(&self, dir: V3, rotation: f32) -> [f32; 3] {
        let (u, v) = uv(dir, rotation);
        let (w, h) = (self.largeur, self.hauteur);
        let x = u * w as f32 - 0.5;
        let y = (v * h as f32 - 0.5).clamp(0.0, h as f32 - 1.0);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let px = |xi: i64, yi: i64| {
            let xi = xi.rem_euclid(w as i64) as usize;
            let yi = yi.clamp(0, h as i64 - 1) as usize;
            let o = (yi * w + xi) * 4;
            lin([self.rgba[o], self.rgba[o + 1], self.rgba[o + 2]])
        };
        let (xi, yi) = (x0 as i64, y0 as i64);
        let (a, b, c, d) = (px(xi, yi), px(xi + 1, yi), px(xi, yi + 1), px(xi + 1, yi + 1));
        [0, 1, 2].map(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
    }

    /// La couleur moyenne (sRGB) de la bande d'élévations `[e0, e1]` (degrés), pour les essais et la galerie.
    pub fn moyenne(&self, e0: f32, e1: f32) -> [f32; 3] {
        let (w, h) = (self.largeur, self.hauteur);
        let (mut s, mut n) = ([0.0f32; 3], 0.0f32);
        for y in 0..h {
            let e = 90.0 - (y as f32 + 0.5) / h as f32 * 180.0;
            if e < e0 || e > e1 {
                continue;
            }
            for x in 0..w {
                let o = (y * w + x) * 4;
                for k in 0..3 {
                    s[k] += self.rgba[o + k] as f32;
                }
                n += 1.0;
            }
        }
        s.map(|v| v / n.max(1.0))
    }
}

/// La place (u, v de 0 à 1) dans l'image équirectangulaire de la direction `dir`, tournée de `rotation`.
pub fn uv(dir: V3, rotation: f32) -> (f32, f32) {
    let d = m::normalize(dir);
    let az = d[0].atan2(d[2]) - rotation;
    let u = (az / std::f32::consts::TAU + 0.5).rem_euclid(1.0);
    let v = (std::f32::consts::FRAC_PI_2 - d[1].clamp(-1.0, 1.0).asin()) / std::f32::consts::PI;
    (u, v)
}

/// Un bruit de valeurs lissé, de graine `g`, au point (x, y, z).
fn bruit(g: u32, x: f32, y: f32, z: f32) -> f32 {
    let h = |i: i32, j: i32, k: i32| {
        let mut n = (i as u32).wrapping_mul(0x8da6b343) ^ (j as u32).wrapping_mul(0xd8163841) ^ (k as u32).wrapping_mul(0xcb1ab31f) ^ g.wrapping_mul(0x9e3779b9);
        n ^= n >> 13;
        n = n.wrapping_mul(0x5bd1e995);
        n ^= n >> 15;
        (n & 0xffff) as f32 / 65535.0
    };
    let (xi, yi, zi) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let (fx, fy, fz) = (x - x.floor(), y - y.floor(), z - z.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy, sz) = (s(fx), s(fy), s(fz));
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c00 = l(h(xi, yi, zi), h(xi + 1, yi, zi), sx);
    let c10 = l(h(xi, yi + 1, zi), h(xi + 1, yi + 1, zi), sx);
    let c01 = l(h(xi, yi, zi + 1), h(xi + 1, yi, zi + 1), sx);
    let c11 = l(h(xi, yi + 1, zi + 1), h(xi + 1, yi + 1, zi + 1), sx);
    l(l(c00, c10, sy), l(c01, c11, sy), sz)
}

fn fbm(g: u32, p: V3, octaves: u32) -> f32 {
    let (mut s, mut a, mut f, mut n) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        s += a * bruit(g.wrapping_add(o * 7919), p[0] * f, p[1] * f, p[2] * f);
        n += a;
        a *= 0.5;
        f *= 2.03;
    }
    s / n
}

/// La hauteur (degrés d'élévation) de la silhouette d'horizon à l'azimut `az` (radians) : rien (Jour, Nuageux, Coucher, Nuit,
/// Brouillard, Mer, Studio), des crêtes (Montagne, trois couches), des immeubles (Ville). Rend aussi la couche (0 : la plus proche) et,
/// pour la ville, si le point est une fenêtre allumée.
fn silhouette(c: &Cyclo, az: f32, e: f32) -> Option<(usize, bool)> {
    let g = c.graine;
    match c.preset {
        Preset::Montagne => {
            // (De la plus proche, basse et sombre, à la plus lointaine, haute et pâle : une ligne de crêtes par couche ; la plus proche
            // qui couvre le point le cache.)
            if e < -0.5 {
                return None;
            }
            for couche in (0..3usize).rev() {
                let k = couche as f32;
                let f = 1.6 + k * 1.3;
                let p = [az.cos() * f, az.sin() * f, k * 3.7];
                let n = ((fbm(g.wrapping_add(couche as u32 * 101), p, 5) - 0.3) / 0.4).clamp(0.0, 1.0);
                let haut = 1.5 + (2 - couche) as f32 * 2.5 + n * (16.0 - k * 4.5);
                if e <= haut {
                    return Some((couche, false));
                }
            }
            None
        }
        Preset::Ville => {
            // Des immeubles : des largeurs et des hauteurs tirées de la graine, autour de l'horizon.
            let pas = 0.06;
            let i = (az / pas).floor() as i64;
            let tire = |i: i64, s: u32| {
                let mut n = (i as u32).wrapping_mul(0x27d4eb2d) ^ g.wrapping_mul(0x165667b1) ^ s;
                n ^= n >> 15;
                n = n.wrapping_mul(0x2c1b3c6d);
                n ^= n >> 12;
                (n & 0xffff) as f32 / 65535.0
            };
            let haut = 1.5 + tire(i, 1).powf(1.5) * 11.0;
            if e > haut || e < -0.5 {
                return None;
            }
            // Les fenêtres : une grille, une sur trois allumée la nuit.
            let fx = ((az / pas).fract() * 6.0).fract();
            let fy = (e / 0.6).fract();
            let col = ((az / pas).fract() * 6.0).floor() as i64;
            let rang = (e / 0.6).floor() as i64;
            let fenetre = fx > 0.3 && fx < 0.75 && fy > 0.35 && fy < 0.8 && tire(i * 131 + col * 7 + rang * 977, 3) > 0.66;
            Some((0, fenetre))
        }
        _ => None,
    }
}

/// L'image du cyclo `c`, de `largeur` × `hauteur` pixels. Répartie sur les cœurs ; la même graine fait toujours la même image.
pub fn generer(c: &Cyclo, largeur: usize, hauteur: usize) -> Equirect {
    let c = c.borne();
    let v = c.vu();
    let mut rgba = vec![0u8; largeur * hauteur * 4];
    let (nuit, studio) = (c.nuit(), c.preset == Preset::Studio);
    let soleil_dir = {
        let (a, e) = ((c.azimut_soleil() - c.rotation).to_radians(), c.hauteur_soleil().clamp(-10.0, 89.0).to_radians());
        [e.cos() * a.sin(), e.sin(), e.cos() * a.cos()]
    };
    let brume = mix3(v.horizon, [0.8, 0.8, 0.8].map(|x: f32| x * (1.0 - nuit * 0.8)), 0.3);
    crate::render::par_chunks(&mut rgba, largeur * 4, |y, ligne| {
        let e = 90.0 - (y as f32 + 0.5) / hauteur as f32 * 180.0;
        let er = e.to_radians();
        for x in 0..largeur {
            let az = ((x as f32 + 0.5) / largeur as f32 - 0.5) * std::f32::consts::TAU;
            let dir = [er.cos() * az.sin(), er.sin(), er.cos() * az.cos()];
            let mut col = if studio {
                // Le cyclo des photographes : le fond monte du sol sans arête, un gris à peine plus clair en haut.
                mix3(v.sol, v.zenith, smooth(-30.0, 60.0, e))
            } else if e >= 0.0 {
                let k = (e / 90.0).powf(0.55);
                mix3(v.horizon, v.zenith, k)
            } else {
                // Le sol au loin, qui rejoint l'horizon dans la brume.
                mix3(v.sol, v.horizon, (1.0 + e / 12.0).clamp(0.0, 1.0) * 0.6)
            };
            if !studio {
                // Le soleil (ou la lune) : un halo, et son disque.
                let cs = m::dot(dir, soleil_dir);
                // (La lune : un halo plus serré.)
                let halo = if nuit > 0.5 { smooth(0.985, 1.0, cs) * 0.25 } else { smooth(0.9, 1.0, cs) * 0.35 } * (1.0 - c.nuages * 0.7);
                let disque = smooth(0.9993, 0.9996, cs);
                let teinte = if nuit > 0.5 { [0.9, 0.92, 1.0] } else { mix3([1.0, 0.95, 0.85], [1.0, 0.6, 0.35], c.chaleur()) };
                col = mix3(col, teinte, (halo + disque * (1.0 - c.nuages * 0.8)).min(1.0));
                // Les étoiles, la nuit, au-dessus de l'horizon.
                if nuit > 0.5 && e > 5.0 {
                    let n = bruit(c.graine.wrapping_add(4242), x as f32 * 0.9, y as f32 * 0.9, 0.0);
                    if n > 0.985 {
                        col = mix3(col, [0.9, 0.92, 1.0], (n - 0.985) / 0.015 * (1.0 - c.nuages));
                    }
                }
                // Les nuages : un bruit sur la sphère, aplati vers l'horizon, sa couverture, ses bords doux.
                if e > -2.0 && c.nuages > 0.0 {
                    let p = [dir[0] / (dir[1].abs() + 0.25) * 1.4, dir[1] * 2.0, dir[2] / (dir[1].abs() + 0.25) * 1.4];
                    let n = fbm(c.graine, p, 5);
                    let seuil = 0.72 - c.nuages * 0.42;
                    // (Vers le zénith, l'image équirectangulaire étire tout : les nuages s'effacent.)
                    let a = smooth(seuil, seuil + 0.12, n) * smooth(-2.0, 8.0, e) * (1.0 - smooth(45.0, 75.0, e));
                    let blanc = mix3([0.92, 0.92, 0.94], [0.62, 0.64, 0.68], c.nuages * 0.6);
                    let blanc = mix3(blanc, [1.0, 0.7, 0.5], c.chaleur() * 0.6);
                    let blanc = mix3(blanc, [0.08, 0.1, 0.16], nuit);
                    col = mix3(col, blanc, a);
                }
                // L'horizon : les silhouettes, plus pâles au loin (la perspective aérienne).
                if let Some((couche, fenetre)) = silhouette(&c, az, e) {
                    let sombre = match c.preset {
                        Preset::Montagne => mix3(v.sol.map(|x| x * 0.6), v.horizon, [0.55, 0.32, 0.08][couche.min(2)]),
                        _ => mix3([0.16, 0.17, 0.2], v.horizon, 0.35),
                    };
                    let sombre = sombre.map(|x| x * (1.0 - nuit * 0.75));
                    col = if fenetre && nuit > 0.5 { [1.0, 0.82, 0.45] } else { sombre };
                }
                if c.preset == Preset::Mer && e < 0.0 {
                    col = mix3(v.sol, v.horizon, (1.0 + e / 6.0).clamp(0.0, 1.0) * 0.7);
                }
                // La brume : vers l'horizon.
                let b = c.brume * (1.0 - smooth(0.0, 25.0, e.abs()));
                col = mix3(col, brume, b);
            }
            let o = x * 4;
            for k in 0..3 {
                ligne[o + k] = (crate::decor3drendu::vers_srgb(col[k]) * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            ligne[o + 3] = 255;
        }
    });
    Equirect { largeur, hauteur, rgba }
}

// ------------------------------------------------------------------------------------------------ le cache

#[derive(Default)]
struct Cache {
    /// Les images prêtes, par empreinte (les dernières, `GARDEES`).
    pretes: Vec<(u64, Arc<Equirect>)>,
    /// Celles en cours sur un autre fil.
    en_cours: HashMap<u64, std::thread::JoinHandle<Equirect>>,
    /// Une image vient d'arriver : il faut redessiner.
    arrivee: bool,
    /// Combien d'images ont été faites depuis le début (essais : rien ne se refait quand la caméra bouge).
    faites: u64,
    /// Combien de fois chaque image a été faite (essais).
    par_cle: HashMap<u64, u32>,
}

/// Les images gardées (2048 × 1024 en RGBA : 8 Mo chacune). Les essais, qui tournent ensemble, en gardent plus.
const GARDEES: usize = if cfg!(test) { 64 } else { 8 };

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

fn cache<R>(f: impl FnOnce(&mut Cache) -> R) -> R {
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Cache::default))
}

fn ranger(c: &mut Cache, cle: u64, img: Arc<Equirect>) {
    c.pretes.retain(|(k, _)| *k != cle);
    c.pretes.push((cle, img));
    if c.pretes.len() > GARDEES {
        c.pretes.remove(0);
    }
    c.faites += 1;
    *c.par_cle.entry(cle).or_default() += 1;
}

/// L'image prête du cyclo d'empreinte `cle`, s'il y en a une.
pub fn texture(cle: u64) -> Option<Arc<Equirect>> {
    cache(|c| c.pretes.iter().find(|(k, _)| *k == cle).map(|(_, i)| i.clone()))
}

/// L'image du cyclo `cy` : prête, ou, sur le fil de l'interface, demandée à un autre fil (rien en attendant) ; ailleurs (export,
/// essais, vignettes), faite tout de suite.
pub fn assurer(cy: &Cyclo) -> Option<Arc<Equirect>> {
    let cle = cy.cle();
    if let Some(t) = texture(cle) {
        return Some(t);
    }
    if crate::decor3d::sur_l_interface() {
        let c = *cy;
        cache(|k| {
            if !k.en_cours.contains_key(&cle) {
                let h = std::thread::Builder::new().name("cyclo".into()).spawn(move || generer(&c, LARGEUR, HAUTEUR));
                if let Ok(h) = h {
                    k.en_cours.insert(cle, h);
                }
            }
        });
        return None;
    }
    let img = Arc::new(generer(cy, LARGEUR, HAUTEUR));
    cache(|c| ranger(c, cle, img.clone()));
    Some(img)
}

/// À chaque image d'écran : range les images finies ; vrai si l'une vient d'arriver (il faut redessiner).
pub fn poll() -> bool {
    cache(|c| {
        let finies: Vec<u64> = c.en_cours.iter().filter(|(_, h)| h.is_finished()).map(|(k, _)| *k).collect();
        for k in finies {
            if let Some(h) = c.en_cours.remove(&k) {
                if let Ok(img) = h.join() {
                    ranger(c, k, Arc::new(img));
                    c.arrivee = true;
                }
            }
        }
        std::mem::take(&mut c.arrivee)
    })
}

/// Une image est en cours sur un autre fil.
pub fn en_cours() -> bool {
    cache(|c| !c.en_cours.is_empty())
}

/// Combien d'images de cyclo ont été faites (essais).
#[allow(dead_code)]
pub fn faites() -> u64 {
    cache(|c| c.faites)
}

/// Combien de fois l'image d'empreinte `cle` a été faite (essais : une fois, quoi que fasse la caméra).
#[cfg(test)]
pub fn faites_de(cle: u64) -> u32 {
    cache(|c| c.par_cle.get(&cle).copied().unwrap_or(0))
}

/// Essais : attend les images en cours.
#[cfg(test)]
pub fn attendre() {
    let debut = std::time::Instant::now();
    while en_cours() {
        poll();
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(debut.elapsed() < std::time::Duration::from_secs(60));
    }
    poll();
}

/// La vignette d'un choix (à ses réglages d'origine), pour la galerie : petite, faite tout de suite, gardée.
pub fn vignette(p: Preset) -> Arc<Equirect> {
    static VIGNETTES: Mutex<Vec<(Preset, Arc<Equirect>)>> = Mutex::new(Vec::new());
    let mut g = VIGNETTES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, v)) = g.iter().find(|(q, _)| *q == p) {
        return v.clone();
    }
    let v = Arc::new(generer(&p.cyclo(), VIGNETTE[0], VIGNETTE[1]));
    g.push((p, v.clone()));
    v
}
