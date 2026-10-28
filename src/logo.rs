//! Logo et icône de SHOT, dessinés en vecteur puis rasterisés (tiny-skia, sans bibliothèque SVG).
//!
//! Les formes viennent de `assets/shot-logo.svg` (le logo d'Anthony, fichier de 960 × 720) :
//! rectangles, le « S » (un chemin M/Q/H/V/L/Z), l'anneau du « O » (un chemin à arcs, règle
//! pair-impair) et la ligne « FRAME THE STORY » en petits traits. Les chemins sont recopiés tels
//! quels dans ce fichier (`chemin` lit les commandes M L H V Q A Z), pour ne pas risquer une faute
//! de recopie des coordonnées. « STORY » est en couleur du fond parce que, dans le logo, il est
//! écrit en creux dans la barre claire en bas à droite.
//!
//! Ce fichier ne dépend que de tiny-skia : `build.rs` l'inclut pour fabriquer l'icône de
//! l'exécutable (`shot.ico`) sans copier le dessin.

use tiny_skia::{FillRule, LineJoin, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform};

/// Cadre du logo dans le fichier SVG (le logo seul, avec un peu d'air), en unités du SVG.
pub const VUE_X: f32 = 234.0;
pub const VUE_Y: f32 = 196.0;
pub const VUE_L: f32 = 474.0;
pub const VUE_H: f32 = 266.0;

/// Les couleurs de la charte graphique de SHOT (planche d'Anthony, 2026-10-28).
pub const NOIR_CINEMA: [u8; 3] = [0x0E, 0x0E, 0x0E];
pub const BLANC_CASSE: [u8; 3] = [0xF4, 0xF1, 0xEA];
pub const ORANGE_SHOT: [u8; 3] = [0xE7, 0x6F, 0x3C];

/// Logo clair sur fond sombre (l'original) ou sombre sur fond clair.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Variante {
    Sombre,
    Clair,
}

/// Couleurs du logo : lettres, angles et ligne « FRAME THE STORY », puis le fond de la barre pour
/// « STORY », qui y est écrit en creux.
struct Couleurs {
    logo: [u8; 3],
    creux: [u8; 3],
}

