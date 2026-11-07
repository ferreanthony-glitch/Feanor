//! Où se pose le numéro d'un plan sur la planche des Vignettes (Anthony, 2026-11-06 : « ils parasitent souvent la lisibilité »).
//!
//! Le numéro vit **dans la marge**, au-dessus de la case A de son plan, à une place fixe : le coin haut-gauche de la case. Il ne se pose
//! **jamais** sur une image, ni sur un cadre de caméra (A, B…), ni sur la zone qu'une caméra balaie (l'enveloppe d'un plan à mouvement,
//! le sien ou celui d'un voisin), ni sur le texte sous les cases, ni sur un autre numéro. Quand la marge est prise (un mouvement voisin
//! s'étend dessus, ou un petit zoom la réduit), il cherche une place libre **dans un ordre toujours le même** (rien ne saute au hasard) :
//!
//! 1. la marge au-dessus de A, de son coin gauche vers la droite, puis vers la gauche (dans l'écart entre deux cases) ;
//! 2. la même chose une rangée plus haut, jusqu'à trois ; puis, si la zone balayée du plan monte au-dessus de A, au-dessus de cette zone ;
//! 3. à chaque fois d'abord à sa taille, puis un peu plus petit (jamais sous 5,5 points) : aux petits zooms le numéro s'amincit pour
//!    tenir dans la marge plutôt que de passer sur une image.
//!
//! Rien ne tient, même tout petit : il n'est pas peint (il ne couvre jamais une image).
//!
//! Tout se calcule à l'écran, pour les cases qui s'y voient, quand la planche se peint : rien n'est gardé, rien n'est écrit dans le projet.

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::board::BoardLayout;
use crate::boardui::Xf;

/// Les tailles essayées, de la taille voulue à la plus petite (parts de la taille voulue).
pub const ECHELLES: [f32; 6] = [1.0, 0.85, 0.7, 0.58, 0.46, 0.36];
/// Le plus petit corps d'un numéro peint, en points d'écran.
pub const CORPS_MIN: f32 = 5.5;
/// Jusqu'où le numéro remonte, en rangées de marge.
const RANGEES: usize = 3;
/// De combien un numéro peut frôler un obstacle sans compter qu'il le touche (points d'écran).
const FRAIS: f32 = 0.25;

/// Ce sur quoi le numéro ne se pose pas : une boîte et, quand l'obstacle n'est pas un rectangle, le polygone convexe qu'elle contient.
#[derive(Clone, Debug)]
pub struct Obstacle {
    pub boite: Rect,
    pub poly: Option<Vec<Pos2>>,
}

impl Obstacle {
    pub fn rect(r: Rect) -> Obstacle {
        Obstacle { boite: r, poly: None }
    }

    pub fn polygone(p: Vec<Pos2>) -> Obstacle {
        let boite = p.iter().fold(Rect::NOTHING, |r, q| r.union(Rect::from_min_max(*q, *q)));
        Obstacle { boite, poly: Some(p) }
    }

    /// Le rectangle `r` recouvre-t-il l'obstacle (d'une épaisseur de plus d'un frais) ?
    pub fn touche(&self, r: Rect) -> bool {
        let b = self.boite;
        if !(r.min.x < b.max.x - FRAIS && b.min.x < r.max.x - FRAIS && r.min.y < b.max.y - FRAIS && b.min.y < r.max.y - FRAIS) {
            return false;
        }
        let Some(poly) = &self.poly else { return true };
        if poly.len() < 3 {
            return true;
        }
        // Axe séparateur : les côtés du polygone (ceux du rectangle sont déjà vus par les boîtes).
        let coins = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
        for i in 0..poly.len() {
            let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
            let axe = vec2(q.y - p.y, p.x - q.x);
            let n = axe.length();
            if n < 1e-9 {
                continue;
            }
            let axe = axe / n;
            let proj = |s: &mut dyn Iterator<Item = Pos2>| s.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v.to_vec2().dot(axe)), hi.max(v.to_vec2().dot(axe))));
            let (a0, a1) = proj(&mut poly.iter().copied());
            let (b0, b1) = proj(&mut coins.iter().copied());
            if a1 <= b0 + FRAIS || b1 <= a0 + FRAIS {
                return false;
            }
        }
        true
    }
}

