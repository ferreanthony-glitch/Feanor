//! Le rendu du décor 3D par le processeur (partie 180) : pour les **exports** (l'image exacte, calculée tout de suite) et pour
//! l'affichage **sans carte graphique** (OpenGL de secours, composition par le processeur), où il tourne sur un autre fil et ne
//! fait jamais attendre l'interface (`decor3d::montre`). À l'écran avec la carte graphique, c'est elle qui dessine le décor
//! (`decor3dgpu.rs`), avec les mêmes formules : ce qu'on voit est ce qui sera exporté.
//!
//! C'est le rasteriseur de l'ancienne scène 3D (`scene3drender.rs`, retiré le 2026-10-21), repris : tampon de profondeur par
//! bandes de lignes réparties sur les cœurs, sur-échantillonnage pour des bords lisses, et maintenant la **texture** (lue avec
//! des coordonnées justes en perspective) et la couleur de base de chaque matière. L'aspect (`Aspect`, « low poly doux »,
//! Anthony) : facettes ou lissé, lumière hémisphère (ciel froid, sol chaud) et soleil chaud, ciel en dégradé pastel, brume
//! vers l'horizon. **Pas d'ombres portées ici** : elles sont de la carte graphique (`decor3dgpu.rs`) ; le rendu de secours
//! (sans carte) et l'export sans carte n'en ont pas (dit dans `docs/INTERFACE.md`, partie 180).
//!
//! **Vu de l'œil de la caméra, aligné sur la feuille** (`Prise::de_la_feuille`) : l'image est la feuille telle que l'œil la voit,
//! comme une carte en profondeur ; la caméra 2D du plan ajoute ensuite son panoramique, sa bascule, son zoom et sa rotation à tout
//! le plan (l'homographie de toujours). Le décor défile donc avec la même parallaxe que les cartes.

use std::sync::Mutex;

use crate::decor3dgltf::{Scene3d, Texture3d};
use crate::espace3d::Monde;
use crate::espace3dmath::{self as m, M4, V3};

/// L'aspect « low poly doux » (couleurs **linéaires**). Les mêmes chiffres dans le shader de la carte (`decor3dgpu.rs`).
pub const SOLEIL_COULEUR: [f32; 3] = [1.0, 0.9, 0.76];
pub const SOLEIL_FORCE: f32 = 0.78;
/// L'ambiance du ciel (froide, d'en haut) et du sol (chaude, d'en bas) : la lumière hémisphère.
pub const AMBIANCE_CIEL: [f32; 3] = [0.42, 0.5, 0.66];
pub const AMBIANCE_SOL: [f32; 3] = [0.36, 0.3, 0.25];
/// Le ciel : son zénith, et l'horizon (qui est aussi la couleur de la brume).
pub const ZENITH: [f32; 3] = [0.36, 0.55, 0.85];
pub const HORIZON: [f32; 3] = [0.88, 0.8, 0.76];
/// La brume la plus forte, au plus loin du décor (le décor ne disparaît jamais tout à fait).
pub const BRUME_MAX: f32 = 0.78;

/// Comment le décor se montre (réglable par décor, `decors3d.json`). D'origine : facettes, ciel, brume, ombres, soleil en haut
/// à gauche.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Aspect {
    /// Normales par face (low poly) ; faux : les normales du modèle (« lisse »).
    #[serde(default = "oui")]
    pub facettes: bool,
    #[serde(default = "oui")]
    pub ciel: bool,
    #[serde(default = "oui")]
    pub brume: bool,
    #[serde(default = "oui")]
    pub ombres: bool,
    /// Le soleil : sa direction (degrés : autour, 0 = derrière la caméra, et au-dessus de l'horizon).
    #[serde(default = "azimut_d_origine")]
    pub soleil_azimut: f32,
    #[serde(default = "hauteur_d_origine")]
    pub soleil_hauteur: f32,
}

