//! Aperçu des effets à la taille de l'écran (« brouillon »). Pendant la lecture, le défilement et
//! les gestes (curseur d'un effet), l'image n'a pas besoin de la résolution de la feuille : à
//! 50 % du zoom, un dessin réduit de moitié donne les mêmes pixels à l'écran en quatre fois moins
//! de calcul, de mémoire et d'envoi à la carte graphique. Le dessin d'origine est réduit
//! (moyenne des pixels prémultipliés), les effets passent à l'échelle (rayons, décalages, bruit),
//! et la carte graphique agrandit le résultat. Au repos (un quart de seconde sans mouvement) la
//! pleine résolution est calculée en arrière-plan et prend la place ; l'export, les vignettes et
//! l'enregistrement n'utilisent jamais l'aperçu.
//!
//! Seuls les effets dont le résultat suit l'échelle sont admis (flous, réglages de couleur,
//! netteté, bruit, ombre, lueur) ; la mosaïque (grille de la feuille), les déformations et le
//! rendu (centres et motifs en pixels de la feuille) restent en pleine résolution.

use eframe::egui::Color32;

use crate::fx::{EffectKind, FxStep};
use crate::raster::{tile_bounds, Raster, TILE};

/// Zoom d'affichage (pixels d'écran par pixel de la feuille) sous lequel l'aperçu est réduit de
/// moitié, puis du quart ; au-delà, la pleine résolution (sinon l'agrandissement se verrait).
const HALF_BELOW: f32 = 0.8;
const QUARTER_BELOW: f32 = 0.35;

/// Niveau de réduction de l'aperçu (0 : pleine résolution) pour un zoom d'affichage donné.
pub fn level_for_zoom(zoom: f32) -> u8 {
    if zoom <= QUARTER_BELOW {
        2
    } else if zoom <= HALF_BELOW {
        1
    } else {
        0
    }
}

/// Comme `level_for_zoom`, avec un niveau minimal `floor` (réglage « effets en aperçu réduit » du
/// diagnostic des performances : même zoomé, l'aperçu est au moins à moitié de la résolution).
pub fn level_with_floor(zoom: f32, floor: u8) -> u8 {
    level_for_zoom(zoom).max(floor)
}

/// Tous les effets de la pile admettent-ils l'aperçu ?
pub fn proxyable(steps: &[FxStep]) -> bool {
    steps.iter().all(|s| {
        !matches!(
            s.kind,
            EffectKind::Mosaic
                | EffectKind::Wave
                | EffectKind::Ripple
                | EffectKind::Twirl
                | EffectKind::Pinch
                | EffectKind::Clouds
                | EffectKind::Lighting
                | EffectKind::Gradient
        )
    })
}

/// Les paramètres de `kind` pour un dessin réduit `1 << level` fois : ceux qui sont des longueurs
/// en pixels sont réduits d'autant. `level` 0 : tels quels.
pub fn scaled_values(kind: EffectKind, values: &[f32], level: u8) -> Vec<f32> {
    let mut v = values.to_vec();
    if level == 0 {
        return v;
    }
    let s = 1.0 / (1u32 << level) as f32;
    let scale = |v: &mut Vec<f32>, ks: &[usize]| {
        for &k in ks {
            if let Some(x) = v.get_mut(k) {
                *x *= s;
            }
        }
    };
    match kind {
        EffectKind::GaussianBlur => scale(&mut v, &[0]),
        EffectKind::MotionBlur => scale(&mut v, &[0]),
        EffectKind::Sharpen => scale(&mut v, &[1]),
        EffectKind::Glow => scale(&mut v, &[0]),
        EffectKind::DropShadow => scale(&mut v, &[0, 1, 2]),
        EffectKind::Bloom => scale(&mut v, &[0]),
        EffectKind::Shake => scale(&mut v, &[0, 1]),
        EffectKind::Aberration => scale(&mut v, &[0]),
        _ => {}
    }
    v
}

/// `raster` réduit `1 << level` fois : chaque pixel est la moyenne (prémultipliée, arrondie) du
/// carré de pixels qu'il couvre. Les tuiles vides restent vides.
pub fn downscale(raster: &Raster, level: u8) -> Raster {
    if level == 0 {
        return raster.clone();
    }
    let f = 1usize << level;
    let (w, h) = (raster.width.div_ceil(f), raster.height.div_ceil(f));
    let mut out = Raster::new(w, h);
    let tiles_x = out.tiles_x.max(1);
    let n = (f * f) as u32;
    crate::render::par_chunks(&mut out.tiles, tiles_x, |ty, row| {
        for (tx, slot) in row.iter_mut().enumerate() {
            let (ox, oy, tw, th) = tile_bounds(w, h, ty * tiles_x + tx);
            // Les tuiles du dessin d'origine que cette tuile recouvre : toutes vides, rien à faire.
            let (sx0, sy0) = (ox * f, oy * f);
            let (sx1, sy1) = (((ox + tw) * f).min(raster.width), ((oy + th) * f).min(raster.height));
            let covered = (sy0 / TILE..=(sy1 - 1) / TILE)
                .any(|sty| (sx0 / TILE..=(sx1 - 1) / TILE).any(|stx| raster.tiles[sty * raster.tiles_x + stx].is_some()));
            if !covered {
                continue;
            }
            let mut tile = crate::raster::empty_tile();
            // (Le tableau, pris une fois : écrire par la tuile passerait chaque fois par `make_mut`.)
            let pixels: &mut [Color32; crate::raster::TILE_PIXELS] = &mut tile;
            let mut any = false;
            for y in 0..th {
                for x in 0..tw {
                    let mut acc = [0u32; 4];
                    for j in 0..f {
                        let sy = (oy + y) * f + j;
                        if sy >= raster.height {
                            break;
                        }
                        for i in 0..f {
                            let sx = (ox + x) * f + i;
                            if sx >= raster.width {
                                break;
                            }
                            let c = raster.get(sx, sy);
                            acc[0] += c.r() as u32;
                            acc[1] += c.g() as u32;
                            acc[2] += c.b() as u32;
                            acc[3] += c.a() as u32;
                        }
                    }
                    let m = |v: u32| ((v + n / 2) / n) as u8;
                    let c = Color32::from_rgba_premultiplied(m(acc[0]), m(acc[1]), m(acc[2]), m(acc[3]));
                    if c.a() != 0 {
                        any = true;
                        pixels[y * TILE + x] = c;
                    }
                }
            }
            if any {
                *slot = Some(tile);
            }
        }
    });
    out
}
