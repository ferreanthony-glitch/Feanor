//! Logo et icône de FEANOR : l'**étoile** de la charte du 17/11/2026, dessinée en vecteur puis rasterisée (tiny-skia, sans
//! bibliothèque SVG). Une étoile à quatre branches aux flancs creusés, la branche verticale plus longue, sans facettes, au
//! dégradé vertical jade → turquoise → bleu ciel → bleu et lilas pâles.
//!
//! Le dessin est fait sur 100 unités (`DESSIN`), puis ramené sur la grille de 64 (`GRILLE`) où vivent l'animation et
//! l'icône. Les fichiers lisibles sont `assets/feanor-logo*.svg` (copie de `feanor-site/logo/silmaril/public`, que
//! `fabrique.mjs` refait ; un essai vérifie que la forme et les couleurs sont les mêmes). Variantes :
//! - **Jade** (référence, partout, dès 16 px) et **Iris** (moments forts : accueil) : l'étoile en dégradé.
//!   `JadePetit` est gardé pour les appels existants : l'étoile tient à 16 px, c'est le même dessin.
//! - **Mono** : une seule encre, en aplat.
//! - **Icône** : carré arrondi sombre, filet intérieur, étoile réduite à 80 %.
//!
//! Ce fichier ne dépend que de tiny-skia : `build.rs` l'inclut pour fabriquer l'icône de l'exécutable (`feanor.ico`).

use tiny_skia::{Color, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point, SpreadMode, Stroke, Transform};

/// Côté de la grille du logo (animation, icône).
pub const GRILLE: f32 = 64.0;
/// Côté du dessin de l'étoile : ses formes sont données sur 100 unités, ramenées sur la grille par `GRILLE / DESSIN`.
pub const DESSIN: f32 = 100.0;

// ---------------------------------------------------------------------------------------------
// Couleurs du livre de marque (le logo ; celles de l'interface sont dans `theme.rs`).

pub const SOL: [u8; 3] = [0x12, 0x12, 0x14];
pub const CANEVAS: [u8; 3] = [0x2C, 0x2C, 0x2E];
pub const ENCRE_SOMBRE: [u8; 3] = [0xED, 0xED, 0xEF];
pub const ENCRE_CLAIRE: [u8; 3] = [0x12, 0x12, 0x14];
/// La couleur d'identité, en haut de l'étoile.
pub const JADE: [u8; 3] = [0x2F, 0xD4, 0xC0];
pub const JADE_PROFOND: [u8; 3] = [0x0E, 0x7C, 0x7B];
pub const TURQUOISE: [u8; 3] = [0x2F, 0xCC, 0xDB];
pub const BLEU_CIEL: [u8; 3] = [0x86, 0xC7, 0xEF];
pub const BLEU_PALE: [u8; 3] = [0xB4, 0xC1, 0xF4];
pub const LILAS_PALE: [u8; 3] = [0xD6, 0xC3, 0xEC];
// (Les couleurs irisées de l'ancien Joyau servent encore aux barres de chargement, `barre.rs`.)
pub const IRIS_LILAS: [u8; 3] = [0xC7, 0xB5, 0xFF];
pub const IRIS_ROSE: [u8; 3] = [0xFF, 0xB3, 0xD1];
pub const IRIS_PALE: [u8; 3] = [0xFF, 0xF1, 0xA8];

/// Le dégradé de l'étoile, de haut (0) en bas (1).
pub const DEGRADE: [(f32, [u8; 3]); 6] = [(0.0, JADE), (0.1, JADE), (0.38, TURQUOISE), (0.66, BLEU_CIEL), (0.88, BLEU_PALE), (1.0, LILAS_PALE)];

// ---------------------------------------------------------------------------------------------
// Forme (sur 100 unités).

/// L'étoile : les quatre pointes (haut, droite, bas, gauche) et, entre elles, les points de contrôle des flancs creusés
/// (courbes quadratiques) : pointe, contrôle, pointe, contrôle…
pub const ETOILE: [(f32, f32); 8] = [(50.0, 2.0), (59.0, 41.0), (82.0, 50.0), (59.0, 59.0), (50.0, 98.0), (41.0, 59.0), (18.0, 50.0), (41.0, 41.0)];
/// Les pointes sont très légèrement arrondies : chaque flanc est raccourci de cette part de sa longueur (en paramètre de
/// courbe) à chaque bout, et les deux bouts se rejoignent par une petite courbe dont la pointe est le point de contrôle.
pub const ARRONDI_POINTES: f32 = 0.09;
/// Ce que l'étoile occupe, sur 100 : haut, bas, gauche, droite.
pub const PIERRE_HAUT: f32 = 2.0;
pub const PIERRE_BAS: f32 = 98.0;
pub const PIERRE_GAUCHE: f32 = 18.0;
pub const PIERRE_DROITE: f32 = 82.0;
/// La seconde étoile de l'animation d'ouverture : la même, à cette échelle.
pub const ECHELLE_SECONDE: f32 = 0.45;

