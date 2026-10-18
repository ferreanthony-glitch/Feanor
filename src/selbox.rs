//! Boîte de sélection façon Storyboard Pro (Anthony, 2026-10-18 : « revoir le système de
//! transformation après sélection pour singer celui de Storyboard Pro ») : un cadre aux huit
//! poignées carrées (quatre coins, quatre milieux de côtés) et un pivot rond au centre.
//!
//! - glisser dedans : déplacer ;
//! - un coin : mettre à l'échelle dans les deux sens ; le milieu d'un côté : dans un seul sens ;
//! - juste à l'extérieur d'un coin : tourner autour du pivot ;
//! - un bord, entre les poignées : incliner (cisaillement le long de ce bord) ;
//! - le pivot : le tirer ailleurs (centre de la rotation).
//!
//! Tout est ici de la géométrie pure, sans interface : la boîte (`SelFrame`), ses parties sous le
//! pointeur (`hit_test`, en pixels d'écran, donc la même à tout zoom), la transformation qu'un
//! geste produit (`gesture`) et les valeurs chiffrées de la boîte (`BoxParams`). La transformation
//! d'un geste est une `Affine` (échelle, rotation, cisaillement, déplacement) : la feuille
//! flottante (`flottante.rs`) la pose au zéro latence, comme un déplacement.
//!
//! Choix de touches, à vérifier avec le vrai Storyboard Pro (rien n'est tiré de son code) :
//! - Maj sur un coin ou un côté : proportions gardées ; Maj en déplaçant : un seul axe ; Maj en
//!   tournant : par pas de 15° ;
//! - Ctrl en tirant une poignée ou un bord : autour du pivot au lieu du côté opposé (Alt est pris
//!   par la pipette) ;
//! - le réglage « Garder les proportions » inverse l'effet de Maj sur les poignées.

use eframe::egui::{pos2, vec2, Pos2, Rect};

use crate::vector::Affine;

/// Rayon de prise d'une poignée carrée (coin ou milieu de côté), en pixels d'écran.
pub const GRAB_HANDLE: f32 = 9.0;
/// Rayon de prise du pivot, en pixels d'écran.
pub const GRAB_PIVOT: f32 = 8.0;
/// Demi-épaisseur de la bande d'un bord (incliner), de part et d'autre du trait, en pixels d'écran.
pub const GRAB_SKEW: f32 = 5.0;
/// Distance d'un coin jusqu'où l'extérieur fait tourner, en pixels d'écran.
pub const GRAB_ROTATE: f32 = 26.0;
/// Pas de la rotation avec Maj, en degrés.
pub const ROTATE_STEP: f32 = 15.0;
/// Inclinaison la plus forte (degrés) : au-delà, le dessin est écrasé.
pub const SKEW_MAX: f32 = 80.0;

/// Une partie de la boîte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// L'intérieur : déplacer.
    Move,
    /// Coins dans l'ordre : haut-gauche, haut-droite, bas-droite, bas-gauche.
    Corner(usize),
    /// Milieux des côtés : haut, droite, bas, gauche (mise à l'échelle dans un seul sens).
    Edge(usize),
    /// Juste à l'extérieur du coin `k` : tourner.
    Rotate(usize),
    /// Le bord `k` (haut, droite, bas, gauche), entre ses poignées : incliner.
    Skew(usize),
    /// Le pivot.
    Pivot,
}

impl Handle {
    /// Le nom, en français, de ce que le geste fait (nom de l'étape d'annulation).
    pub fn step_name(self) -> &'static str {
        match self {
            Handle::Move | Handle::Pivot => "Déplacer",
            Handle::Corner(_) | Handle::Edge(_) => "Mettre à l'échelle",
            Handle::Rotate(_) => "Tourner",
            Handle::Skew(_) => "Incliner",
        }
    }
}

/// Touches et réglage d'un geste.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    /// Autour du pivot au lieu du côté opposé.
    pub ctrl: bool,
    /// Réglage « Garder les proportions » : inverse l'effet de Maj sur les poignées.
    pub keep: bool,
}

impl Mods {
    /// Les proportions sont-elles gardées ?
    pub fn proportional(self) -> bool {
        self.keep != self.shift
    }
}

/// La boîte : le rectangle d'origine `rect0` de la sélection, la transformation `m` qui l'amène où
/// on la voit (rotation et inclinaison comprises : la boîte reste « tournée » après un geste) et
/// le pivot, dans les coordonnées de `rect0` (le centre s'il n'est pas déplacé).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelFrame {
    pub rect0: Rect,
    pub m: Affine,
    pub pivot: Option<Pos2>,
}

impl SelFrame {
    pub fn new(rect0: Rect) -> SelFrame {
        SelFrame { rect0, m: Affine::IDENTITY, pivot: None }
    }

