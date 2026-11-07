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

/// Les clés que pose la trajectoire (Anthony, 2026-11-07, « deux choix ») : **interpolées**, une par image, le calque glisse le
/// long du chemin ; ou **par images**, une à chaque jalon (le départ, chaque marque, l'arrivée), tenue jusqu'à la suivante : le calque
/// saute de marque en marque, sans interpolation, comme des poses tenues.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cles {
    /// D'origine (Anthony, 2026-11-07 : « garde clés d'images comme option initiale »).
    #[default]
    Images,
    Interpolation,
}

impl Cles {
    pub fn label(self) -> &'static str {
        match self {
            Cles::Interpolation => "Clés d'interpolation",
            Cles::Images => "Clés d'images",
        }
    }
}

/// Le point du dessin qui suit le chemin (Anthony, 2026-11-07 : « une option qui aligne le point d'ancrage du calque sur la
/// trajectoire, qui aligne le pixel le plus bas ou autres options pertinentes »).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Ancre {
    /// Là où part le trait : le calque garde sa place au départ et suit la forme du chemin (déplacement relatif).
    #[default]
    Depart,
    /// Le bas du dessin (le milieu de son bord bas) : les pieds sur le chemin.
    Bas,
    /// Le centre du dessin.
    Centre,
    /// Le haut du dessin (le milieu de son bord haut).
    Haut,
    /// Le bord gauche du dessin (à mi-hauteur).
    Gauche,
    /// Le bord droit du dessin (à mi-hauteur).
    Droite,
    /// Un point du dessin choisi d'un clic, en coordonnées du dessin du calque.
    Point { x: f32, y: f32 },
}

impl Ancre {
    /// Les ancrages tout faits, dans l'ordre des réglages (le point cliqué se choisit à part).
    pub const CHOIX: [Ancre; 6] = [Ancre::Depart, Ancre::Bas, Ancre::Centre, Ancre::Haut, Ancre::Gauche, Ancre::Droite];

    pub fn label(self) -> &'static str {
        match self {
            Ancre::Depart => "Là où part le trait",
            Ancre::Bas => "Le bas du dessin",
            Ancre::Centre => "Le centre du dessin",
            Ancre::Haut => "Le haut du dessin",
            Ancre::Gauche => "Le bord gauche",
            Ancre::Droite => "Le bord droit",
            Ancre::Point { .. } => "Un point choisi d'un clic",
        }
    }

    /// Le point d'ancrage dans la boîte `(min, max)` du dessin (coordonnées du dessin) ; rien pour « là où part le trait » et le point choisi.
    pub fn dans_la_boite(self, min: [f32; 2], max: [f32; 2]) -> Option<[f32; 2]> {
        let milieu = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
        match self {
            Ancre::Bas => Some([milieu[0], max[1]]),
            Ancre::Centre => Some(milieu),
            Ancre::Haut => Some([milieu[0], min[1]]),
            Ancre::Gauche => Some([min[0], milieu[1]]),
            Ancre::Droite => Some([max[0], milieu[1]]),
            Ancre::Depart | Ancre::Point { .. } => None,
        }
    }
}

/// Un dessin posé par la trajectoire en « clés d'images » : à un jalon (une marque, ou l'arrivée), le numéro du dessin et le
/// déplacement (pixels de la feuille) qu'il porte par rapport au dessin du départ.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DessinJalon {
    pub dessin: u64,
    pub dx: f32,
    pub dy: f32,
}

