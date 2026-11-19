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

/// La profondeur d'un rendu du décor : l'inverse de la distance à l'œil à chaque pixel de la feuille, ligne par ligne (0 : rien).
pub type Profondeurs = std::sync::Arc<Vec<f32>>;

/// Ce qu'il faut à la composition par le processeur pour cacher une carte derrière le décor : la profondeur du décor (une feuille
/// de `w` × `h`) et le plan de la carte.
#[derive(Clone, Copy)]
pub struct Masque<'a> {
    pub prof: &'a [f32],
    pub w: usize,
    pub h: usize,
    pub plan: PlanCarte,
}

impl Masque<'_> {
    /// Le pixel (`x`, `y`) du monde (la feuille, ou la toile autour) est-il caché ? Hors de la feuille, le décor n'est pas : non.
    #[inline]
    pub fn cache(&self, x: i64, y: i64) -> bool {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return false;
        }
        cache(&self.plan, self.prof[y as usize * self.w + x as usize], x as f32 + 0.5, y as f32 + 0.5)
    }
}

/// La profondeur de l'élément du décor dans une composition par le processeur : celle du rendu montré (`decor3d::profondeur_du_rendu`,
/// par le numéro de version de son dessin), si elle est à la taille de la feuille.
pub fn du_decor(cel: &crate::document::Cel, w: usize, h: usize) -> Option<Profondeurs> {
    crate::decor3d::profondeur_du_rendu(cel.version).filter(|p| p.len() == w * h)
}

/// Le masque d'un élément du plan, s'il porte le plan de sa carte et qu'un décor avec sa profondeur est dessous.
#[inline]
pub fn masque<'a>(item: &crate::render::PlanItem, prof: Option<&'a Profondeurs>, w: usize, h: usize) -> Option<Masque<'a>> {
    let plan = item.profondeur?;
    let prof = prof?;
    Some(Masque { prof: prof.as_slice(), w, h, plan })
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