    /// Les coins de `rect0` : haut-gauche, haut-droite, bas-droite, bas-gauche.
    pub fn corners_local(&self) -> [Pos2; 4] {
        let r = self.rect0;
        [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()]
    }

    /// Les coins de la boîte, tels qu'on les voit.
    pub fn corners(&self) -> [Pos2; 4] {
        self.corners_local().map(|p| self.m.apply(p))
    }

    /// Le milieu du côté `k` de `rect0` : haut, droite, bas, gauche.
    pub fn mid_local(&self, k: usize) -> Pos2 {
        let c = self.corners_local();
        let (a, b) = (c[k % 4], c[(k + 1) % 4]);
        pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
    }

    /// Les milieux des côtés, tels qu'on les voit.
    #[cfg(test)]
    pub fn mids(&self) -> [Pos2; 4] {
        [0, 1, 2, 3].map(|k| self.m.apply(self.mid_local(k)))
    }

    pub fn pivot_local(&self) -> Pos2 {
        self.pivot.unwrap_or_else(|| self.rect0.center())
    }

    /// Le pivot, tel qu'on le voit.
    pub fn pivot_world(&self) -> Pos2 {
        self.m.apply(self.pivot_local())
    }

    /// Un point vu, ramené dans les coordonnées de `rect0`.
    pub fn to_local(&self, p: Pos2) -> Option<Pos2> {
        self.m.inverse().map(|inv| inv.apply(p))
    }

    /// La même boîte après la transformation `d` (en coordonnées vues). Le pivot suit.
    pub fn then(&self, d: &Affine) -> SelFrame {
        SelFrame { m: self.m.then(d), ..*self }
    }

    /// La transformation `d` (en coordonnées vues) qui amène cette boîte sur `target` : `target.m`
    /// vaut `self.m.then(d)`. Rien si la boîte est écrasée.
    pub fn delta_to(&self, target: &SelFrame) -> Option<Affine> {
        Some(self.m.inverse()?.then(&target.m))
    }

    /// Les valeurs chiffrées de la boîte.
    pub fn params(&self) -> BoxParams {
        let m = self.m;
        let pivot = self.pivot_world();
        let sx = m.a.hypot(m.c);
        let (u, angle) = if sx > 1e-6 { ((m.a / sx, m.c / sx), m.c.atan2(m.a).to_degrees()) } else { ((1.0, 0.0), 0.0) };
        // La deuxième colonne de `m`, en repère tourné : `k` le long de l'axe des x, `h` à angle droit.
        let k = m.b * u.0 + m.d * u.1;
        let h = -m.b * u.1 + m.d * u.0;
        let skew = if h.abs() > 1e-6 { (k / h).atan().to_degrees() } else { 0.0 };
        BoxParams { x: pivot.x, y: pivot.y, width: sx * self.rect0.width(), height: h.abs() * self.rect0.height(), angle, skew }
    }

    /// La boîte qui a les valeurs `p` : le pivot est à (x, y), la boîte a la largeur et la hauteur
    /// voulues, tournée de `angle` et inclinée de `skew` (degrés). Un miroir (hauteur négative
    /// dans la décomposition) est gardé. Rien si `rect0` est vide.
    pub fn with_params(&self, p: &BoxParams) -> Option<SelFrame> {
        let (w0, h0) = (self.rect0.width(), self.rect0.height());
        if w0 < 1e-3 || h0 < 1e-3 {
            return None;
        }
        let m = self.m;
        let sx = m.a.hypot(m.c);
        let (ux, uy) = if sx > 1e-6 { (m.a / sx, m.c / sx) } else { (1.0, 0.0) };
        let flipped = (-m.b * uy + m.d * ux) < 0.0;
        let sx = (p.width / w0).max(1e-3);
        let h = (p.height / h0).max(1e-3) * if flipped { -1.0 } else { 1.0 };
        let skew = p.skew.clamp(-SKEW_MAX, SKEW_MAX).to_radians();
        let k = h * skew.tan();
        let (s, c) = p.angle.to_radians().sin_cos();
        let (a, cc) = (c * sx, s * sx);
        let (b, d) = (c * k - s * h, s * k + c * h);
        let pl = self.pivot_local();
        let new = Affine { a, b, c: cc, d, tx: p.x - (a * pl.x + b * pl.y), ty: p.y - (cc * pl.x + d * pl.y) };
        Some(SelFrame { m: new, ..*self })
    }
}

/// Valeurs chiffrées de la boîte (champs du panneau Réglages) : position du pivot (x, y), largeur
/// et hauteur de la boîte (le long de ses axes), angle et inclinaison, en degrés.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxParams {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub angle: f32,
    pub skew: f32,
}

/// Distance d'un point au segment [a, b].
pub fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-9 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Les rayons de prise, en pixels d'écran.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grabs {
    pub handle: f32,
    pub pivot: f32,
    pub skew: f32,
    pub rotate: f32,
}

