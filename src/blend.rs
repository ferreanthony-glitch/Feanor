//! Modes de fusion des calques (phases 8 et 9), comme dans les logiciels de retouche : le calque
//! se mélange à ce qui est dessous selon une formule. Formules de la norme ouverte W3C
//! « Compositing and Blending Level 1 » pour celles qu'elle décrit, formules usuelles pour les
//! autres (Densité linéaire +, Lumière vive, linéaire, ponctuelle, Mélange maximal, Soustraction,
//! Division, Couleur plus foncée / claire, Fondu), en valeurs gamma (comme les logiciels de
//! retouche par défaut). Le même calcul est fait sur la carte graphique (`gpu.rs`, `fs_blend`).

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

/// Mode de fusion d'un calque.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "fondu")]
    Dissolve,
    #[serde(rename = "obscurcir")]
    Darken,
    #[serde(rename = "produit")]
    Multiply,
    #[serde(rename = "densite_couleur_plus")]
    ColorBurn,
    #[serde(rename = "densite_lineaire_plus")]
    LinearBurn,
    #[serde(rename = "couleur_plus_foncee")]
    DarkerColor,
    #[serde(rename = "eclaircir")]
    Lighten,
    #[serde(rename = "superposition")]
    Screen,
    #[serde(rename = "densite_couleur_moins")]
    ColorDodge,
    #[serde(rename = "densite_lineaire_moins")]
    Add,
    #[serde(rename = "couleur_plus_claire")]
    LighterColor,
    #[serde(rename = "incrustation")]
    Overlay,
    #[serde(rename = "lumiere_tamisee")]
    SoftLight,
    #[serde(rename = "lumiere_crue")]
    HardLight,
    #[serde(rename = "lumiere_vive")]
    VividLight,
    #[serde(rename = "lumiere_lineaire")]
    LinearLight,
    #[serde(rename = "lumiere_ponctuelle")]
    PinLight,
    #[serde(rename = "melange_maximal")]
    HardMix,
    #[serde(rename = "difference")]
    Difference,
    #[serde(rename = "exclusion")]
    Exclusion,
    #[serde(rename = "soustraction")]
    Subtract,
    #[serde(rename = "division")]
    Divide,
    #[serde(rename = "teinte")]
    Hue,
    #[serde(rename = "saturation")]
    Saturation,
    #[serde(rename = "couleur")]
    Color,
    #[serde(rename = "luminosite")]
    Luminosity,
}

impl BlendMode {
    /// Tous les modes, par familles : normal, assombrir, éclaircir, contraste, écarts, couleur.
    /// L'ordre donne aussi le numéro passé au shader.
    pub const ALL: [BlendMode; 27] = [
        BlendMode::Normal,
        BlendMode::Dissolve,
        BlendMode::Darken,
        BlendMode::Multiply,
        BlendMode::ColorBurn,
        BlendMode::LinearBurn,
        BlendMode::DarkerColor,
        BlendMode::Lighten,
        BlendMode::Screen,
        BlendMode::ColorDodge,
        BlendMode::Add,
        BlendMode::LighterColor,
        BlendMode::Overlay,
        BlendMode::SoftLight,
        BlendMode::HardLight,
        BlendMode::VividLight,
        BlendMode::LinearLight,
        BlendMode::PinLight,
        BlendMode::HardMix,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Subtract,
        BlendMode::Divide,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];

    /// Premier mode de chaque famille (pour séparer le menu), sauf la première.
    pub const FAMILY_STARTS: [BlendMode; 5] =
        [BlendMode::Darken, BlendMode::Lighten, BlendMode::Overlay, BlendMode::Difference, BlendMode::Hue];