fn oui() -> bool {
    true
}
fn azimut_d_origine() -> f32 {
    -35.0
}
fn hauteur_d_origine() -> f32 {
    50.0
}

impl Default for Aspect {
    fn default() -> Aspect {
        Aspect { facettes: true, ciel: true, brume: true, ombres: true, soleil_azimut: azimut_d_origine(), soleil_hauteur: hauteur_d_origine() }
    }
}

impl Aspect {
    /// Vers le soleil (longueur 1).
    pub fn soleil(&self) -> V3 {
        let (a, e) = (self.soleil_azimut.to_radians(), self.soleil_hauteur.clamp(5.0, 89.0).to_radians());
        m::normalize([e.cos() * a.sin(), e.sin(), e.cos() * a.cos()])
    }
    pub fn couleur_soleil(&self) -> [f32; 3] {
        SOLEIL_COULEUR.map(|c| c * SOLEIL_FORCE)
    }
    pub fn ambiances(&self) -> ([f32; 3], [f32; 3]) {
        (AMBIANCE_CIEL, AMBIANCE_SOL)
    }
    pub fn horizon(&self) -> [f32; 3] {
        HORIZON
    }
    pub fn zenith(&self) -> [f32; 3] {
        ZENITH
    }
    /// Début et fin de la brume, d'après la distance du coin du décor le plus loin de l'œil.
    pub fn brume_de(&self, dist_max: f32) -> (f32, f32) {
        (dist_max * 0.3, dist_max)
    }
    /// La couleur du ciel dans la direction `dir` (longueur 1).
    pub fn ciel_vers(&self, dir: V3) -> [f32; 3] {
        let k = dir[1].clamp(0.0, 1.0).powf(0.6);
        [0, 1, 2].map(|i| HORIZON[i] + (ZENITH[i] - HORIZON[i]) * k)
    }
    /// La couleur (linéaire) d'un point de couleur de base `c`, de normale `n` (tournée vers l'œil), à `d` de l'œil, la brume
    /// allant de `brume` (début, fin). Sans ombre (le rendu par le processeur n'en a pas).
    pub fn ombrer(&self, c: [f32; 3], n: V3, d: f32, brume: (f32, f32)) -> [f32; 3] {
        let t = n[1] * 0.5 + 0.5;
        let (sa, ga) = self.ambiances();
        let k = self.couleur_soleil();
        let sun = m::dot(n, self.soleil()).max(0.0);
        let lit = [0, 1, 2].map(|i| ga[i] + (sa[i] - ga[i]) * t + k[i] * sun);
        let c = [c[0] * lit[0], c[1] * lit[1], c[2] * lit[2]];
        if !self.brume {
            return c;
        }
        let f = ((d - brume.0) / (brume.1 - brume.0).max(1e-4)).clamp(0.0, 1.0) * BRUME_MAX;
        let f = f * f * (3.0 - 2.0 * f);
        [0, 1, 2].map(|i| c[i] + (HORIZON[i] - c[i]) * f)
    }
}

/// Lignes de l'image par bande de travail.
const BANDE: usize = 16;

/// D'où et comment on regarde : œil, axes, échelle (pixels par unité de x/z) et point de l'image où l'on regarde.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prise {
    pub oeil: V3,
    pub droite: V3,
    pub haut: V3,
    pub avant: V3,
    pub px_par_unite: f32,
    pub centre: [f32; 2],
    /// Plan proche (mètres devant l'œil).
    pub proche: f32,
}

