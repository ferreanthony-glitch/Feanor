//! Logo et icône de FEANOR : le **joyau** de la charte du 17/11/2026, dessiné en vecteur puis rasterisé (tiny-skia, sans
//! bibliothèque SVG). Une pierre en losange aux coins adoucis, taillée en huit facettes autour d'une étoile blanche à quatre
//! branches ; dégradé vertical jade → turquoise → bleu ciel → bleu et lilas pâles ; lumière venue d'en haut à gauche (les
//! facettes tournées vers elle s'éclairent de blanc, les autres s'ombrent dans leur propre couleur, en « produit »).
//!
//! Le dessin est fait sur 100 unités (`DESSIN`), puis ramené sur la grille de 64 (`GRILLE`) où vivent l'animation et
//! l'icône. Les fichiers lisibles sont `assets/feanor-logo*.svg` (copie de `feanor-site/logo/silmaril/public`, que
//! `fabrique.mjs` refait ; un essai vérifie que les formes et les couleurs sont les mêmes). Variantes :
//! - **Jade** (référence, partout, dès 16 px) et **Iris** (moments forts : accueil) : le joyau en couleur.
//!   `JadePetit` est gardé pour les appels existants : le joyau tient à 16 px, c'est le même dessin.
//! - **Mono** : une seule encre, la pierre en aplat et l'étoile découpée ; pas de facettes.
//! - **Icône** : carré arrondi sombre, filet intérieur, joyau réduit à 80 %.
//!
//! Ce fichier ne dépend que de tiny-skia : `build.rs` l'inclut pour fabriquer l'icône de l'exécutable (`feanor.ico`).

use tiny_skia::{BlendMode, Color, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point, SpreadMode, Stroke, Transform};

/// Côté de la grille du logo (animation, icône).
pub const GRILLE: f32 = 64.0;
/// Côté du dessin du joyau : ses formes sont données sur 100 unités, ramenées sur la grille par `GRILLE / DESSIN`.
pub const DESSIN: f32 = 100.0;

// ---------------------------------------------------------------------------------------------
// Couleurs du livre de marque (le logo ; celles de l'interface sont dans `theme.rs`).

pub const SOL: [u8; 3] = [0x12, 0x12, 0x14];
pub const CANEVAS: [u8; 3] = [0x2C, 0x2C, 0x2E];
pub const ENCRE_SOMBRE: [u8; 3] = [0xED, 0xED, 0xEF];
pub const ENCRE_CLAIRE: [u8; 3] = [0x12, 0x12, 0x14];
/// La couleur d'identité, en haut du joyau.
pub const JADE: [u8; 3] = [0x2F, 0xD4, 0xC0];
pub const JADE_PROFOND: [u8; 3] = [0x0E, 0x7C, 0x7B];
/// L'étoile, blanche dans toutes les versions en couleur.
pub const ETOILE_BLANCHE: [u8; 3] = [0xFF, 0xFF, 0xFF];
pub const TURQUOISE: [u8; 3] = [0x2F, 0xCC, 0xDB];
pub const BLEU_CIEL: [u8; 3] = [0x86, 0xC7, 0xEF];
pub const BLEU_PALE: [u8; 3] = [0xB4, 0xC1, 0xF4];
pub const LILAS_PALE: [u8; 3] = [0xD6, 0xC3, 0xEC];
/// Le fond (jade très pâle) sous les deux quarts du bas : le bas du joyau tire vers le jade.
pub const VOILE: [u8; 3] = [0xD3, 0xF5, 0xEF];
/// Les ombres des facettes, posées en « produit » : turquoise profond à droite, bleu violet en bas.
pub const OMBRE_TURQUOISE: [u8; 3] = [0x2E, 0x9F, 0xBF];
pub const OMBRE_VIOLETTE: [u8; 3] = [0x6A, 0x6F, 0xD6];
// (Les couleurs irisées de l'ancien Joyau servent encore aux barres de chargement, `barre.rs`.)
pub const IRIS_LILAS: [u8; 3] = [0xC7, 0xB5, 0xFF];
pub const IRIS_ROSE: [u8; 3] = [0xFF, 0xB3, 0xD1];
pub const IRIS_PALE: [u8; 3] = [0xFF, 0xF1, 0xA8];

/// Le dégradé du joyau, de haut (0) en bas (1).
pub const DEGRADE: [(f32, [u8; 3]); 6] = [(0.0, JADE), (0.1, JADE), (0.38, TURQUOISE), (0.66, BLEU_CIEL), (0.88, BLEU_PALE), (1.0, LILAS_PALE)];

// ---------------------------------------------------------------------------------------------
// Formes (sur 100 unités).