/// La trajectoire d'un calque, telle qu'elle s'écrit dans le projet (`trajectoire` d'un calque).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trajectoire {
    /// Les points du chemin, en pixels de la feuille, du départ à l'arrivée ; au moins deux, à `PAS` l'un de l'autre.
    pub points: Vec<[f32; 2]>,
    /// Les marques, en fraction de la longueur du chemin (0 : le départ, 1 : l'arrivée), strictement entre les deux, triées.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marques: Vec<f32>,
    /// L'image du plan où le chemin commence : la tête de lecture au moment du tracé (Anthony : « la clé de départ démarre sur la
    /// tête de lecture »). Avant, le calque ne bouge pas.
    #[serde(default, skip_serializing_if = "est_zero_usize")]
    pub depart: usize,
    /// L'image du plan où le chemin finit ; rien : la dernière image du plan (Anthony : « il faut pouvoir bouger la dernière clé et
    /// que ça recale toutes les autres clés entre la première et la dernière »). Après, le calque reste au bout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fin: Option<usize>,
    /// Le temps de chaque marque, en fraction du temps entre le départ et la fin (strictement entre 0 et 1, croissant, une valeur
    /// par marque) ; rien : le temps est partagé en parts égales entre les jalons (la feuille de timing). Posé dès qu'une clé est
    /// recalée à la main sur la timeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temps: Option<Vec<f32>>,
    /// Les clés posées : par images (d'origine) ou interpolées.
    #[serde(default, skip_serializing_if = "est_images")]
    pub cles: Cles,
    /// Le point du dessin qui suit le chemin.
    #[serde(default, skip_serializing_if = "est_depart")]
    pub ancre: Ancre,
    /// Le déplacement que le dernier calcul a donné au calque à l'image du départ (clés d'interpolation : la clé du départ vaut la pose
    /// propre du calque plus ce déplacement) : ce qu'il faut retirer pour retrouver la pose propre au calcul suivant.
    #[serde(default, skip_serializing_if = "est_zero_2")]
    pub decalage_depart: [f32; 2],
    /// En « clés d'images » : le dessin d'origine du départ (celui que la trajectoire copie à chaque jalon), noté au premier calcul.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<u64>,
    /// En « clés d'images » : les dessins posés aux jalons, un par jalon (le départ compris : le dessin d'origine lui-même quand il ne
    /// bouge pas).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dessins: Vec<DessinJalon>,
}

fn est_zero_usize(v: &usize) -> bool {
    *v == 0
}
fn est_images(c: &Cles) -> bool {
    *c == Cles::Images
}
fn est_depart(a: &Ancre) -> bool {
    *a == Ancre::Depart
}
fn est_zero_2(v: &[f32; 2]) -> bool {
    v[0] == 0.0 && v[1] == 0.0
}

impl Trajectoire {
    /// Un chemin fait des points d'un trait (feuille) : lissé, rééchantillonné. Rien si le trait est trop court.
    pub fn nouvelle(brut: &[[f32; 2]]) -> Option<Trajectoire> {
        let points = echantillonner(&lisser(brut));
        (points.len() >= 2 && longueur_de(&points) >= LONGUEUR_MIN).then_some(Trajectoire {
            points,
            marques: Vec::new(),
            depart: 0,
            fin: None,
            temps: None,
            cles: Cles::default(),
            ancre: Ancre::Depart,
            decalage_depart: [0.0, 0.0],
            source: None,
            dessins: Vec::new(),
        })
    }