impl Prise {
    /// L'œil `oeil` (mètres) regardant la feuille de face, l'image étant la feuille (pixels de la feuille) : un point du monde
    /// tombe là où `Monde::to_sheet` le met. Rien si l'œil n'est pas devant la feuille (les cartes ne s'y voient pas non plus).
    pub fn de_la_feuille(monde: &Monde, oeil: V3) -> Option<Prise> {
        let proche = monde.d0 * crate::espace3d::NEAR_FRACTION;
        if oeil[2] <= proche || !oeil.iter().all(|v| v.is_finite()) {
            return None;
        }
        Some(Prise {
            oeil,
            droite: [1.0, 0.0, 0.0],
            haut: [0.0, 1.0, 0.0],
            avant: [0.0, 0.0, -1.0],
            px_par_unite: oeil[2] / monde.k,
            centre: [oeil[0] / monde.k + monde.stage[0] / 2.0, -oeil[1] / monde.k + monde.stage[1] / 2.0],
            proche,
        })
    }

    /// Point dans le repère de l'œil : (droite, haut, profondeur).
    fn vers_oeil(&self, p: V3) -> V3 {
        let d = m::sub(p, self.oeil);
        [m::dot(d, self.droite), m::dot(d, self.haut), m::dot(d, self.avant)]
    }
}

/// sRGB (0–255) vers linéaire.
fn lineaire_lut() -> &'static [f32; 256] {
    static LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0.0; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *v = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        }
        t
    })
}

/// Linéaire vers sRGB (0–1).
pub fn vers_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// La couleur (linéaire) et l'alpha de la texture au point (`u`, `v`), répétée, lue en bilinéaire.
fn lire_texture(t: &Texture3d, u: f32, v: f32) -> [f32; 4] {
    let lut = lineaire_lut();
    let (w, h) = (t.largeur as usize, t.hauteur as usize);
    let x = (u - u.floor()) * w as f32 - 0.5;
    let y = (v - v.floor()) * h as f32 - 0.5;
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let px = |xi: isize, yi: isize| {
        let (xi, yi) = (xi.rem_euclid(w as isize) as usize, yi.rem_euclid(h as isize) as usize);
        let o = (yi * w + xi) * 4;
        let p = &t.rgba[o..o + 4];
        [lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize], p[3] as f32 / 255.0]
    };
    let (xi, yi) = (x0 as isize, y0 as isize);
    let (a, b, c, d) = (px(xi, yi), px(xi + 1, yi), px(xi, yi + 1), px(xi + 1, yi + 1));
    [0, 1, 2, 3].map(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
}

/// Un triangle prêt : sommets (x, y en échantillons, 1/z), normales et coordonnées de texture divisées par z (interpolation
/// juste en perspective), son objet (partie 181 : un décor en a plusieurs) et sa primitive.
struct Tri {
    p: [[f32; 3]; 3],
    n: [V3; 3],
    uv: [[f32; 2]; 3],
    obj: u32,
    prim: u32,
    y: (f32, f32),
}

