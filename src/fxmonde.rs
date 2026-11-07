//! Les effets au-delà de la feuille (partie 121). La feuille se calcule comme toujours (`fx::apply_with`), au pixel près. Ce qui la dépasse se calcule ici, **dans les
//! coordonnées du monde** : un point du monde reçoit de chaque effet ce qu'il recevrait d'une feuille assez grande pour le contenir, et les réglages qui parlent de la
//! feuille (centre, pourcentages) restent ceux de la vraie feuille. Le trait ne se casse donc pas à son bord.
//!
//! - **Rien de plein** : le calcul ne couvre jamais le rectangle du dessin. Les effets qui vont lire leurs voisins (flou, lueur, ombre, mosaïque…) passent par **zones**
//!   (`BLOC` tuiles de côté, plus leur portée de chaque côté) où il y a de l'encre ; ceux qui déplacent le dessin (onde, secousse, tourbillon…) par tuiles, seulement celles
//!   où quelque chose peut arriver. Un trait long d'une feuille ne coûte que ce qu'il touche, un point perdu très loin ne coûte qu'une zone.
//! - **Côté feuille** : une tuile de la feuille assez près de ce qui dépasse pour le lire (`Voisines`) prend le résultat d'ici, qui voit des deux côtés ; le reste de la
//!   feuille ne bouge pas. Un dessin qui ne dépasse pas ne passe pas par ici : il ne paie qu'un test.

use eframe::egui::Color32;

use crate::fx::{EffectKind, FxStep, Repere};
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS};

/// Côté d'une zone, en tuiles (512 pixels). Plus grand, chaque zone tiendrait plus de mémoire le temps du calcul ; plus petit, il y en aurait plus à préparer.
/// Assez petit pour que quelques zones tenues ensemble ne pèsent que quelques dizaines de mégaoctets.
pub(crate) const BLOC: i32 = 8;
/// Zones calculées à la fois : au-delà, la mémoire de travail monte sans gain sensible.
const ZONES_A_LA_FOIS: usize = 8;

/// Un rectangle de pixels du monde, bornes exclues à droite et en bas.
#[derive(Clone, Copy, Debug)]
struct Pave {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Pave {
    fn de_tuile(tx: i32, ty: i32) -> Pave {
        let t = TILE as f32;
        Pave { x0: tx as f32 * t, y0: ty as f32 * t, x1: (tx + 1) as f32 * t, y1: (ty + 1) as f32 * t }
    }

    fn grossi(self, m: f32) -> Pave {
        Pave { x0: self.x0 - m, y0: self.y0 - m, x1: self.x1 + m, y1: self.y1 + m }
    }

    fn coins(self) -> [(f32, f32); 4] {
        [(self.x0, self.y0), (self.x1, self.y0), (self.x1, self.y1), (self.x0, self.y1)]
    }

    fn touche(self, o: Pave) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }

}

/// Les tuiles de la feuille à moins de `r` tuiles de ce qui la dépasse.
pub(crate) struct Voisines {
    cols: i32,
    rows: i32,
    grille: Vec<bool>,
}

impl Voisines {
    pub(crate) fn autour(base: &Raster, debord: &[(i32, i32)], r: i32) -> Voisines {
        let (cols, rows) = (base.tiles_x as i32, base.tiles_y() as i32);
        // (Une différence en deux dimensions, puis sa somme : le coût ne dépend ni de `r` ni du nombre de tuiles.)
        let (w, h) = (cols as usize + 1, rows as usize + 1);
        let mut d = vec![0i32; w * h];
        for &(tx, ty) in debord {
            let (x0, x1) = ((tx.saturating_sub(r)).max(0), (tx.saturating_add(r).saturating_add(1)).min(cols));
            let (y0, y1) = ((ty.saturating_sub(r)).max(0), (ty.saturating_add(r).saturating_add(1)).min(rows));
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            d[y0 as usize * w + x0 as usize] += 1;
            d[y0 as usize * w + x1 as usize] -= 1;
            d[y1 as usize * w + x0 as usize] -= 1;
            d[y1 as usize * w + x1 as usize] += 1;
        }
        let mut grille = vec![false; cols as usize * rows as usize];
        for y in 0..rows as usize {
            for x in 0..cols as usize {
                let (a, b, c) = (if x > 0 { d[y * w + x - 1] } else { 0 }, if y > 0 { d[(y - 1) * w + x] } else { 0 }, if x > 0 && y > 0 { d[(y - 1) * w + x - 1] } else { 0 });
                d[y * w + x] += a + b - c;
                grille[y * cols as usize + x] = d[y * w + x] > 0;
            }
        }
        Voisines { cols, rows, grille }
    }