impl Variante {
    fn couleurs(self) -> Couleurs {
        match self {
            // Blanc cassé sur noir cinéma (la charte).
            Variante::Sombre => Couleurs { logo: BLANC_CASSE, creux: NOIR_CINEMA },
            // Le même dessin en négatif, pour un thème clair : noir cinéma sur blanc cassé.
            Variante::Clair => Couleurs { logo: NOIR_CINEMA, creux: BLANC_CASSE },
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Formes du logo (coordonnées du SVG).

/// Rectangles pleins : x, y, largeur, hauteur (angle haut gauche, H, T, angle bas droite).
const RECTS: [[f32; 4]; 9] = [
    [242.6, 204.0, 97.0, 23.0],  // corner-tl-bar
    [242.6, 204.0, 20.0, 66.0],  // corner-tl-stem
    [393.0, 242.0, 28.0, 176.0], // h-stem-left
    [446.0, 242.0, 28.0, 176.0], // h-stem-right
    [393.0, 316.0, 81.0, 25.0],  // h-bar
    [610.0, 242.0, 82.0, 25.0],  // t-bar
    [637.0, 242.0, 28.0, 176.0], // t-stem
    [680.0, 388.0, 20.0, 66.0],  // corner-br-stem
    [603.0, 431.0, 97.0, 23.0],  // corner-br-box
];

/// Le « S » de la typo.
pub const S_GLYPHE: &str = "M322 420Q299.3 420 289.3 410.7Q279.2 401.5 278.3 379.9Q278.2 375.6 278.2 371Q278.2 366.4 278.4 361.8Q278.7 357.2 279.1 352.7H305.7Q305.3 361.2 305.2 369.4Q305.2 377.6 305.7 384.9Q306.3 390.9 310.2 393.9Q314.2 396.8 321.9 396.8Q329.3 396.8 333.1 393.9Q336.9 390.9 337.5 384.9Q337.9 380.8 338.1 376.6Q338.2 372.4 338 368.1Q337.8 363.9 337.5 359.8Q337.1 354 333.7 350Q330.4 345.9 324.4 344.5L306.7 340.4Q295.9 337.9 289.7 332.8Q283.5 327.6 280.8 319.5Q278 311.5 277.7 300.5Q277.6 295 277.6 290Q277.6 285 277.7 279.8Q278.2 265.5 282.9 256.8Q287.6 248.1 297.1 244Q306.6 240 321.9 240Q344.2 240 354.2 249.2Q364.2 258.4 365 279.8Q365.2 285.2 365.1 292.5Q365 299.8 364.7 306.5H337.5Q337.8 298.2 338 290.4Q338.1 282.6 337.7 274.9Q337.5 268.9 333.6 266.1Q329.6 263.2 321.9 263.2Q314.4 263.2 310.5 266.1Q306.6 268.9 305.9 274.9Q305.4 281.4 305.4 287.7Q305.4 294 305.9 300.5Q306.4 306.5 308.9 310.4Q311.4 314.4 317.2 315.6L333 319Q344.7 321.6 351.8 327Q358.9 332.3 362.1 340.5Q365.4 348.7 365.9 359.8Q366 363 366 366.5Q366 369.9 365.9 373.3Q365.9 376.8 365.7 379.9Q364.8 401.5 354.7 410.7Q344.6 420 322 420Z";

/// L'anneau du « O » (règle pair-impair : le trou intérieur reste vide).
const O_ANNEAU: &str = "M541 241H554A36 30 0 0 1 590 271V388A36 30 0 0 1 554 418H541A36 30 0 0 1 505 388V271A36 30 0 0 1 541 241Z M547 263A14 9 0 0 1 561 272V387A14 9 0 0 1 547 396A14 9 0 0 1 533 387V272A14 9 0 0 1 547 263Z";

/// Un trait de la ligne « FRAME THE STORY » : décalage (translate du SVG) et chemin. Tracés avec un
/// trait de 2, jointures biseautées.
type Trait = (f32, f32, &'static str);

/// « FRAME » (de la couleur du logo).
const FRAME: [Trait; 5] = [
    (411.0, 437.0, "M1 0V12M1 1H8M1 6H7"),
    (430.0, 437.0, "M1 0V12M1 1H5.8Q8 1 8 3.5Q8 6 5.8 6H1M4.6 6.4L8.2 12"),
    (448.0, 437.0, "M0.9 12L5 1L9.1 12M2.5 9H7.5"),
    (467.0, 437.0, "M1 12V1L5 7L9 1V12"),
    (487.0, 437.0, "M1 0V12M1 1H8M1 6H7.2M1 11H8"),
];
/// « THE » (de la couleur du logo).
const THE: [Trait; 3] = [
    (529.0, 437.0, "M0 1H8M4 0V12"),
    (547.0, 437.0, "M1 0V12M9 0V12M1 6H9"),
    (567.0, 437.0, "M1 0V12M1 1H8M1 6H7.2M1 11H8"),
];
/// « STORY » (couleur du fond, dans la barre claire). Le « O » est un rectangle arrondi.
const STORY: [Trait; 5] = [
    (609.0, 437.0, "M8 3.2Q8 1 4.5 1Q1 1 1 3.3Q1 5.4 4.5 6Q8 6.6 8 8.8Q8 11 4.5 11Q1 11 1 8.8"),
    (628.0, 437.0, "M0 1H10M5 0V12"),
    (646.0, 437.0, "M5.5 1H6.5A4.5 4.5 0 0 1 11 5.5V6.5A4.5 4.5 0 0 1 6.5 11H5.5A4.5 4.5 0 0 1 1 6.5V5.5A4.5 4.5 0 0 1 5.5 1Z"),
    (665.0, 437.0, "M1 0V12M1 1H6.4Q8.6 1 8.6 3.5Q8.6 6 6.4 6H1M5 6.4L8.8 12"),
    (684.0, 437.0, "M0.6 0L5.5 6.5L10.4 0M5.5 6V12"),
];

// ---------------------------------------------------------------------------------------------
// Lecture des chemins SVG (M L H V Q A Z, coordonnées absolues).

enum Jeton {
    Cmd(char),
    Nombre(f32),
}

fn jetons(d: &str) -> Vec<Jeton> {
    let c: Vec<char> = d.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i].is_ascii_alphabetic() {
            out.push(Jeton::Cmd(c[i]));
            i += 1;
        } else if c[i].is_ascii_digit() || c[i] == '-' || c[i] == '.' {
            let debut = i;
            i += 1;
            let mut point = c[debut] == '.';
            while i < c.len() && (c[i].is_ascii_digit() || (c[i] == '.' && !point)) {
                point |= c[i] == '.';
                i += 1;
            }
            out.push(Jeton::Nombre(c[debut..i].iter().collect::<String>().parse().expect("nombre du chemin")));
        } else {
            i += 1;
        }
    }
    out
}

/// Le chemin décrit par `d`. Les données sont celles du logo : une commande inconnue est une
/// faute de programme, pas d'entrée utilisateur.
pub fn chemin(d: &str) -> Path {
    let t = jetons(d);
    let mut pb = PathBuilder::new();
    let (mut x, mut y, mut debut_x, mut debut_y) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    let mut cmd = ' ';
    let mut i = 0;
    let nombres = |i: &mut usize, n: usize| -> Vec<f32> {
        (0..n)
            .map(|_| {
                let v = match t.get(*i) {
                    Some(Jeton::Nombre(v)) => *v,
                    _ => panic!("chemin SVG incomplet : {d}"),
                };
                *i += 1;
                v
            })
            .collect()
    };
    while i < t.len() {
        if let Jeton::Cmd(c) = t[i] {
            cmd = c;
            i += 1;
            if c == 'Z' {
                pb.close();
                x = debut_x;
                y = debut_y;
                continue;
            }
        }
        match cmd {
            'M' => {
                let v = nombres(&mut i, 2);
                (x, y, debut_x, debut_y) = (v[0], v[1], v[0], v[1]);
                pb.move_to(x, y);
                cmd = 'L'; // les paires suivantes sont des lignes
            }
            'L' => {
                let v = nombres(&mut i, 2);
                (x, y) = (v[0], v[1]);
                pb.line_to(x, y);
            }
            'H' => {
                x = nombres(&mut i, 1)[0];
                pb.line_to(x, y);
            }
            'V' => {
                y = nombres(&mut i, 1)[0];
                pb.line_to(x, y);
            }
            'Q' => {
                let v = nombres(&mut i, 4);
                pb.quad_to(v[0], v[1], v[2], v[3]);
                (x, y) = (v[2], v[3]);
            }
            'A' => {
                let v = nombres(&mut i, 7);
                arc(&mut pb, (x, y), (v[0], v[1]), v[2], v[3] != 0.0, v[4] != 0.0, (v[5], v[6]));
                (x, y) = (v[5], v[6]);
            }
            autre => panic!("commande de chemin SVG non prise en charge : {autre}"),
        }
    }
    pb.finish().expect("chemin du logo non vide")
}

/// Arc elliptique SVG (annexe F.6 de la spécification) en courbes de Bézier cubiques, une par
/// quart de tour au plus.
fn arc(pb: &mut PathBuilder, (x1, y1): (f32, f32), (rx, ry): (f32, f32), degres: f32, grand: bool, sens: bool, (x2, y2): (f32, f32)) {
    let (mut rx, mut ry) = (rx.abs() as f64, ry.abs() as f64);
    if rx == 0.0 || ry == 0.0 || (x1 == x2 && y1 == y2) {
        pb.line_to(x2, y2);
        return;
    }
    let (x1, y1, x2, y2) = (x1 as f64, y1 as f64, x2 as f64, y2 as f64);
    let phi = (degres as f64).to_radians();
    let (cos, sin) = (phi.cos(), phi.sin());
    let (dx, dy) = ((x1 - x2) / 2.0, (y1 - y2) / 2.0);
    let (x1p, y1p) = (cos * dx + sin * dy, -sin * dx + cos * dy);
    let trop = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry);
    if trop > 1.0 {
        rx *= trop.sqrt();
        ry *= trop.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let coef = if grand == sens { -1.0 } else { 1.0 } * (num / den).max(0.0).sqrt();
    let (cxp, cyp) = (coef * rx * y1p / ry, -coef * ry * x1p / rx);
    let (cx, cy) = (cos * cxp - sin * cyp + (x1 + x2) / 2.0, sin * cxp + cos * cyp + (y1 + y2) / 2.0);
    let theta1 = ((y1p - cyp) / ry).atan2((x1p - cxp) / rx);
    let theta2 = ((-y1p - cyp) / ry).atan2((-x1p - cxp) / rx);
    let mut delta = theta2 - theta1;
    if sens && delta < 0.0 {
        delta += std::f64::consts::TAU;
    } else if !sens && delta > 0.0 {
        delta -= std::f64::consts::TAU;
    }
    let n = (delta.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0);
    let pas = delta / n;
    let k = 4.0 / 3.0 * (pas / 4.0).tan();
    let point = |u: f64, v: f64| ((cx + rx * cos * u - ry * sin * v) as f32, (cy + rx * sin * u + ry * cos * v) as f32);
    for j in 0..n as usize {
        let (a, b) = (theta1 + pas * j as f64, theta1 + pas * (j + 1) as f64);
        let (c1x, c1y) = point(a.cos() - k * a.sin(), a.sin() + k * a.cos());
        let (c2x, c2y) = point(b.cos() + k * b.sin(), b.sin() - k * b.cos());
        let (ex, ey) = if j + 1 == n as usize { (x2 as f32, y2 as f32) } else { point(b.cos(), b.sin()) };
        pb.cubic_to(c1x, c1y, c2x, c2y, ex, ey);
    }
}

// ---------------------------------------------------------------------------------------------
// Dessin.

fn peinture(c: [u8; 3]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], 255);
    p.anti_alias = true;
    p
}

