//! Aides au cadrage : des repères tracés par-dessus le cadre de la caméra pour composer l'image
//! (Anthony, 2026-10-19 : « intégrer en layout sur la caméra des aides au cadrage, le nombre d'or
//! également »). Elles ne servent qu'à l'écran : rien n'en passe dans le film, l'export ni les
//! vignettes. (Les « guides » de `guides.rs` sont ceux de la perspective, c'est autre chose.)
//!
//! Tout est calculé ici en géométrie pure, dans un cadre de `w` × `h` (l'origine en haut à
//! gauche) :
//! - les tiers (grille 3 × 3) ;
//! - la grille d'or (phi) : lignes à 1 / 2,618 et 1,618 / 2,618 de la largeur et de la hauteur
//!   (proportions 1 : 0,618 : 1), un point à chacune des quatre intersections ;
//! - la spirale d'or : la suite de carrés d'un rectangle d'or (chaque carré est coupé dans le
//!   rectangle restant, en tournant dans le sens des aiguilles d'une montre) avec un quart de
//!   cercle dans chacun, étirée aux proportions du cadre : si le cadre n'est pas un rectangle d'or
//!   (16:9, 9:16, 2,39:1…), les « carrés » deviennent des rectangles qui remplissent quand même le
//!   cadre. Huit orientations (quatre coins × retournement horizontal ou vertical) ;
//! - les diagonales : les deux diagonales du cadre et leurs réciproques (symétrie dynamique), tirées
//!   de chaque coin perpendiculairement à la diagonale qui ne passe pas par lui ;
//! - la croix centrale ;
//! - les zones de sécurité (action et titres) et le cadre d'un format (2,39:1, 1,85:1, 16:9…), dont
//!   le choix reste dans la disposition (`workspace::Layout`) et le tracé dans `viewbar.rs`.
//!
//! Le calcul d'un jeu de repères ne se refait que si la taille du cadre ou les repères choisis
//! changent (`Cache`) ; `cadrageui.rs` les place sur l'écran, avec la caméra.

use eframe::egui::{pos2, vec2, Color32, Pos2, Rect};
use serde::{Deserialize, Serialize};

/// Le nombre d'or.
pub const PHI: f32 = 1.618_034;

/// Zones de sécurité : deux paires de valeurs courantes en diffusion (à vérifier avant de s'y fier
/// pour une livraison), en part de la largeur et de la hauteur du cadre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafeZones {
    /// Action 93 %, titres 90 % (recommandation EBU R 95), le réglage de toujours dans SHOT.
    #[default]
    Ebu,
    /// Action 90 %, titres 80 % (valeurs courantes des anciennes zones de sécurité ; à vérifier).
    Large,
}

impl SafeZones {
    pub const ALL: [SafeZones; 2] = [SafeZones::Ebu, SafeZones::Large];

    /// (action, titres), en part du cadre.
    pub fn parts(self) -> (f32, f32) {
        match self {
            SafeZones::Ebu => (crate::frames::ACTION_SAFE, crate::frames::TITLE_SAFE),
            SafeZones::Large => (0.90, 0.80),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SafeZones::Ebu => "Action 93 % · titres 90 % (EBU R 95)",
            SafeZones::Large => "Action 90 % · titres 80 % (à vérifier)",
        }
    }
}

/// Les aides au cadrage choisies et leur aspect, retenues d'une séance à l'autre
/// (`[aides_cadrage]` de `reglages.toml`). Les zones de sécurité (allumées) et le cadre de format
/// restent dans la disposition, où ils étaient déjà.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FramingAids {
    #[serde(rename = "tiers")]
    pub thirds: bool,
    #[serde(rename = "grille_or")]
    pub phi_grid: bool,
    #[serde(rename = "spirale")]
    pub spiral: bool,
    /// Orientation de la spirale, de 0 à 7.
    #[serde(rename = "spirale_orientation")]
    pub spiral_variant: u8,
    #[serde(rename = "diagonales")]
    pub diagonals: bool,
    #[serde(rename = "croix_centrale")]
    pub cross: bool,
    /// Zones de sécurité choisies (action et titres).
    #[serde(rename = "zones_securite")]
    pub safe: SafeZones,
    #[serde(rename = "couleur")]
    pub color: [u8; 3],
    /// Opacité des traits, de 0,05 à 1.
    #[serde(rename = "opacite")]
    pub opacity: f32,
    /// Épaisseur des traits, en points d'écran.
    #[serde(rename = "epaisseur")]
    pub width: f32,
    /// Version des valeurs par défaut de l'aspect : un fichier d'avant (sans ce champ) vaut 0, et
    /// ses traits restés à l'ancien défaut (1,3 px, 85 %) passent au nouveau, plus discret.
    #[serde(rename = "version_aspect", default = "old_look_version")]
    pub look_version: u8,
}