/// Les variantes de l'étoile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Joyau {
    Jade,
    /// Gardé pour les appels existants : la même étoile (elle tient à 16 pixels).
    JadePetit,
    Iris,
    /// Une seule encre (clair sur fond sombre : `ENCRE_SOMBRE` ; sombre sur fond clair : `ENCRE_CLAIRE`).
    Mono([u8; 3]),
}

/// Le contour de l'étoile (sur 100), pointes arrondies.
pub fn etoile() -> Path {
    let q = |p0: (f32, f32), c: (f32, f32), p2: (f32, f32), t: f32| {
        let u = 1.0 - t;
        (u * u * p0.0 + 2.0 * t * u * c.0 + t * t * p2.0, u * u * p0.1 + 2.0 * t * u * c.1 + t * t * p2.1)
    };
    // Le point de contrôle du morceau de courbe entre t0 et t1.
    let sous = |p0: (f32, f32), c: (f32, f32), p2: (f32, f32), t0: f32, t1: f32| {
        let (a, b, d) = ((1.0 - t0) * (1.0 - t1), t0 * (1.0 - t1) + t1 * (1.0 - t0), t0 * t1);
        (a * p0.0 + b * c.0 + d * p2.0, a * p0.1 + b * c.1 + d * p2.1)
    };
    let r = ARRONDI_POINTES;
    let flanc = |k: usize| (ETOILE[2 * k], ETOILE[2 * k + 1], ETOILE[(2 * k + 2) % 8]);
    let mut pb = PathBuilder::new();
    for k in 0..4 {
        let (p0, c, p2) = flanc(k);
        let (a, b, m) = (q(p0, c, p2, r), q(p0, c, p2, 1.0 - r), sous(p0, c, p2, r, 1.0 - r));
        if k == 0 {
            pb.move_to(a.0, a.1);
        } else {
            pb.quad_to(p0.0, p0.1, a.0, a.1);
        }
        pb.quad_to(m.0, m.1, b.0, b.1);
    }
    let (p0, c, p2) = flanc(0);
    let debut = q(p0, c, p2, r);
    pb.quad_to(p0.0, p0.1, debut.0, debut.1);
    pb.close();
    pb.finish().expect("étoile du logo")
}

fn peinture(c: [u8; 3], alpha: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(c[0], c[1], c[2], (alpha.clamp(0.0, 1.0) * 255.0).round() as u8));
    p.anti_alias = true;
    p
}

/// Le dégradé vertical de l'étoile (sur 100), à l'opacité `alpha`.
fn degrade(alpha: f32) -> Paint<'static> {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    let arrets = DEGRADE.iter().map(|&(p, c)| GradientStop::new(p, Color::from_rgba8(c[0], c[1], c[2], a))).collect();
    let shader = LinearGradient::new(Point::from_xy(50.0, 2.0), Point::from_xy(50.0, 98.0), arrets, SpreadMode::Pad, Transform::identity()).expect("dégradé du logo");
    let mut p = Paint::default();
    p.shader = shader;
    p.anti_alias = true;
    p
}

// ---------------------------------------------------------------------------------------------
// Animation d'ouverture (jouée une fois à l'accueil, puis figée) : 1,15 s. L'étoile s'ouvre en tourbillon.

/// Durée de l'animation d'ouverture, en secondes.
pub const DUREE_ANIMATION: f32 = 1.15;

/// L'état des deux étoiles à un instant de l'animation. Les angles sont ceux du livre de marque (CSS : -180° est la
/// pose de repos de l'étoile).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EtatAnim {
    pub etoile_angle: f32,
    pub etoile_echelle: f32,
    pub seconde_angle: f32,
    pub seconde_echelle: f32,
    pub seconde_opacite: f32,
}

