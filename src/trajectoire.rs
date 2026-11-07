//! La trajectoire d'un calque (Anthony, 2026-11-07) : « Lié à un calque, un outil spécial permet de dessiner une trajectoire
//! et un timing qui ensuite pose des clés de mouvement. La ligne principale est la trajectoire, les traits qui la découpent
//! sont les clés de spacing, comme en animation. »
//!
//! Le modèle, sans écran : le **chemin** (un trait sur la feuille), ses **marques** (des fractions de sa longueur, posées par
//! de petits traits en travers), et les **clés de position** (`poskeys.rs`) qu'ils posent sur le calque : une clé par image,
//! le calque avançant à vitesse constante entre deux marques (marques serrées : lent ; espacées : vite), le temps partagé en
//! parts égales entre les marques — la durée du plan (d'origine), ou « une marque = N images ». Le déplacement est
//! **relatif** : le calque garde sa pose au départ du chemin et en suit la forme (on part le trait de là où est le personnage).

use serde::{Deserialize, Serialize};

use crate::document::LayerTransform;
use crate::poskeys::PosKey;

/// Le pas entre deux points du chemin une fois rééchantillonné, en pixels de la feuille.
pub const PAS: f32 = 2.0;
/// Au plus tant de points gardés pour un chemin (le pas s'allonge au-delà).
pub const POINTS_MAX: usize = 4000;
/// Un trait plus court que ça (pixels de la feuille) n'est qu'un clic : ni chemin, ni marque.
pub const LONGUEUR_MIN: f32 = 8.0;
/// Deux marques ne sont jamais plus proches que cette fraction du chemin, ni une marque de l'un des deux bouts.
pub const ECART_MIN: f32 = 0.01;
/// Un trait est une marque s'il est plus court que cette fraction de la longueur du chemin…
pub const MARQUE_FRACTION_MAX: f32 = 0.5;
/// … et s'il croise le chemin, ou passe à moins de tant de pixels de la feuille.
pub const MARQUE_DISTANCE_MAX: f32 = 6.0;

/// La trajectoire d'un calque, telle qu'elle s'écrit dans le projet (`trajectoire` d'un calque).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trajectoire {
    /// Les points du chemin, en pixels de la feuille, du départ à l'arrivée ; au moins deux, à `PAS` l'un de l'autre.
    pub points: Vec<[f32; 2]>,
    /// Les marques, en fraction de la longueur du chemin (0 : le départ, 1 : l'arrivée), strictement entre les deux, triées.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marques: Vec<f32>,
    /// « Une marque = N images » ; rien : la durée du plan partagée en parts égales entre les marques.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images_par_marque: Option<usize>,
}

impl Trajectoire {
    /// Un chemin fait des points d'un trait (feuille) : lissé, rééchantillonné. Rien si le trait est trop court.
    pub fn nouvelle(brut: &[[f32; 2]]) -> Option<Trajectoire> {
        let points = echantillonner(&lisser(brut));
        (points.len() >= 2 && longueur_de(&points) >= LONGUEUR_MIN).then_some(Trajectoire { points, marques: Vec::new(), images_par_marque: None })
    }

    /// Une trajectoire relue d'un fichier : ses points filtrés (abîmés, doublons), ses marques remises en ordre et dans leurs
    /// bornes. Rien si le chemin ne tient pas debout.
    pub fn assainie(mut self) -> Option<Trajectoire> {
        self.points.retain(|p| p[0].is_finite() && p[1].is_finite());
        self.points.dedup_by(|a, b| distance(*a, *b) < 1e-3);
        if self.points.len() < 2 || longueur_de(&self.points) < LONGUEUR_MIN {
            return None;
        }
        let marques = std::mem::take(&mut self.marques);
        for m in marques {
            if m.is_finite() {
                self.ajouter_marque(m);
            }
        }
        if self.images_par_marque == Some(0) {
            self.images_par_marque = None;
        }
        Some(self)
    }

