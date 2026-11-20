//! **La profondeur partagée entre le décor 3D et les cartes dessinées** (partie 183, `docs/INTERFACE.md` ; `docs/ESPACE3D.md`,
//! parties 14 et 16).
//!
//! Jusqu'ici, le décor 3D d'une séquence (parties 180 à 182) était toujours derrière toutes les cartes : un personnage posé
//! derrière un arbre passait quand même devant lui. Maintenant, les cartes et les objets du décor se cachent selon leur vraie
//! distance à l'œil de la caméra du plan : un personnage passe derrière un arbre et devant un banc.
//!
//! **Comment.** Le décor est rendu une fois par image, comme avant, en gardant **sa profondeur** : à chaque pixel de la feuille,
//! l'inverse de la distance à l'œil (le long de l'axe de la caméra) de ce que le décor y montre, 0 là où il n'y a rien (le ciel
//! et les pixels vides ne cachent jamais rien). Sur la carte graphique, une texture de plus à côté de celle du décor
//! (`decor3dgpu.rs`, `gpu.rs`) ; sur le processeur, un tampon de plus à côté de l'image (`decor3drendu.rs`, `decor3d.rs`). Le décor
//! reste l'élément le plus bas de l'image du plan.
//!
//! Chaque carte porte, dans son élément du plan (`render::PlanItem::profondeur`), **le plan où elle est posée**, vu de l'œil de
//! l'image : trois nombres `(a, b, c)` tels que l'inverse de la distance du point de la carte qui tombe au pixel (x, y) de la
//! feuille soit `a·x + b·y + c` (pour un plan, l'inverse de la profondeur varie linéairement sur l'image : une carte inclinée est
//! juste aussi). À la composition, un pixel de la carte est **caché** si le décor y est plus près qu'elle (`cache`) : le décor étant
//! dessous, c'est lui qui se voit à travers. Le coût : une lecture de la profondeur du décor par pixel de carte.
//!
//! **Ce qui n'est jamais caché** (`partage`) : un dossier, un calque d'effet (ajustement, incrustation de texte), un calque
//! d'annotation, un calque accroché à la caméra (il est dans le cadre, pas dans le monde), et un calque marqué « Toujours devant le
//! décor 3D » (`Layer::devant_decor`). Ni ce qui n'est pas une carte : la couleur unie d'un calque, les pelures d'oignon, les
//! silhouettes de la trajectoire. Entre elles, les cartes gardent l'ordre de toujours (la pile, ou la distance d'un plan en
//! Espace 3D).
//!
//! **Les bords doux** (partie 193) : la profondeur du décor est gardée en quatre échantillons par pixel (`ProfDecor`), et un pixel
//! de carte est caché pour la part de ses échantillons où le décor est plus près (`part_cachee`, `Masque::visible`) : son opacité
//! est multipliée par la part qui se voit. Les bords d'occlusion sont lissés sur un pixel ; l'intérieur ne change pas.
//!
//! **Sans décor**, aucun élément ne porte de plan : la composition est exactement celle d'avant, par les mêmes chemins.

use crate::document::{CameraSeen, Layer};
use crate::espace3d::Monde;
use crate::espace3dmath::{self as m, M4, V3};

/// Le plan d'une carte dans l'image de la feuille, vu de l'œil de l'image : l'inverse de la distance à l'œil (mètres, le long de
/// son axe) du point de la carte qui tombe au pixel (x, y) de la feuille est `a·x + b·y + c` (x, y en pixels de la feuille, le
/// centre d'un pixel à + 0,5).
pub type PlanCarte = [f32; 3];

/// Tolérance relative : le décor ne cache une carte que s'il est plus près qu'elle d'au moins 0,4 % de sa distance…
pub const TOLERANCE_RELATIVE: f32 = 0.004;
/// … plus un centimètre. Une carte posée exactement sur une dalle (l'aimant, partie 182 : à 1 mm près) ne scintille pas à ses
/// pieds, et une carte plaquée contre un mur du décor reste devant lui.
pub const TOLERANCE_METRES: f32 = 0.01;