    pub(crate) fn a(&self, tx: i32, ty: i32) -> bool {
        tx >= 0 && ty >= 0 && tx < self.cols && ty < self.rows && self.grille[ty as usize * self.cols as usize + tx as usize]
    }
}

/// Des pixels (une portée) en tuiles entières.
pub(crate) fn en_tuiles(px: f32) -> i32 {
    if !px.is_finite() {
        return i32::MAX / 4;
    }
    (px.max(0.0).ceil() as i64).div_euclid(TILE as i64).min(i32::MAX as i64 / 4) as i32 + i32::from(px > 0.0)
}

/// Jusqu'où l'effet peut aller lire ou porter un pixel de `encre` (pixels du monde).
fn portee(s: &FxStep, encre: Pave, feuille: (usize, usize)) -> f32 {
    let at = |k: usize| s.values.get(k).copied().unwrap_or(0.0);
    let cercle = |rayon: f32, cx: f32, cy: f32| {
        let (ox, oy, half) = crate::fxwarp::centre_de(feuille, cx, cy);
        let r = (half * rayon / 100.0).max(1.0);
        if encre.touche(Pave { x0: ox - r, y0: oy - r, x1: ox + r, y1: oy + r }) {
            2.0 * r + 2.0
        } else {
            0.0
        }
    };
    match s.kind {
        EffectKind::GaussianBlur => at(0) + 2.0,
        EffectKind::MotionBlur => at(0) / 2.0 + 2.0,
        EffectKind::Sharpen => at(1).max(0.5) + 2.0,
        EffectKind::Mosaic => at(0).round().max(1.0) * 2.0,
        EffectKind::DropShadow => at(0).abs().max(at(1).abs()) + at(2).max(0.0) + 3.0,
        EffectKind::Glow => at(0).max(1.0) + 2.0,
        EffectKind::Bloom => at(0) + 4.0 * (1u32 << crate::fxcine::blur_level(at(0))) as f32 + 2.0,
        EffectKind::Boil => {
            let r = crate::tremblement::Settings::of(&s.values);
            r.intensity.ceil() + 2.0
        }
        EffectKind::Outline => crate::fxmatiere::portee_du_lisere(&s.values),
        EffectKind::Bleed => crate::fxmatiere::portee_de_la_bavure(&s.values),
        EffectKind::Dry => 4.0,
        EffectKind::Wave | EffectKind::Ripple => at(0).abs() + 2.0,
        EffectKind::Twirl | EffectKind::Pinch => cercle(at(1), at(2), at(3)),
        EffectKind::Shake => {
            let m = crate::fxcine::Secousse::de(feuille, &s.values);
            encre.coins().iter().map(|&p| m.deplacement(p)).fold(0.0, f32::max) + 2.0
        }
        EffectKind::Aberration => portee_de_l_aberration(at(0), feuille),
        _ => 0.0,
    }
}

/// L'écart des couleurs de l'aberration, au plus (`fxcine::aberration_pixel`) : deux fois sa force, pour tout point du monde.
fn portee_de_l_aberration(amount: f32, feuille: (usize, usize)) -> f32 {
    let (w, h) = (feuille.0 as f32, feuille.1 as f32);
    let diagonale = (w * w + h * h).sqrt();
    amount.abs() * diagonale / (diagonale / 2.0).max(1.0) + 2.0
}

