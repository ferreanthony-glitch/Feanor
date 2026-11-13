//! La trajectoire d'un calque (Anthony, 2026-11-07) : « Lié à un calque, un outil spécial permet de dessiner une trajectoire
//! et un timing qui ensuite pose des clés de mouvement. La ligne principale est la trajectoire, les traits qui la découpent
//! sont les clés de spacing, comme en animation. »
//!
//! Le modèle, sans écran : le **chemin** (un trait sur la feuille), ses **marques** (des fractions de sa longueur, posées par
//! de petits traits en travers), et les **clés de position** (`poskeys.rs`) qu'ils posent sur le calque : une clé par image,
//! le calque avançant à vitesse constante entre deux marques (marques serrées : lent ; espacées : vite), le temps partagé en
//! parts égales entre les marques — la durée du plan (d'origine), ou « une marque = N images ». Le déplacement est
//! **relatif** : le calque garde sa pose au départ du chemin et en suit la forme (on part le trait de là où est le personnage).
//!
//! Deux compléments (Anthony, 2026-11-08) : **le dessin peut tourner avec le chemin** (`tourne` : il pivote pour regarder où va le
//! chemin, en gardant son orientation du départ, autour du point d'ancrage), et **insérer ou retirer des images décale la
//! trajectoire** (`decaler_images` : le départ, la fin et chaque marque restent sur leur image, comme les clés et les dessins).
//!
//! **Les poignées** (partie 154 ; Anthony, 2026-11-13 : « avec l'outil trajectoire ctrl+clic sur la trajectoire permet
//! d'ajuster la trajectoire en ajoutant une poignée ») : des fractions du chemin (`poignees`), comme les marques. Tirer une
//! poignée tord le chemin en douceur autour d'elle (`tirer_poignee`), les autres poignées et les bouts restant fixes. Dans
//! l'Espace 3D, une poignée se tire aussi sur les axes x, y, z du monde, **hors du plan porteur** : chaque point du chemin a
//! alors une hauteur le long de la normale de ce plan (`hauteurs`), et son point dans l'espace est
//! `support.monde(p) + h · normale` (`Support::monde3`). Les marques, le temps, le tirage et le recalage restent les calculs
//! à plat, dans le plan : les fractions se mesurent sur le chemin tel qu'il est posé dans son plan.

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
/// La direction du chemin se lit sur au moins tant de pixels de la feuille de part et d'autre du point (2 % du chemin au moins) : un
/// dessin qui tourne avec le chemin ne tremble pas au gré des points du trait.
pub const FENETRE_DIRECTION: f32 = 3.0 * PAS;

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
/// déplacement (pixels de la feuille) qu'il porte par rapport au dessin du départ, et sa rotation (degrés, autour du point
/// d'ancrage) quand le dessin tourne avec le chemin.
///
/// **Chaque dessin appartient à son jalon** (Anthony, 2026-11-13 : « règle ce qui reste imparfait » ; avant, une marque posée
/// au milieu du chemin décalait d'une marque les poses déjà redessinées). Deux jalons qui se suivent avec **le même dessin** :
/// le second vient d'être posé (`Trajectoire::ajouter_marque`), il montre encore le dessin du jalon d'avant, celui que l'image
/// montrait là ; le calcul suivant lui en fait une copie à lui (`trajectoireui.rs`, `trajectoire3d.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DessinJalon {
    pub dessin: u64,
    pub dx: f32,
    pub dy: f32,
    #[serde(default, skip_serializing_if = "est_zero_f32")]
    pub angle: f32,
}

/// **Le support d'une trajectoire tracée dans l'Espace 3D** (`docs/INTERFACE.md`, partie 149) : le plan de l'espace sur lequel le
/// chemin est posé (une carte, le sol, ou un plan face à la vue). Les points du chemin restent deux nombres, lus dans ce plan : le
/// point (a, b) du chemin est, dans l'espace, `origine + a · u + b · v` (mètres). Les deux axes sont perpendiculaires et de même
/// longueur : celle d'un pixel de la feuille au plan de référence (`espace3d::Monde::k` mètres), pour que les mesures du chemin (le
/// pas, la longueur d'un clic, le rayon du tirage) gardent une taille sensée. Sans support, la trajectoire est sur la feuille.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Support {
    /// Le point de l'espace (mètres) où le chemin a ses coordonnées (0, 0).
    pub origine: [f32; 3],
    /// Le premier axe du plan : une unité du chemin, en mètres.
    pub u: [f32; 3],
    /// Le second axe, perpendiculaire au premier, de la même longueur.
    pub v: [f32; 3],
}

