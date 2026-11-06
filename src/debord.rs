//! Le trait qui déborde de la vignette, pendant qu'on le trace (Anthony, 2026-10-21).
//!
//! Dans la vue Vignettes, on dessine directement dans la case. Le geste ne s'arrête pas au bord : tant que la
//! main est en bas, on voit **tout le trait**, même par-dessus l'écart, les barres, les bandes de note et les
//! cases voisines. Au lâcher, il est réparti (`horschamp.rs`) et reste visuellement le même : chaque case montre
//! sa part, sa marge se voit sur le papier (partie 111) ; le trait tracé reste peint tant que ces parts ne sont
//! pas rendues (`Tenu`).
//!
//! Comment, sans rien coûter de plus : la vignette courante est le dessin lui-même, peint dans sa case et coupé
//! à son bord. Ici, on ne repeint que **ce qui est dehors**, d'après le tracé que le moteur de dessin garde de
//! toute façon (`StrokeState::path`, la ligne médiane lissée et le rayon de chaque point) :
//! - un trait qui ne quitte pas la case ne produit rien (un seul parcours des points, sans forme) ;
//! - un trait qui sort ne produit que les tronçons qui touchent le dehors, quelques lignes épaisses ;
//! - la pointe (du dernier point posé à la pointe réelle du stylet) en fait partie : le trait reste attaché à la
//!   croix, dehors comme dedans, sans retard ;
//! - les aperçus des formes (ligne, rectangle, ellipse, courbe, polyligne) suivent la même règle.
//!
//! Les formes sont calculées pendant que la vue du dessin est celle de la vignette (`card_canvas`), en pixels
//! d'écran, puis peintes en dernier par la planche (`board_view`), par-dessus tout, avec quatre fenêtres de coupe
//! autour de la case (le dedans est déjà peint, exact, par la vignette elle-même).

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke};

use crate::app::App;

/// Ce qui déborde de la vignette courante pendant le geste : la case à l'écran, et les formes à peindre dehors
/// (chacune avec ce qu'elle couvre).
#[derive(Clone, Debug)]
pub(crate) struct Debord {
    pub thumb: Rect,
    /// Une zone de dessin qui n'est pas un rectangle (l'enveloppe d'un mouvement de caméra, à l'écran) : le trait en est dehors
    /// là où il en sort, même à l'intérieur du rectangle `thumb` qui la borde.
    pub zone: Option<Vec<Pos2>>,
    pub shapes: Vec<(Rect, Shape)>,
}

/// Le trait qui vient d'être réparti, gardé à l'écran le temps que ses parts soient rendues en fond. Il part dès qu'elles sont prêtes, ou si la
/// planche bouge, ou si l'historique change.
#[derive(Clone, Debug)]
pub(crate) struct Tenu {
    pub debord: Debord,
    pub xf: crate::boardui::Xf,
    pub etapes: usize,
    pub debut: std::time::Instant,
}

/// Le plus longtemps qu'un trait réparti reste tenu (un rendu qui n'arrive pas ne le laisse pas à l'écran pour de bon).
pub(crate) const TENU_MAX: std::time::Duration = std::time::Duration::from_secs(4);

// (Le trait qui déborde avait la couleur pâle d'un fantôme tant que la main était en bas ; depuis le 2026-11-05 (« le trait doit garder la
// même couleur partout ») il garde sa couleur et son opacité, dedans comme dehors.)
/// Épaisseur (points d'écran) en deçà de laquelle deux tronçons voisins gardent le même trait.
const SAME_WIDTH: f32 = 0.3;
/// Deux points d'écran plus proches que cela ne font qu'un.
const SAME_POINT: f32 = 1.0;

impl App {
    /// Les formes du trait ou de l'aperçu en cours qui sortent de la case `thumb` (écran), ou rien si aucun geste ne
    /// déborde. À appeler pendant que la vue du dessin est celle de la vignette courante.
    pub(crate) fn debord_collect(&self, thumb: Rect, zone: Option<&[Pos2]>, shift: bool) -> Option<Debord> {
        let dedans = |p: Pos2| match zone {
            Some(z) => crate::vignettecam::dans_polygone(z, p),
            None => thumb.contains(p),
        };
        let mut shapes: Vec<(Rect, Shape)> = Vec::new();
        if let Some(s) = &self.stroke {
            if let Some((rgb, alpha)) = s.tip_paint() {
                let zoom = self.layer_zoom();
                let color = Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], (alpha.clamp(0.0, 1.0) * 255.0).round() as u8);

                let mut points: Vec<(Pos2, f32)> = Vec::with_capacity(s.path.len() + 2);
                points.extend(s.path.iter().map(|p| (self.layer_to_screen(p.pos()), p.r * zoom)));
                if let Some((last, raw, r)) = s.tip() {
                    let w = r * zoom;
                    points.push((self.layer_to_screen(last), w));
                    points.push((self.layer_to_screen(raw), w));
                }
                outside_runs(&points, &dedans, color, &mut shapes);
            }
        }
        for (pts, stroke) in self.shape_preview_lines(shift) {
            if pts.len() >= 2 && !pts.iter().all(|p| dedans(*p)) {
                let bounds = bounds_of(&pts).expand(stroke.width);
                shapes.push((bounds, Shape::line(pts, Stroke::new(stroke.width, stroke.color))));
            }
        }
        (!shapes.is_empty()).then_some(Debord { thumb, zone: zone.map(<[Pos2]>::to_vec), shapes })
    }
}

