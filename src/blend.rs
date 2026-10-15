//! Modes de fusion des calques (phase 8), comme dans les logiciels de retouche : le calque se
//! mélange à ce qui est dessous selon une formule. Formules de la norme ouverte W3C
//! « Compositing and Blending Level 1 », en valeurs gamma (comme les logiciels de retouche par
//! défaut). Le même calcul est fait sur la carte graphique (`gpu.rs`, `fs_blend`).

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

/// Mode de fusion d'un calque.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "produit")]
    Multiply,
    #[serde(rename = "superposition")]
    Screen,
    #[serde(rename = "incrustation")]
    Overlay,
    #[serde(rename = "obscurcir")]
    Darken,
    #[serde(rename = "eclaircir")]
    Lighten,
    #[serde(rename = "densite_couleur_plus")]
    ColorBurn,
    #[serde(rename = "densite_couleur_moins")]
    ColorDodge,
    #[serde(rename = "densite_lineaire_moins")]
    Add,
    #[serde(rename = "lumiere_crue")]
    HardLight,
    #[serde(rename = "lumiere_tamisee")]
    SoftLight,
    #[serde(rename = "difference")]
    Difference,
    #[serde(rename = "exclusion")]
    Exclusion,
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
    pub const ALL: [BlendMode; 17] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorBurn,
        BlendMode::ColorDodge,
        BlendMode::Add,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];

    /// Numéro passé au shader (même ordre que `ALL`).
    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0) as u32
    }

    /// Nom affiché (les noms usuels en français) et nom de la norme.
    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Multiply => "Produit",
            BlendMode::Screen => "Superposition",
            BlendMode::Overlay => "Incrustation",
            BlendMode::Darken => "Obscurcir",
            BlendMode::Lighten => "Éclaircir",
            BlendMode::ColorBurn => "Densité couleur +",
            BlendMode::ColorDodge => "Densité couleur −",
            BlendMode::Add => "Densité linéaire − (addition)",
            BlendMode::HardLight => "Lumière crue",
            BlendMode::SoftLight => "Lumière tamisée",
            BlendMode::Difference => "Différence",
            BlendMode::Exclusion => "Exclusion",
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
            BlendMode::Multiply => "Assombrit : le blanc ne change rien (ombres, encrage sur la couleur). Norme : multiply.",
            BlendMode::Screen => "Éclaircit : le noir ne change rien (lumières, halos). Norme : screen.",
            BlendMode::Overlay => "Renforce le contraste du dessous. Norme : overlay.",
            BlendMode::Darken => "Garde la plus sombre des deux couleurs. Norme : darken.",
            BlendMode::Lighten => "Garde la plus claire des deux couleurs. Norme : lighten.",
            BlendMode::ColorBurn => "Assombrit en saturant. Norme : color-burn.",
            BlendMode::ColorDodge => "Éclaircit fortement (brillances). Norme : color-dodge.",
            BlendMode::Add => "Additionne les couleurs (lumière forte).",
            BlendMode::HardLight => "Contraste fort, selon le calque. Norme : hard-light.",
            BlendMode::SoftLight => "Contraste doux, selon le calque. Norme : soft-light.",
            BlendMode::Difference => "Écart entre les couleurs (vérifier un calage). Norme : difference.",
            BlendMode::Exclusion => "Comme Différence, plus doux. Norme : exclusion.",
            BlendMode::Hue => "Teinte du calque, saturation et luminosité du dessous. Norme : hue.",
            BlendMode::Saturation => "Saturation du calque. Norme : saturation.",
            BlendMode::Color => "Teinte et saturation du calque : colorier un dessin. Norme : color.",
            BlendMode::Luminosity => "Luminosité du calque. Norme : luminosity.",
        }
    }

    /// B(Cb, Cs) : la couleur mélangée, pour une couleur du dessous `cb` et du calque `cs`
    /// (composantes de 0 à 1, non prémultipliées).
    pub fn mix(self, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
        match self {
            BlendMode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
            BlendMode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
            BlendMode::Color => set_lum(cs, lum(cb)),
            BlendMode::Luminosity => set_lum(cb, lum(cs)),
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

/// Modes « séparables » : composante par composante.
fn channel(mode: BlendMode, cb: f32, cs: f32) -> f32 {
    match mode {
        BlendMode::Multiply => cb * cs,
        BlendMode::Screen => cb + cs - cb * cs,
        BlendMode::Overlay => hard_light(cs, cb),
        BlendMode::Darken => cb.min(cs),
        BlendMode::Lighten => cb.max(cs),
        BlendMode::ColorBurn => {
            if cb >= 1.0 {
                1.0
            } else if cs <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - cb) / cs).min(1.0)
            }
        }
        BlendMode::ColorDodge => {
            if cb <= 0.0 {
                0.0
            } else if cs >= 1.0 {
                1.0
            } else {
                (cb / (1.0 - cs)).min(1.0)
            }
        }
        BlendMode::Add => (cb + cs).min(1.0),
        BlendMode::HardLight => hard_light(cb, cs),
        BlendMode::SoftLight => {
            if cs <= 0.5 {
                cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
            } else {
                let d = if cb <= 0.25 { ((16.0 * cb - 12.0) * cb + 4.0) * cb } else { cb.sqrt() };
                cb + (2.0 * cs - 1.0) * (d - cb)
            }
        }
        BlendMode::Difference => (cb - cs).abs(),
        BlendMode::Exclusion => cb + cs - 2.0 * cb * cs,
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

/// `src` (prémultiplié) sur `dst` (prémultiplié) avec l'opacité `opacity`, selon `mode`. Le
/// mode Normal et la pelure d'oignon (teinte) passent par `render::over`.
pub fn blend_over(dst: Color32, src: Color32, opacity: u8, tint: Option<[u8; 3]>, mode: BlendMode) -> Color32 {
    if mode == BlendMode::Normal || tint.is_some() {
        return crate::render::over(dst, src, opacity, tint);
    }
    let a = src.a() as f32 / 255.0;
    let sa = a * opacity as f32 / 255.0;
    if sa <= 0.0 {
        return dst;
    }
    let cs = [src.r(), src.g(), src.b()].map(|v| (v as f32 / 255.0 / a).min(1.0));
    let ab = dst.a() as f32 / 255.0;
    let cb = if ab > 0.0 { [dst.r(), dst.g(), dst.b()].map(|v| (v as f32 / 255.0 / ab).min(1.0)) } else { [0.0; 3] };
    let mixed = mode.mix(cb, cs);
    let out = [0, 1, 2].map(|k| {
        let c = (1.0 - ab) * cs[k] + ab * mixed[k].clamp(0.0, 1.0);
        sa * c + (1.0 - sa) * ab * cb[k]
    });
    let ao = sa + ab * (1.0 - sa);
    let byte = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    let ao = byte(ao);
    Color32::from_rgba_premultiplied(byte(out[0]).min(ao), byte(out[1]).min(ao), byte(out[2]).min(ao), ao)
}
