//! Logo et icône de FEANOR : le « Joyau » (une pierre taillée avec une étoile de lumière, qui est aussi la croix de
//! centrage d'un cadre), dessiné en vecteur sur une grille de 64 × 64 puis rasterisé (tiny-skia, sans bibliothèque SVG).
//!
//! Les formes et les couleurs sont celles du livre de marque (`assets/feanor-*.svg` en sont la copie lisible ; un essai
//! vérifie que les points sont les mêmes). Quatre variantes :
//! - **Jade** (référence, partout, dès 16 px, sur fond sombre) : pierre jade, facette basse jade profond, étoile blanche.
//! - **Irisé** (moments forts, jamais sous 32 px : accueil, À propos) : pierre en dégradé jade, lilas, rose, jaune pâle ;
//!   facette basse jade profond à 42 %. Une animation d'ouverture (`etat_animation`) le fait apparaître une fois.
//! - **Mono** : une seule encre, où la pierre et l'étoile s'échangent le plein et le vide (pair-impair) ; pas de facette.
//! - **Icône** : carré arrondi sombre, filet intérieur, Joyau jade réduit à 70 %.
//!
//! Ce fichier ne dépend que de tiny-skia : `build.rs` l'inclut pour fabriquer l'icône de l'exécutable (`feanor.ico`).

use tiny_skia::{Color, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point, SpreadMode, Stroke, Transform};

/// Côté de la grille du logo.
pub const GRILLE: f32 = 64.0;

// ---------------------------------------------------------------------------------------------
// Couleurs du livre de marque (le logo ; celles de l'interface sont dans `theme.rs`).

pub const SOL: [u8; 3] = [0x12, 0x12, 0x14];
pub const CANEVAS: [u8; 3] = [0x2C, 0x2C, 0x2E];
pub const ENCRE_SOMBRE: [u8; 3] = [0xED, 0xED, 0xEF];
pub const ENCRE_CLAIRE: [u8; 3] = [0x12, 0x12, 0x14];
pub const JADE: [u8; 3] = [0x2F, 0xD4, 0xC0];
pub const JADE_PROFOND: [u8; 3] = [0x0E, 0x7C, 0x7B];
pub const ETOILE_BLANCHE: [u8; 3] = [0xF6, 0xFF, 0xFD];
pub const IRIS_LILAS: [u8; 3] = [0xC7, 0xB5, 0xFF];
pub const IRIS_ROSE: [u8; 3] = [0xFF, 0xB3, 0xD1];
pub const IRIS_PALE: [u8; 3] = [0xFF, 0xF1, 0xA8];

// ---------------------------------------------------------------------------------------------
// Formes (grille 64 × 64).

/// La pierre.
pub const PIERRE: [(f32, f32); 5] = [(18.0, 17.0), (44.0, 13.0), (53.0, 35.0), (39.0, 52.0), (14.0, 43.0)];
/// La facette basse.
pub const FACETTE: [(f32, f32); 4] = [(53.0, 35.0), (39.0, 52.0), (14.0, 43.0), (32.0, 32.0)];
/// L'étoile de lumière.
pub const ETOILE: [(f32, f32); 8] = [(32.0, 2.0), (35.0, 29.0), (60.0, 32.0), (35.0, 35.0), (32.0, 62.0), (29.0, 35.0), (4.0, 32.0), (29.0, 29.0)];
/// L'étoile du petit Joyau (sous 24 pixels : bras plus épais pour tenir à 16 px).
pub const ETOILE_PETITE: [(f32, f32); 8] = [(32.0, 2.0), (36.0, 28.0), (60.0, 32.0), (36.0, 36.0), (32.0, 62.0), (28.0, 36.0), (4.0, 32.0), (28.0, 28.0)];
/// Taille (pixels) en dessous de laquelle on dessine le petit Joyau.
pub const PETIT_SOUS: u32 = 24;
/// La seconde étoile, petite, de l'animation d'ouverture.
pub const ETOILE_SECONDE: [(f32, f32); 8] = [(32.0, 14.0), (33.6, 30.4), (50.0, 32.0), (33.6, 33.6), (32.0, 50.0), (30.4, 33.6), (14.0, 32.0), (30.4, 30.4)];

/// Les quatre variantes du Joyau.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Joyau {
    Jade,
    /// Le Joyau jade du dessous de 24 pixels : même pierre et même facette, étoile aux bras plus épais, sans dégradé.
    JadePetit,
    Iris,
    /// Une seule encre (clair sur fond sombre : `ENCRE_SOMBRE` ; sombre sur fond clair : `ENCRE_CLAIRE`).
    Mono([u8; 3]),
}

fn polygone(points: &[(f32, f32)]) -> Path {
    let mut pb = PathBuilder::new();
    pb.move_to(points[0].0, points[0].1);
    for p in &points[1..] {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish().expect("polygone du logo")
}

fn peinture(c: [u8; 3], alpha: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::from_rgba8(c[0], c[1], c[2], (alpha.clamp(0.0, 1.0) * 255.0).round() as u8));
    p.anti_alias = true;
    p
}