/// Le nom de la case, dans le menu du calque et le panneau de la carte.
pub(crate) const NOM: &str = "Toujours devant le décor 3D";
pub(crate) const PHRASE: &str = "La carte de ce calque n'est jamais cachée par le décor 3D de la séquence : elle passe devant tous ses objets, quelle que soit sa distance, tout en restant dans la scène (sa place, sa profondeur et sa parallaxe ne changent pas ; pour coller un calque au cadre, comme un titre, accroche-le plutôt à la caméra). Décoché, la carte et les objets du décor se cachent selon leur vraie distance à la caméra.";
pub(crate) const ACTION: &str = "calque.devant_decor_3d";

/// La carte de ce calque partage-t-elle la profondeur du décor (peut-elle être cachée par lui) ? Non pour un dossier, un calque
/// d'effet, un calque d'annotation, un calque accroché à la caméra, et un calque « Toujours devant le décor 3D ».
pub fn partage(layer: &Layer) -> bool {
    crate::espace3d::is_card(layer) && !layer.accroche && !layer.annotation && !layer.devant_decor
}

/// Le plan de la carte de matrice `mx` (du carré unité de la carte vers le repère où l'on regarde), vue de l'œil `eye` de ce
/// repère, dans l'image de la feuille de `world`. Rien si la carte est vue par la tranche (son plan passe par l'œil) ou abîmée.
pub fn plan_de(world: &Monde, mx: &M4, eye: V3) -> Option<PlanCarte> {
    let o = m::point(mx, [0.0; 3]);
    let n = m::cross(m::dir(mx, [1.0, 0.0, 0.0]), m::dir(mx, [0.0, 1.0, 0.0]));
    let len = m::length(n);
    if !(len.is_finite() && len > 1e-12) {
        return None;
    }
    let n = m::scale(n, 1.0 / len);
    // Le regard qui passe par le pixel (x, y) va de l'œil au point q = ((x − W/2)·k, −(y − H/2)·k, 0) de la feuille ; il touche la
    // carte en œil + t·(q − œil), à la distance t·œil.z le long de l'axe. D'où 1/distance = n·(q − œil) / (œil.z · (n·o − n·œil)).
    let ecart = m::dot(n, o) - m::dot(n, eye);
    let d = eye[2] * ecart;
    // (Le plan passe par l'œil, à un centième de millimètre près pour un œil à 10 m : la carte est vue par la tranche.)
    if !(d.is_finite() && ecart.abs() > 1e-6 * (1.0 + m::length(eye) + m::length(o)) && d.abs() > 1e-12) {
        return None;
    }
    let (k, [w, h]) = (world.k, world.stage);
    let plan = [n[0] * k / d, -n[1] * k / d, (-n[0] * k * w / 2.0 + n[1] * k * h / 2.0 - m::dot(n, eye)) / d];
    plan.iter().all(|v| v.is_finite()).then_some(plan)
}

/// Le plan de la carte du calque `layer` à l'image `frame`, tel que l'image du plan la pose (`espace3d::place_dans` : la caméra
/// `camera`, et `repere` pour un plan dont l'image passe par le changement de repère de sa caméra), vue du même œil que le décor
/// (`decor3d::item`). Rien si le calque ne partage pas la profondeur du décor (`partage`).
pub fn plan_du_calque(world: &Monde, layer: &Layer, frame: usize, camera: &CameraSeen, repere: Option<&M4>) -> Option<PlanCarte> {
    if !partage(layer) {
        return None;
    }
    match repere {
        Some(r) => {
            let xf = world.card_xf_de(layer, &layer.pose(frame), layer.depth_libre_at(frame));
            plan_de(world, &m::mul(r, &xf.matrix()), world.eye_reference())
        }
        None => {
            let xf = world.card_xf_de(layer, &layer.pose(frame), layer.depth_at(frame));
            plan_de(world, &xf.matrix(), world.eye_seen(camera))
        }
    }
}

/// L'inverse de la distance du point de la carte de plan `plan` au pixel (x, y) de la feuille.
#[inline]
pub fn inverse_distance(plan: &PlanCarte, x: f32, y: f32) -> f32 {
    plan[0] * x + plan[1] * y + plan[2]
}