fn v3_moins(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn v3_scalaire(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn v3_norme(a: [f32; 3]) -> f32 {
    v3_scalaire(a, a).sqrt()
}

impl Support {
    /// Le plan qui passe par `origine`, d'axes dirigés par `u` et `v` (redressés : le second est rendu perpendiculaire au premier),
    /// une unité du chemin valant `unite` mètres. Rien si les directions ne font pas un plan (nulles, alignées, abîmées).
    pub fn nouveau(origine: [f32; 3], u: [f32; 3], v: [f32; 3], unite: f32) -> Option<Support> {
        let nu = v3_norme(u);
        if !(nu.is_finite() && nu > 1e-9 && unite.is_finite() && unite > 1e-9) || !origine.iter().all(|x| x.is_finite()) {
            return None;
        }
        let u = u.map(|x| x / nu);
        let k = v3_scalaire(v, u);
        let v = [v[0] - u[0] * k, v[1] - u[1] * k, v[2] - u[2] * k];
        let nv = v3_norme(v);
        if !(nv.is_finite() && nv > 1e-6) {
            return None;
        }
        Some(Support { origine, u: u.map(|x| x * unite), v: v.map(|x| x / nv * unite) })
    }

    /// Le point de l'espace (mètres) du point `p` du chemin.
    pub fn monde(&self, p: [f32; 2]) -> [f32; 3] {
        [0, 1, 2].map(|i| self.origine[i] + p[0] * self.u[i] + p[1] * self.v[i])
    }

    /// Le déplacement dans l'espace (mètres) du déplacement `d` dans le plan.
    pub fn deplacement(&self, d: [f32; 2]) -> [f32; 3] {
        [0, 1, 2].map(|i| d[0] * self.u[i] + d[1] * self.v[i])
    }

    /// La longueur d'une unité du chemin, en mètres.
    pub fn unite(&self) -> f32 {
        v3_norme(self.u)
    }

    /// Le point de l'espace (mètres) du point `p` du chemin et de sa hauteur `p[2]` (unités du chemin, le long de la normale
    /// du plan ; partie 154).
    pub fn monde3(&self, p: [f32; 3]) -> [f32; 3] {
        let (n, k) = (self.normale(), p[2] * self.unite());
        [0, 1, 2].map(|i| self.origine[i] + p[0] * self.u[i] + p[1] * self.v[i] + k * n[i])
    }

    /// Le déplacement dans l'espace (mètres) du déplacement `d` dans le plan et de la hauteur `d[2]`.
    pub fn deplacement3(&self, d: [f32; 3]) -> [f32; 3] {
        let (n, k) = (self.normale(), d[2] * self.unite());
        [0, 1, 2].map(|i| d[0] * self.u[i] + d[1] * self.v[i] + k * n[i])
    }

    /// Un déplacement dans l'espace (mètres) lu dans le plan : sa part le long de chaque axe et sa hauteur, en unités du
    /// chemin (l'inverse de `deplacement3`).
    pub fn decomposer(&self, d: [f32; 3]) -> [f32; 3] {
        let u2 = v3_scalaire(self.u, self.u).max(1e-12);
        [v3_scalaire(d, self.u) / u2, v3_scalaire(d, self.v) / v3_scalaire(self.v, self.v).max(1e-12), v3_scalaire(d, self.normale()) / self.unite().max(1e-9)]
    }

    /// Le plan parallèle, à la hauteur `h` (unités du chemin) : un point y a les mêmes coordonnées que sa projection sur le
    /// plan d'origine.
    pub fn a_la_hauteur(self, h: f32) -> Support {
        let (n, k) = (self.normale(), h * self.unite());
        self.deplace(n.map(|x| x * k))
    }

    /// Le point du plan le plus proche du point `q` de l'espace (sa projection à angle droit), en coordonnées du chemin.
    pub fn dans_le_plan(&self, q: [f32; 3]) -> [f32; 2] {
        let d = v3_moins(q, self.origine);
        [v3_scalaire(d, self.u) / v3_scalaire(self.u, self.u), v3_scalaire(d, self.v) / v3_scalaire(self.v, self.v)]
    }

    /// La normale du plan (longueur 1).
    pub fn normale(&self) -> [f32; 3] {
        let (a, b) = (self.u, self.v);
        let n = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let l = v3_norme(n).max(1e-12);
        n.map(|x| x / l)
    }

    /// Là où le rayon parti de `depart` dans la direction `direction` traverse le plan, en coordonnées du chemin. Rien s'il lui est
    /// parallèle, s'il le traverse derrière son départ, ou trop loin pour que ce soit encore un point du chemin (près de l'horizon
    /// d'un sol, le point part à l'infini).
    pub fn couper(&self, depart: [f32; 3], direction: [f32; 3]) -> Option<[f32; 2]> {
        let n = self.normale();
        let den = v3_scalaire(direction, n);
        if !den.is_finite() || den.abs() < 1e-6 * v3_norme(direction).max(1e-12) {
            return None;
        }
        let t = v3_scalaire(v3_moins(self.origine, depart), n) / den;
        if !(t.is_finite() && t > 0.0) {
            return None;
        }
        let p = self.dans_le_plan([0, 1, 2].map(|i| depart[i] + direction[i] * t));
        (p[0].is_finite() && p[1].is_finite() && p[0].abs() < COORDONNEE_MAX && p[1].abs() < COORDONNEE_MAX).then_some(p)
    }

    /// Le plan déplacé de `d` mètres : le chemin qu'il porte se déplace d'autant dans l'espace.
    pub fn deplace(self, d: [f32; 3]) -> Support {
        Support { origine: [0, 1, 2].map(|i| self.origine[i] + d[i]), ..self }
    }

    /// Le plan vu dans un miroir dressé en x = `x` (retourner un plan de gauche à droite).
    pub fn miroir_x(self, x: f32) -> Support {
        let m = |p: [f32; 3]| [-p[0], p[1], p[2]];
        Support { origine: [2.0 * x - self.origine[0], self.origine[1], self.origine[2]], u: m(self.u), v: m(self.v) }
    }

    /// Un support relu d'un fichier : rien s'il ne fait pas un plan (valeurs abîmées, axes nuls ou alignés). Des axes
    /// perpendiculaires et de même longueur reviennent tels quels, au chiffre près ; sinon ils sont redressés (`nouveau`).
    pub fn assaini(self) -> Option<Support> {
        let fini = |p: [f32; 3]| p.iter().all(|x| x.is_finite() && x.abs() < 1e6);
        if !(fini(self.origine) && fini(self.u) && fini(self.v)) {
            return None;
        }
        let (nu, nv) = (v3_norme(self.u), v3_norme(self.v));
        if !(nu > 1e-9 && nv > 1e-9) {
            return None;
        }
        let droit = (v3_scalaire(self.u, self.v) / (nu * nv)).abs() < 1e-3 && (nu - nv).abs() <= 1e-3 * nu;
        if droit { Some(self) } else { Support::nouveau(self.origine, self.u, self.v, nu) }
    }
}

/// Au-delà de cette distance de l'origine de son plan (unités du chemin), un point n'est plus un point du chemin.
pub const COORDONNEE_MAX: f32 = 2.0e5;

/// La trajectoire d'un calque, telle qu'elle s'écrit dans le projet (`trajectoire` d'un calque) ; ou, avec un support, celle de la
/// caméra d'un plan en Espace 3D (`trajectoire_camera` d'un plan).
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
    /// bouge pas), dans l'ordre des jalons. **Chacun suit le sien** : poser, retirer une marque ou toutes les retirer garde à chaque
    /// autre jalon son dessin (`ajouter_marque`, `retirer_marque`, `retirer_les_marques`, `assainie`) ; glisser une marque, recaler
    /// le temps ou retracer le chemin ne change pas leur ordre.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dessins: Vec<DessinJalon>,
    /// Les dessins des marques retirées depuis le dernier calcul (`retirer_marque`, `retirer_les_marques`) : c'est la trajectoire
    /// qui les avait posés, et le calcul qui suit les reprend aux images qui les montrent encore. Sans cette note, il les
    /// prendrait pour des dessins posés à la main, qu'il ne recouvre jamais (`trajectoireui.rs`,
    /// `trajectoire_exposer_les_dessins`). **Jamais écrite dans le fichier** : le calcul, qui suit toujours, la vide.
    #[serde(skip)]
    pub retires: Vec<u64>,
    /// Le dessin tourne avec le chemin (Anthony, 2026-11-08) : il pivote, autour du point d'ancrage, pour regarder où va le chemin,
    /// en gardant son orientation du départ. Faux d'origine : il garde son orientation sur tout le chemin.
    #[serde(default, skip_serializing_if = "est_faux")]
    pub tourne: bool,
    /// **Tracée dans l'Espace 3D** (format 55, partie 149) : le plan de l'espace où le chemin est posé. Ses points se lisent alors
    /// dans ce plan, et ce qui la suit (une carte, la caméra du plan) se déplace dans l'espace du déplacement du chemin depuis son
    /// départ. Rien : une trajectoire sur la feuille, celle de toujours. L'ancrage et « le dessin tourne » n'y servent pas. En
    /// « clés d'images », une carte a ses `dessins` comme sur la feuille (partie 152), sans déplacement ni angle : la carte se
    /// déplace elle-même, par une clé de position tenue à chaque jalon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support: Option<Support>,
    /// **Le lissage du chemin** (partie 151 ; Anthony, 2026-11-12 : « la courbe tracée pour une trajectoire doit pouvoir être
    /// lissée dans les réglages d'outils »), de 0 (le trait tel qu'il a été tracé) à 1 (le plus lisse). `points` est alors le
    /// chemin brut (`brut`) lissé d'autant (`lissage_de`), ses deux bouts à leur place. Sans changer le format : une version
    /// qui ne le connaît pas lit `points`, le chemin lissé, et le garde tel quel.
    #[serde(default, skip_serializing_if = "est_zero_f32")]
    pub lissage: f32,
    /// Le chemin brut quand il est lissé : le trait tel qu'il a été tracé (repris au pas), tiré, déplacé ou retourné avec le
    /// chemin. C'est de lui que le lissage est recalculé : on peut le remonter ou le redescendre à tout moment. Rien : le
    /// chemin n'est pas lissé, `points` est le chemin brut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brut: Option<Vec<[f32; 2]>>,
    /// Le point du chemin d'où se mesure le déplacement de ce qui le suit (`point_de_reference`) ; rien : son départ, comme
    /// toujours. Posé quand le chemin est **prolongé par son début** (Maj + un trait depuis le début, partie 151) : c'est
    /// l'ancien départ, et ce qui suivait le chemin passe toujours au même endroit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<[f32; 2]>,
    /// **Les poignées** (partie 154) : des fractions du chemin (0 : le départ, 1 : l'arrivée), triées, à `ECART_MIN` au moins
    /// l'une de l'autre. Tirer une poignée tord le chemin autour d'elle ; les autres poignées et les bouts restent fixes. Des
    /// fractions, comme les marques : elles restent à leur place relative quand le chemin est lissé ou retracé, et gardent
    /// leur place quand il est prolongé. Rien : pas de poignée (le format reste 55).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poignees: Vec<f32>,
    /// **La hauteur de chaque point** de `points` hors du plan porteur (partie 154), le long de sa normale, en unités du
    /// chemin : une par point, ou rien (0 partout, le chemin est dans son plan ; un vieux fichier). Seule une trajectoire
    /// dans l'espace en a : c'est une flèche de poignée qui l'en fait sortir.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hauteurs: Vec<f32>,
    /// Les hauteurs des points du chemin brut (`brut`), quand le chemin est lissé : lissées avec lui.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hauteurs_brut: Vec<f32>,
}

/// À lissage 1, la demi-fenêtre de la moyenne glissante (`lissage_de`), en fraction de la longueur du chemin.
pub const LISSAGE_FENETRE: f32 = 0.08;
/// Le nombre de passes de la moyenne glissante (trois passes d'une moyenne simple : une cloche).
const LISSAGE_PASSES: usize = 3;

fn est_zero_usize(v: &usize) -> bool {
    *v == 0
}
fn est_zero_f32(v: &f32) -> bool {
    *v == 0.0
}
fn est_faux(v: &bool) -> bool {
    !*v
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
            retires: Vec::new(),
            tourne: false,
            support: None,
            lissage: 0.0,
            brut: None,
            reference: None,
            poignees: Vec::new(),
            hauteurs: Vec::new(),
            hauteurs_brut: Vec::new(),
        })
    }

    // -------------------------------------------------------------------------------------- hauteurs (partie 154)

    /// La hauteur du point `i` du chemin hors de son plan (unités du chemin) ; 0 sans hauteurs.
    pub fn hauteur_du_point(&self, i: usize) -> f32 {
        self.hauteurs.get(i).copied().unwrap_or(0.0)
    }

    /// La hauteur du chemin à la fraction `f`.
    pub fn hauteur_a(&self, f: f32) -> f32 {
        if self.hauteurs.is_empty() {
            return 0.0;
        }
        let (i, w) = self.segment_a(f);
        let (a, b) = (self.hauteur_du_point(i), self.hauteur_du_point(i + 1));
        a + (b - a) * w
    }

    /// Le point `i` du chemin et sa hauteur.
    pub fn point3(&self, i: usize) -> [f32; 3] {
        let p = self.points[i];
        [p[0], p[1], self.hauteur_du_point(i)]
    }

    /// Le point du chemin à la fraction `f` et sa hauteur.
    pub fn point3_a(&self, f: f32) -> [f32; 3] {
        let p = self.point_a(f);
        [p[0], p[1], self.hauteur_a(f)]
    }

    /// Les points du chemin avec leurs hauteurs.
    pub fn points3(&self) -> impl Iterator<Item = [f32; 3]> + '_ {
        (0..self.points.len()).map(|i| self.point3(i))
    }

    /// Le point de l'espace (mètres) du chemin à la fraction `f` ; rien pour une trajectoire sur la feuille.
    pub fn monde_a(&self, f: f32) -> Option<[f32; 3]> {
        Some(self.support?.monde3(self.point3_a(f)))
    }

    /// La hauteur du point d'où se mesure le déplacement de ce qui suit le chemin (`point_de_reference`).
    fn hauteur_de_reference(&self) -> f32 {
        match (self.hauteurs.is_empty(), self.reference) {
            (true, _) => 0.0,
            (false, None) => self.hauteur_du_point(0),
            (false, Some(r)) => self.hauteur_a(self.plus_proche(r).0),
        }
    }

    /// Le chemin brut et ses hauteurs (le chemin lui-même quand il n'est pas lissé).
    fn chemin_brut3(&self) -> Vec<[f32; 3]> {
        match &self.brut {
            Some(b) => joindre(b, &self.hauteurs_brut),
            None => joindre(&self.points, &self.hauteurs),
        }
    }

    /// Le plan porteur vu dans un miroir dressé en x = `x` (`Support::miroir_x`) : la normale du plan s'y retourne, les
    /// hauteurs changent de signe pour que le chemin reste du même côté.
    pub fn miroir_x(&mut self, x: f32) {
        if let Some(s) = self.support {
            self.support = Some(s.miroir_x(x));
            for h in self.hauteurs.iter_mut().chain(self.hauteurs_brut.iter_mut()) {
                *h = -*h;
            }
        }
    }

    // -------------------------------------------------------------------------------------- poignées (partie 154)

    /// Pose une poignée à la fraction `f` (bornée au chemin) : son rang, ou rien s'il y en a déjà une à `ECART_MIN` près.
    pub fn ajouter_poignee(&mut self, f: f32) -> Option<usize> {
        if !f.is_finite() {
            return None;
        }
        let f = f.clamp(0.0, 1.0);
        if self.poignees.iter().any(|p| (p - f).abs() < ECART_MIN) {
            return None;
        }
        let i = self.poignees.iter().position(|p| *p > f).unwrap_or(self.poignees.len());
        self.poignees.insert(i, f);
        Some(i)
    }

    /// Retire la poignée `i`.
    pub fn retirer_poignee(&mut self, i: usize) {
        if i < self.poignees.len() {
            self.poignees.remove(i);
        }
    }

    /// La poignée la plus proche de la fraction `f`, à `ECART_MIN` près : celle qui, sur une autre copie de la même
    /// trajectoire (un autre sujet choisi), répond à la poignée prise.
    pub fn poignee_a_la_fraction(&self, f: f32) -> Option<usize> {
        self.poignees
            .iter()
            .enumerate()
            .map(|(i, p)| (i, (p - f).abs()))
            .filter(|(_, d)| *d < ECART_MIN)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// La poignée dont le point est à moins de `tolerance` du point `p` du plan (la plus proche) : sur la feuille.
    pub fn poignee_pres_de(&self, p: [f32; 2], tolerance: f32) -> Option<usize> {
        self.poignees
            .iter()
            .enumerate()
            .map(|(i, &f)| (i, distance(self.point_a(f), p)))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// **Tire la poignée `i`** de `d` (dans le plan, et en hauteur `d[2]`) : comme une corde, le point de la poignée suit,
    /// ses voisins d'autant moins qu'ils sont loin, en cloche (cosinus), sur `rayon` de chemin au plus de chaque côté ; mais
    /// jamais au-delà des poignées voisines ni des bouts, qui restent fixes. Les marques gardent leurs fractions.
    pub fn tirer_poignee(&mut self, i: usize, d: [f32; 3], rayon: f32) {
        let Some(&f) = self.poignees.get(i) else { return };
        let bas = if i > 0 { self.poignees[i - 1] } else { 0.0 };
        let haut = self.poignees.get(i + 1).copied().unwrap_or(1.0);
        self.tirer_entre(f, d, rayon, Some(bas), Some(haut));
    }

    /// Le point du chemin d'où se mesure le déplacement de ce qui le suit : son départ, ou l'ancien départ d'un chemin
    /// prolongé par son début (`reference`).
    pub fn point_de_reference(&self) -> [f32; 2] {
        self.reference.unwrap_or(self.points[0])
    }

    /// **Prolonge le chemin** du trait `trait_` (points de la feuille, ou du plan du chemin dans l'Espace 3D), tracé depuis un
    /// de ses bouts : `devant`, depuis le début (le trait est ajouté devant, le chemin commence au bout du trait) ; sinon
    /// depuis la fin (ajouté derrière). Ce qui suivait le chemin continue de le suivre : les marques gardent leur place
    /// (leurs fractions sont recalculées sur la nouvelle longueur), le déplacement se mesure toujours depuis le même point
    /// (`reference`). Le temps se répartit de nouveau en parts égales entre les jalons, comme pour un chemin neuf. Le lissage
    /// du chemin s'applique au morceau ajouté. Faux (rien ne change) si le trait est trop court.
    pub fn prolonger(&mut self, trait_: &[[f32; 2]], devant: bool) -> bool {
        let base = self.chemin_brut3();
        let Some((&debut, &fin)) = base.first().zip(base.last()) else { return false };
        let bout = if devant { debut } else { fin };
        let mut brut = vec![[bout[0], bout[1]]];
        brut.extend(trait_.iter().copied().filter(|p| p[0].is_finite() && p[1].is_finite()));
        let morceau = echantillonner(&lisser(&brut));
        if morceau.len() < 2 || longueur_de(&morceau) < LONGUEUR_MIN {
            return false;
        }
        // (Le morceau ajouté part du bout, à sa hauteur, et la garde : partie 154.)
        let morceau: Vec<[f32; 3]> = morceau.iter().map(|p| [p[0], p[1], bout[2]]).collect();
        // Les marques et les poignées, en longueur depuis le bout qui ne bouge pas.
        let ancienne = self.longueur();
        let depuis = |f: f32| if devant { (1.0 - f) * ancienne } else { f * ancienne };
        let depuis_le_bout_fixe: Vec<f32> = self.marques.iter().map(|&f| depuis(f)).collect();
        let poignees: Vec<f32> = self.poignees.iter().map(|&f| depuis(f)).collect();
        if devant && self.reference.is_none() {
            self.reference = Some(self.points[0]);
        }
        let neuf: Vec<[f32; 3]> = if devant { morceau.iter().rev().copied().chain(base.iter().skip(1).copied()).collect() } else { base.iter().copied().chain(morceau.iter().skip(1).copied()).collect() };
        (self.points, self.hauteurs) = separer(&echantillonner3(&neuf));
        (self.brut, self.hauteurs_brut) = (None, Vec::new());
        self.relisser();
        let longueur = self.longueur().max(1e-6);
        self.poignees = poignees.iter().map(|&s| (if devant { 1.0 - s / longueur } else { s / longueur }).clamp(0.0, 1.0)).collect();
        self.poignees.sort_by(|a, b| a.total_cmp(b));
        self.temps = None;
        // (Toutes les marques restent, dans leur ordre : une marque tout près du bout fixe n'est pas jetée, elle est gardée
        // à `ECART_MIN` des bouts et de ses voisines. En clés d'images, les dessins sont rangés par jalon : en perdre une
        // décalerait ceux qui suivent.)
        let mut marques: Vec<f32> = depuis_le_bout_fixe.iter().map(|&s| if devant { 1.0 - s / longueur } else { s / longueur }).collect();
        marques.sort_by(|a, b| a.total_cmp(b));
        let n = marques.len();
        for (i, m) in marques.iter_mut().enumerate() {
            let (bas, haut) = (ECART_MIN * (i + 1) as f32, 1.0 - ECART_MIN * (n - i) as f32);
            *m = if bas <= haut { m.clamp(bas, haut) } else { (i + 1) as f32 / (n + 1) as f32 };
        }
        for i in 1..n {
            marques[i] = marques[i].max(marques[i - 1] + ECART_MIN);
        }
        self.marques = marques;
        true
    }

    /// Le chemin brut : le trait tel qu'il a été tracé, avant le lissage (`points` quand le chemin n'est pas lissé).
    pub fn chemin_brut(&self) -> &[[f32; 2]] {
        self.brut.as_deref().unwrap_or(&self.points)
    }

    /// **Lisse le chemin** à `lissage` (de 0 à 1), recalculé depuis le chemin brut : le curseur se remonte et se redescend à
    /// tout moment, 0 rend le trait tel qu'il a été tracé. Les deux bouts restent à leur place ; les marques, des fractions,
    /// gardent leur place relative le long du chemin.
    pub fn lisser_a(&mut self, lissage: f32) {
        let l = if lissage.is_finite() { lissage.clamp(0.0, 1.0) } else { 0.0 };
        // (Les hauteurs hors du plan, partie 154, sont lissées avec le reste.)
        let base = self.chemin_brut3();
        (self.brut, self.hauteurs_brut) = (None, Vec::new());
        self.lissage = l;
        if l <= 0.0 {
            (self.points, self.hauteurs) = separer(&base);
            return;
        }
        let (lisse, hauteurs) = separer(&lissage_de3(&base, l));
        (self.points, self.hauteurs) = if lisse.len() >= 2 && longueur_de(&lisse) >= LONGUEUR_MIN { (lisse, hauteurs) } else { separer(&base) };
        let (brut, hauteurs_brut) = separer(&base);
        (self.brut, self.hauteurs_brut) = (Some(brut), hauteurs_brut);
    }

    /// Le chemin brut vient de changer (tiré, déplacé…) : le chemin lissé est refait d'après lui.
    fn relisser(&mut self) {
        let l = self.lissage;
        self.lisser_a(l);
    }

    /// Un chemin tracé dans l'Espace 3D : les points `brut` du trait, lus dans le plan `support`. Rien si le trait est trop court.
    pub fn nouvelle_dans_l_espace(brut: &[[f32; 2]], support: Support) -> Option<Trajectoire> {
        Trajectoire::nouvelle(brut).map(|t| Trajectoire { support: Some(support), ..t })
    }

    /// La trajectoire est sur la feuille (sans support) : ses points sont des pixels de la feuille, et tout ce qui déplace, retourne
    /// ou agrandit la feuille les déplace avec elle. Faux : elle est dans l'espace, ces gestes n'y touchent pas.
    pub fn sur_la_feuille(&self) -> bool {
        self.support.is_none()
    }

    /// Le déplacement dans l'espace (mètres) depuis le départ du chemin jusqu'à sa fraction `f` ; rien pour une trajectoire sur la
    /// feuille.
    pub fn deplacement_a(&self, f: f32) -> Option<[f32; 3]> {
        let s = self.support?;
        let (p, d) = (self.point_a(f), self.point_de_reference());
        // (Et la hauteur hors du plan gagnée depuis le départ, partie 154 : la carte et la caméra montent avec le chemin.)
        let h = if self.hauteurs.is_empty() { 0.0 } else { self.hauteur_a(f) - self.hauteur_de_reference() };
        Some(s.deplacement3([p[0] - d[0], p[1] - d[1], h]))
    }

    /// La fraction du chemin où est, à l'image `image` d'un plan de `duree` images, ce qui suit la trajectoire : 0 avant le départ,
    /// 1 après la fin. En clés d'interpolation, le long du chemin au rythme des marques (`fraction_a_l_image`) ; en clés d'images, le
    /// dernier jalon atteint (le départ, une marque, l'arrivée), tenu jusqu'au suivant. Rien si le plan n'a pas au moins une image
    /// après le départ : rien ne bouge encore.
    pub fn fraction_suivie(&self, image: usize, duree: usize) -> Option<f32> {
        let fin = self.derniere_image(duree)?;
        if image < self.depart {
            return Some(0.0);
        }
        if image >= fin {
            return Some(1.0);
        }
        match self.cles {
            Cles::Interpolation => self.fraction_a_l_image(image, duree),
            Cles::Images => {
                let (jalons, images) = (self.jalons(), self.images_des_jalons(duree));
                let k = images.iter().rposition(|&i| i <= image)?;
                jalons.get(k).copied()
            }
        }
    }

    /// Le déplacement dans l'espace (mètres), à l'image `image` d'un plan de `duree` images, de ce qui suit la trajectoire : rien
    /// avant le départ, celui du bout du chemin après la fin (`fraction_suivie`). Rien pour une trajectoire sur la feuille, ou si le
    /// plan s'arrête avant que le chemin ne commence.
    pub fn deplacement_a_l_image(&self, image: usize, duree: usize) -> Option<[f32; 3]> {
        self.deplacement_a(self.fraction_suivie(image, duree)?)
    }

    /// Les images où une trajectoire dans l'espace pose une clé de position, avec la fraction du chemin de chacune : une par image du
    /// départ à la fin en clés d'interpolation, une par jalon (tenue jusqu'au suivant) en clés d'images. Vide si le plan s'arrête
    /// avant que le chemin ne commence.
    pub fn images_et_fractions(&self, duree: usize) -> Vec<(usize, f32)> {
        let Some(fin) = self.derniere_image(duree) else { return Vec::new() };
        match self.cles {
            Cles::Interpolation => (self.depart..=fin).filter_map(|f| Some((f, self.fraction_a_l_image(f, duree)?))).collect(),
            Cles::Images => {
                let mut out: Vec<(usize, f32)> = Vec::new();
                for (image, fraction) in self.images_des_jalons(duree).into_iter().zip(self.jalons()) {
                    // (Deux jalons arrondis à la même image : le dernier l'emporte, comme pour les dessins des clés d'images.)
                    match out.last_mut() {
                        Some(d) if d.0 == image => d.1 = fraction,
                        _ => out.push((image, fraction)),
                    }
                }
                out
            }
        }
    }

    /// Une trajectoire relue d'un fichier : ses points filtrés (abîmés, doublons), ses marques remises en ordre et dans leurs
    /// bornes, ses temps dans les leurs. Rien si le chemin ne tient pas debout.
    pub fn assainie(mut self) -> Option<Trajectoire> {
        // (Les hauteurs hors du plan, partie 154, suivent leurs points ; une par point, finies, ou aucune.)
        (self.points, self.hauteurs) = filtrer3(&self.points, &self.hauteurs);
        if self.points.len() < 2 || longueur_de(&self.points) < LONGUEUR_MIN {
            return None;
        }
        let marques = std::mem::take(&mut self.marques);
        let temps = self.temps.take();
        // (Les dessins des jalons suivent leurs marques : remises en ordre, une marque abîmée ou en double laissée, chacune garde
        // le sien. Sans un dessin par jalon, ils restent tels quels, dans l'ordre : le calcul suivant les range.)
        let dessins = std::mem::take(&mut self.dessins);
        let alignes = dessins.len() == marques.len() + 2;
        let mut origines: Vec<usize> = Vec::with_capacity(marques.len());
        for (j, m) in marques.into_iter().enumerate() {
            if m.is_finite() {
                if let Some(i) = self.ajouter_marque(m) {
                    origines.insert(i, j);
                }
            }
        }
        self.dessins = if alignes {
            let mut d = Vec::with_capacity(origines.len() + 2);
            d.push(dessins[0]);
            d.extend(origines.iter().map(|&j| dessins[j + 1]));
            d.push(dessins[dessins.len() - 1]);
            // (Le dessin d'une marque laissée : la trajectoire l'avait posé, le calcul suivant le reprend, `retires`.)
            self.retires = dessins.iter().map(|x| x.dessin).filter(|c| !d.iter().any(|y| y.dessin == *c)).collect();
            d
        } else {
            dessins
        };
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
        self.dessins.retain(|d| d.dx.is_finite() && d.dy.is_finite() && d.angle.is_finite());
        if !self.decalage_depart[0].is_finite() || !self.decalage_depart[1].is_finite() {
            self.decalage_depart = [0.0, 0.0];
        }
        if let Ancre::Point { x, y } = self.ancre {
            if !x.is_finite() || !y.is_finite() {
                self.ancre = Ancre::Depart;
            }
        }
        // Une trajectoire dans l'espace dont le support ne fait pas un plan ne se lit pas : ses points ne sont pas des pixels de la
        // feuille, la garder la ferait prendre pour une autre.
        if let Some(s) = self.support {
            self.support = Some(s.assaini()?);
            (self.ancre, self.tourne, self.decalage_depart) = (Ancre::Depart, false, [0.0, 0.0]);
            // (Les dessins des clés d'images d'une carte sont gardés, avec leur dessin d'origine : sans eux, le calcul suivant
            // referait des copies et laisserait de côté ce qu'on y a redessiné. Dans l'espace ils ne sont ni déplacés ni
            // tournés : c'est la carte qui porte le mouvement. Partie 152.)
            for d in &mut self.dessins {
                (d.dx, d.dy, d.angle) = (0.0, 0.0, 0.0);
            }
        }
        // Le lissage : entre 0 et 1 ; le chemin brut, filtré comme les points, n'est gardé que s'il tient debout (sinon le
        // chemin lissé devient le chemin brut).
        self.lissage = if self.lissage.is_finite() { self.lissage.clamp(0.0, 1.0) } else { 0.0 };
        let hauteurs_brut = std::mem::take(&mut self.hauteurs_brut);
        if let Some(b) = self.brut.take() {
            let (b, hb) = filtrer3(&b, &hauteurs_brut);
            if self.lissage > 0.0 && b.len() >= 2 && longueur_de(&b) >= LONGUEUR_MIN {
                (self.brut, self.hauteurs_brut) = (Some(b), hb);
            }
        }
        if self.brut.is_none() {
            self.lissage = 0.0;
        }
        self.reference = self.reference.filter(|r| r[0].is_finite() && r[1].is_finite());
        // Les poignées : finies, sur le chemin, en ordre, à `ECART_MIN` l'une de l'autre. Les hauteurs : seulement dans
        // l'espace (une trajectoire sur la feuille n'a pas de plan à quitter).
        let mut poignees: Vec<f32> = self.poignees.iter().copied().filter(|f| f.is_finite()).map(|f| f.clamp(0.0, 1.0)).collect();
        poignees.sort_by(|a, b| a.total_cmp(b));
        poignees.dedup_by(|a, b| (*a - *b).abs() < ECART_MIN);
        self.poignees = poignees;
        if self.support.is_none() {
            (self.hauteurs, self.hauteurs_brut) = (Vec::new(), Vec::new());
        }
        Some(self)
    }

    /// Un nouveau chemin ; les marques sont gardées, à leurs fractions, et le lissage aussi. Faux (rien ne change) si le trait
    /// est trop court.
    pub fn retracer(&mut self, brut: &[[f32; 2]]) -> bool {
        match Trajectoire::nouvelle(brut) {
            Some(t) => {
                // (Le trait neuf est posé à plat : les hauteurs s'en vont ; les poignées restent, à leurs fractions.)
                self.points = t.points;
                (self.brut, self.reference, self.hauteurs, self.hauteurs_brut) = (None, None, Vec::new(), Vec::new());
                self.relisser();
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

    /// La direction du chemin (vecteur unité) à la fraction `f`, lue sur une petite fenêtre de part et d'autre (`FENETRE_DIRECTION`,
    /// ou 2 % du chemin) : elle ne saute pas d'un point du trait à l'autre.
    pub fn direction_lissee_a(&self, f: f32) -> [f32; 2] {
        let l = self.longueur().max(1e-6);
        let w = FENETRE_DIRECTION.max(l * 0.02) / l;
        let (a, b) = (self.point_a((f - w).max(0.0)), self.point_a((f + w).min(1.0)));
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let n = (dx * dx + dy * dy).sqrt();
        if n > 1e-6 { [dx / n, dy / n] } else { self.direction_a(f) }
    }

    /// L'angle (degrés, entre −180 et 180) dont le chemin a tourné à la fraction `f` depuis sa direction au départ : ce que le dessin
    /// tourne quand il suit le chemin (`tourne`) ; 0 au départ.
    pub fn angle_a(&self, f: f32) -> f32 {
        // (Un chemin prolongé par son début : l'orientation de départ reste celle de l'ancien départ, `reference`.)
        // (Lue vers l'avant seulement, comme au départ d'un chemin : le coin du raccord n'y entre pas.)
        let d0 = match self.reference {
            None => self.direction_lissee_a(0.0),
            Some(r) => {
                let f0 = self.plus_proche(r).0;
                let l = self.longueur().max(1e-6);
                let (a, b) = (self.point_a(f0), self.point_a((f0 + FENETRE_DIRECTION.max(l * 0.02) / l).min(1.0)));
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let n = (dx * dx + dy * dy).sqrt();
                if n > 1e-6 { [dx / n, dy / n] } else { self.direction_lissee_a(f0) }
            }
        };
        let d = self.direction_lissee_a(f);
        let a = (d[1].atan2(d[0]) - d0[1].atan2(d0[0])).to_degrees();
        let a = a.rem_euclid(360.0);
        if a > 180.0 { a - 360.0 } else { a }
    }

    /// `angle_a(f)`, choisi parmi ses équivalents à un tour près au plus proche de l'angle `precedent` : d'une image à la suivante, le
    /// dessin ne fait jamais le tour dans l'autre sens.
    pub fn angle_depuis(&self, f: f32, precedent: f32) -> f32 {
        let mut a = self.angle_a(f);
        while a - precedent > 180.0 {
            a -= 360.0;
        }
        while a - precedent < -180.0 {
            a += 360.0;
        }
        a
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
        plus_proche_sur(&self.points, p)
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

    /// Les images des jalons quand la fin est posée à l'image `fin` (sans regarder la durée du plan).
    fn images_des_jalons_jusqu_a(&self, fin: usize) -> Vec<usize> {
        let portee = fin.saturating_sub(self.depart) as f32;
        self.temps_des_jalons().iter().map(|t| self.depart + (t * portee).round() as usize).collect()
    }

    /// `n` images insérées à l'image `at` du plan (`removed` : retirées de `at` à `at + n`) : le départ, la fin et chaque marque
    /// restent sur leur image, comme les clés de position et les dessins du calque (`Layer::shift_frames`). Une insertion au milieu du
    /// chemin allonge l'intervalle où elle tombe : le temps des marques devient explicite pour cela. Sans fin posée (anciens
    /// projets : la fin est la dernière image du plan), seul le départ suit, s'il est à l'image ou après.
    pub fn decaler_images(&mut self, at: usize, n: usize, removed: bool) {
        if n == 0 {
            return;
        }
        let bouge = |f: usize| -> usize {
            if removed {
                if f >= at + n {
                    f - n
                } else if f >= at {
                    at
                } else {
                    f
                }
            } else if f >= at {
                f + n
            } else {
                f
            }
        };
        let Some(fin) = self.fin.filter(|&f| f > self.depart) else {
            self.depart = bouge(self.depart);
            return;
        };
        let dedans = if removed { at <= fin && at + n > self.depart } else { at > self.depart && at <= fin };
        let images: Vec<usize> = self.images_des_jalons_jusqu_a(fin).into_iter().map(bouge).collect();
        let (depart, fin) = (bouge(self.depart), bouge(fin).max(bouge(self.depart) + 1));
        self.depart = depart;
        self.fin = Some(fin);
        if !dedans || images.len() < 3 {
            return;
        }
        // Chaque marque garde son image : son temps en est déduit, sur la nouvelle portée.
        let portee = (fin - depart) as f32;
        let temps: Vec<f32> = images[1..images.len() - 1].iter().map(|&i| i.saturating_sub(depart) as f32 / portee).collect();
        let bons = temps.windows(2).all(|w| w[0] < w[1]) && temps.first().is_some_and(|v| *v > 0.0) && temps.last().is_some_and(|v| *v < 1.0);
        if bons {
            self.temps = Some(temps);
        }
    }

    /// Tout le chemin décalé de `delta` images dans le temps (négatif : plus tôt) : son départ et sa fin posée, les marques avec
    /// eux. Le plan a gagné ou perdu des images à son début (`rognage.rs`). Un départ qui passerait avant la première image s'y
    /// arrête.
    pub fn decaler_de(&mut self, delta: isize) {
        let bouge = |f: usize| (f as isize + delta).max(0) as usize;
        self.depart = bouge(self.depart);
        self.fin = self.fin.map(bouge).filter(|&f| f > self.depart);
    }

    /// Le déplacement du calque à chaque jalon, pour que le point d'ancrage `ancre` (pixels de la feuille) soit sur le chemin.
    pub fn decalages(&self, ancre: [f32; 2]) -> Vec<[f32; 2]> {
        self.jalons().iter().map(|&f| { let p = self.point_a(f); [p[0] - ancre[0], p[1] - ancre[1]] }).collect()
    }

    /// À chaque jalon : le déplacement qui met le point d'ancrage `ancre` sur le chemin, et l'angle (degrés) dont le dessin tourne
    /// autour de lui — 0 partout s'il ne tourne pas avec le chemin.
    pub fn poses(&self, ancre: [f32; 2]) -> Vec<([f32; 2], f32)> {
        let mut precedent = 0.0;
        self.jalons()
            .iter()
            .map(|&f| {
                let p = self.point_a(f);
                let angle = if self.tourne { self.angle_depuis(f, precedent) } else { 0.0 };
                precedent = angle;
                ([p[0] - ancre[0], p[1] - ancre[1]], angle)
            })
            .collect()
    }

    /// La pose qui met le point d'ancrage `ancre` (feuille, sous la pose `base`) sur le point `p` du chemin, le dessin tourné de `theta`
    /// degrés autour de ce point. Un calque tourne autour du centre de la feuille (`pivot`, `LayerTransform::affine`) : le déplacement
    /// compense pour que ce soit le point d'ancrage qui reste sur le chemin.
    fn pose_tournee(base: LayerTransform, ancre: [f32; 2], pivot: [f32; 2], p: [f32; 2], theta: f32) -> LayerTransform {
        let (s, c) = theta.to_radians().sin_cos();
        let v = [ancre[0] - pivot[0] - base.x, ancre[1] - pivot[1] - base.y];
        let rv = [v[0] * c - v[1] * s, v[0] * s + v[1] * c];
        LayerTransform { x: p[0] - pivot[0] - rv[0], y: p[1] - pivot[1] - rv[1], angle: base.angle + theta, ..base }
    }

    /// Le chemin retourné de gauche à droite (`horizontal`) ou de haut en bas, autour de son départ : le calque, retourné de même,
    /// suit le chemin retourné.
    pub fn retourner(&mut self, horizontal: bool) {
        let d = self.point_de_reference();
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
    /// feuille sous la pose `base`, sur le chemin ; le dessin tourné d'autant que le chemin s'il tourne avec lui, `pivot` étant le centre
    /// de la feuille autour duquel un calque tourne). Aucune en **clés d'images** (ce sont des dessins, posés par l'application :
    /// `trajectoireui.rs`), ni si le plan n'a pas au moins une image après le départ.
    pub fn cles(&self, duree: usize, base: LayerTransform, ancre: [f32; 2], pivot: [f32; 2]) -> Vec<PosKey> {
        let Some(fin) = self.derniere_image(duree) else { return Vec::new() };
        if self.cles == Cles::Images {
            return Vec::new();
        }
        let mut precedent = 0.0;
        (self.depart..=fin)
            .filter_map(|f| {
                let fraction = self.fraction_a_l_image(f, duree)?;
                let p = self.point_a(fraction);
                if !self.tourne {
                    return Some(PosKey::new(f, LayerTransform { x: base.x + p[0] - ancre[0], y: base.y + p[1] - ancre[1], ..base }));
                }
                let theta = self.angle_depuis(fraction, precedent);
                precedent = theta;
                Some(PosKey::new(f, Self::pose_tournee(base, ancre, pivot, p, theta)))
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
    /// temps posés à la main : la marque prend le milieu du temps de son intervalle. En clés d'images, son jalon prend place parmi
    /// les dessins avec, en attendant sa copie à lui, le dessin du jalon d'avant (`DessinJalon`) : les autres jalons gardent les
    /// leurs.
    pub fn ajouter_marque(&mut self, f: f32) -> Option<usize> {
        if !(ECART_MIN..=1.0 - ECART_MIN).contains(&f) || self.marques.iter().any(|m| (m - f).abs() < ECART_MIN) {
            return None;
        }
        let i = self.marques.iter().position(|m| *m > f).unwrap_or(self.marques.len());
        if self.dessins.len() == self.marques.len() + 2 {
            let avant = self.dessins[i];
            self.dessins.insert(i + 1, avant);
        }
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

    /// Retire la marque `i` ; en clés d'images, son dessin s'en va avec elle (noté dans `retires` : le calcul suivant le reprend
    /// aux images qui le montrent), les autres jalons gardent les leurs.
    pub fn retirer_marque(&mut self, i: usize) {
        if i < self.marques.len() {
            if self.dessins.len() == self.marques.len() + 2 {
                let retire = self.dessins.remove(i + 1);
                self.retires.push(retire.dessin);
            }
            self.marques.remove(i);
            match self.temps.as_mut() {
                Some(t) if i < t.len() => {
                    t.remove(i);
                }
                _ => self.temps = None,
            }
        }
    }

    /// Toutes les marques retirées (« Retirer les marques ») : le chemin reste, à vitesse constante, en parts égales. En clés
    /// d'images, le départ et l'arrivée gardent leurs dessins ; ceux des marques s'en vont avec elles (notés dans `retires`).
    pub fn retirer_les_marques(&mut self) {
        if self.dessins.len() == self.marques.len() + 2 && self.dessins.len() > 2 {
            let arrivee = self.dessins[self.dessins.len() - 1];
            let dernier = self.dessins.len() - 1;
            self.retires.extend(self.dessins[1..dernier].iter().map(|d| d.dessin));
            self.dessins.truncate(1);
            self.dessins.push(arrivee);
        }
        self.marques.clear();
        self.temps = None;
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
    ///
    /// Un chemin lissé : c'est le chemin brut qui est tiré (au même endroit, sur la même part de sa longueur), puis le chemin
    /// est lissé de nouveau (partie 151).
    ///
    /// Les poignées (partie 154) bornent le tirage : il ne passe pas la poignée la plus proche de chaque côté, qui reste
    /// fixe ; sans poignée, rien ne change (un bout se tire comme le reste).
    pub fn tirer(&mut self, fraction: f32, dx: f32, dy: f32, rayon: f32) {
        let f = fraction.clamp(0.0, 1.0);
        let bas = self.poignees.iter().copied().filter(|p| *p < f).last();
        let haut = self.poignees.iter().copied().find(|p| *p > f);
        self.tirer_entre(f, [dx, dy, 0.0], rayon, bas, haut);
    }

    /// Le chemin tiré en `fraction` de `d` (dans le plan, et en hauteur `d[2]`), en cloche sur `rayon` de chemin de chaque
    /// côté, sans passer les fractions `bas` et `haut` (fixes ; rien : pas de borne de ce côté). Les poignées sont suivies :
    /// leurs fractions sont lues sur le chemin tiré ; les marques gardent les leurs.
    fn tirer_entre(&mut self, fraction: f32, d: [f32; 3], rayon: f32, bas: Option<f32>, haut: Option<f32>) {
        let part = rayon.max(1e-3) / self.longueur().max(1e-6);
        let base = self.chemin_brut3();
        let total = longueur3(&base);
        if base.len() < 2 || total <= 0.0 {
            return;
        }
        // Un chemin lissé avec des poignées : leurs fractions, et celles du tirage, sont lues sur le chemin lissé (celui qu'on
        // voit) ; elles sont ramenées au chemin brut, qui est tiré, par le point du chemin.
        let lisse = self.brut.is_some() && !self.poignees.is_empty();
        let brut2: Vec<[f32; 2]> = base.iter().map(|q| [q[0], q[1]]).collect();
        let vers_brut = |f: f32| if lisse { plus_proche_sur(&brut2, self.point_a(f)).0 } else { f };
        let (fraction, bas, haut) = (vers_brut(fraction.clamp(0.0, 1.0)), bas.map(vers_brut), haut.map(vers_brut));
        // (Un sommet à chaque poignée : on sait où elle est après le tirage.)
        let mut cibles: Vec<f32> = self.poignees.iter().map(|&f| vers_brut(f) * total).collect();
        cibles.sort_by(|a, b| a.total_cmp(b));
        let (mut pts, suivis) = inserer_sommets(&base, &cibles);
        let s0 = fraction * total;
        let r = (part * total).max(1e-3);
        let poids = poids_de_tirage(s0, r, bas.map(|b| b * total), haut.map(|h| h * total));
        let mut avant = vec![0.0f32; pts.len()];
        for j in 1..pts.len() {
            avant[j] = avant[j - 1] + d2(pts[j - 1], pts[j]);
        }
        for j in 0..pts.len() {
            let w = poids(avant[j]);
            if w > 0.0 {
                pts[j] = [pts[j][0] + d[0] * w, pts[j][1] + d[1] * w, pts[j][2] + d[2] * w];
            }
        }
        let repris = echantillonner3(&pts);
        if repris.len() < 2 || longueur3(&repris) < LONGUEUR_MIN {
            return;
        }
        // (Le point de référence d'un chemin prolongé par son début suit le chemin là où il est, du même poids.)
        if let Some(rf) = self.reference {
            let w = poids(self.plus_proche(rf).0 * total);
            if w > 0.0 {
                self.reference = Some([rf[0] + d[0] * w, rf[1] + d[1] * w]);
            }
        }
        // Les poignées, là où le tirage les a mises.
        let mut cumul = vec![0.0f32; pts.len()];
        for j in 1..pts.len() {
            cumul[j] = cumul[j - 1] + d2(pts[j - 1], pts[j]);
        }
        let neuf = cumul.last().copied().unwrap_or(0.0).max(1e-6);
        self.poignees = suivis.iter().map(|&j| (cumul[j] / neuf).clamp(0.0, 1.0)).collect();
        let (points, hauteurs) = separer(&repris);
        if self.brut.is_some() {
            (self.brut, self.hauteurs_brut) = (Some(points), hauteurs);
            self.relisser();
            // (Les poignées, de nouveau lues sur le chemin lissé.)
            if lisse {
                self.poignees = suivis.iter().map(|&j| self.plus_proche([pts[j][0], pts[j][1]]).0).collect();
            }
        } else {
            (self.points, self.hauteurs) = (points, hauteurs);
        }
        self.poignees.sort_by(|a, b| a.total_cmp(b));
    }

    /// Chaque point du chemin passé par `f` (étirer, tourner…) ; les marques, des fractions, suivent d'elles-mêmes. Le chemin
    /// brut d'un chemin lissé aussi.
    pub fn transformer(&mut self, f: impl Fn([f32; 2]) -> [f32; 2]) {
        for p in &mut self.points {
            *p = f(*p);
        }
        for p in self.brut.iter_mut().flatten() {
            *p = f(*p);
        }
        if let Some(r) = self.reference.as_mut() {
            *r = f(*r);
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

/// La fraction de la ligne `points` la plus proche du point `p`, et la distance qui les sépare.
fn plus_proche_sur(points: &[[f32; 2]], p: [f32; 2]) -> (f32, f32) {
    let total = longueur_de(points).max(1e-6);
    let mut meilleur = (0.0, f32::INFINITY);
    let mut parcouru = 0.0;
    for i in 0..points.len().saturating_sub(1) {
        let (a, b) = (points[i], points[i + 1]);
        let (w, d) = projection(p, a, b);
        let l = distance(a, b);
        if d < meilleur.1 {
            meilleur = (((parcouru + w * l) / total).clamp(0.0, 1.0), d);
        }
        parcouru += l;
    }
    meilleur
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

// ------------------------------------------------------------------------------------------------ points et hauteurs

/// Les points `points` et leurs hauteurs `h` (une par point ; manquante : 0), ensemble.
fn joindre(points: &[[f32; 2]], h: &[f32]) -> Vec<[f32; 3]> {
    points.iter().enumerate().map(|(i, p)| [p[0], p[1], h.get(i).copied().unwrap_or(0.0)]).collect()
}

/// Les points et leurs hauteurs, séparés ; aucune hauteur quand toutes sont nulles (le chemin est dans son plan).
fn separer(v: &[[f32; 3]]) -> (Vec<[f32; 2]>, Vec<f32>) {
    let p = v.iter().map(|q| [q[0], q[1]]).collect();
    let h: Vec<f32> = v.iter().map(|q| q[2]).collect();
    (p, if h.iter().all(|x| x.abs() < 1e-6) { Vec::new() } else { h })
}

/// Des points relus : les abîmés et les doublons laissés, leurs hauteurs avec eux (gardées si une par point et finies).
fn filtrer3(points: &[[f32; 2]], h: &[f32]) -> (Vec<[f32; 2]>, Vec<f32>) {
    let h: &[f32] = if h.len() == points.len() && h.iter().all(|x| x.is_finite()) { h } else { &[] };
    let mut v = joindre(points, h);
    v.retain(|p| p[0].is_finite() && p[1].is_finite());
    v.dedup_by(|a, b| d2(*a, *b) < 1e-3);
    separer(&v)
}

/// La distance de deux points, lue dans le plan : les fractions du chemin se mesurent à plat (partie 154).
fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
    distance([a[0], a[1]], [b[0], b[1]])
}

/// La longueur, dans le plan, d'une ligne brisée avec hauteurs.
fn longueur3(points: &[[f32; 3]]) -> f32 {
    points.windows(2).map(|s| d2(s[0], s[1])).sum()
}

fn melange3(a: [f32; 3], b: [f32; 3], w: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * w, a[1] + (b[1] - a[1]) * w, a[2] + (b[2] - a[2]) * w]
}

/// La ligne `base` avec un sommet de plus à chacune des longueurs `cibles` (triées, le long du chemin) ; et, pour chaque
/// cible, le rang de son sommet.
fn inserer_sommets(base: &[[f32; 3]], cibles: &[f32]) -> (Vec<[f32; 3]>, Vec<usize>) {
    let mut out: Vec<[f32; 3]> = Vec::with_capacity(base.len() + cibles.len());
    let mut rangs = vec![0usize; cibles.len()];
    let mut k = 0;
    out.push(base[0]);
    while k < cibles.len() && cibles[k] <= 1e-4 {
        k += 1;
    }
    let mut parcouru = 0.0;
    for j in 1..base.len() {
        let l = d2(base[j - 1], base[j]);
        while k < cibles.len() && cibles[k] < parcouru + l - 1e-4 {
            let p = melange3(base[j - 1], base[j], ((cibles[k] - parcouru) / l.max(1e-9)).clamp(0.0, 1.0));
            if out.last().is_none_or(|q| d2(*q, p) > 1e-4) {
                out.push(p);
            }
            rangs[k] = out.len() - 1;
            k += 1;
        }
        out.push(base[j]);
        parcouru += l;
        while k < cibles.len() && cibles[k] <= parcouru + 1e-4 {
            rangs[k] = out.len() - 1;
            k += 1;
        }
    }
    while k < cibles.len() {
        rangs[k] = out.len() - 1;
        k += 1;
    }
    (out, rangs)
}

/// Le poids du tirage (de 0 à 1) à la longueur `s` du chemin, tiré en `s0` sur `rayon` de chaque côté, en cloche (cosinus) :
/// 1 au point pris, 0 à `rayon` de lui ; et jamais au-delà des bornes `bas` et `haut`, où il retombe à 0 (le rayon se
/// resserre de ce côté jusqu'à la borne).
fn poids_de_tirage(s0: f32, rayon: f32, bas: Option<f32>, haut: Option<f32>) -> impl Fn(f32) -> f32 {
    let rb = bas.map_or(rayon, |b| (s0 - b).min(rayon));
    let rh = haut.map_or(rayon, |h| (h - s0).min(rayon));
    move |s: f32| {
        let e = s - s0;
        if e.abs() < 1e-5 {
            return 1.0;
        }
        let r = if e < 0.0 { rb } else { rh };
        if r <= 0.0 || e.abs() >= r {
            0.0
        } else {
            0.5 * (1.0 + (std::f32::consts::PI * e.abs() / r).cos())
        }
    }
}

/// **Le lissage d'un chemin** (partie 151) : `base` (des points à peu près au pas) passé `LISSAGE_PASSES` fois par une
/// moyenne glissante dont la demi-fenêtre est `lissage` × `LISSAGE_FENETRE` de sa longueur, puis repris au pas. Près des
/// bouts, la fenêtre se resserre autant des deux côtés : **les deux bouts restent fixes**, et le chemin n'y est pas tiré
/// vers l'intérieur. Un calcul léger (des sommes cumulées, quelques milliers de points au plus) : il se refait à chaque
/// image d'écran quand on tire le curseur.
pub fn lissage_de(base: &[[f32; 2]], lissage: f32) -> Vec<[f32; 2]> {
    separer(&lissage_de3(&joindre(base, &[]), lissage)).0
}

/// `lissage_de`, les hauteurs hors du plan lissées avec le reste (partie 154).
fn lissage_de3(base: &[[f32; 3]], lissage: f32) -> Vec<[f32; 3]> {
    let n = base.len();
    if n < 3 || !(lissage > 0.0) {
        return base.to_vec();
    }
    let total = longueur3(base);
    let pas = (total / (n - 1) as f32).max(1e-6);
    let demi = ((lissage.min(1.0) * LISSAGE_FENETRE * total / pas).round() as usize).clamp(1, n / 2);
    let mut p = base.to_vec();
    let mut cumul = vec![[0.0f64; 3]; n + 1];
    for _ in 0..LISSAGE_PASSES {
        for i in 0..n {
            cumul[i + 1] = [0, 1, 2].map(|c| cumul[i][c] + p[i][c] as f64);
        }
        for i in 1..n - 1 {
            let h = demi.min(i).min(n - 1 - i);
            let (a, b) = (i - h, i + h + 1);
            let k = (b - a) as f64;
            p[i] = [0, 1, 2].map(|c| ((cumul[b][c] - cumul[a][c]) / k) as f32);
        }
    }
    echantillonner3(&p)
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
    separer(&echantillonner3(&joindre(p, &[]))).0
}

/// `echantillonner`, les hauteurs hors du plan reprises avec leurs points (le pas se mesure dans le plan, partie 154).
fn echantillonner3(p: &[[f32; 3]]) -> Vec<[f32; 3]> {
    if p.len() < 2 {
        return p.to_vec();
    }
    let total = longueur3(p);
    let pas = PAS.max(total / POINTS_MAX as f32);
    let mut out = vec![p[0]];
    let mut prochain = pas;
    let mut parcouru = 0.0;
    for s in p.windows(2) {
        let l = d2(s[0], s[1]);
        while l > 0.0 && prochain <= parcouru + l {
            out.push(melange3(s[0], s[1], (prochain - parcouru) / l));
            prochain += pas;
        }
        parcouru += l;
    }
    // Le dernier point, tel quel (un tout dernier pas plus court, jamais plus long).
    let dernier = p[p.len() - 1];
    if out.last().is_none_or(|l| d2(*l, dernier) > 1e-3) {
        out.push(dernier);
    }
    out
}