impl Default for Grabs {
    fn default() -> Self {
        Grabs { handle: GRAB_HANDLE, pivot: GRAB_PIVOT, skew: GRAB_SKEW, rotate: GRAB_ROTATE }
    }
}

/// La partie de la boîte sous le pointeur. `quad` : les coins de la boîte à l'écran (haut-gauche,
/// haut-droite, bas-droite, bas-gauche), `pivot` : le pivot à l'écran, `p` : le pointeur. Les
/// tailles de prise sont en pixels d'écran, donc les mêmes à tout zoom ; sur une petite boîte
/// (moins de 40 px de côté) elles rétrécissent pour que chaque partie reste atteignable.
/// Ordre : pivot, coins, milieux des côtés, bords (incliner), intérieur (déplacer), puis
/// l'extérieur près d'un coin (tourner).
pub fn hit_test(quad: [Pos2; 4], pivot: Pos2, p: Pos2, g: &Grabs) -> Option<Handle> {
    let side = |k: usize| (quad[(k + 1) % 4] - quad[k]).length();
    let small = side(0).min(side(1));
    let scale = (small / 40.0).clamp(0.45, 1.0);
    let (rh, rp, rs) = (g.handle * scale, g.pivot * scale, g.skew * scale.max(0.6));
    if p.distance(pivot) <= rp {
        return Some(Handle::Pivot);
    }
    let nearest = |points: [Pos2; 4]| {
        (0..4).map(|k| (k, points[k].distance(p))).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or((0, f32::MAX))
    };
    let (k, d) = nearest(quad);
    if d <= rh {
        return Some(Handle::Corner(k));
    }
    let mids = [0, 1, 2, 3].map(|k| quad[k] + (quad[(k + 1) % 4] - quad[k]) * 0.5);
    let (k, d) = nearest(mids);
    if d <= rh {
        return Some(Handle::Edge(k));
    }
    let edge = (0..4)
        .map(|k| (k, segment_distance(p, quad[k], quad[(k + 1) % 4])))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, f32::MAX));
    if edge.1 <= rs {
        return Some(Handle::Skew(edge.0));
    }
    if crate::vector::point_in_polygon(p, &quad) {
        return Some(Handle::Move);
    }
    let (k, d) = nearest(quad);
    (d <= g.rotate).then_some(Handle::Rotate(k))
}

/// Rapport entre deux longueurs, sans division par un nombre nul ni facteur trop petit.
fn ratio(num: f32, den: f32) -> f32 {
    if den.abs() < 1e-3 {
        return 1.0;
    }
    let r = num / den;
    if r.abs() < 0.01 {
        0.01f32.copysign(r)
    } else {
        r
    }
}

/// `l` (une transformation dans les coordonnées de `rect0`) exprimée en coordonnées vues.
fn in_view(frame: &SelFrame, l: &Affine) -> Affine {
    match frame.m.inverse() {
        Some(inv) => inv.then(l).then(&frame.m),
        None => Affine::IDENTITY,
    }
}