fn clip_proche(poly: &[(V3, V3, [f32; 2])], proche: f32) -> Vec<(V3, V3, [f32; 2])> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (ina, inb) = (a.0[2] >= proche, b.0[2] >= proche);
        if ina {
            out.push(a);
        }
        if ina != inb {
            let t = (proche - a.0[2]) / (b.0[2] - a.0[2]);
            let lerp = |x: V3, y: V3| m::add(x, m::scale(m::sub(y, x), t));
            out.push((lerp(a.0, b.0), lerp(a.1, b.1), [a.2[0] + (b.2[0] - a.2[0]) * t, a.2[1] + (b.2[1] - a.2[1]) * t]));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn triangles(tris: &mut Vec<Tri>, obj: u32, scene: &Scene3d, matrice: &M4, o: &Prise, taille: [usize; 2], ss: usize, facettes: bool) {
    let s = o.px_par_unite;
    let ssf = ss as f32;
    let (wmax, hmax) = ((taille[0] * ss) as f32, (taille[1] * ss) as f32);
    let nm = normal_matrix(matrice);
    for (pi, prim) in scene.primitives.iter().enumerate() {
        let monde: Vec<V3> = prim.positions.iter().map(|p| m::point(matrice, *p)).collect();
        let cam: Vec<V3> = monde.iter().map(|p| o.vers_oeil(*p)).collect();
        let nrm: Vec<V3> = prim.normales.iter().map(|n| m::normalize(m::dir(&nm, *n))).collect();
        for t in prim.indices.chunks_exact(3) {
            let idx = [t[0] as usize, t[1] as usize, t[2] as usize];
            if idx.iter().all(|&i| cam[i][2] < o.proche) {
                continue;
            }
            let uv = |i: usize| prim.uvs.get(i).copied().unwrap_or([0.0; 2]);
            // Facettes : la normale de la face, la même aux trois coins (le low poly).
            let face = facettes.then(|| crate::decor3dgltf::normale_face(monde[idx[0]], monde[idx[1]], monde[idx[2]]));
            let poly: Vec<(V3, V3, [f32; 2])> = idx.iter().map(|&i| (cam[i], face.unwrap_or(nrm[i]), uv(i))).collect();
            let poly = if poly.iter().all(|v| v.0[2] >= o.proche) { poly } else { clip_proche(&poly, o.proche) };
            if poly.len() < 3 {
                continue;
            }
            let v: Vec<([f32; 3], V3, [f32; 2])> = poly
                .iter()
                .map(|(c, n, t)| {
                    let inv = 1.0 / c[2];
                    ([(o.centre[0] + c[0] * inv * s) * ssf, (o.centre[1] - c[1] * inv * s) * ssf, inv], m::scale(*n, inv), [t[0] * inv, t[1] * inv])
                })
                .collect();
            if v.iter().any(|p| !(p.0[0].is_finite() && p.0[1].is_finite())) {
                continue;
            }
            // (Hors de l'image : rien à garder.)
            let (x0, x1) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0[0]), b.max(p.0[0])));
            let (y0, y1) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0[1]), b.max(p.0[1])));
            if x1 < 0.0 || y1 < 0.0 || x0 >= wmax || y0 >= hmax {
                continue;
            }
            for k in 1..v.len() - 1 {
                let p = [v[0].0, v[k].0, v[k + 1].0];
                let y = (p[0][1].min(p[1][1]).min(p[2][1]), p[0][1].max(p[1][1]).max(p[2][1]));
                tris.push(Tri { p, n: [v[0].1, v[k].1, v[k + 1].1], uv: [v[0].2, v[k].2, v[k + 1].2], obj, prim: pi as u32, y });
            }
        }
    }
}

/// La matrice des normales (inverse transposée de la partie 3×3).
pub fn normal_matrix(mx: &M4) -> M4 {
    let inv = m::inverse_affine(mx);
    let mut n = m::IDENT;
    for r in 0..3 {
        for c in 0..3 {
            n[c * 4 + r] = inv[r * 4 + c];
        }
    }
    n
}

/// Ce que le rasteriseur sait de la prise pour l'ombrage : l'aspect, la brume, et de quoi refaire la distance à l'œil.
struct Ombrage {
    aspect: Aspect,
    brume: (f32, f32),
    centre: [f32; 2],
    s: f32,
    ss: f32,
}

struct Bande {
    w: usize,
    y0: usize,
    lignes: usize,
    invz: Vec<f32>,
    couleur: Vec<[f32; 3]>,
    couvert: Vec<bool>,
}