/// **Le pixel (x, y) de la carte de plan `plan` est-il caché par le décor**, dont l'inverse de la distance y est `decor`
/// (0 : rien, le ciel) ? Oui si le décor est plus près que la carte, au-delà de la tolérance.
#[inline]
pub fn cache(plan: &PlanCarte, decor: f32, x: f32, y: f32) -> bool {
    if decor <= 0.0 {
        return false;
    }
    let carte = inverse_distance(plan, x, y);
    if carte <= 0.0 {
        return false;
    }
    // (1 / décor < (1 − r) / carte − m, multiplié par décor · carte, tous deux positifs : sans division, comme la carte graphique.)
    carte * (1.0 + TOLERANCE_METRES * decor) < decor * (1.0 - TOLERANCE_RELATIVE)
}

/// **Les bords doux** (partie 193) : la profondeur du décor est gardée en **quatre échantillons par pixel**, aux places où la passe
/// du décor les prend (`Motif`). Un pixel de carte est caché **pour la part de ses échantillons** où le décor est plus près
/// qu'elle (`part_cachee`) : 0, ¼, ½, ¾ ou tout. Le bord d'un objet du décor qui passe devant une carte, la ligne où une carte
/// inclinée entre dans un objet, et le bord du décor contre le ciel sont lissés sur un pixel, comme les bords du décor lui-même ;
/// l'intérieur reste tout caché ou tout visible, comme avant.
///
/// Les places des quatre échantillons dans un pixel, depuis son coin haut gauche (en pixels de la feuille).
pub type Motif = [[f32; 2]; 4];
/// Le motif de la carte graphique : le lissage à quatre échantillons, aux places standard (les mêmes sur toutes les cartes).
pub const MOTIF_CARTE: Motif = [[0.375, 0.125], [0.875, 0.375], [0.125, 0.625], [0.625, 0.875]];
/// Le motif du rendu par le processeur à deux échantillons par côté (`decor3drendu`, une feuille jusqu'à 2,5 Mpx).
pub const MOTIF_GRILLE: Motif = [[0.25, 0.25], [0.75, 0.25], [0.25, 0.75], [0.75, 0.75]];
/// Un seul échantillon, au centre, répété quatre fois (le rendu par le processeur d'une grande feuille) : le bord au pixel, comme
/// avant la partie 193.
pub const MOTIF_CENTRE: Motif = [[0.5; 2]; 4];

/// La profondeur d'un rendu du décor (parties 183 et 193) : à chaque pixel de la feuille, ligne par ligne, l'inverse de la distance
/// à l'œil (le long de son axe) de ce que le décor montre à chacun de ses quatre échantillons (0 : rien, le ciel), en flottants de
/// 16 bits (à 0,05 % près ; la tolérance est de 0,4 %), comme la texture de la carte graphique (`decor3dgpu::PROF`).
pub struct ProfDecor {
    pub w: usize,
    pub h: usize,
    /// Une valeur par pixel, celle de la partie 183 (le centre du pixel ; par le processeur, le plus proche des échantillons qui
    /// couvrent au moins la moitié du pixel) : les ombres des cartes la lisent (partie 185), et elle cache les cartes sans les bords
    /// doux.
    un: Vec<u16>,
    /// Les quatre échantillons de chaque pixel (les bords doux), s'ils sont faits.
    quatre: Option<Vec<[u16; 4]>>,
    pub motif: Motif,
}

impl ProfDecor {
    /// Une profondeur de `w` × `h` pixels : `ech`, quatre inverses de distance par pixel ; la valeur de chaque pixel en est tirée
    /// (`un_de`).
    pub fn new(w: usize, h: usize, ech: Vec<[u16; 4]>, motif: Motif) -> ProfDecor {
        debug_assert_eq!(ech.len(), w * h);
        let un = ech.iter().map(|e| vers_16_bits(un_de(e.map(de_16_bits)))).collect();
        ProfDecor { w, h, un, quatre: Some(ech), motif }
    }

    /// Une profondeur d'une valeur par pixel (la partie 183 : le bord au pixel), et, si `quatre`, ses quatre échantillons aux places
    /// `motif`.
    pub fn avec(w: usize, h: usize, un: Vec<u16>, quatre: Option<Vec<[u16; 4]>>, motif: Motif) -> ProfDecor {
        debug_assert!(un.len() == w * h && quatre.as_ref().is_none_or(|q| q.len() == w * h));
        ProfDecor { w, h, un, quatre, motif }
    }