fn rect(r: [f32; 4]) -> Option<Path> {
    Rect::from_xywh(r[0], r[1], r[2], r[3]).map(PathBuilder::from_rect)
}

/// Dessine le logo (sans fond) dans `pm`, `global` envoyant les unités du SVG sur les pixels.
fn dessiner_logo(pm: &mut Pixmap, global: Transform, v: Variante) {
    let c = v.couleurs();
    let encre = peinture(c.logo);
    for r in RECTS {
        if let Some(p) = rect(r) {
            pm.fill_path(&p, &encre, FillRule::Winding, global, None);
        }
    }
    pm.fill_path(&chemin(S_GLYPHE), &encre, FillRule::Winding, global, None);
    pm.fill_path(&chemin(O_ANNEAU), &encre, FillRule::EvenOdd, global, None);
    let trait_ = Stroke { width: 2.0, line_join: LineJoin::Bevel, ..Stroke::default() };
    for (couleur, traits) in [(c.logo, &FRAME[..]), (c.logo, &THE[..]), (c.creux, &STORY[..])] {
        let p = peinture(couleur);
        for &(tx, ty, d) in traits {
            pm.stroke_path(&chemin(d), &p, &trait_, global.pre_translate(tx, ty), None);
        }
    }
}

/// Le logo seul (fond transparent), `largeur` pixels de large ; la hauteur suit le cadre du logo.
/// Pixels RVBA **prémultipliés** (la forme qu'attend `ColorImage::from_rgba_premultiplied`).
/// Renvoie (largeur, hauteur, pixels).
pub fn image(largeur: u32, v: Variante) -> (u32, u32, Vec<u8>) {
    let largeur = largeur.max(8);
    let hauteur = ((largeur as f32 * VUE_H / VUE_L).round() as u32).max(1);
    let mut pm = Pixmap::new(largeur, hauteur).expect("taille du logo");
    let s = largeur as f32 / VUE_L;
    dessiner_logo(&mut pm, Transform::from_row(s, 0.0, 0.0, s, -VUE_X * s, -VUE_Y * s), v);
    (largeur, hauteur, pm.take())
}