/// Courbe de Bézier cubique CSS `cubic-bezier(x1, y1, x2, y2)` : la progression pour un avancement `t` (0 à 1).
pub fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let coord = |a: f32, b: f32, u: f32| 3.0 * a * u * (1.0 - u) * (1.0 - u) + 3.0 * b * u * u * (1.0 - u) + u * u * u;
    let (mut bas, mut haut) = (0.0_f32, 1.0_f32);
    let mut u = t;
    for _ in 0..32 {
        let x = coord(x1, x2, u);
        if (x - t).abs() < 1e-5 {
            break;
        }
        if x < t {
            bas = u;
        } else {
            haut = u;
        }
        u = (bas + haut) / 2.0;
    }
    coord(y1, y2, u)
}

/// Une valeur le long de segments de clés : `(part, valeur)` croissants, `facile[k]` est la courbe du segment k.
fn le_long(p: f32, cles: &[(f32, f32)], faciles: &[(f32, f32, f32, f32)]) -> f32 {
    for k in 0..cles.len() - 1 {
        let ((p0, v0), (p1, v1)) = (cles[k], cles[k + 1]);
        if p <= p1 {
            let t = if p1 > p0 { (p - p0) / (p1 - p0) } else { 1.0 };
            let (a, b, c, d) = faciles[k];
            return v0 + (v1 - v0) * bezier(a, b, c, d, t);
        }
    }
    cles[cles.len() - 1].1
}

/// L'état de l'animation à `t` secondes (au-delà de `DUREE_ANIMATION` : l'état final, qui est la pose de repos).
pub fn etat_animation(t: f32) -> EtatAnim {
    let p = (t / DUREE_ANIMATION).clamp(0.0, 1.0);
    let premiere = (0.15, 0.6, 0.3, 1.0);
    let autres = (0.45, 0.0, 0.25, 1.0);
    let etoile_angle = le_long(p, &[(0.0, -200.0), (0.34, -180.0), (0.72, -180.0), (1.0, -180.0)], &[premiere, autres, autres]);
    let etoile_echelle = le_long(p, &[(0.0, 0.0), (0.34, 1.25), (0.72, 0.96), (1.0, 1.0)], &[premiere, autres, autres]);
    let seconde_angle = le_long(p, &[(0.0, -155.0), (0.34, -135.0), (0.8, -135.0), (1.0, -135.0)], &[premiere, autres, autres]);
    let seconde_echelle = le_long(p, &[(0.0, 0.0), (0.34, 1.15), (0.8, 0.45), (1.0, 0.45)], &[premiere, autres, autres]);
    let seconde_opacite = le_long(p, &[(0.0, 0.9), (0.34, 0.9), (0.8, 0.0), (1.0, 0.0)], &[premiere, autres, autres]);
    EtatAnim { etoile_angle, etoile_echelle, seconde_angle, seconde_echelle, seconde_opacite }
}

/// Le Joyau au repos vu comme un état d'animation : l'étoile à -180° à l'échelle 1, la seconde étoile invisible.
pub fn etat_repos() -> EtatAnim {
    etat_animation(DUREE_ANIMATION)
}

// ---------------------------------------------------------------------------------------------
// Dessin.

/// Transformation d'une étoile autour du centre (32, 32) de la grille.
fn autour_du_centre(global: Transform, degres: f32, echelle: f32) -> Transform {
    global.pre_translate(32.0, 32.0).pre_rotate(degres).pre_scale(echelle, echelle).pre_translate(-32.0, -32.0)
}

/// Dessine l'étoile (sans fond) dans `pm`, `global` envoyant la grille de 64 sur les pixels. `anim` : l'état de
/// l'animation d'ouverture (l'étoile s'ouvre en tourbillon, une seconde étoile plus petite s'efface).
pub fn dessiner(pm: &mut Pixmap, global: Transform, v: Joyau, anim: Option<&EtatAnim>) {
    let k = GRILLE / DESSIN;
    let forme = etoile();
    let peint = |alpha: f32| match v {
        Joyau::Mono(encre) => peinture(encre, alpha),
        _ => degrade(alpha),
    };
    match anim {
        None => pm.fill_path(&forme, &peint(1.0), FillRule::Winding, global.pre_scale(k, k), None),
        Some(a) => {
            // (L'étoile à -180° est sa pose de repos : on tourne depuis le repos, de `angle + 180`.)
            if a.seconde_echelle > 0.001 && a.seconde_opacite > 0.002 {
                let t = autour_du_centre(global, a.seconde_angle, a.seconde_echelle * ECHELLE_SECONDE).pre_scale(k, k);
                pm.fill_path(&forme, &peint(a.seconde_opacite), FillRule::Winding, t, None);
            }
            if a.etoile_echelle > 0.001 {
                pm.fill_path(&forme, &peint(1.0), FillRule::Winding, autour_du_centre(global, a.etoile_angle + 180.0, a.etoile_echelle).pre_scale(k, k), None);
            }
        }
    }
}