fn raster(t: &Tri, b: &mut Bande, scenes: &[(&Scene3d, M4)], om: &Ombrage) {
    let scene = scenes[t.obj as usize].0;
    let p = &t.p;
    let (x0, x1) = (p[0][0].min(p[1][0]).min(p[2][0]), p[0][0].max(p[1][0]).max(p[2][0]));
    let (ya, yb) = t.y;
    let (by0, by1) = (b.y0 as f32, (b.y0 + b.lignes) as f32);
    if x1 < 0.0 || x0 >= b.w as f32 || yb < by0 || ya >= by1 {
        return;
    }
    let aire = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
    if aire.abs() < 1e-9 {
        return;
    }
    let prim = &scene.primitives[t.prim as usize];
    let tex = prim.texture.and_then(|i| scene.textures.get(i));
    let base = prim.couleur;
    let (xs, xe) = (x0.max(0.0).floor() as usize, (x1.ceil() as usize).min(b.w - 1));
    let (ys, ye) = ((ya.max(by0).floor() as usize).max(b.y0), (yb.ceil() as usize).min(b.y0 + b.lignes - 1));
    for y in ys..=ye {
        let py = y as f32 + 0.5;
        let row = (y - b.y0) * b.w;
        for x in xs..=xe {
            let px = x as f32 + 0.5;
            let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[2][0] - px) * (p[1][1] - py)) / aire;
            let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[0][0] - px) * (p[2][1] - py)) / aire;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let invz = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
            let at = row + x;
            if invz <= b.invz[at] {
                continue;
            }
            let mut c = [base[0], base[1], base[2]];
            if let Some(tex) = tex {
                let u = (w0 * t.uv[0][0] + w1 * t.uv[1][0] + w2 * t.uv[2][0]) / invz;
                let v = (w0 * t.uv[0][1] + w1 * t.uv[1][1] + w2 * t.uv[2][1]) / invz;
                let s = lire_texture(tex, u, v);
                if prim.seuil_alpha.is_some_and(|seuil| s[3] * base[3] < seuil) {
                    continue;
                }
                c = [c[0] * s[0], c[1] * s[1], c[2] * s[2]];
            } else if prim.seuil_alpha.is_some_and(|seuil| base[3] < seuil) {
                continue;
            }
            let n = m::normalize([0, 1, 2].map(|k| (w0 * t.n[0][k] + w1 * t.n[1][k] + w2 * t.n[2][k]) / invz));
            b.invz[at] = invz;
            b.couvert[at] = true;
            // La distance à l'œil (pour la brume) : la profondeur, allongée selon l'écart au centre de l'image.
            let (dx, dy) = ((px / om.ss - om.centre[0]) / om.s, (py / om.ss - om.centre[1]) / om.s);
            let d = (1.0 / invz) * (1.0 + dx * dx + dy * dy).sqrt();
            b.couleur[at] = om.aspect.ombrer(c, n, d, om.brume);
        }
    }
}

/// Rend la scène (placée par `matrice`) vue par `prise`, à `taille` pixels : octets RGBA **non prémultipliés**, en sRGB.
/// `ss` : sur-échantillonnage par côté (1 à 4) ; `fils` : fils de calcul (0 : tous les cœurs). Avec le ciel (`aspect.ciel`),
/// toute l'image est couverte : le dégradé derrière le décor.
pub fn rendre(scene: &Scene3d, matrice: &M4, prise: &Prise, taille: [usize; 2], ss: usize, fils: usize, aspect: &Aspect) -> Vec<u8> {
    rendre_objets(&[(scene, *matrice)], prise, taille, ss, fils, aspect)
}

/// Comme `rendre`, pour plusieurs objets (partie 181) : chacun sa scène et sa matrice, tous dans le même tampon de profondeur
/// (un arbre derrière une maison est caché par elle).
pub fn rendre_objets(objets: &[(&Scene3d, M4)], prise: &Prise, taille: [usize; 2], ss: usize, fils: usize, aspect: &Aspect) -> Vec<u8> {
    rendre_objets_prof(objets, prise, taille, ss, fils, aspect).0
}