/// Épaisseur par défaut des repères (points d'écran) : fine, ils ne doivent pas gêner le dessin.
pub const DEFAULT_WIDTH: f32 = 1.0;
/// Opacité par défaut des repères.
pub const DEFAULT_OPACITY: f32 = 0.7;
/// Version courante des valeurs par défaut de l'aspect.
const LOOK_VERSION: u8 = 1;
/// Ancien défaut d'épaisseur et d'opacité (version 0).
const OLD_WIDTH: f32 = 1.3;
const OLD_OPACITY: f32 = 0.85;

fn old_look_version() -> u8 {
    0
}

impl Default for FramingAids {
    fn default() -> Self {
        FramingAids {
            thirds: false,
            phi_grid: false,
            spiral: false,
            spiral_variant: 0,
            diagonals: false,
            cross: false,
            safe: SafeZones::default(),
            color: [90, 220, 255],
            opacity: DEFAULT_OPACITY,
            width: DEFAULT_WIDTH,
            look_version: LOOK_VERSION,
        }
    }
}

impl FramingAids {
    /// Réglages ramenés dans des bornes sûres (fichier modifié à la main…), et l'aspect d'un
    /// ancien fichier mis au nouveau défaut s'il n'avait pas été changé.
    pub fn sanitized(mut self) -> FramingAids {
        if self.look_version < LOOK_VERSION {
            if (self.width - OLD_WIDTH).abs() < 1e-3 {
                self.width = DEFAULT_WIDTH;
            }
            if (self.opacity - OLD_OPACITY).abs() < 1e-3 {
                self.opacity = DEFAULT_OPACITY;
            }
            self.look_version = LOOK_VERSION;
        }
        self.spiral_variant %= 8;
        self.opacity = if self.opacity.is_finite() { self.opacity.clamp(0.05, 1.0) } else { DEFAULT_OPACITY };
        self.width = if self.width.is_finite() { self.width.clamp(0.5, 6.0) } else { DEFAULT_WIDTH };
        self
    }

    /// Au moins un repère est tracé par ces réglages.
    pub fn any_on(&self) -> bool {
        self.thirds || self.phi_grid || self.spiral || self.diagonals || self.cross
    }

    /// Éteint tous les repères (pas les zones de sécurité ni le cadre de format).
    pub fn clear(&mut self) {
        self.thirds = false;
        self.phi_grid = false;
        self.spiral = false;
        self.diagonals = false;
        self.cross = false;
    }

    /// Orientation suivante de la spirale (huit, en boucle).
    pub fn next_variant(&mut self) {
        self.spiral_variant = (self.spiral_variant + 1) % 8;
    }