/// L'étoile seule (fond transparent), carré de `taille` pixels. Pixels RVBA **prémultipliés** (la forme qu'attend
/// `ColorImage::from_rgba_premultiplied`).
pub fn image(taille: u32, v: Joyau, anim: Option<&EtatAnim>) -> Vec<u8> {
    let taille = taille.max(8);
    let mut pm = Pixmap::new(taille, taille).expect("taille du logo");
    let s = taille as f32 / GRILLE;
    dessiner(&mut pm, Transform::from_scale(s, s), v, anim);
    pm.take()
}

/// Un rectangle arrondi (courbes de Bézier cubiques).
fn rect_arrondi(x: f32, y: f32, l: f32, h: f32, r: f32) -> Path {
    let k = r * 0.552_284_75;
    let (d, b) = (x + l, y + h);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(d - r, y);
    pb.cubic_to(d - r + k, y, d, y + r - k, d, y + r);
    pb.line_to(d, b - r);
    pb.cubic_to(d, b - r + k, d - r + k, b, d - r, b);
    pb.line_to(x + r, b);
    pb.cubic_to(x + r - k, b, x, b - r + k, x, b - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish().expect("rectangle arrondi")
}

/// L'icône de l'application, carrée, de `taille` pixels, en RVBA **non prémultipliés** (`IconData`, `.ico`) : carré
/// arrondi `SOL` (rayon 14 sur 64, soit 22 %), filet intérieur `CANEVAS`, puis l'étoile à 80 %, centrée.
pub fn icone(taille: u32) -> Vec<u8> {
    let taille = taille.max(8);
    let mut pm = Pixmap::new(taille, taille).expect("taille de l'icône");
    let s = taille as f32 / GRILLE;
    let g = Transform::from_scale(s, s);
    pm.fill_path(&rect_arrondi(0.0, 0.0, 64.0, 64.0, 14.0), &peinture(SOL, 1.0), FillRule::Winding, g, None);
    let filet = Stroke { width: 1.0, ..Stroke::default() };
    pm.stroke_path(&rect_arrondi(0.5, 0.5, 63.0, 63.0, 13.5), &peinture(CANEVAS, 1.0), &filet, g, None);
    dessiner(&mut pm, g.pre_translate(6.4, 6.4).pre_scale(0.8, 0.8), Joyau::Jade, None);
    // Pixmap prémultiplié -> non prémultiplié.
    let mut px = pm.take();
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a != 0 && a != 255 {
            for v in &mut p[..3] {
                *v = ((*v as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    px
}

/// Un fichier `.ico` (images 32 bits en BMP, lisibles par `rc.exe` et par l'Explorateur) avec une
/// icône par taille de `tailles` (au plus 256).
pub fn ico(tailles: &[u32]) -> Vec<u8> {
    let images: Vec<(u32, Vec<u8>)> = tailles
        .iter()
        .map(|&t| {
            let t = t.clamp(8, 256);
            let rvba = icone(t);
            let mut dib = Vec::new();
            for v in [40u32, t, t * 2] {
                dib.extend_from_slice(&v.to_le_bytes());
            }
            dib.extend_from_slice(&1u16.to_le_bytes()); // plans
            dib.extend_from_slice(&32u16.to_le_bytes()); // bits par pixel
            dib.extend_from_slice(&[0u8; 24]); // compression, taille, résolutions, palette
            // Pixels BVRA, de bas en haut.
            for y in (0..t as usize).rev() {
                for x in 0..t as usize {
                    let p = &rvba[(y * t as usize + x) * 4..][..4];
                    dib.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
                }
            }
            // Masque ET (1 bit par pixel, lignes sur 4 octets) : tout à 0, l'opacité est dans alpha.
            dib.resize(dib.len() + (t as usize).div_ceil(32) * 4 * t as usize, 0);
            (t, dib)
        })
        .collect();
    let mut out = vec![0, 0, 1, 0];
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (t, dib) in &images {
        let cote = if *t >= 256 { 0 } else { *t as u8 };
        out.extend_from_slice(&[cote, cote, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(dib.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += dib.len() as u32;
    }
    for (_, dib) in images {
        out.extend_from_slice(&dib);
    }
    out
}