fn bounds_of(points: &[Pos2]) -> Rect {
    points.iter().fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(*p, *p)))
}

/// Les tronçons de la ligne `points` (écran, avec le rayon de chaque point) qui ne sont pas entièrement dans
/// `thumb`, en lignes d'épaisseur presque égale (la pression la fait varier) à bouts arrondis. Les tronçons
/// dedans, déjà peints par la vignette, sont sautés.
fn outside_runs(points: &[(Pos2, f32)], dedans: &dyn Fn(Pos2) -> bool, color: Color32, out: &mut Vec<(Rect, Shape)>) {
    let mut run: Vec<Pos2> = Vec::new();
    let mut run_w = 0.0_f32;
    let flush = |run: &mut Vec<Pos2>, w: f32, out: &mut Vec<(Rect, Shape)>| {
        if run.len() >= 2 {
            // Plus fin qu'un pixel d'écran : un pixel de large, aussi léger que le montrent la case et sa marge (au lâcher, rien ne change).
            let (w, color) = if w < 0.5 { (0.5, color.gamma_multiply(1.0 - egui::ecolor::gamma_from_linear(1.0 - w * 2.0))) } else { (w, color) };
            let bounds = bounds_of(run).expand(w);
            // Bouts arrondis : un disque à chaque bout du tronçon (la ligne d'egui est coupée net).
            for end in [run[0], run[run.len() - 1]] {
                out.push((Rect::from_center_size(end, egui::Vec2::splat(w * 2.0)), Shape::circle_filled(end, w, color)));
            }
            out.push((bounds, Shape::line(std::mem::take(run), Stroke::new(w * 2.0, color))));
        }
        run.clear();
    };
    let mut prev: Option<(Pos2, f32)> = None;
    for &(p, w) in points {
        let Some((a, aw)) = prev else {
            prev = Some((p, w));
            continue;
        };
        if a.distance(p) < SAME_POINT {
            continue;
        }
        prev = Some((p, w));
        if dedans(a) && dedans(p) {
            flush(&mut run, run_w, out);
            continue;
        }
        if run.is_empty() {
            run.push(a);
            run_w = (aw + w) * 0.5;
        } else if (w - run_w).abs() > SAME_WIDTH {
            // L'épaisseur change : un nouveau tronçon, qui repart du dernier point.
            let last = *run.last().expect("tronçon non vide");
            flush(&mut run, run_w, out);
            run.push(last);
            run_w = w;
        }
        run.push(p);
    }
    flush(&mut run, run_w, out);
}

/// Peint le débord par-dessus tout : seulement hors de la case, dans les quatre fenêtres qui l'entourent (haut, bas,
/// gauche, droite), et dans la vue `view`.
pub(crate) fn paint(painter: &Painter, view: Rect, d: &Debord) {
    // Une zone de dessin à plusieurs côtés : tout ce qui sort se peint dans la vue (le dedans est du même trait, plein : le trait pâle
    // par-dessus ne s'y voit pas).
    if d.zone.is_some() {
        let clipped = painter.with_clip_rect(view);
        for (bounds, shape) in &d.shapes {
            if bounds.intersects(view) {
                clipped.add(shape.clone());
            }
        }
        return;
    }
    let t = d.thumb;
    let regions = [
        Rect::from_min_max(view.min, egui::pos2(view.max.x, t.top())),
        Rect::from_min_max(egui::pos2(view.min.x, t.bottom()), view.max),
        Rect::from_min_max(egui::pos2(view.min.x, t.top()), egui::pos2(t.left(), t.bottom())),
        Rect::from_min_max(egui::pos2(t.right(), t.top()), egui::pos2(view.max.x, t.bottom())),
    ];
    for region in regions {
        let region = region.intersect(view);
        if region.width() <= 0.0 || region.height() <= 0.0 {
            continue;
        }
        let clipped = painter.with_clip_rect(region);
        for (bounds, shape) in &d.shapes {
            if bounds.intersects(region) {
                clipped.add(shape.clone());
            }
        }
    }
}