    /// Couleur des traits, avec l'opacité.
    pub fn stroke_color(&self) -> Color32 {
        let [r, g, b] = self.color;
        Color32::from_rgba_unmultiplied(r, g, b, (self.opacity.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

/// Le nom de l'orientation `v` de la spirale : « 3 / 8 ».
pub fn variant_label(v: u8) -> String {
    format!("{} / 8", v % 8 + 1)
}

// ---------------------------------------------------------------------- les repères

/// Un tracé : une suite de points.
pub type Line = Vec<Pos2>;

fn seg(a: Pos2, b: Pos2) -> Line {
    vec![a, b]
}

/// Les lignes des tiers : deux verticales et deux horizontales.
pub fn thirds(w: f32, h: f32) -> Vec<Line> {
    let mut out = Vec::new();
    for k in 1..3 {
        let f = k as f32 / 3.0;
        out.push(seg(pos2(w * f, 0.0), pos2(w * f, h)));
        out.push(seg(pos2(0.0, h * f), pos2(w, h * f)));
    }
    out
}

/// Les deux positions d'or sur une longueur : à 1 / 2,618 et 1,618 / 2,618 (proportions
/// 1 : 0,618 : 1).
pub fn phi_positions(len: f32) -> [f32; 2] {
    [len / (1.0 + PHI), len * PHI / (1.0 + PHI)]
}

/// La grille d'or : ses quatre lignes et les quatre points d'intersection.
pub fn phi_grid(w: f32, h: f32) -> (Vec<Line>, Vec<Pos2>) {
    let (px, py) = (phi_positions(w), phi_positions(h));
    let mut lines = Vec::new();
    let mut dots = Vec::new();
    for x in px {
        lines.push(seg(pos2(x, 0.0), pos2(x, h)));
    }
    for y in py {
        lines.push(seg(pos2(0.0, y), pos2(w, y)));
    }
    for x in px {
        for y in py {
            dots.push(pos2(x, y));
        }
    }
    (lines, dots)
}

/// Les deux diagonales du cadre, puis leurs quatre réciproques : de chaque coin, le segment
/// perpendiculaire à la diagonale qui ne passe pas par lui, jusqu'à elle.
pub fn diagonals(w: f32, h: f32) -> Vec<[Pos2; 2]> {
    let (tl, tr, br, bl) = (pos2(0.0, 0.0), pos2(w, 0.0), pos2(w, h), pos2(0.0, h));
    // Pied de la perpendiculaire menée de `c` à la droite (a, b).
    let foot = |a: Pos2, b: Pos2, c: Pos2| {
        let d = b - a;
        let t = (c - a).dot(d) / d.length_sq().max(1e-9);
        a + d * t
    };
    vec![
        [tl, br],
        [tr, bl],
        // La diagonale haut-gauche / bas-droite reçoit les perpendiculaires des deux autres coins.
        [tr, foot(tl, br, tr)],
        [bl, foot(tl, br, bl)],
        // L'autre reçoit celles de haut-gauche et de bas-droite.
        [tl, foot(tr, bl, tl)],
        [br, foot(tr, bl, br)],
    ]
}

/// La croix centrale : deux petits traits, de 2,5 % du plus petit côté de chaque côté du centre.
pub fn center_cross(w: f32, h: f32) -> Vec<Line> {
    let c = pos2(w / 2.0, h / 2.0);
    let r = 0.025 * w.min(h);
    vec![seg(c - vec2(r, 0.0), c + vec2(r, 0.0)), seg(c - vec2(0.0, r), c + vec2(0.0, r))]
}

/// Nombre de carrés de la spirale.
pub const SPIRAL_SQUARES: usize = 11;
/// Segments d'un quart de cercle.
const ARC_STEPS: usize = 20;

/// La spirale d'or et ses carrés dans un cadre de `w` × `h`.
#[derive(Clone, Debug, PartialEq)]
pub struct Spiral {
    /// Les carrés (des rectangles si le cadre n'est pas un rectangle d'or), du plus grand au plus petit.
    pub squares: Vec<Rect>,
    /// La spirale, d'un seul tenant, du grand carré vers le centre.
    pub arc: Line,
}

/// La spirale d'orientation `variant` (0 à 7) : celle d'un rectangle d'or de 1,618 × 1, ramenée au
/// carré unité, retournée selon `variant` (bit 1 : transposée, bit 2 : retournée en largeur,
/// bit 4 : en hauteur), puis étirée à `w` × `h`.
pub fn spiral(w: f32, h: f32, variant: u8) -> Spiral {
    let (mut x0, mut y0, mut x1, mut y1) = (0.0_f32, 0.0_f32, PHI, 1.0_f32);
    let mut squares: Vec<Rect> = Vec::new();
    let mut arc: Line = Vec::new();
    for i in 0..SPIRAL_SQUARES {
        // (carré, début de l'arc, fin, centre, côté) : on coupe à gauche, en haut, à droite, en bas…
        let (sq, start, end, center, s) = match i % 4 {
            0 => {
                let s = y1 - y0;
                let out = (Rect::from_min_max(pos2(x0, y0), pos2(x0 + s, y1)), pos2(x0, y1), pos2(x0 + s, y0), pos2(x0 + s, y1), s);
                x0 += s;
                out
            }
            1 => {
                let s = x1 - x0;
                let out = (Rect::from_min_max(pos2(x0, y0), pos2(x1, y0 + s)), pos2(x0, y0), pos2(x1, y0 + s), pos2(x0, y0 + s), s);
                y0 += s;
                out
            }
            2 => {
                let s = y1 - y0;
                let out = (Rect::from_min_max(pos2(x1 - s, y0), pos2(x1, y1)), pos2(x1, y0), pos2(x1 - s, y1), pos2(x1 - s, y0), s);
                x1 -= s;
                out
            }
            _ => {
                let s = x1 - x0;
                let out = (Rect::from_min_max(pos2(x0, y1 - s), pos2(x1, y1)), pos2(x1, y1), pos2(x0, y1 - s), pos2(x1, y1 - s), s);
                y1 -= s;
                out
            }
        };
        squares.push(sq);
        // Quart de cercle de `start` à `end` autour de `center` (par le plus court chemin).
        let a0 = (start - center).angle();
        let mut delta = (end - center).angle() - a0;
        while delta > std::f32::consts::PI {
            delta -= std::f32::consts::TAU;
        }
        while delta < -std::f32::consts::PI {
            delta += std::f32::consts::TAU;
        }
        for k in 0..=ARC_STEPS {
            if k == 0 && !arc.is_empty() {
                continue;
            }
            let a = a0 + delta * k as f32 / ARC_STEPS as f32;
            arc.push(center + vec2(a.cos(), a.sin()) * s);
        }
    }
    // Du rectangle d'or au carré unité, retourné selon `variant`, puis étiré au cadre.
    let map = |p: Pos2| {
        let (mut u, mut v) = (p.x / PHI, p.y);
        if variant & 1 != 0 {
            std::mem::swap(&mut u, &mut v);
        }
        if variant & 2 != 0 {
            u = 1.0 - u;
        }
        if variant & 4 != 0 {
            v = 1.0 - v;
        }
        pos2(u * w, v * h)
    };
    Spiral { squares: squares.iter().map(|r| Rect::from_two_pos(map(r.min), map(r.max))).collect(), arc: arc.into_iter().map(map).collect() }
}

// ---------------------------------------------------------------------- le jeu de repères

/// Tous les repères choisis, dans un cadre de `w` × `h`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    /// Traits pleins.
    pub lines: Vec<Line>,
    /// Traits discrets (les carrés de la spirale).
    pub faint: Vec<Line>,
    /// Points (les intersections de la grille d'or).
    pub dots: Vec<Pos2>,
}

/// Le jeu de repères choisis par `s` dans un cadre de `w` × `h`.
pub fn build(w: f32, h: f32, s: &FramingAids) -> Geometry {
    let mut g = Geometry::default();
    if s.thirds {
        g.lines.extend(thirds(w, h));
    }
    if s.phi_grid {
        let (lines, dots) = phi_grid(w, h);
        g.lines.extend(lines);
        g.dots.extend(dots);
    }
    if s.diagonals {
        g.lines.extend(diagonals(w, h).into_iter().map(|[a, b]| seg(a, b)));
    }
    if s.spiral {
        let sp = spiral(w, h, s.spiral_variant);
        for r in &sp.squares {
            g.faint.push(vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()]);
        }
        g.lines.push(sp.arc);
    }
    if s.cross {
        g.lines.extend(center_cross(w, h));
    }
    g
}

/// Le jeu de repères du dernier cadre, gardé tant que ni la taille du cadre ni les repères
/// choisis ne changent (l'aspect — couleur, opacité, épaisseur — ne compte pas).
#[derive(Default)]
pub struct Cache {
    key: Option<([u32; 2], [bool; 5], u8)>,
    geometry: Geometry,
    /// Combien de fois le jeu a été recalculé (pour les tests).
    pub builds: usize,
}

impl Cache {
    /// Le jeu de repères pour un cadre de `w` × `h` ; recalculé seulement s'il a changé.
    pub fn get(&mut self, w: f32, h: f32, s: &FramingAids) -> &Geometry {
        let key = ([w.to_bits(), h.to_bits()], [s.thirds, s.phi_grid, s.spiral, s.diagonals, s.cross], if s.spiral { s.spiral_variant } else { 0 });
        if self.key != Some(key) {
            self.geometry = build(w, h, s);
            self.key = Some(key);
            self.builds += 1;
        }
        &self.geometry
    }
}