    /// Numéro passé au shader (même ordre que `ALL`).
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0) as u32
    }

    /// Nom affiché (les noms usuels en français).
    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Dissolve => "Fondu",
            BlendMode::Darken => "Obscurcir",
            BlendMode::Multiply => "Produit",
            BlendMode::ColorBurn => "Densité couleur +",
            BlendMode::LinearBurn => "Densité linéaire +",
            BlendMode::DarkerColor => "Couleur plus foncée",
            BlendMode::Lighten => "Éclaircir",
            BlendMode::Screen => "Superposition",
            BlendMode::ColorDodge => "Densité couleur −",
            BlendMode::Add => "Densité linéaire − (addition)",
            BlendMode::LighterColor => "Couleur plus claire",
            BlendMode::Overlay => "Incrustation",
            BlendMode::SoftLight => "Lumière tamisée",
            BlendMode::HardLight => "Lumière crue",
            BlendMode::VividLight => "Lumière vive",
            BlendMode::LinearLight => "Lumière linéaire",
            BlendMode::PinLight => "Lumière ponctuelle",
            BlendMode::HardMix => "Mélange maximal",
            BlendMode::Difference => "Différence",
            BlendMode::Exclusion => "Exclusion",
            BlendMode::Subtract => "Soustraction",
            BlendMode::Divide => "Division",
            BlendMode::Hue => "Teinte",
            BlendMode::Saturation => "Saturation",
            BlendMode::Color => "Couleur",
            BlendMode::Luminosity => "Luminosité",
        }
    }

    /// Ce que fait le mode, en quelques mots.
    pub fn hint(self) -> &'static str {
        match self {
            BlendMode::Normal => "Le calque recouvre ce qui est dessous.",
            BlendMode::Dissolve => "Les parties transparentes deviennent des points pleins, plus ou moins nombreux (grain).",
            BlendMode::Darken => "Garde la plus sombre des deux couleurs, composante par composante.",
            BlendMode::Multiply => "Assombrit : le blanc ne change rien (ombres, encrage sur la couleur).",
            BlendMode::ColorBurn => "Assombrit en saturant.",
            BlendMode::LinearBurn => "Assombrit fortement (somme moins le blanc).",
            BlendMode::DarkerColor => "Garde la plus sombre des deux couleurs, entière.",
            BlendMode::Lighten => "Garde la plus claire des deux couleurs, composante par composante.",
            BlendMode::Screen => "Éclaircit : le noir ne change rien (lumières, halos).",
            BlendMode::ColorDodge => "Éclaircit fortement (brillances).",
            BlendMode::Add => "Additionne les couleurs (lumière forte).",
            BlendMode::LighterColor => "Garde la plus claire des deux couleurs, entière.",
            BlendMode::Overlay => "Renforce le contraste du dessous.",
            BlendMode::SoftLight => "Contraste doux, selon le calque.",
            BlendMode::HardLight => "Contraste fort, selon le calque.",
            BlendMode::VividLight => "Densité couleur + ou − selon le calque.",
            BlendMode::LinearLight => "Densité linéaire + ou − selon le calque.",
            BlendMode::PinLight => "Remplace les sombres ou les claires selon le calque.",
            BlendMode::HardMix => "Couleurs poussées au tout ou rien (affiches).",
            BlendMode::Difference => "Écart entre les couleurs (vérifier un calage).",
            BlendMode::Exclusion => "Comme Différence, plus doux.",
            BlendMode::Subtract => "Retire la couleur du calque de celle du dessous.",
            BlendMode::Divide => "Divise la couleur du dessous par celle du calque.",
            BlendMode::Hue => "Teinte du calque, saturation et luminosité du dessous.",
            BlendMode::Saturation => "Saturation du calque.",
            BlendMode::Color => "Teinte et saturation du calque : colorier un dessin.",
            BlendMode::Luminosity => "Luminosité du calque.",
        }
    }

    /// B(Cb, Cs) : la couleur mélangée, pour une couleur du dessous `cb` et du calque `cs`
    /// (composantes de 0 à 1, non prémultipliées). Le Fondu n'en a pas (voir `blend_over`).
    pub fn mix(self, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
        match self {
            BlendMode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
            BlendMode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
            BlendMode::Color => set_lum(cs, lum(cb)),
            BlendMode::Luminosity => set_lum(cb, lum(cs)),
            BlendMode::DarkerColor => {
                if lum(cs) < lum(cb) {
                    cs
                } else {
                    cb
                }
            }
            BlendMode::LighterColor => {
                if lum(cs) > lum(cb) {
                    cs
                } else {
                    cb
                }
            }
            m => [0, 1, 2].map(|k| channel(m, cb[k], cs[k])),
        }
    }
}

fn hard_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        cb * 2.0 * cs
    } else {
        let s = 2.0 * cs - 1.0;
        cb + s - cb * s
    }
}

fn color_burn(cb: f32, cs: f32) -> f32 {
    if cb >= 1.0 {
        1.0
    } else if cs <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - cb) / cs).min(1.0)
    }
}

fn color_dodge(cb: f32, cs: f32) -> f32 {
    if cb <= 0.0 {
        0.0
    } else if cs >= 1.0 {
        1.0
    } else {
        (cb / (1.0 - cs)).min(1.0)
    }
}

