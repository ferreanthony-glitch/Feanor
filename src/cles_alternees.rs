//! Clés de dessin mieux identifiables (Anthony, 2026-11-07 : sur une piste de calque jade, « on voit mal où une clé
//! commence et s'arrête »).
//!
//! Sur une piste de calque, les segments de dessins s'alternent : un segment à la couleur du calque, le suivant un peu plus
//! clair, et ainsi de suite, dans la timeline comme dans le montage. L'alternance suit le **rang de la clé à partir du début du
//! plan** (les dessins seulement : une plage vide n'a pas de rang), jamais ce qu'on voit : elle ne change donc pas quand la
//! timeline défile ni quand le zoom change.
//!
//! Le « plus clair » se mesure en luminosité perceptuelle (OKLab, `PAS`), pas en ajoutant du blanc : un jade sombre et un
//! orange vif gagnent le même écart visible. Une couleur déjà très claire ne peut pas s'éclaircir : elle s'assombrit du même
//! écart (le contraste reste le même, dans l'autre sens).

use eframe::egui::Color32;

use crate::document::Layer;

#[cfg(test)]
thread_local! {
    /// Vrai : `alternee` rend toujours la couleur du calque (l'écran d'avant, pour les captures de comparaison).
    pub(crate) static SANS_ALTERNANCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Écart de luminosité (échelle OKLab, 0 à 1) entre deux segments voisins : visible sans être criard.
pub(crate) const PAS: f32 = 0.075;

/// Au-dessus de cette luminosité, on ne peut plus éclaircir : le segment impair s'assombrit.
const TROP_CLAIR: f32 = 0.86;

fn lineaire(c: u8) -> f32 {
    let x = f32::from(c) / 255.0;
    if x <= 0.040_45 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn gamma(x: f32) -> u8 {
    let x = x.clamp(0.0, 1.0);
    let v = if x <= 0.003_130_8 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Rouge, vert, bleu (sRGB) vers (L, a, b) d'OKLab.
fn oklab(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (lineaire(r), lineaire(g), lineaire(b));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    (
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    )
}

/// (L, a, b) d'OKLab vers rouge, vert, bleu (sRGB), ramenés dans la gamme.
fn srgb(l: f32, a: f32, b: f32) -> [u8; 3] {
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    [
        gamma(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_93 * s),
        gamma(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_4 * s),
        gamma(-0.004_196_086 * l - 0.703_418_6 * m + 1.707_614_7 * s),
    ]
}

/// La couleur du segment de rang `rang` sur un calque de couleur `base` : le rang pair garde la couleur du calque, le rang
/// impair est un peu plus clair (`PAS`) ; trop clair déjà, un peu plus sombre.
pub(crate) fn alternee(base: Color32, rang: usize) -> Color32 {
    // (Les captures « avant » des essais montrent l'écran sans l'alternance.)
    #[cfg(test)]
    if SANS_ALTERNANCE.with(std::cell::Cell::get) {
        return base;
    }
    if rang % 2 == 0 {
        return base;
    }
    let [r, g, b, a] = base.to_srgba_unmultiplied();
    let (l, ca, cb) = oklab(r, g, b);
    let l2 = if l > TROP_CLAIR { l - PAS } else { l + PAS };
    let [r, g, b] = srgb(l2, ca, cb);
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Le rang du dessin qui commence à l'image `start` du calque : combien de dessins (expositions non vides) le précèdent
/// depuis le début du plan.
pub(crate) fn rang_de(layer: &Layer, start: usize) -> usize {
    let mut f = 0;
    let mut n = 0;
    while f < start.min(layer.frames.len()) {
        let (_, e) = layer.run_bounds(f);
        if layer.frames[f].is_some() {
            n += 1;
        }
        f = e;
    }
    n
}

/// La luminosité OKLab d'une couleur (pour les essais).
#[cfg(test)]
pub(crate) fn luminosite(c: Color32) -> f32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    oklab(r, g, b).0
}