/// Pierre irisée : dégradé linéaire (12, 12) → (54, 52), réfléchi, jade / lilas / rose / jaune pâle.
fn degrade_iris(debut: (f32, f32), fin: (f32, f32), alpha: f32) -> Paint<'static> {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    let arret = |p: f32, c: [u8; 3]| GradientStop::new(p, Color::from_rgba8(c[0], c[1], c[2], a));
    let shader = LinearGradient::new(
        Point::from_xy(debut.0, debut.1),
        Point::from_xy(fin.0, fin.1),
        vec![arret(0.0, JADE), arret(0.38, IRIS_LILAS), arret(0.66, IRIS_ROSE), arret(1.0, IRIS_PALE)],
        SpreadMode::Reflect,
        Transform::identity(),
    )
    .expect("dégradé du logo");
    let mut p = Paint::default();
    p.shader = shader;
    p.anti_alias = true;
    p
}

// ---------------------------------------------------------------------------------------------
// Animation d'ouverture (jouée une fois sur le Joyau irisé, puis figée) : 1,15 s.

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

/// Transformation d'une étoile autour du centre (32, 32).
fn autour_du_centre(global: Transform, degres: f32, echelle: f32) -> Transform {
    global.pre_translate(32.0, 32.0).pre_rotate(degres).pre_scale(echelle, echelle).pre_translate(-32.0, -32.0)
}

/// Dessine le Joyau (sans fond) dans `pm`, `global` envoyant la grille de 64 sur les pixels. `anim` : l'état de
/// l'animation d'ouverture (variante irisée).
pub fn dessiner(pm: &mut Pixmap, global: Transform, v: Joyau, anim: Option<&EtatAnim>) {
    match v {
        Joyau::Mono(encre) => {
            // Pair-impair de la pierre et de l'étoile : là où elles se recouvrent, c'est vide ; la facette n'existe pas.
            let mut pb = PathBuilder::new();
            for poly in [&PIERRE[..], &ETOILE[..]] {
                pb.move_to(poly[0].0, poly[0].1);
                for p in &poly[1..] {
                    pb.line_to(p.0, p.1);
                }
                pb.close();
            }
            pm.fill_path(&pb.finish().expect("mono"), &peinture(encre, 1.0), FillRule::EvenOdd, global, None);
        }
        Joyau::Jade | Joyau::JadePetit | Joyau::Iris => {
            let forme_etoile: &[(f32, f32)] = if v == Joyau::JadePetit { &ETOILE_PETITE } else { &ETOILE };
            let (pierre, facette) = if v == Joyau::Iris {
                // (La facette basse reste #0E7C7B à 42 %, au repos comme pendant l'animation.)
                (degrade_iris((12.0, 12.0), (54.0, 52.0), 1.0), peinture(JADE_PROFOND, 0.42))
            } else {
                (peinture(JADE, 1.0), peinture(JADE_PROFOND, 1.0))
            };
            pm.fill_path(&polygone(&PIERRE), &pierre, FillRule::Winding, global, None);
            pm.fill_path(&polygone(&FACETTE), &facette, FillRule::Winding, global, None);
            let blanc = peinture(ETOILE_BLANCHE, 1.0);
            match anim {
                None => pm.fill_path(&polygone(forme_etoile), &blanc, FillRule::Winding, global, None),
                Some(a) => {
                    // (L'étoile à -180° est sa pose de repos : on tourne depuis le repos, de `angle + 180`.)
                    if a.etoile_echelle > 0.001 {
                        pm.fill_path(&polygone(forme_etoile), &blanc, FillRule::Winding, autour_du_centre(global, a.etoile_angle + 180.0, a.etoile_echelle), None);
                    }
                    if a.seconde_echelle > 0.001 && a.seconde_opacite > 0.002 {
                        pm.fill_path(&polygone(&ETOILE_SECONDE), &peinture(ETOILE_BLANCHE, a.seconde_opacite), FillRule::Winding, autour_du_centre(global, a.seconde_angle, a.seconde_echelle), None);
                    }
                }
            }
        }
    }
}

/// Le Joyau seul (fond transparent), carré de `taille` pixels. Pixels RVBA **prémultipliés** (la forme qu'attend
/// `ColorImage::from_rgba_premultiplied`).
pub fn image(taille: u32, v: Joyau, anim: Option<&EtatAnim>) -> Vec<u8> {
    let taille = taille.max(8);
    // Sous 24 pixels, le Joyau jade est le petit.
    let v = if v == Joyau::Jade && taille < PETIT_SOUS { Joyau::JadePetit } else { v };
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
/// arrondi `SOL` (rayon 14 sur 64, soit 22 %), filet intérieur `CANEVAS`, puis le Joyau jade à 80 %, centré.
pub fn icone(taille: u32) -> Vec<u8> {
    let taille = taille.max(8);
    let mut pm = Pixmap::new(taille, taille).expect("taille de l'icône");
    let s = taille as f32 / GRILLE;
    let g = Transform::from_scale(s, s);
    pm.fill_path(&rect_arrondi(0.0, 0.0, 64.0, 64.0, 14.0), &peinture(SOL, 1.0), FillRule::Winding, g, None);
    let filet = Stroke { width: 1.0, ..Stroke::default() };
    pm.stroke_path(&rect_arrondi(0.5, 0.5, 63.0, 63.0, 13.5), &peinture(CANEVAS, 1.0), &filet, g, None);
    // (À 16 pixels, le petit Joyau.)
    let joyau = if taille < PETIT_SOUS { Joyau::JadePetit } else { Joyau::Jade };
    dessiner(&mut pm, g.pre_translate(6.4, 6.4).pre_scale(0.8, 0.8), joyau, None);
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