/// Les quatre sommets du losange : haut, droite, bas, gauche (sens des aiguilles d'une montre).
pub const SOMMETS: [(f32, f32); 4] = [(50.0, 2.0), (86.0, 50.0), (50.0, 98.0), (14.0, 50.0)];
/// Rayon des coins adoucis, mesuré le long des côtés.
pub const RAYON_COINS: f32 = 5.0;
/// L'étoile : pointes et creux, en partant du haut.
pub const ETOILE: [(f32, f32); 8] = [(50.0, 30.0), (54.2, 45.8), (68.0, 50.0), (54.2, 54.2), (50.0, 70.0), (45.8, 54.2), (32.0, 50.0), (45.8, 45.8)];
/// La seconde étoile, petite, de l'animation d'ouverture.
pub const ETOILE_SECONDE: [(f32, f32); 8] = [(50.0, 38.0), (51.3, 48.7), (62.0, 50.0), (51.3, 51.3), (50.0, 62.0), (48.7, 51.3), (38.0, 50.0), (48.7, 48.7)];
/// Ce que la pierre occupe vraiment (coins arrondis compris), sur 100 : haut, bas, gauche, droite.
pub const PIERRE_HAUT: f32 = 4.0;
pub const PIERRE_BAS: f32 = 96.0;
pub const PIERRE_GAUCHE: f32 = 15.5;
pub const PIERRE_DROITE: f32 = 84.5;

/// Les quatre quarts (0 haut droite, 1 bas droite, 2 bas gauche, 3 haut gauche) : opacité du dégradé et fond dessous.
pub const QUARTS: [(f32, [u8; 3]); 4] = [(0.78, [0xFF, 0xFF, 0xFF]), (0.46, VOILE), (0.62, VOILE), (1.0, [0xFF, 0xFF, 0xFF])];
/// La lumière de chacune des huit facettes, dans l'ordre du dessin (pour chaque quart : la moitié côté du sommet de départ,
/// puis celle côté du sommet d'arrivée). `Some((couleur, opacité, produit))` ; le blanc s'ajoute, l'ombre se multiplie.
pub const LUMIERES: [(([u8; 3], f32), bool); 8] = [
    (([0xFF, 0xFF, 0xFF], 0.04), false), // haut droite, côté sommet
    ((OMBRE_TURQUOISE, 0.26), true),     // haut droite, côté droit
    ((OMBRE_VIOLETTE, 0.34), true),      // bas droite, côté droit
    ((OMBRE_VIOLETTE, 0.48), true),      // bas droite, côté pointe (la plus sombre)
    ((OMBRE_VIOLETTE, 0.34), true),      // bas gauche, côté pointe
    ((OMBRE_VIOLETTE, 0.16), true),      // bas gauche, côté gauche
    (([0xFF, 0xFF, 0xFF], 0.06), false), // haut gauche, côté gauche
    (([0xFF, 0xFF, 0xFF], 0.20), false), // haut gauche, côté sommet (la plus claire)
];

/// Les variantes du joyau.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Joyau {
    Jade,
    /// Gardé pour les appels existants : le même joyau (il tient à 16 pixels).
    JadePetit,
    Iris,
    /// Une seule encre (clair sur fond sombre : `ENCRE_SOMBRE` ; sombre sur fond clair : `ENCRE_CLAIRE`).
    Mono([u8; 3]),
}

/// Le coin arrondi d'un sommet : entrée, contrôles et milieu de la courbe (coupée en son milieu, chaque moitié appartenant
/// à une facette).
#[derive(Clone, Copy)]
struct Coin {
    a: (f32, f32),
    b: (f32, f32),
    m: (f32, f32),
    c1: (f32, f32),
    c2: (f32, f32),
}

fn vers(a: (f32, f32), b: (f32, f32), d: f32) -> (f32, f32) {
    let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    (a.0 + (b.0 - a.0) * d / l, a.1 + (b.1 - a.1) * d / l)
}

fn coins() -> [Coin; 4] {
    std::array::from_fn(|i| {
        let v = SOMMETS[i];
        let a = vers(v, SOMMETS[(i + 3) % 4], RAYON_COINS);
        let b = vers(v, SOMMETS[(i + 1) % 4], RAYON_COINS);
        Coin { a, b, m: (0.25 * a.0 + 0.5 * v.0 + 0.25 * b.0, 0.25 * a.1 + 0.5 * v.1 + 0.25 * b.1), c1: ((a.0 + v.0) / 2.0, (a.1 + v.1) / 2.0), c2: ((v.0 + b.0) / 2.0, (v.1 + b.1) / 2.0) }
    })
}