/// Modes « séparables » : composante par composante.
fn channel(mode: BlendMode, cb: f32, cs: f32) -> f32 {
    match mode {
        BlendMode::Darken => cb.min(cs),
        BlendMode::Multiply => cb * cs,
        BlendMode::ColorBurn => color_burn(cb, cs),
        BlendMode::LinearBurn => (cb + cs - 1.0).max(0.0),
        BlendMode::Lighten => cb.max(cs),
        BlendMode::Screen => cb + cs - cb * cs,
        BlendMode::ColorDodge => color_dodge(cb, cs),
        BlendMode::Add => (cb + cs).min(1.0),
        BlendMode::Overlay => hard_light(cs, cb),
        BlendMode::SoftLight => {
            if cs <= 0.5 {
                cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
            } else {
                let d = if cb <= 0.25 { ((16.0 * cb - 12.0) * cb + 4.0) * cb } else { cb.sqrt() };
                cb + (2.0 * cs - 1.0) * (d - cb)
            }
        }
        BlendMode::HardLight => hard_light(cb, cs),
        BlendMode::VividLight => {
            if cs <= 0.5 {
                color_burn(cb, 2.0 * cs)
            } else {
                color_dodge(cb, 2.0 * cs - 1.0)
            }
        }
        BlendMode::LinearLight => (cb + 2.0 * cs - 1.0).clamp(0.0, 1.0),
        BlendMode::PinLight => {
            if cs <= 0.5 {
                cb.min(2.0 * cs)
            } else {
                cb.max(2.0 * cs - 1.0)
            }
        }
        BlendMode::HardMix => {
            if cb + cs >= 1.0 {
                1.0
            } else {
                0.0
            }
        }
        BlendMode::Difference => (cb - cs).abs(),
        BlendMode::Exclusion => cb + cs - 2.0 * cb * cs,
        BlendMode::Subtract => (cb - cs).max(0.0),
        BlendMode::Divide => {
            if cs <= 0.0 {
                if cb > 0.0 {
                    1.0
                } else {
                    0.0
                }
            } else {
                (cb / cs).min(1.0)
            }
        }
        _ => cs,
    }
}

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut o = c;
    if n < 0.0 {
        o = o.map(|v| l + (v - l) * l / (l - n).max(1e-6));
    }
    if x > 1.0 {
        o = o.map(|v| l + (v - l) * (1.0 - l) / (x - l).max(1e-6));
    }
    o
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|v| v + d))
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    if mx <= mn {
        return [0.0; 3];
    }
    c.map(|v| (v - mn) * s / (mx - mn))
}

/// Bruit du Fondu au pixel (`x`, `y`) de la feuille, de 0 à 1 : le même à chaque rendu, sur le
/// processeur comme sur la carte graphique (même calcul en entiers dans `fs_blend`).
pub fn dissolve_noise(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / 16_777_216.0
}

/// `src` (prémultiplié) sur `dst` (prémultiplié) avec l'opacité `opacity`, selon `mode`, au pixel
/// `at` de la feuille (pour le Fondu). Le mode Normal et la pelure d'oignon (teinte) passent par
/// `render::over`.
pub fn blend_over(dst: Color32, src: Color32, opacity: u8, tint: Option<[u8; 3]>, mode: BlendMode, at: (usize, usize)) -> Color32 {
    if mode == BlendMode::Normal || tint.is_some() {
        return crate::render::over(dst, src, opacity, tint);
    }
    let a = src.a() as f32 / 255.0;
    let sa = a * opacity as f32 / 255.0;
    if sa <= 0.0 {
        return dst;
    }
    let cs = [src.r(), src.g(), src.b()].map(|v| (v as f32 / 255.0 / a).min(1.0));
    let byte = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    if mode == BlendMode::Dissolve {
        // Un point plein, ou rien : d'autant plus de points que le calque est opaque.
        return if dissolve_noise(at.0 as u32, at.1 as u32) < sa {
            Color32::from_rgb(byte(cs[0]), byte(cs[1]), byte(cs[2]))
        } else {
            dst
        };
    }
    let ab = dst.a() as f32 / 255.0;
    let cb = if ab > 0.0 { [dst.r(), dst.g(), dst.b()].map(|v| (v as f32 / 255.0 / ab).min(1.0)) } else { [0.0; 3] };
    let mixed = mode.mix(cb, cs);
    let out = [0, 1, 2].map(|k| {
        let c = (1.0 - ab) * cs[k] + ab * mixed[k].clamp(0.0, 1.0);
        sa * c + (1.0 - sa) * ab * cb[k]
    });
    let ao = byte(sa + ab * (1.0 - sa));
    Color32::from_rgba_premultiplied(byte(out[0]).min(ao), byte(out[1]).min(ao), byte(out[2]).min(ao), ao)
}