/// Cadre du petit logotype (le mot « SHOT » et ses deux angles, sans la devise), en unités du SVG :
/// de l'angle haut gauche (242,6 ; 204) au coin bas droit de l'angle bas droit (700 ; 454).
pub const LOGOTYPE_X: f32 = 242.6;
pub const LOGOTYPE_Y: f32 = 204.0;
pub const LOGOTYPE_L: f32 = 457.4;
pub const LOGOTYPE_H: f32 = 250.0;

/// Le petit logotype : « SHOT » avec ses deux angles, sans « FRAME THE STORY », `hauteur` pixels de
/// haut ; la largeur suit le cadre. Les lettres sont de la couleur de la variante ; les deux angles,
/// de `angles` si elle est donnée (l'orange de l'identité), sinon des lettres. Fond transparent,
/// pixels RVBA **prémultipliés**. Renvoie (largeur, hauteur, pixels).
pub fn logotype(hauteur: u32, v: Variante, angles: Option<[u8; 3]>) -> (u32, u32, Vec<u8>) {
    let hauteur = hauteur.max(8);
    let largeur = ((hauteur as f32 * LOGOTYPE_L / LOGOTYPE_H).round() as u32).max(1);
    let mut pm = Pixmap::new(largeur, hauteur).expect("taille du logotype");
    let s = hauteur as f32 / LOGOTYPE_H;
    let global = Transform::from_row(s, 0.0, 0.0, s, -LOGOTYPE_X * s, -LOGOTYPE_Y * s);
    let c = v.couleurs();
    let (lettres, coins) = (peinture(c.logo), peinture(angles.unwrap_or(c.logo)));
    // (Les rectangles 0 et 1 sont l'angle haut gauche, 7 et 8 l'angle bas droit.)
    for (i, r) in RECTS.iter().enumerate() {
        if let Some(p) = rect(*r) {
            pm.fill_path(&p, if matches!(i, 0 | 1 | 7 | 8) { &coins } else { &lettres }, FillRule::Winding, global, None);
        }
    }
    pm.fill_path(&chemin(S_GLYPHE), &lettres, FillRule::Winding, global, None);
    pm.fill_path(&chemin(O_ANNEAU), &lettres, FillRule::EvenOdd, global, None);
    (largeur, hauteur, pm.take())
}