/// Le rectangle d'encre de `base`, en pixels du monde.
fn pave_de(base: &Raster) -> Option<Pave> {
    let (tx0, ty0, tx1, ty1) = base.drawn_tile_range()?;
    let t = TILE as f32;
    Some(Pave { x0: tx0 as f32 * t, y0: ty0 as f32 * t, x1: (tx1 + 1) as f32 * t, y1: (ty1 + 1) as f32 * t })
}

/// Ce que la pile peut déplacer ou aller lire, au plus, sur un dessin d'encre `encre`.
pub(crate) fn portee_de_la_pile(encre: (f32, f32, f32, f32), feuille: (usize, usize), steps: &[&FxStep]) -> f32 {
    let mut encre = Pave { x0: encre.0, y0: encre.1, x1: encre.2, y1: encre.3 };
    let mut total = 0.0;
    for s in steps {
        let p = portee(s, encre, feuille);
        total += p;
        encre = encre.grossi(p);
    }
    total
}

/// `out`, calculé sur la feuille, reçoit ce que `base` passé par `steps` met au-delà d'elle, et les tuiles de la feuille assez près pour lire au-delà.
/// Un dessin qui ne dépasse pas ne change pas.
pub(crate) fn effets_hors_de_la_feuille(out: &mut Raster, base: &Raster, steps: &[&FxStep]) {
    let debord = base.tuiles_du_debord();
    if debord.is_empty() {
        return;
    }
    let feuille = (base.width, base.height);
    let Some(encre) = pave_de(base) else { return };
    let r = en_tuiles(portee_de_la_pile((encre.x0, encre.y0, encre.x1, encre.y1), feuille, steps));
    let voisines = Voisines::autour(base, &debord, r);
    let lecture = Voisines::autour(base, &debord, r.saturating_mul(2).saturating_add(1));
    let mut image = Raster::new(base.width, base.height);
    for id in base.drawn_ids() {
        let (tx, ty) = base.key_of(id);
        if !base.in_window(tx, ty) || lecture.a(tx, ty) {
            image.replace_slot(id, base.slot(id).cloned());
        }
    }
    for s in steps {
        image = un_pas(&image, s, feuille, &voisines);
    }
    fondre(out, &image, &voisines);
}

/// Le résultat `monde` posé dans `out` : tout ce qui dépasse de la feuille, et, dans la feuille, les seules tuiles `voisines`. (Ce qu'`out` avait au-delà de la feuille n'est plus bon.)
pub(crate) fn fondre(out: &mut Raster, monde: &Raster, voisines: &Voisines) {
    out.oublier_le_debord();
    for tx in 0..voisines.cols {
        for ty in 0..voisines.rows {
            if voisines.a(tx, ty) {
                let id = out.id_of(tx, ty);
                out.replace_slot(id, monde.tile_at(tx, ty).cloned());
            }
        }
    }
    for id in monde.drawn_ids() {
        let (tx, ty) = monde.key_of(id);
        if voisines.a(tx, ty) {
            continue;
        }
        if let Some(t) = monde.slot(id) {
            // (Une tuile hors de la fenêtre sans encre ne compte pas : elle ne pèserait que de la mémoire.)
            if monde.in_window(tx, ty) || t.iter().any(|c| c.a() != 0) {
                out.absorb_beyond(tx, ty, t.clone());
            }
        }
    }
}

/// Un effet sur `src`, dans le monde.
fn un_pas(src: &Raster, s: &FxStep, feuille: (usize, usize), voisines: &Voisines) -> Raster {
    match s.kind {
        EffectKind::Wave | EffectKind::Ripple | EffectKind::Twirl | EffectKind::Pinch | EffectKind::Shake | EffectKind::Aberration => deformation(src, s, feuille),
        _ => par_zones(src, s, feuille, voisines),
    }
}

// ---------------------------------------------------------------------- par zones

/// Les zones en cours de calcul, pour tout le processus : le fil de fond et chaque fil d'export se partagent `ZONES_A_LA_FOIS`, ils ne les multiplient pas.
static ZONES_EN_COURS: (std::sync::Mutex<usize>, std::sync::Condvar) = (std::sync::Mutex::new(0), std::sync::Condvar::new());