    pub fn len(&self) -> usize {
        self.un.len()
    }

    pub fn is_empty(&self) -> bool {
        self.un.is_empty()
    }

    /// Les quatre échantillons par pixel (les bords doux) sont-ils là ?
    pub fn quatre(&self) -> bool {
        self.quatre.is_some()
    }

    /// Les quatre échantillons du pixel `i` (sans eux, sa valeur répétée).
    #[inline]
    pub fn echantillons(&self, i: usize) -> [f32; 4] {
        match &self.quatre {
            Some(q) => q[i].map(de_16_bits),
            None => [de_16_bits(self.un[i]); 4],
        }
    }

    /// **La profondeur du pixel `i`, en une valeur** : celle de la partie 183, ce que les ombres des cartes lisent pour retrouver le
    /// point du décor (partie 185).
    #[inline]
    pub fn un(&self, i: usize) -> f32 {
        de_16_bits(self.un[i])
    }
}

/// La profondeur d'un pixel en une valeur tirée de ses quatre échantillons (`ProfDecor::new`) : le plus proche de ceux que le décor
/// couvre, s'il en couvre au moins la moitié (la règle du rendu par le processeur) ; 0 sinon.
#[inline]
pub fn un_de(e: [f32; 4]) -> f32 {
    let couverts = e.iter().filter(|&&v| v > 0.0).count();
    if couverts >= 2 { e.iter().fold(0.0f32, |a, &b| a.max(b)) } else { 0.0 }
}

/// **La part du pixel de coin (`x0`, `y0`) de la carte de plan `plan` que le décor cache** (partie 193) : la part de ses quatre
/// échantillons `ech` (aux places `motif`) où le décor est plus près qu'elle (`cache`, la carte prise à la place de chacun).
#[inline]
pub fn part_cachee(plan: &PlanCarte, ech: [f32; 4], motif: &Motif, x0: f32, y0: f32) -> f32 {
    let mut n = 0u32;
    for s in 0..4 {
        if cache(plan, ech[s], x0 + motif[s][0], y0 + motif[s][1]) {
            n += 1;
        }
    }
    n as f32 * 0.25
}

/// Un flottant de 16 bits (IEEE 754) en flottant de 32 bits.
#[inline]
pub fn de_16_bits(h: u16) -> f32 {
    let (signe, exp, frac) = ((h >> 15) as u32, ((h >> 10) & 0x1f) as u32, (h & 0x3ff) as u32);
    let v = match exp {
        0 => frac as f32 * (1.0 / 16_777_216.0),
        31 => {
            if frac == 0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => f32::from_bits(((exp + 112) << 23) | (frac << 13)),
    };
    if signe == 1 { -v } else { v }
}

/// Un flottant de 32 bits en flottant de 16 bits, arrondi au plus près (ce que la carte graphique écrit dans `decor3dgpu::PROF`).
pub fn vers_16_bits(v: f32) -> u16 {
    let b = v.to_bits();
    let signe = ((b >> 16) & 0x8000) as u16;
    let a = v.abs();
    if a.is_nan() {
        return signe | 0x7e00;
    }
    if a >= 65520.0 {
        return signe | 0x7c00;
    }
    if a < 6.103_515_6e-5 {
        // (Sous-normal : un multiple de 2^-24.)
        return signe | (a * 16_777_216.0).round() as u16;
    }
    let e = ((a.to_bits() >> 23) as i32) - 127 + 15;
    let m = a.to_bits() & 0x7f_ffff;
    let mut h = ((e as u32) << 10) | (m >> 13);
    // (Au plus près, à égalité vers le pair.)
    let reste = m & 0x1fff;
    if reste > 0x1000 || (reste == 0x1000 && h & 1 == 1) {
        h += 1;
    }
    signe | h as u16
}

/// La profondeur d'un rendu du décor, gardée avec lui (`decor3d::profondeur_du_rendu`).
pub type Profondeurs = std::sync::Arc<ProfDecor>;

/// Ce qu'il faut à la composition par le processeur pour cacher une carte derrière le décor : la profondeur du décor (une feuille
/// de `w` × `h`) et le plan de la carte.
#[derive(Clone, Copy)]
pub struct Masque<'a> {
    pub prof: &'a ProfDecor,
    pub w: usize,
    pub h: usize,
    pub plan: PlanCarte,
}

