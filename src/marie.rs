//! « Marie-louise » (Anthony, 2026-10-21) : un passe-partout peint par-dessus tout ce qui est hors de l'intérieur des
//! cases, pour ne montrer que les vignettes (noir d'origine, ou la couleur choisie). Les traits d'annotation qui
//! débordent d'une case sont masqués avec le reste. Le même calcul sert à l'écran (affichage de la planche) et à
//! l'export de la planche.
//!
//! Le calcul est un découpage en bandes : `hors_des_cases` rend des rectangles qui ne se recouvrent pas et dont la
//! réunion est exactement le cadre moins les cases.

use eframe::egui::{pos2, Rect};

/// Noir d'origine.
pub const COULEUR_PAR_DEFAUT: [u8; 3] = [0, 0, 0];

/// Les rectangles qui couvrent `cadre` sauf l'intérieur de `trous` : sans recouvrement, ni surface vide.
pub fn hors_des_cases(cadre: Rect, trous: &[Rect]) -> Vec<Rect> {
    if !(cadre.width() > 0.0 && cadre.height() > 0.0) {
        return Vec::new();
    }
    let clip = |t: &Rect| t.intersect(cadre);
    let trous: Vec<Rect> = trous.iter().map(clip).filter(|t| t.width() > 0.0 && t.height() > 0.0).collect();
    let mut ys: Vec<f32> = vec![cadre.top(), cadre.bottom()];
    for t in &trous {
        ys.push(t.top());
        ys.push(t.bottom());
    }
    ys.sort_by(f32::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    let mut out = Vec::new();
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        if y1 - y0 <= 1e-4 {
            continue;
        }
        // Les trous qui couvrent toute la bande, de gauche à droite.
        let mut xs: Vec<(f32, f32)> = trous.iter().filter(|t| t.top() <= y0 + 1e-4 && t.bottom() >= y1 - 1e-4).map(|t| (t.left(), t.right())).collect();
        xs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut x = cadre.left();
        for (a, b) in xs {
            if a > x + 1e-4 {
                out.push(Rect::from_min_max(pos2(x, y0), pos2(a.min(cadre.right()), y1)));
            }
            x = x.max(b);
        }
        if cadre.right() > x + 1e-4 {
            out.push(Rect::from_min_max(pos2(x, y0), pos2(cadre.right(), y1)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aire(r: &[Rect]) -> f32 {
        r.iter().map(|r| r.width() * r.height()).sum()
    }

    fn r(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
    }

    #[test]
    fn the_mat_is_the_frame_minus_the_boxes() {
        let cadre = r(0.0, 0.0, 100.0, 60.0);
        let trous = [r(10.0, 10.0, 40.0, 30.0), r(50.0, 10.0, 90.0, 30.0), r(10.0, 40.0, 40.0, 55.0)];
        let mat = hors_des_cases(cadre, &trous);
        let attendu = 100.0 * 60.0 - aire(&trous);
        assert!((aire(&mat) - attendu).abs() < 0.01, "aire {} au lieu de {attendu}", aire(&mat));
        for (i, a) in mat.iter().enumerate() {
            for t in &trous {
                let o = a.intersect(*t);
                assert!(!(o.width() > 1e-3 && o.height() > 1e-3), "le passe-partout recouvre une case");
            }
            for b in &mat[i + 1..] {
                let o = a.intersect(*b);
                assert!(!(o.width() > 1e-3 && o.height() > 1e-3), "deux rectangles se recouvrent");
            }
        }
    }

    #[test]
    fn no_box_means_a_full_mat_and_boxes_outside_the_frame_are_ignored() {
        let cadre = r(0.0, 0.0, 50.0, 50.0);
        assert_eq!(hors_des_cases(cadre, &[]), vec![cadre]);
        assert_eq!(hors_des_cases(cadre, &[r(100.0, 100.0, 120.0, 120.0)]), vec![cadre]);
        assert!(hors_des_cases(r(0.0, 0.0, 0.0, 10.0), &[]).is_empty());
    }

    #[test]
    fn a_box_covering_the_frame_leaves_nothing() {
        let cadre = r(0.0, 0.0, 50.0, 50.0);
        assert!(aire(&hors_des_cases(cadre, &[r(-10.0, -10.0, 60.0, 60.0)])) < 1e-3);
    }

    #[test]
    fn overlapping_boxes_are_merged_not_counted_twice() {
        let cadre = r(0.0, 0.0, 100.0, 100.0);
        let trous = [r(10.0, 10.0, 60.0, 60.0), r(40.0, 40.0, 90.0, 90.0)];
        let mat = hors_des_cases(cadre, &trous);
        let union = 50.0 * 50.0 * 2.0 - 20.0 * 20.0;
        assert!((aire(&mat) - (10_000.0 - union)).abs() < 0.01);
    }
}