/// Rend sa place à la zone suivante, même si l'effet plante.
struct PlaceDeZone;

impl Drop for PlaceDeZone {
    fn drop(&mut self) {
        *ZONES_EN_COURS.0.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        ZONES_EN_COURS.1.notify_one();
    }
}

/// Une zone : `f` sans lancer de fils (une zone à la fois sur chaque fil, pas de fils dans les fils), quand une place est libre. La limite d'avant est remise : avec une seule zone, ce fil est celui de l'appelant.
pub(crate) fn sans_fils<R>(f: impl FnOnce() -> R) -> R {
    let _place = {
        let mut n = ZONES_EN_COURS.0.lock().unwrap_or_else(|e| e.into_inner());
        while *n >= ZONES_A_LA_FOIS {
            n = ZONES_EN_COURS.1.wait(n).unwrap_or_else(|e| e.into_inner());
        }
        *n += 1;
        PlaceDeZone
    };
    let avant = crate::render::thread_cap();
    crate::render::set_thread_cap(1);
    let r = f();
    crate::render::set_thread_cap(avant);
    r
}

/// L'effet remplit la feuille là où rien n'est dessiné : les tuiles de la feuille que ce calcul remplace sont à calculer même sans encre.
fn remplit_la_feuille(kind: EffectKind) -> bool {
    matches!(kind, EffectKind::Flash | EffectKind::Impact)
}

/// L'effet, zone par zone : chaque zone voit autour d'elle ce que l'effet peut aller lire, donc les zones se raccordent au pixel près (essai :
/// `computing_by_zones_gives_the_same_pixels_as_the_whole_drawing`).
fn par_zones(src: &Raster, s: &FxStep, feuille: (usize, usize), voisines: &Voisines) -> Raster {
    let encre = pave_de(src).unwrap_or(Pave { x0: 0.0, y0: 0.0, x1: 0.0, y1: 0.0 });
    let marge = en_tuiles(portee(s, encre, feuille));
    let mut zones = std::collections::BTreeSet::new();
    for id in src.drawn_ids() {
        let (tx, ty) = src.key_of(id);
        for by in (ty - marge).div_euclid(BLOC)..=(ty + marge).div_euclid(BLOC) {
            for bx in (tx - marge).div_euclid(BLOC)..=(tx + marge).div_euclid(BLOC) {
                zones.insert((bx, by));
            }
        }
    }
    if remplit_la_feuille(s.kind) {
        for ty in 0..voisines.rows {
            for tx in 0..voisines.cols {
                if voisines.a(tx, ty) {
                    zones.insert((tx.div_euclid(BLOC), ty.div_euclid(BLOC)));
                }
            }
        }
    }
    let zones: Vec<(i32, i32)> = zones.into_iter().collect();
    let cote = (BLOC + 2 * marge) as usize;
    let cap = crate::render::thread_cap();
    let max = if cap == 0 { ZONES_A_LA_FOIS } else { cap.min(ZONES_A_LA_FOIS) };
    let faites = crate::parallele::par_map(&zones, max, |_, &(bx, by)| sans_fils(|| {
        let (ox, oy) = (bx * BLOC - marge, by * BLOC - marge);
        let mut image = Raster::new(cote * TILE, cote * TILE);
        for ty in oy..oy + cote as i32 {
            for tx in ox..ox + cote as i32 {
                if let Some(t) = src.tile_at(tx, ty) {
                    image.tiles[(ty - oy) as usize * cote + (tx - ox) as usize] = Some(t.clone());
                }
            }
        }
        let rep = Repere { origine: (ox as i64 * TILE as i64, oy as i64 * TILE as i64), feuille };
        let resultat = crate::fx::un_effet(&image, s, 0, rep);
        let mut gardees = Vec::new();
        for j in 0..BLOC {
            for i in 0..BLOC {
                let at = (j + marge) as usize * cote + (i + marge) as usize;
                if let Some(t) = &resultat.tiles[at] {
                    if t.iter().any(|c| c.a() != 0) {
                        gardees.push(((bx * BLOC + i, by * BLOC + j), t.clone()));
                    }
                }
            }
        }
        gardees
    }));
    let mut out = Raster::new(src.width, src.height);
    for ((tx, ty), t) in faites.into_iter().flatten() {
        let id = out.id_of(tx, ty);
        out.replace_slot(id, Some(t));
    }
    out
}