/// La transformation (en coordonnées vues) qu'un geste produit : `handle` pris en `start`, le
/// pointeur est maintenant en `p`. La boîte de départ est `frame` ; après le geste elle devient
/// `frame.then(&gesture(..))`. Le pivot n'est pas un geste de transformation (`move_pivot`).
pub fn gesture(handle: Handle, frame: &SelFrame, start: Pos2, p: Pos2, mods: Mods) -> Affine {
    match handle {
        Handle::Move => {
            let mut d = p - start;
            if mods.shift {
                if d.x.abs() > d.y.abs() {
                    d.y = 0.0;
                } else {
                    d.x = 0.0;
                }
            }
            Affine::translate(d)
        }
        Handle::Rotate(_) => {
            let c = frame.pivot_world();
            let mut a = (p - c).angle() - (start - c).angle();
            if mods.shift {
                let step = ROTATE_STEP.to_radians();
                a = (a / step).round() * step;
            }
            Affine::rotate_about(c, a)
        }
        Handle::Corner(k) => {
            let (Some(l), Some(l0)) = (frame.to_local(p), frame.to_local(start)) else { return Affine::IDENTITY };
            let c = frame.corners_local();
            let corner = c[k % 4];
            let anchor = if mods.ctrl { frame.pivot_local() } else { c[(k + 2) % 4] };
            let target = corner + (l - l0);
            let (mut sx, mut sy) = (ratio(target.x - anchor.x, corner.x - anchor.x), ratio(target.y - anchor.y, corner.y - anchor.y));
            if mods.proportional() {
                let d = corner - anchor;
                let f = (target - anchor).dot(d) / d.length_sq().max(1e-6);
                let f = if f.abs() < 0.01 { 0.01f32.copysign(f) } else { f };
                (sx, sy) = (f, f);
            }
            in_view(frame, &Affine::scale_about(anchor, sx, sy))
        }
        Handle::Edge(k) => {
            let (Some(l), Some(l0)) = (frame.to_local(p), frame.to_local(start)) else { return Affine::IDENTITY };
            let mid = frame.mid_local(k);
            let anchor = if mods.ctrl { frame.pivot_local() } else { frame.mid_local(k + 2) };
            let target = mid + (l - l0);
            let (sx, sy) = if k % 2 == 1 {
                let s = ratio(target.x - anchor.x, mid.x - anchor.x);
                (s, if mods.proportional() { s } else { 1.0 })
            } else {
                let s = ratio(target.y - anchor.y, mid.y - anchor.y);
                (if mods.proportional() { s } else { 1.0 }, s)
            };
            in_view(frame, &Affine::scale_about(anchor, sx, sy))
        }
        Handle::Skew(k) => {
            let (Some(l), Some(l0)) = (frame.to_local(p), frame.to_local(start)) else { return Affine::IDENTITY };
            let r = frame.rect0;
            let limit = SKEW_MAX.to_radians().tan();
            let pl = frame.pivot_local();
            let shear = if k % 2 == 0 {
                // Haut ou bas : les x glissent, proportionnellement à la distance au bord opposé.
                let (edge, other) = if k == 0 { (r.min.y, r.max.y) } else { (r.max.y, r.min.y) };
                let anchor = if mods.ctrl { pl.y } else { other };
                let den = edge - anchor;
                let f = if den.abs() < 1e-3 { 0.0 } else { ((l.x - l0.x) / den).clamp(-limit, limit) };
                Affine { a: 1.0, b: f, c: 0.0, d: 1.0, tx: -f * anchor, ty: 0.0 }
            } else {
                let (edge, other) = if k == 3 { (r.min.x, r.max.x) } else { (r.max.x, r.min.x) };
                let anchor = if mods.ctrl { pl.x } else { other };
                let den = edge - anchor;
                let f = if den.abs() < 1e-3 { 0.0 } else { ((l.y - l0.y) / den).clamp(-limit, limit) };
                Affine { a: 1.0, b: 0.0, c: f, d: 1.0, tx: 0.0, ty: -f * anchor }
            };
            in_view(frame, &shear)
        }
        Handle::Pivot => Affine::IDENTITY,
    }
}

/// Le pivot tiré de `start` à `p` : sa nouvelle place, dans les coordonnées de `rect0` ; `None`
/// quand il retombe près du centre (à `snap` pixels vus du centre : il s'y remet, c'est le pivot
/// par défaut). `px_per_unit` : pixels d'écran par pixel du dessin.
pub fn move_pivot(frame: &SelFrame, start: Pos2, p: Pos2, snap: f32, px_per_unit: f32) -> Option<Pos2> {
    let now = frame.pivot_world() + (p - start);
    let local = frame.to_local(now)?;
    let centre = frame.m.apply(frame.rect0.center());
    if now.distance(centre) * px_per_unit <= snap {
        return None;
    }
    Some(local)
}

/// Direction, à l'écran, d'une poignée depuis le centre de la boîte : pour choisir le curseur de
/// redimensionnement (diagonale, horizontale, verticale) même quand la boîte est tournée.
pub fn handle_direction(quad: [Pos2; 4], handle: Handle) -> Option<eframe::egui::Vec2> {
    let centre = pos2((quad[0].x + quad[2].x) * 0.5, (quad[0].y + quad[2].y) * 0.5);
    let at = match handle {
        Handle::Corner(k) | Handle::Rotate(k) => quad[k % 4],
        Handle::Edge(k) | Handle::Skew(k) => quad[k % 4] + (quad[(k + 1) % 4] - quad[k % 4]) * 0.5,
        _ => return None,
    };
    let v = at - centre;
    (v.length() > 1e-3).then_some(v)
}

/// Les huit directions des curseurs de redimensionnement : 0 horizontal, 1 diagonale « \ » (haut-
/// gauche / bas-droite), 2 vertical, 3 diagonale « / ». Une direction vue (y vers le bas).
pub fn resize_cursor_axis(dir: eframe::egui::Vec2) -> usize {
    let deg = dir.y.atan2(dir.x).to_degrees().rem_euclid(180.0);
    // 0° et 180° : horizontal ; 45° : « \ » (vers le bas à droite) ; 90° : vertical ; 135° : « / ».
    match ((deg + 22.5) / 45.0).floor() as usize % 4 {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 3,
    }
}

/// Un carré de côté `side` centré en `c` (poignée).
pub fn handle_rect(c: Pos2, side: f32) -> Rect {
    Rect::from_center_size(c, vec2(side, side))
}