/// L'icône de l'application, carrée, de `taille` pixels, en RVBA **non prémultipliés** (`IconData`,
/// `.ico`) : la variante **orange** de la charte (l'orange est l'identité du logiciel, Anthony,
/// 2026-10-28). Elle sert à l'exécutable, à la barre des tâches et à la fenêtre.
pub fn icone(taille: u32) -> Vec<u8> {
    icone_en(taille, ORANGE_SHOT, NOIR_CINEMA)
}

/// La variante sombre de l'icône (carré noir cinéma, « S » et angles blanc cassé) : disponible,
/// pas utilisée pour l'instant.
#[allow(dead_code)]
pub fn icone_sombre(taille: u32) -> Vec<u8> {
    icone_en(taille, NOIR_CINEMA, BLANC_CASSE)
}

/// Une icône : un carré arrondi de la couleur `fond`, le « S » du logo entre deux angles de la couleur
/// `encre` (haut gauche et bas droit, ceux du logo).
///
/// Le choix : un mot entier ne se lit plus à 16 ou 32 pixels ; le « S » et ses deux angles, si, et
/// c'est la silhouette la plus reconnaissable du logo. Coins arrondis à 22 % du côté (la planche).
fn icone_en(taille: u32, fond: [u8; 3], encre: [u8; 3]) -> Vec<u8> {
    let taille = taille.max(8);
    let mut pm = Pixmap::new(taille, taille).expect("taille de l'icône");
    let g = taille as f32 / 100.0;
    // Fond : carré arrondi (rayon 22 sur 100).
    let forme = chemin("M22 0H78A22 22 0 0 1 100 22V78A22 22 0 0 1 78 100H22A22 22 0 0 1 0 78V22A22 22 0 0 1 22 0Z");
    pm.fill_path(&forme, &peinture(fond), FillRule::Winding, Transform::from_scale(g, g), None);
    let encre = peinture(encre);
    // Angles du logo (barre de 8, bras de 32), à 12 du bord.
    for r in [[12.0, 12.0, 32.0, 8.0], [12.0, 12.0, 8.0, 32.0], [80.0, 56.0, 8.0, 32.0], [56.0, 80.0, 32.0, 8.0]] {
        if let Some(p) = rect(r) {
            pm.fill_path(&p, &encre, FillRule::Winding, Transform::from_scale(g, g), None);
        }
    }
    // Le « S » : 52 de haut (sur 100), centré. Il occupe x 277,6..366 et y 240..420 dans le SVG.
    let k = 52.0 / 180.0;
    let (cx, cy) = ((277.6 + 366.0) / 2.0, (240.0 + 420.0) / 2.0);
    let placer = Transform::from_row(k, 0.0, 0.0, k, 50.0 - cx * k, 50.0 - cy * k).post_scale(g, g);
    pm.fill_path(&chemin(S_GLYPHE), &encre, FillRule::Winding, placer, None);
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