/// Les huit facettes (sur 100), dans l'ordre de `LUMIERES`.
pub fn facettes() -> Vec<Path> {
    let c = coins();
    let pointe = |k: usize| ETOILE[2 * (k % 4)];
    let creux = |k: usize| ETOILE[(2 * k + 1) % 8];
    let mut v = Vec::new();
    for i in 0..4 {
        let (d, f) = (c[i], c[(i + 1) % 4]);
        let milieu = ((d.b.0 + f.a.0) / 2.0, (d.b.1 + f.a.1) / 2.0);
        let mut pb = PathBuilder::new();
        pb.move_to(d.m.0, d.m.1);
        pb.quad_to(d.c2.0, d.c2.1, d.b.0, d.b.1);
        pb.line_to(milieu.0, milieu.1);
        pb.line_to(creux(i).0, creux(i).1);
        pb.line_to(pointe(i).0, pointe(i).1);
        pb.close();
        v.push(pb.finish().expect("facette"));
        let mut pb = PathBuilder::new();
        pb.move_to(milieu.0, milieu.1);
        pb.line_to(f.a.0, f.a.1);
        pb.quad_to(f.c1.0, f.c1.1, f.m.0, f.m.1);
        pb.line_to(pointe(i + 1).0, pointe(i + 1).1);
        pb.line_to(creux(i).0, creux(i).1);
        pb.close();
        v.push(pb.finish().expect("facette"));
    }
    v
}

/// Le contour de la pierre (sur 100), et l'étoile en second sous-chemin si `avec_etoile` (pour le mono, en pair-impair).
pub fn contour(avec_etoile: bool) -> Path {
    let c = coins();
    let mut pb = PathBuilder::new();
    pb.move_to(c[0].m.0, c[0].m.1);
    for i in 0..4 {
        let (d, f) = (c[i], c[(i + 1) % 4]);
        pb.quad_to(d.c2.0, d.c2.1, d.b.0, d.b.1);
        pb.line_to(f.a.0, f.a.1);
        pb.quad_to(f.c1.0, f.c1.1, f.m.0, f.m.1);
    }
    pb.close();
    if avec_etoile {
        pb.move_to(ETOILE[0].0, ETOILE[0].1);
        for p in &ETOILE[1..] {
            pb.line_to(p.0, p.1);
        }
        pb.close();
    }
    pb.finish().expect("contour du joyau")
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

/// Le dégradé vertical du joyau (sur 100), à l'opacité `alpha`.
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
// Animation d'ouverture (jouée une fois à l'accueil, puis figée) : 1,15 s. L'étoile blanche s'ouvre en tourbillon.

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

/// Dessine le joyau (sans fond) dans `pm`, `global` envoyant la grille de 64 sur les pixels. `anim` : l'état de
/// l'animation d'ouverture (l'étoile blanche s'ouvre, la pierre ne bouge pas).
pub fn dessiner(pm: &mut Pixmap, global: Transform, v: Joyau, anim: Option<&EtatAnim>) {
    let k = GRILLE / DESSIN;
    let g = global.pre_scale(k, k);
    match v {
        Joyau::Mono(encre) => {
            // La pierre en aplat, l'étoile découpée (pair-impair) ; pas de facettes.
            pm.fill_path(&contour(true), &peinture(encre, 1.0), FillRule::EvenOdd, g, None);
        }
        Joyau::Jade | Joyau::JadePetit | Joyau::Iris => {
            let f = facettes();
            for (n, chemin) in f.iter().enumerate() {
                let (opacite, fond) = QUARTS[n / 2];
                pm.fill_path(chemin, &peinture(fond, 1.0), FillRule::Winding, g, None);
                pm.fill_path(chemin, &degrade(opacite), FillRule::Winding, g, None);
            }
            for (chemin, &((couleur, opacite), produit)) in f.iter().zip(LUMIERES.iter()) {
                let mut p = peinture(couleur, opacite);
                if produit {
                    p.blend_mode = BlendMode::Multiply;
                }
                pm.fill_path(chemin, &p, FillRule::Winding, g, None);
            }
            let blanc = peinture(ETOILE_BLANCHE, 1.0);
            match anim {
                None => pm.fill_path(&polygone(&ETOILE), &blanc, FillRule::Winding, g, None),
                Some(a) => {
                    // (L'étoile à -180° est sa pose de repos : on tourne depuis le repos, de `angle + 180`.)
                    if a.etoile_echelle > 0.001 {
                        pm.fill_path(&polygone(&ETOILE), &blanc, FillRule::Winding, autour_du_centre(global, a.etoile_angle + 180.0, a.etoile_echelle).pre_scale(k, k), None);
                    }
                    if a.seconde_echelle > 0.001 && a.seconde_opacite > 0.002 {
                        pm.fill_path(&polygone(&ETOILE_SECONDE), &peinture(ETOILE_BLANCHE, a.seconde_opacite), FillRule::Winding, autour_du_centre(global, a.seconde_angle, a.seconde_echelle).pre_scale(k, k), None);
                    }
                }
            }
        }
    }
}

/// Le joyau seul (fond transparent), carré de `taille` pixels. Pixels RVBA **prémultipliés** (la forme qu'attend
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
/// arrondi `SOL` (rayon 14 sur 64, soit 22 %), filet intérieur `CANEVAS`, puis le joyau à 80 %, centré.
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