impl Masque<'_> {
    /// **La part du pixel (`x`, `y`) du monde (la feuille, ou la toile autour) qui se voit** : 1 tout visible, 0 tout caché, entre
    /// les deux au bord (partie 193). Hors de la feuille, le décor n'est pas : 1.
    #[inline]
    pub fn visible(&self, x: i64, y: i64) -> f32 {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return 1.0;
        }
        let i = y as usize * self.w + x as usize;
        let h = match &self.prof.quatre {
            Some(q) => q[i],
            // (Une valeur par pixel : le test de la partie 183, au centre, tout ou rien.)
            None => return if cache(&self.plan, de_16_bits(self.prof.un[i]), x as f32 + 0.5, y as f32 + 0.5) { 0.0 } else { 1.0 },
        };
        if h == [0; 4] {
            return 1.0;
        }
        // (Le cas de presque tous les pixels : les quatre échantillons à la même distance, une carte à plat. Un seul test.)
        if h[0] == h[1] && h[1] == h[2] && h[2] == h[3] && self.plan[0] == 0.0 && self.plan[1] == 0.0 {
            return if cache(&self.plan, de_16_bits(h[0]), x as f32 + 0.5, y as f32 + 0.5) { 0.0 } else { 1.0 };
        }
        1.0 - part_cachee(&self.plan, h.map(de_16_bits), &self.prof.motif, x as f32, y as f32)
    }

    /// Le pixel (`x`, `y`) est-il tout caché ?
    #[inline]
    pub fn cache(&self, x: i64, y: i64) -> bool {
        self.visible(x, y) <= 0.0
    }
}

/// L'opacité `opacite` d'un calque à un pixel dont la part `visible` se voit (`Masque::visible`) : la même si tout se voit (au
/// bit près, comme avant), rien si tout est caché.
#[inline]
pub fn opacite_visible(opacite: u8, visible: f32) -> u8 {
    if visible >= 1.0 {
        opacite
    } else if visible <= 0.0 {
        0
    } else {
        (opacite as f32 * visible).round() as u8
    }
}

/// La profondeur de l'élément du décor dans une composition par le processeur : celle du rendu montré (`decor3d::profondeur_du_rendu`,
/// par le numéro de version de son dessin), si elle est à la taille de la feuille.
pub fn du_decor(cel: &crate::document::Cel, w: usize, h: usize) -> Option<Profondeurs> {
    crate::decor3d::profondeur_du_rendu(cel.version).filter(|p| p.w == w && p.h == h && p.len() == w * h)
}

/// Le masque d'un élément du plan, s'il porte le plan de sa carte et qu'un décor avec sa profondeur est dessous.
#[inline]
pub fn masque<'a>(item: &crate::render::PlanItem, prof: Option<&'a Profondeurs>, w: usize, h: usize) -> Option<Masque<'a>> {
    let plan = item.profondeur?;
    let prof = prof?;
    Some(Masque { prof: prof.as_ref(), w, h, plan })
}

// ------------------------------------------------------------------------------------------- la case du calque

impl crate::app::App {
    /// Coche ou décoche « Toujours devant le décor 3D » sur le calque `li` (une étape d'annulation).
    pub(crate) fn basculer_devant_decor(&mut self, li: usize) {
        let Some(l) = self.layers().get(li) else { return };
        if !crate::espace3d::is_card(l) {
            self.status = "Un dossier ou un calque d'effet n'a pas de carte : le décor 3D ne le cache jamais.".into();
            return;
        }
        let on = !l.devant_decor;
        self.end_stroke();
        let snap = self.snapshot();
        self.layers_mut()[li].devant_decor = on;
        self.commit_named(snap, if on { "Toujours devant le décor 3D" } else { "Caché par le décor 3D selon la distance" });
        self.renderer.invalidate_all();
        self.status = if on {
            "Toujours devant le décor 3D : aucun objet du décor ne cache plus cette carte.".into()
        } else {
            "La carte et le décor 3D se cachent selon leur distance à la caméra.".into()
        };
    }
}