// ---------------------------------------------------------------------- les effets qui déplacent le dessin

/// Lit le dessin à un point du monde (la tuile d'avant reste sous la main : la plupart des lectures y retombent).
struct Lecteur<'a> {
    src: &'a Raster,
    derniere: (i32, i32),
    tuile: Option<&'a Tile>,
}

impl<'a> Lecteur<'a> {
    fn sur(src: &'a Raster) -> Lecteur<'a> {
        Lecteur { src, derniere: (i32::MIN, i32::MIN), tuile: None }
    }

    #[inline]
    fn px(&mut self, x: i64, y: i64) -> [f32; 4] {
        let t = TILE as i64;
        let cle = (x.div_euclid(t) as i32, y.div_euclid(t) as i32);
        if cle != self.derniere {
            self.derniere = cle;
            self.tuile = self.src.tile_at(cle.0, cle.1);
        }
        match self.tuile {
            Some(tuile) => {
                let c = tuile[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize];
                [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
            }
            None => [0.0; 4],
        }
    }

    /// Comme `fxwarp::sample`, sans la borne de la feuille.
    #[inline]
    fn sample(&mut self, x: f32, y: f32) -> [f32; 4] {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (i, j) = (ix as i64, iy as i64);
        let (a, b, c, d) = (self.px(i, j), self.px(i + 1, j), self.px(i, j + 1), self.px(i + 1, j + 1));
        std::array::from_fn(|k| {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bottom = c[k] + (d[k] - c[k]) * tx;
            top + (bottom - top) * ty
        })
    }
}

/// Chaque pixel des tuiles `cibles` calculé par `f` (lecteur, x et y du centre du pixel, dans le monde).
fn par_tuiles(src: &Raster, cibles: &[(i32, i32)], f: impl Fn(&mut Lecteur, f32, f32) -> Color32 + Sync) -> Raster {
    let cap = crate::render::thread_cap();
    let max = if cap == 0 { 16 } else { cap };
    let faites = crate::parallele::par_map(cibles, max, |_, &(tx, ty)| {
        let mut lecteur = Lecteur::sur(src);
        let (ox, oy) = (tx as i64 * TILE as i64, ty as i64 * TILE as i64);
        let mut px = [Color32::TRANSPARENT; TILE_PIXELS];
        let mut encre = false;
        for y in 0..TILE {
            for x in 0..TILE {
                let c = f(&mut lecteur, (ox + x as i64) as f32 + 0.5, (oy + y as i64) as f32 + 0.5);
                if c.a() != 0 {
                    encre = true;
                    px[y * TILE + x] = c;
                }
            }
        }
        encre.then(|| ((tx, ty), Tile::new(px)))
    });
    let mut out = Raster::new(src.width, src.height);
    for ((tx, ty), t) in faites.into_iter().flatten() {
        let id = out.id_of(tx, ty);
        out.replace_slot(id, Some(t));
    }
    out
}

/// Une couleur prémultipliée lue en réels, remise en pixel (comme `fxwarp::warp`).
#[inline]
fn pixel(c: [f32; 4]) -> Color32 {
    let a = c[3].round().clamp(0.0, 255.0);
    let ch = |v: f32| v.round().clamp(0.0, a) as u8;
    Color32::from_rgba_premultiplied(ch(c[0]), ch(c[1]), ch(c[2]), a as u8)
}

fn ajouter(cibles: &mut std::collections::BTreeSet<(i32, i32)>, p: Pave) {
    let t = TILE as f32;
    for ty in (p.y0 / t).floor() as i32..=((p.y1 - 1.0) / t).floor() as i32 {
        for tx in (p.x0 / t).floor() as i32..=((p.x1 - 1.0) / t).floor() as i32 {
            cibles.insert((tx, ty));
        }
    }
}

/// Onde, ondulation, tourbillon, pincement, secousse, aberration : chaque point va lire le dessin ailleurs. Seules les tuiles où de l'encre peut arriver sont visitées.
fn deformation(src: &Raster, s: &FxStep, feuille: (usize, usize)) -> Raster {
    let at = |k: usize| s.values.get(k).copied().unwrap_or(0.0);
    let mut cibles = std::collections::BTreeSet::new();
    let tuiles: Vec<(i32, i32)> = src.drawn_ids().iter().map(|&id| src.key_of(id)).collect();
    // Où l'encre de la tuile peut arriver.
    match s.kind {
        EffectKind::Wave | EffectKind::Ripple => {
            for &(tx, ty) in &tuiles {
                ajouter(&mut cibles, Pave::de_tuile(tx, ty).grossi(at(0).abs() + 2.0));
            }
        }
        EffectKind::Twirl | EffectKind::Pinch => {
            let (ox, oy, half) = crate::fxwarp::centre_de(feuille, at(2), at(3));
            let r = (half * at(1) / 100.0).max(1.0);
            let cercle = Pave { x0: ox - r, y0: oy - r, x1: ox + r, y1: oy + r };
            let mut touche = false;
            for &(tx, ty) in &tuiles {
                let p = Pave::de_tuile(tx, ty);
                touche |= p.touche(cercle);
                ajouter(&mut cibles, p);
            }
            if touche {
                ajouter(&mut cibles, cercle.grossi(2.0));
            }
        }
        EffectKind::Shake => {
            let m = crate::fxcine::Secousse::de(feuille, &s.values);
            for &(tx, ty) in &tuiles {
                let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
                for p in Pave::de_tuile(tx, ty).coins() {
                    for q in m.arrivees(p) {
                        lo = (lo.0.min(q.0), lo.1.min(q.1));
                        hi = (hi.0.max(q.0), hi.1.max(q.1));
                    }
                }
                ajouter(&mut cibles, Pave { x0: lo.0, y0: lo.1, x1: hi.0, y1: hi.1 }.grossi(2.0));
            }
        }
        _ => {
            let portee = portee_de_l_aberration(at(0), feuille);
            for &(tx, ty) in &tuiles {
                ajouter(&mut cibles, Pave::de_tuile(tx, ty).grossi(portee));
            }
        }
    }
    let cibles: Vec<(i32, i32)> = cibles.into_iter().collect();
    match s.kind {
        EffectKind::Wave => {
            let m = crate::fxwarp::wave_map(at(0), at(1), at(2));
            par_tuiles(src, &cibles, |l, x, y| {
                let (sx, sy) = m(x, y);
                pixel(l.sample(sx, sy))
            })
        }
        EffectKind::Ripple => {
            let m = crate::fxwarp::ripple_map(feuille, at(0), at(1), at(2), at(3));
            par_tuiles(src, &cibles, |l, x, y| {
                let (sx, sy) = m(x, y);
                pixel(l.sample(sx, sy))
            })
        }
        EffectKind::Twirl => {
            let m = crate::fxwarp::twirl_map(feuille, at(0), at(1), at(2), at(3));
            par_tuiles(src, &cibles, |l, x, y| {
                let (sx, sy) = m(x, y);
                pixel(l.sample(sx, sy))
            })
        }
        EffectKind::Pinch => {
            let m = crate::fxwarp::pinch_map(feuille, at(0), at(1), at(2), at(3));
            par_tuiles(src, &cibles, |l, x, y| {
                let (sx, sy) = m(x, y);
                pixel(l.sample(sx, sy))
            })
        }
        EffectKind::Shake => {
            let m = crate::fxcine::Secousse::de(feuille, &s.values);
            par_tuiles(src, &cibles, |l, x, y| m.pixel(x, y, |sx, sy| l.sample(sx, sy)))
        }
        _ => {
            let (amount, cx, cy) = (at(0), at(1), at(2));
            par_tuiles(src, &cibles, |l, x, y| crate::fxcine::aberration_pixel(feuille, amount, cx, cy, x, y, |sx, sy| l.sample(sx, sy)))
        }
    }
}
