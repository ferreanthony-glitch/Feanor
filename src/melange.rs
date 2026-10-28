//! Mélange soustractif des couleurs (la brosse « La couleur se mélange », outil Mélangeur, palette de
//! mélange) : bleu + jaune donnent un vert, pas un gris.
//!
//! Le mélange de deux couleurs RVB en moyenne (comme le fait une brosse « à l'écran ») est additif : bleu
//! et jaune donnent un gris. Un vrai mélange de peintures est soustractif : chaque pigment absorbe une
//! part de la lumière, et la lumière qui reste est celle que **les deux** laissent passer. Ce module le
//! reprend de principes publics, écrits ici (aucun code d'un autre logiciel) :
//!
//! 1. une couleur RVB (en lumière linéaire) devient une courbe de réflectance sur 13 bandes de 400 à
//!    700 nm : la part de gris commune (le minimum des trois canaux) est une courbe plate, le reste est
//!    réparti sur trois courbes douces (rouge, vert, bleu) dont la somme vaut 1 à chaque longueur
//!    d'onde. Un bleu pur réfléchit donc aussi un peu de vert, un jaune pur aussi un peu de vert : c'est
//!    ce recouvrement qui fait le vert du mélange ;
//! 2. deux réflectances se mêlent par leur moyenne géométrique pondérée (la loi de Beer-Lambert : les
//!    absorptions s'ajoutent, c'est la forme la plus simple de la théorie de Kubelka-Munk) ;
//! 3. le résultat est ramené en RVB par trois fenêtres étroites (une par canal) ; l'écart d'aller-retour
//!    de chacune des deux couleurs de départ est corrigé, pondéré par sa part dans le mélange : mêler une
//!    couleur avec elle-même, ou avec un poids nul, la rend exactement.
//!
//! Pour une couleur (0 à 255, sans prémultiplication) : `melanger(a, b, t)`, `t` de 0 (a) à 1 (b).

use std::sync::OnceLock;

/// Nombre de bandes de longueur d'onde (400 à 700 nm, tous les 25 nm).
const BANDES: usize = 13;
/// Réflectance minimale d'une bande : un noir pur n'absorbe pas tout (sinon tout mélange avec lui serait
/// noir, et le noir plus le blanc serait presque noir).
const PLANCHER: f32 = 0.01;

struct Tables {
    /// Part de chaque bande dans la courbe du rouge, du vert, du bleu (somme 1 à chaque bande).
    base: [[f32; 3]; BANDES],
    /// Fenêtre de lecture de chaque canal (somme 1 sur les bandes).
    lecture: [[f32; 3]; BANDES],
    /// Lumière linéaire de chaque valeur sRVB de 0 à 255.
    lineaire: [f32; 256],
}

fn lisse(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut base = [[0.0; 3]; BANDES];
        let mut lecture = [[0.0; 3]; BANDES];
        let mut somme = [0.0f32; 3];
        for i in 0..BANDES {
            let l = 400.0 + 25.0 * i as f32;
            let rouge = lisse(560.0, 630.0, l);
            let bleu = 1.0 - lisse(470.0, 540.0, l);
            base[i] = [rouge, (1.0 - rouge - bleu).max(0.0), bleu];
            let fenetre = |centre: f32, largeur: f32| (-((l - centre) * (l - centre)) / (2.0 * largeur * largeur)).exp();
            lecture[i] = [fenetre(630.0, 40.0), fenetre(535.0, 40.0), fenetre(455.0, 35.0)];
            for k in 0..3 {
                somme[k] += lecture[i][k];
            }
        }
        for band in lecture.iter_mut() {
            for k in 0..3 {
                band[k] /= somme[k];
            }
        }
        let lineaire = std::array::from_fn(|v| {
            let c = v as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        });
        Tables { base, lecture, lineaire }
    })
}

/// sRVB (0 à 255) vers lumière linéaire (0 à 1).
fn en_lineaire(t: &Tables, c: f32) -> f32 {
    let c = c.clamp(0.0, 255.0);
    let (i, f) = (c.floor() as usize, c.fract());
    let (a, b) = (t.lineaire[i], t.lineaire[(i + 1).min(255)]);
    a + (b - a) * f
}

/// Lumière linéaire (0 à 1) vers sRVB (0 à 255).
fn en_srvb(l: f32) -> f32 {
    let l = l.clamp(0.0, 1.0);
    255.0 * if l <= 0.003_130_8 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
}

/// La réflectance d'une couleur linéaire.
fn spectre(t: &Tables, lin: [f32; 3]) -> [f32; BANDES] {
    let gris = lin[0].min(lin[1]).min(lin[2]);
    let reste = [lin[0] - gris, lin[1] - gris, lin[2] - gris];
    std::array::from_fn(|i| {
        let b = t.base[i];
        (gris + reste[0] * b[0] + reste[1] * b[1] + reste[2] * b[2]).max(PLANCHER)
    })
}

/// Les trois canaux linéaires lus dans une réflectance.
fn lire(t: &Tables, s: &[f32; BANDES]) -> [f32; 3] {
    let mut out = [0.0; 3];
    for i in 0..BANDES {
        for k in 0..3 {
            out[k] += s[i] * t.lecture[i][k];
        }
    }
    out
}

/// Ce que redonne l'aller-retour d'une couleur linéaire (sa réflectance relue en trois canaux).
fn aller_retour(t: &Tables, lin: [f32; 3]) -> [f32; 3] {
    lire(t, &spectre(t, lin))
}

/// Mélange soustractif de `a` et `b` (sRVB, 0 à 255), `t` de 0 (a) à 1 (b).
pub fn melanger(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let tab = tables();
    let la = [en_lineaire(tab, a[0]), en_lineaire(tab, a[1]), en_lineaire(tab, a[2])];
    let lb = [en_lineaire(tab, b[0]), en_lineaire(tab, b[1]), en_lineaire(tab, b[2])];
    let (sa, sb) = (spectre(tab, la), spectre(tab, lb));
    let mixte: [f32; BANDES] = std::array::from_fn(|i| sa[i].powf(1.0 - t) * sb[i].powf(t));
    let m = lire(tab, &mixte);
    let (ra, rb) = (aller_retour(tab, la), aller_retour(tab, lb));
    std::array::from_fn(|k| en_srvb(m[k] + (1.0 - t) * (la[k] - ra[k]) + t * (lb[k] - rb[k])))
}

/// Mélange de deux couleurs à un octet par canal (les essais).
#[cfg(test)]
pub fn melanger_u8(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = melanger([a[0] as f32, a[1] as f32, a[2] as f32], [b[0] as f32, b[1] as f32, b[2] as f32], t);
    std::array::from_fn(|k| (m[k] + 0.5).clamp(0.0, 255.0) as u8)
}