    /// Un nouveau chemin ; les marques sont gardées, à leurs fractions. Faux (rien ne change) si le trait est trop court.
    pub fn retracer(&mut self, brut: &[[f32; 2]]) -> bool {
        match Trajectoire::nouvelle(brut) {
            Some(t) => {
                self.points = t.points;
                true
            }
            None => false,
        }
    }

    /// La longueur du chemin, en pixels de la feuille.
    pub fn longueur(&self) -> f32 {
        longueur_de(&self.points)
    }

    /// Le point du chemin à la fraction `f` de sa longueur (0 : le départ, 1 : l'arrivée).
    pub fn point_a(&self, f: f32) -> [f32; 2] {
        let (i, w) = self.segment_a(f);
        let (a, b) = (self.points[i], self.points[i + 1]);
        [a[0] + (b[0] - a[0]) * w, a[1] + (b[1] - a[1]) * w]
    }

    /// La direction du chemin (vecteur unité) à la fraction `f`.
    pub fn direction_a(&self, f: f32) -> [f32; 2] {
        let (i, _) = self.segment_a(f);
        let (a, b) = (self.points[i], self.points[i + 1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let n = (dx * dx + dy * dy).sqrt();
        if n > 0.0 { [dx / n, dy / n] } else { [1.0, 0.0] }
    }

    /// Le segment (son rang, et la position dedans) où tombe la fraction `f`.
    fn segment_a(&self, f: f32) -> (usize, f32) {
        let cible = f.clamp(0.0, 1.0) * self.longueur();
        let mut parcouru = 0.0;
        for i in 0..self.points.len() - 1 {
            let l = distance(self.points[i], self.points[i + 1]);
            if parcouru + l >= cible || i + 2 == self.points.len() {
                let w = if l > 0.0 { ((cible - parcouru) / l).clamp(0.0, 1.0) } else { 0.0 };
                return (i, w);
            }
            parcouru += l;
        }
        (0, 0.0)
    }

    /// La fraction du chemin la plus proche du point `p`, et la distance qui les sépare.
    pub fn plus_proche(&self, p: [f32; 2]) -> (f32, f32) {
        let total = self.longueur().max(1e-6);
        let mut meilleur = (0.0, f32::INFINITY);
        let mut parcouru = 0.0;
        for i in 0..self.points.len() - 1 {
            let (a, b) = (self.points[i], self.points[i + 1]);
            let (w, d) = projection(p, a, b);
            let l = distance(a, b);
            if d < meilleur.1 {
                meilleur = (((parcouru + w * l) / total).clamp(0.0, 1.0), d);
            }
            parcouru += l;
        }
        meilleur
    }

    /// Les jalons du temps : le départ, les marques, l'arrivée.
    pub fn jalons(&self) -> Vec<f32> {
        let mut j = Vec::with_capacity(self.marques.len() + 2);
        j.push(0.0);
        j.extend_from_slice(&self.marques);
        j.push(1.0);
        j
    }

    /// Les images que durent les intervalles entre jalons, pour un plan de `duree` images : la dernière image atteinte par
    /// le chemin (celle du plan, ou `intervalles × N` avec « une marque = N images »), ou rien si le plan est trop court.
    fn derniere_image(&self, duree: usize) -> Option<usize> {
        if duree < 2 {
            return None;
        }
        let intervalles = self.marques.len() + 1;
        Some(match self.images_par_marque {
            Some(n) if n > 0 => intervalles * n,
            _ => duree - 1,
        })
    }

    /// Les images (d'un plan de `duree` images) où tombent les jalons : le départ, chaque marque, l'arrivée ; arrondies à l'image.
    /// Rien si le plan n'a qu'une image.
    pub fn images_des_jalons(&self, duree: usize) -> Vec<usize> {
        let Some(fin) = self.derniere_image(duree) else { return Vec::new() };
        let n = self.marques.len() + 1;
        (0..=n).map(|k| ((k * fin) as f32 / n as f32).round() as usize).filter(|f| *f < duree).collect()
    }

    /// Le chemin retourné de gauche à droite (`horizontal`) ou de haut en bas, autour de son départ : le calque, retourné de même,
    /// suit le chemin retourné.
    pub fn retourner(&mut self, horizontal: bool) {
        let d = self.points[0];
        self.transformer(|p| if horizontal { [2.0 * d[0] - p[0], p[1]] } else { [p[0], 2.0 * d[1] - p[1]] });
    }

    /// La fraction du chemin atteinte à l'image `f` d'un plan de `duree` images, ou rien si le chemin ne la couvre pas.
    pub fn fraction_a_l_image(&self, f: usize, duree: usize) -> Option<f32> {
        let fin = self.derniere_image(duree)?;
        if f > fin {
            return None;
        }
        let jalons = self.jalons();
        let n = jalons.len() - 1;
        let u = f as f32 / fin as f32 * n as f32;
        let i = (u.floor() as usize).min(n - 1);
        let w = (u - i as f32).clamp(0.0, 1.0);
        Some(jalons[i] + (jalons[i + 1] - jalons[i]) * w)
    }

    /// Les clés de position qu'elle pose sur un calque de pose `base` dans un plan de `duree` images : une clé par image
    /// que le chemin couvre, le déplacement relatif au départ du chemin. Aucune pour un plan d'une image.
    pub fn cles(&self, duree: usize, base: LayerTransform) -> Vec<PosKey> {
        let Some(fin) = self.derniere_image(duree) else { return Vec::new() };
        let depart = self.points[0];
        (0..=fin.min(duree - 1))
            .filter_map(|f| {
                let p = self.point_a(self.fraction_a_l_image(f, duree)?);
                Some(PosKey::new(f, LayerTransform { x: base.x + p[0] - depart[0], y: base.y + p[1] - depart[1], ..base }))
            })
            .collect()
    }

    /// Ce trait est-il une marque ? Il l'est s'il est court devant le chemin et le croise (ou le frôle) : la fraction du
    /// chemin où il le croise.
    pub fn marque_de(&self, coup: &[[f32; 2]]) -> Option<f32> {
        let l = longueur_de(coup);
        if l < LONGUEUR_MIN || l > self.longueur() * MARQUE_FRACTION_MAX {
            return None;
        }
        let total = self.longueur().max(1e-6);
        // Un croisement franc d'abord.
        let mut parcouru = 0.0;
        for i in 0..self.points.len() - 1 {
            let (a, b) = (self.points[i], self.points[i + 1]);
            for s in coup.windows(2) {
                if let Some(w) = croisement(a, b, s[0], s[1]) {
                    return Some(((parcouru + w * distance(a, b)) / total).clamp(0.0, 1.0));
                }
            }
            parcouru += distance(a, b);
        }
        // Sinon, le point du trait le plus près du chemin, s'il le frôle.
        coup.iter().map(|&p| self.plus_proche(p)).filter(|(_, d)| *d <= MARQUE_DISTANCE_MAX).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(f, _)| f)
    }

    /// Pose une marque à la fraction `f` : son rang, ou rien si elle tombe sur un bout ou sur une marque (à `ECART_MIN` près).
    pub fn ajouter_marque(&mut self, f: f32) -> Option<usize> {
        if !(ECART_MIN..=1.0 - ECART_MIN).contains(&f) || self.marques.iter().any(|m| (m - f).abs() < ECART_MIN) {
            return None;
        }
        let i = self.marques.iter().position(|m| *m > f).unwrap_or(self.marques.len());
        self.marques.insert(i, f);
        Some(i)
    }

    /// La marque dont le point du chemin est à moins de `tolerance` du point `p` (la plus proche).
    pub fn marque_pres_de(&self, p: [f32; 2], tolerance: f32) -> Option<usize> {
        self.marques
            .iter()
            .enumerate()
            .map(|(i, &m)| (i, distance(self.point_a(m), p)))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    pub fn retirer_marque(&mut self, i: usize) {
        if i < self.marques.len() {
            self.marques.remove(i);
        }
    }

    /// Glisse la marque `i` à la fraction `f`, bornée entre ses voisines (et les bouts) : la fraction retenue.
    pub fn deplacer_marque(&mut self, i: usize, f: f32) -> f32 {
        let bas = if i == 0 { ECART_MIN } else { self.marques[i - 1] + ECART_MIN };
        let haut = if i + 1 == self.marques.len() { 1.0 - ECART_MIN } else { self.marques[i + 1] - ECART_MIN };
        let f = if bas <= haut { f.clamp(bas, haut) } else { self.marques[i] };
        self.marques[i] = f;
        f
    }

    /// Tout le chemin déplacé de (`dx`, `dy`).
    pub fn deplacer(&mut self, dx: f32, dy: f32) {
        self.transformer(|p| [p[0] + dx, p[1] + dy]);
    }

    /// Chaque point du chemin passé par `f` (étirer, tourner…) ; les marques, des fractions, suivent d'elles-mêmes.
    pub fn transformer(&mut self, f: impl Fn([f32; 2]) -> [f32; 2]) {
        for p in &mut self.points {
            *p = f(*p);
        }
    }

    /// L'englobant du chemin : coin haut-gauche, coin bas-droit.
    pub fn englobant(&self) -> ([f32; 2], [f32; 2]) {
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for p in &self.points {
            min = [min[0].min(p[0]), min[1].min(p[1])];
            max = [max[0].max(p[0]), max[1].max(p[1])];
        }
        (min, max)
    }
}

// ------------------------------------------------------------------------------------------------ géométrie

pub fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// La longueur d'une ligne brisée.
pub fn longueur_de(points: &[[f32; 2]]) -> f32 {
    points.windows(2).map(|s| distance(s[0], s[1])).sum()
}

/// La projection de `p` sur le segment `ab` : la position dedans (0 à 1) et la distance.
fn projection(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> (f32, f32) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let w = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (w, distance(p, [a[0] + dx * w, a[1] + dy * w]))
}

/// Le croisement des segments `ab` et `cd` : la position sur `ab` (0 à 1), ou rien.
fn croisement(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> Option<f32> {
    let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-9 {
        return None;
    }
    let ac = [c[0] - a[0], c[1] - a[1]];
    let t = (ac[0] * s[1] - ac[1] * s[0]) / den;
    let u = (ac[0] * r[1] - ac[1] * r[0]) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// Les points d'un trait, sans doublons, un peu lissés (les bouts restent).
pub fn lisser(brut: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut p: Vec<[f32; 2]> = Vec::with_capacity(brut.len());
    for &q in brut {
        if q[0].is_finite() && q[1].is_finite() && p.last().is_none_or(|l| distance(*l, q) > 1e-3) {
            p.push(q);
        }
    }
    for _ in 0..2 {
        if p.len() < 3 {
            break;
        }
        let avant = p.clone();
        for i in 1..p.len() - 1 {
            p[i] = [
                (avant[i - 1][0] + 2.0 * avant[i][0] + avant[i + 1][0]) / 4.0,
                (avant[i - 1][1] + 2.0 * avant[i][1] + avant[i + 1][1]) / 4.0,
            ];
        }
    }
    p
}

/// Les points d'une ligne brisée repris tous les `PAS` pixels (plus large au-delà de `POINTS_MAX` points) ; le premier et le
/// dernier point sont gardés tels quels.
pub fn echantillonner(p: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if p.len() < 2 {
        return p.to_vec();
    }
    let total = longueur_de(p);
    let pas = PAS.max(total / POINTS_MAX as f32);
    let mut out = vec![p[0]];
    let mut prochain = pas;
    let mut parcouru = 0.0;
    for s in p.windows(2) {
        let l = distance(s[0], s[1]);
        while l > 0.0 && prochain <= parcouru + l {
            let w = (prochain - parcouru) / l;
            out.push([s[0][0] + (s[1][0] - s[0][0]) * w, s[0][1] + (s[1][1] - s[0][1]) * w]);
            prochain += pas;
        }
        parcouru += l;
    }
    let dernier = p[p.len() - 1];
    if out.last().is_none_or(|l| distance(*l, dernier) > pas * 0.25) {
        out.push(dernier);
    } else {
        *out.last_mut().unwrap() = dernier;
    }
    out
}