/// Comme `rendre_objets`, avec **la profondeur** de l'image (partie 183, `profondeur.rs`) : à chaque pixel, l'inverse de la
/// distance à l'œil (le long de son axe) de ce que le décor y montre, 0 là où il n'y a rien (le ciel n'y compte pas : il ne cache
/// rien). En quatre échantillons par pixel (partie 193, `profondeur::ProfDecor`) : à deux échantillons par côté, les quatre
/// (`MOTIF_GRILLE`) ; à quatre par côté, les quatre aux places de la carte graphique (`MOTIF_CARTE`) ; sinon, un seul, répété
/// (`MOTIF_CENTRE` : le plus proche de ceux qui couvrent le pixel, s'ils en couvrent la moitié).
pub fn rendre_objets_prof(objets: &[(&Scene3d, M4)], prise: &Prise, taille: [usize; 2], ss: usize, fils: usize, aspect: &Aspect) -> (Vec<u8>, crate::profondeur::ProfDecor) {
    let [w, h] = taille;
    let mut out = vec![0u8; w * h * 4];
    let ss = ss.clamp(1, 4);
    let motif = match ss {
        2 => crate::profondeur::MOTIF_GRILLE,
        4 => crate::profondeur::MOTIF_CARTE,
        _ => crate::profondeur::MOTIF_CENTRE,
    };
    // (Les échantillons gardés, (x, y) dans le pixel sur-échantillonné, aux places du motif.)
    let gardes: [(usize, usize); 4] = match ss {
        2 => [(0, 0), (1, 0), (0, 1), (1, 1)],
        4 => [(1, 0), (3, 1), (0, 2), (2, 3)],
        _ => [(0, 0); 4],
    };
    let quatre = ss == 2 || ss == 4;
    let mut prof = vec![[0u16; 4]; if quatre { w * h } else { 0 }];
    let mut prof1 = vec![0u16; w * h];
    if w == 0 || h == 0 {
        return (out, crate::profondeur::ProfDecor::avec(w, h, prof1, None, motif));
    }
    // Les normales tournées vers l'œil : on retourne celles des faces vues de dos avant de rasteriser, sommet par sommet
    // (dans le repère de l'œil, la normale d'une face visible pointe vers l'œil, z < 0).
    let mut tris = Vec::new();
    for (i, (scene, matrice)) in objets.iter().enumerate() {
        triangles(&mut tris, i as u32, scene, matrice, prise, taille, ss, aspect.facettes);
    }
    // La brume, d'après le coin des objets le plus loin de l'œil (comme sur la carte graphique).
    let dist_max = objets
        .iter()
        .flat_map(|(scene, matrice)| {
            let (lo, hi) = scene.bornes();
            (0..8).map(move |i| m::length(m::sub(m::point(matrice, [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] })), prise.oeil)))
        })
        .fold(0.0f32, f32::max)
        .max(1e-3);
    let om = Ombrage { aspect: *aspect, brume: aspect.brume_de(dist_max), centre: prise.centre, s: prise.px_par_unite, ss: ss as f32 };
    let rot = [prise.droite, prise.haut, prise.avant];
    for t in &mut tris {
        let moy = m::add(m::add(t.n[0], t.n[1]), t.n[2]);
        // Normale (monde) vers le repère de l'œil : sa composante « avant » positive veut dire qu'elle s'éloigne de l'œil.
        if m::dot(moy, rot[2]) > 0.0 {
            for n in &mut t.n {
                *n = m::scale(*n, -1.0);
            }
        }
    }
    let bandes = h.div_ceil(BANDE);
    let mut bins: Vec<Vec<u32>> = vec![Vec::new(); bandes];
    for (i, t) in tris.iter().enumerate() {
        let (ya, yb) = (t.y.0 - 1.0, t.y.1 + 1.0);
        if yb < 0.0 || ya >= (h * ss) as f32 {
            continue;
        }
        let first = (ya.max(0.0) as usize / ss / BANDE).min(bandes - 1);
        let last = ((yb.min((h * ss) as f32 - 1.0)) as usize / ss / BANDE).min(bandes - 1);
        for bin in &mut bins[first..=last] {
            bin.push(i as u32);
        }
    }
    let fils = if fils == 0 { std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16) } else { fils };
    // (Une valeur par pixel, toujours ; quatre échantillons à deux ou quatre par côté, sinon ce tampon est vide, ses bandes aussi.)
    let bandes4: Vec<&mut [[u16; 4]]> = if quatre { prof.chunks_mut(BANDE * w).collect() } else { (0..bandes).map(|_| &mut [][..]).collect() };
    let bandes1: Vec<&mut [u16]> = prof1.chunks_mut(BANDE * w).collect();
    let travail = Mutex::new(out.chunks_mut(BANDE * w * 4).zip(bandes4.into_iter().zip(bandes1)).enumerate());
    let moitie = (ss * ss).div_ceil(2) as f32;
    std::thread::scope(|sc| {
        for _ in 0..fils.min(bandes) {
            sc.spawn(|| loop {
                let next = travail.lock().ok().and_then(|mut it| it.next());
                let Some((bi, (dst, (dprof, dprof1)))) = next else { break };
                let lignes_px = dst.len() / (w * 4);
                let py0 = bi * BANDE;
                let y0 = py0 * ss;
                let n = w * ss * lignes_px * ss;
                let mut b = Bande { w: w * ss, y0, lignes: lignes_px * ss, invz: vec![0.0; n], couleur: vec![[0.0; 3]; n], couvert: vec![false; n] };
                for &ti in &bins[bi] {
                    raster(&tris[ti as usize], &mut b, objets, &om);
                }
                for ry in 0..lignes_px {
                    for x in 0..w {
                        let (mut somme, mut couv, mut pres) = ([0.0f32; 3], 0.0f32, 0.0f32);
                        for sy in 0..ss {
                            for sx in 0..ss {
                                let at = (ry * ss + sy) * b.w + x * ss + sx;
                                if b.couvert[at] {
                                    let c = b.couleur[at];
                                    somme = [somme[0] + c[0], somme[1] + c[1], somme[2] + c[2]];
                                    couv += 1.0;
                                    pres = pres.max(b.invz[at]);
                                }
                            }
                        }
                        if quatre {
                            dprof[ry * w + x] = gardes.map(|(sx, sy)| {
                                let at = (ry * ss + sy) * b.w + x * ss + sx;
                                if b.couvert[at] { crate::profondeur::vers_16_bits(b.invz[at]) } else { 0 }
                            });
                        }
                        if couv >= moitie {
                            dprof1[ry * w + x] = crate::profondeur::vers_16_bits(pres);
                        }
                        let a = couv / (ss * ss) as f32;
                        let px = &mut dst[(ry * w + x) * 4..(ry * w + x) * 4 + 4];
                        if om.aspect.ciel {
                            // Le ciel sous ce que le décor ne couvre pas : la direction du regard à ce pixel.
                            let (dx, dy) = ((x as f32 + 0.5 - prise.centre[0]) / prise.px_par_unite, -((py0 + ry) as f32 + 0.5 - prise.centre[1]) / prise.px_par_unite);
                            let dir = m::normalize(m::add(m::add(m::scale(prise.droite, dx), m::scale(prise.haut, dy)), prise.avant));
                            let ciel = om.aspect.ciel_vers(dir);
                            for k in 0..3 {
                                let c = if couv > 0.0 { somme[k] / couv } else { 0.0 };
                                px[k] = (vers_srgb(c * a + ciel[k] * (1.0 - a)) * 255.0).round() as u8;
                            }
                            px[3] = 255;
                            continue;
                        }
                        if couv == 0.0 {
                            continue;
                        }
                        for k in 0..3 {
                            px[k] = (vers_srgb(somme[k] / couv) * 255.0).round() as u8;
                        }
                        px[3] = (a * 255.0).round() as u8;
                    }
                }
            });
        }
    });
    drop(travail);
    (out, crate::profondeur::ProfDecor::avec(w, h, prof1, quatre.then_some(prof), motif))
}