/// Les obstacles de la planche à l'écran, et les numéros déjà posés.
#[derive(Clone, Debug, Default)]
pub struct Marges {
    obstacles: Vec<Obstacle>,
    places: Vec<Rect>,
}

impl Marges {
    /// Les obstacles de la planche : les images des cases, le texte sous elles, l'enveloppe de chaque mouvement de caméra (ses cadres y
    /// sont, avec tout ce que la caméra balaie). Seuls ceux qui touchent la vue, élargie : les autres ne gênent aucun numéro qu'on voit.
    pub fn pour(layout: &BoardLayout, xf: Xf, view: Rect) -> Marges {
        let zone = view.expand(200.0);
        let mut obstacles = Vec::with_capacity(layout.cards.len() * 2 + layout.moves.len());
        // (Les cases annulées, barrées, sont des cases comme les autres : aucun numéro dessus, `planannule.rs`.)
        for c in layout.cards.iter().chain(&layout.annulees) {
            for r in [Some(c.thumb), c.a_du_texte().then_some(c.note)].into_iter().flatten() {
                let r = xf.rect(r);
                if r.intersects(zone) {
                    obstacles.push(Obstacle::rect(r));
                }
            }
        }
        for m in &layout.moves {
            let o = Obstacle::polygone(m.enveloppe.iter().map(|p| xf.to_screen(*p)).collect());
            if o.boite.intersects(zone) {
                obstacles.push(o);
            }
        }
        Marges { obstacles, places: Vec::new() }
    }

    /// Une place libre pour une étiquette qui fait `mesure(échelle)` de large et de haut à chaque échelle de `ECHELLES` (la plus grande
    /// d'abord), au-dessus de la case `thumb` : la première qui ne recouvre rien, et l'échelle à laquelle elle tient. Elle est retenue :
    /// le numéro suivant ne la recouvre pas. Rien, si aucune ne tient.
    pub fn choisir(&mut self, thumb: Rect, haut: f32, mesure: impl Fn(f32) -> Vec2) -> Option<(Rect, f32)> {
        let grand = mesure(1.0);
        let haut = haut.min(thumb.top());
        // Seuls comptent les obstacles proches du haut de la case (trois rangées de marge, une étiquette de large de chaque côté).
        let zone = Rect::from_min_max(pos2(thumb.left() - grand.x - 4.0, haut - (RANGEES as f32 + 1.0) * (grand.y + 2.0)), pos2(thumb.right() + grand.x + 4.0, thumb.top() + 1.0));
        let proches: Vec<&Obstacle> = self.obstacles.iter().filter(|o| o.boite.intersects(zone)).collect();
        let places: Vec<Rect> = self.places.iter().copied().filter(|p| p.intersects(zone)).collect();
        let libre = |r: Rect| !proches.iter().any(|o| o.touche(r)) && !places.iter().any(|p| p.shrink(FRAIS).intersects(r.shrink(FRAIS)));
        for &sc in &ECHELLES {
            let t = mesure(sc);
            if !(t.x > 0.0 && t.y > 0.0) {
                continue;
            }
            // Au-dessus de A d'abord ; si le mouvement du plan monte au-dessus de A (sa zone prend la marge de A), au-dessus de toute la zone du plan,
            // dans la marge de sa rangée : le numéro reste au coin haut-gauche de ce que le plan occupe.
            let bases = if haut < thumb.top() - 1.0 { vec![thumb.top(), haut] } else { vec![thumb.top()] };
            for (&base, rangee) in bases.iter().flat_map(|b| (0..RANGEES).map(move |r| (b, r))) {
                let haut = base - 2.0 - t.y - rangee as f32 * (t.y + 1.0);
                // Du coin gauche de la case vers la droite (le numéro reste sur la largeur de la case), puis vers la gauche, dans l'écart.
                let pas = (t.x * 0.25).max(2.0);
                let droite = ((thumb.width() - t.x) / pas).floor().max(0.0) as usize;
                let gauche = (t.x / pas).ceil() as usize;
                let essais = (0..=droite).map(|i| thumb.left() + i as f32 * pas).chain((1..=gauche).map(|j| thumb.left() - j as f32 * pas));
                for x in essais {
                    let r = Rect::from_min_size(pos2(x, haut), t);
                    if libre(r) {
                        self.places.push(r);
                        return Some((r, sc));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
#[path = "numerosmarge_tests.rs"]
mod tests;