    /// Une trajectoire relue d'un fichier : ses points filtrés (abîmés, doublons), ses marques remises en ordre et dans leurs
    /// bornes, ses temps dans les leurs. Rien si le chemin ne tient pas debout.
    pub fn assainie(mut self) -> Option<Trajectoire> {
        self.points.retain(|p| p[0].is_finite() && p[1].is_finite());
        self.points.dedup_by(|a, b| distance(*a, *b) < 1e-3);
        if self.points.len() < 2 || longueur_de(&self.points) < LONGUEUR_MIN {
            return None;
        }
        let marques = std::mem::take(&mut self.marques);
        let temps = self.temps.take();
        for m in marques {
            if m.is_finite() {
                self.ajouter_marque(m);
            }
        }
        // Les temps : un par marque, finis, strictement croissants entre 0 et 1 ; sinon, parts égales.
        if let Some(t) = temps {
            let bons = t.len() == self.marques.len() && t.iter().all(|v| v.is_finite()) && t.windows(2).all(|w| w[0] < w[1]) && t.first().is_none_or(|v| *v > 0.0) && t.last().is_none_or(|v| *v < 1.0);
            if bons {
                self.temps = Some(t);
            }
        }
        if self.fin.is_some_and(|f| f <= self.depart) {
            self.fin = None;
        }
        self.dessins.retain(|d| d.dx.is_finite() && d.dy.is_finite());
        if !self.decalage_depart[0].is_finite() || !self.decalage_depart[1].is_finite() {
            self.decalage_depart = [0.0, 0.0];
        }
        if let Ancre::Point { x, y } = self.ancre {
            if !x.is_finite() || !y.is_finite() {
                self.ancre = Ancre::Depart;
            }
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

    /// Les jalons du chemin : le départ, les marques, l'arrivée (fractions de la longueur).
    pub fn jalons(&self) -> Vec<f32> {
        let mut j = Vec::with_capacity(self.marques.len() + 2);
        j.push(0.0);
        j.extend_from_slice(&self.marques);
        j.push(1.0);
        j
    }

    /// Les jalons du temps : le départ (0), chaque marque, l'arrivée (1), en fraction du temps entre le départ et la fin ; parts
    /// égales si aucun temps n'a été posé à la main.
    pub fn temps_des_jalons(&self) -> Vec<f32> {
        let n = self.marques.len() + 1;
        let mut t = Vec::with_capacity(n + 1);
        t.push(0.0);
        match &self.temps {
            Some(v) if v.len() == self.marques.len() => t.extend_from_slice(v),
            _ => t.extend((1..n).map(|k| k as f32 / n as f32)),
        }
        t.push(1.0);
        t
    }

    /// La dernière image que le chemin atteint dans un plan de `duree` images : la fin posée, bornée à la dernière image du plan,
    /// sinon la dernière image du plan. Rien si le plan n'a pas au moins une image après le départ.
    fn derniere_image(&self, duree: usize) -> Option<usize> {
        if duree < self.depart + 2 {
            return None;
        }
        Some(self.fin.map_or(duree - 1, |f| f.clamp(self.depart + 1, duree - 1)))
    }

    /// Les images (d'un plan de `duree` images) où tombent les jalons : le départ, chaque marque, l'arrivée ; arrondies à l'image.
    /// Rien si le plan n'a pas au moins une image après le départ.
    pub fn images_des_jalons(&self, duree: usize) -> Vec<usize> {
        let Some(fin) = self.derniere_image(duree) else { return Vec::new() };
        let portee = (fin - self.depart) as f32;
        self.temps_des_jalons().iter().map(|t| self.depart + (t * portee).round() as usize).collect()
    }

    /// Le jalon `k` (0 : le départ, puis les marques, puis l'arrivée) recalé à l'image `image` d'un plan de `duree` images : le
    /// départ et la fin bougent (bornés l'un par l'autre et par le plan), une marque change de temps (bornée par ses voisines : les
    /// temps deviennent explicites). Rend l'image retenue, ou rien si le jalon n'existe pas ou que le plan est trop court.
    pub fn recaler(&mut self, k: usize, image: usize, duree: usize) -> Option<usize> {
        let fin = self.derniere_image(duree)?;
        let dernier = self.marques.len() + 1;
        if k > dernier {
            return None;
        }
        if k == 0 {
            let v = image.min(fin - 1);
            // La fin posée reste là où elle est ; le temps des marques est une fraction, il suit.
            self.depart = v;
            return Some(v);
        }
        if k == dernier {
            let v = image.clamp(self.depart + 1, duree - 1);
            self.fin = Some(v);
            return Some(v);
        }
        let mut temps = self.temps_des_jalons();
        let portee = (fin - self.depart) as f32;
        let (bas, haut) = (temps[k - 1] + 1.0 / portee, temps[k + 1] - 1.0 / portee);
        if bas > haut {
            return None;
        }
        let t = (((image as f32 - self.depart as f32) / portee).clamp(bas, haut) * portee).round() / portee;
        temps[k] = t.clamp(bas, haut);
        self.temps = Some(temps[1..dernier].to_vec());
        Some(self.depart + (temps[k] * portee).round() as usize)
    }

    /// Le temps repart en parts égales entre les jalons.
    pub fn temps_egaux(&mut self) {
        self.temps = None;
    }

    /// Le déplacement du calque à chaque jalon, pour que le point d'ancrage `ancre` (pixels de la feuille) soit sur le chemin.
    pub fn decalages(&self, ancre: [f32; 2]) -> Vec<[f32; 2]> {
        self.jalons().iter().map(|&f| { let p = self.point_a(f); [p[0] - ancre[0], p[1] - ancre[1]] }).collect()
    }

    /// Le chemin retourné de gauche à droite (`horizontal`) ou de haut en bas, autour de son départ : le calque, retourné de même,
    /// suit le chemin retourné.
    pub fn retourner(&mut self, horizontal: bool) {
        let d = self.points[0];
        self.transformer(|p| if horizontal { [2.0 * d[0] - p[0], p[1]] } else { [p[0], 2.0 * d[1] - p[1]] });
    }

    /// La fraction du chemin atteinte à l'image `f` d'un plan de `duree` images (le temps linéaire entre deux jalons), ou rien si le
    /// chemin ne couvre pas cette image (avant le départ, après la fin).
    pub fn fraction_a_l_image(&self, f: usize, duree: usize) -> Option<f32> {
        let fin = self.derniere_image(duree)?;
        if f < self.depart || f > fin {
            return None;
        }
        let (jalons, temps) = (self.jalons(), self.temps_des_jalons());
        let u = (f - self.depart) as f32 / (fin - self.depart) as f32;
        let i = temps.windows(2).position(|w| u <= w[1]).unwrap_or(jalons.len() - 2);
        let w = ((u - temps[i]) / (temps[i + 1] - temps[i]).max(1e-6)).clamp(0.0, 1.0);
        Some(jalons[i] + (jalons[i + 1] - jalons[i]) * w)
    }

    /// Les clés de position qu'elle pose sur un calque de pose propre `base` dans un plan de `duree` images (**clés
    /// d'interpolation** : une clé par image que le chemin couvre, le déplacement qui met le point d'ancrage `ancre`, en pixels de la
    /// feuille sous la pose `base`, sur le chemin). Aucune en **clés d'images** (ce sont des dessins, posés par l'application :
    /// `trajectoireui.rs`), ni si le plan n'a pas au moins une image après le départ.
    pub fn cles(&self, duree: usize, base: LayerTransform, ancre: [f32; 2]) -> Vec<PosKey> {
        let Some(fin) = self.derniere_image(duree) else { return Vec::new() };
        if self.cles == Cles::Images {
            return Vec::new();
        }
        (self.depart..=fin)
            .filter_map(|f| {
                let p = self.point_a(self.fraction_a_l_image(f, duree)?);
                Some(PosKey::new(f, LayerTransform { x: base.x + p[0] - ancre[0], y: base.y + p[1] - ancre[1], ..base }))
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

    /// Pose une marque à la fraction `f` : son rang, ou rien si elle tombe sur un bout ou sur une marque (à `ECART_MIN` près). Des
    /// temps posés à la main : la marque prend le milieu du temps de son intervalle.
    pub fn ajouter_marque(&mut self, f: f32) -> Option<usize> {
        if !(ECART_MIN..=1.0 - ECART_MIN).contains(&f) || self.marques.iter().any(|m| (m - f).abs() < ECART_MIN) {
            return None;
        }
        let i = self.marques.iter().position(|m| *m > f).unwrap_or(self.marques.len());
        if let Some(t) = self.temps.as_mut().filter(|t| t.len() == self.marques.len()) {
            let (avant, apres) = (if i == 0 { 0.0 } else { t[i - 1] }, if i < t.len() { t[i] } else { 1.0 });
            t.insert(i, (avant + apres) * 0.5);
        } else {
            self.temps = None;
        }
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
            match self.temps.as_mut() {
                Some(t) if i < t.len() => {
                    t.remove(i);
                }
                _ => self.temps = None,
            }
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

    /// Le rayon (pixels de la feuille, le long du chemin) sur lequel tirer le chemin le tord : un bon tiers du chemin, 30 au moins.
    pub fn rayon_de_tirage(&self) -> f32 {
        (self.longueur() * 0.3).max(30.0)
    }

    /// Le chemin tiré en `fraction` de (`dx`, `dy`) : comme une corde, le point pris suit le pointeur, ses voisins le long du chemin
    /// d'autant moins qu'ils sont loin (jusqu'à `rayon` de chemin, en cloche) ; le reste ne bouge pas. Les marques, des fractions,
    /// restent où elles sont le long du chemin. Les points sont ensuite repris au pas.
    pub fn tirer(&mut self, fraction: f32, dx: f32, dy: f32, rayon: f32) {
        let total = self.longueur();
        let s0 = fraction.clamp(0.0, 1.0) * total;
        let rayon = rayon.max(1e-3);
        let mut parcouru = 0.0;
        let mut points = self.points.clone();
        for i in 0..points.len() {
            if i > 0 {
                parcouru += distance(self.points[i - 1], self.points[i]);
            }
            let d = (parcouru - s0).abs();
            if d < rayon {
                let w = 0.5 * (1.0 + (std::f32::consts::PI * d / rayon).cos());
                points[i] = [points[i][0] + dx * w, points[i][1] + dy * w];
            }
        }
        let repris = echantillonner(&points);
        if repris.len() >= 2 && longueur_de(&repris) >= LONGUEUR_MIN {
            self.points = repris;
        }
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
    // Le dernier point, tel quel (un tout dernier pas plus court, jamais plus long).
    let dernier = p[p.len() - 1];
    if out.last().is_none_or(|l| distance(*l, dernier) > 1e-3) {
        out.push(dernier);
    }
    out
}
