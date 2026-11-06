//! L'anneau sous le stylet (2026-11-06, demande d'Anthony après essai d'une maquette au stylet) : la partie **sans
//! interface**. Huit commandes rangées en rose des vents autour de la pointe, et un second anneau, autour du premier, pour
//! les pastilles.
//!
//! Le geste (le bouton du stylet, c'est-à-dire le clic droit, sur le dessin) :
//! - **clic bref sans bouger** : ce que le clic droit faisait déjà (la fenêtre des réglages de l'outil, partie 66 de
//!   `docs/INTERFACE.md`) ;
//! - **appuyer, glisser, relâcher** : la commande de la direction part tout de suite, sans que rien ne s'affiche ;
//! - **maintenir** : l'anneau se dessine après un délai (réglable), le temps d'apprendre les directions.
//!
//! Seule la **direction** compte pour le premier anneau, jamais la distance : un trait long vers le haut reste « haut ».
//! Les directions sont **fixes** (c'est la main qui les retient) : rien n'est jamais rangé par ordre d'usage.
//!
//! **Le second anneau** se déploie quand le pointeur entre dans la part des pastilles : un arc centré sur cette part, une
//! pastille tous les [`PAS_ARC`] degrés. Au-delà de [`R_ARC`], c'est l'angle qui choisit la pastille ; revenir dans une
//! autre part le replie. Relâcher sur la part elle-même le laisse ouvert : un appui choisit alors une pastille, un appui
//! ailleurs referme sans rien faire d'autre.
//!
//! Ce fichier ne connaît ni egui (hors ses points), ni l'application : `anneauui.rs` lit les entrées, dessine et lance les
//! commandes. Essais : `anneau_tests.rs`.

use eframe::egui::{Pos2, Vec2};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Nombre de parts du premier anneau.
pub const PARTS: usize = 8;
/// Rayon mort autour de l'appui, en points : dedans, on vise le centre (les réglages de l'outil).
pub const SEUIL: f32 = 20.0;
/// Rayon du rond central, en points.
pub const R_CENTRE: f32 = 28.0;
/// Bords intérieur et extérieur des parts du premier anneau.
pub const R_PART_DEDANS: f32 = 33.0;
pub const R_PART_DEHORS: f32 = 94.0;
/// Rayon où se posent les icônes des parts.
pub const R_ICONE: f32 = 64.0;
/// Bord du disque du premier anneau.
pub const R_DISQUE: f32 = 97.0;
/// Rayon où s'accrochent les noms des parts.
pub const R_NOM: f32 = 104.0;
/// Au-delà de ce rayon, le pointeur est dans le second anneau quand il est déployé.
pub const R_ARC: f32 = 100.0;
/// Bords intérieur et extérieur de la bande du second anneau, et rayon des centres des pastilles.
pub const R_ARC_DEDANS: f32 = 102.0;
pub const R_ARC_DEHORS: f32 = 147.0;
pub const R_PASTILLE: f32 = 124.5;
/// Rayon d'une pastille dessinée.
pub const RAYON_PASTILLE: f32 = 13.0;
/// Rayon où s'accrochent les noms des pastilles.
pub const R_NOM_PASTILLE: f32 = 156.0;
/// Second anneau resté ouvert : un appui plus loin que ce rayon le referme.
pub const R_ARC_APPUI: f32 = 176.0;
/// Écart entre deux pastilles, en degrés (moins s'il y en a trop pour tenir sur [`TOUR_ARC`]).
pub const PAS_ARC: f32 = 24.0;
/// Ouverture la plus grande du second anneau, en degrés (presque le tour : les deux bouts ne se touchent pas).
pub const TOUR_ARC: f32 = 336.0;

/// Les noms des huit directions, dans l'ordre des parts.
pub const NOMS_DIRECTIONS: [&str; PARTS] =
    ["Droite", "Bas droite", "Bas", "Bas gauche", "Gauche", "Haut gauche", "Haut", "Haut droite"];
/// L'ordre de lecture d'une rose des vents posée en grille de trois sur trois, sans le centre : haut gauche, haut, haut
/// droite, gauche, droite, bas gauche, bas, bas droite.
pub const GRILLE: [usize; PARTS] = [5, 6, 7, 4, 0, 3, 2, 1];

/// La part visée par un déplacement : 0 à droite, puis dans le sens des aiguilles d'une montre (1 bas droite, 2 bas,
/// 3 bas gauche, 4 gauche, 5 haut gauche, 6 haut, 7 haut droite). L'écran a son axe des y vers le bas.
pub fn direction(d: Vec2) -> usize {
    let huitiemes = (d.y.atan2(d.x) / std::f32::consts::FRAC_PI_4).round() as i32;
    huitiemes.rem_euclid(PARTS as i32) as usize
}

/// L'angle du milieu d'une part, en degrés (0 à droite, sens des aiguilles).
pub fn angle_part(part: usize) -> f32 {
    part as f32 * 45.0
}

/// L'écart signé entre deux angles en degrés, ramené entre -180 et 180.
pub fn ecart(a: f32, b: f32) -> f32 {
    (a - b + 180.0).rem_euclid(360.0) - 180.0
}

/// Le point à un rayon et un angle (en degrés) d'un centre.
pub fn point(centre: Pos2, rayon: f32, degres: f32) -> Pos2 {
    let a = degres.to_radians();
    centre + Vec2::new(rayon * a.cos(), rayon * a.sin())
}

/// Le second anneau : un arc de pastilles autour du premier, centré sur la part qui le déploie.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    /// La part du premier anneau qui le déploie.
    pub part: usize,
    /// Le nombre de pastilles.
    pub nombre: usize,
    /// L'écart entre deux pastilles, en degrés.
    pub pas: f32,
}

impl Arc {
    /// L'arc de `nombre` pastilles autour de la part ; rien s'il n'y a pas de pastille.
    pub fn nouveau(part: usize, nombre: usize) -> Option<Arc> {
        (nombre > 0).then(|| Arc { part, nombre, pas: PAS_ARC.min(TOUR_ARC / nombre as f32) })
    }

    /// L'angle du milieu de l'arc, en degrés.
    pub fn centre(&self) -> f32 {
        angle_part(self.part)
    }

    /// L'angle de la pastille `i`, en degrés : elles se suivent dans le sens des aiguilles.
    pub fn angle(&self, i: usize) -> f32 {
        self.centre() + (i as f32 - (self.nombre as f32 - 1.0) / 2.0) * self.pas
    }

    /// La moitié de l'ouverture occupée par les pastilles, en degrés.
    pub fn demi_ouverture(&self) -> f32 {
        self.pas * self.nombre as f32 / 2.0
    }

    /// La pastille visée par un déplacement depuis le centre : il faut être au-delà de [`R_ARC`], en deçà de `max`, et
    /// dans l'ouverture de l'arc.
    pub fn vise(&self, d: Vec2, max: f32) -> Option<usize> {
        let r = d.length();
        if r < R_ARC || r > max {
            return None;
        }
        let rang = ecart(d.y.atan2(d.x).to_degrees(), self.centre()) / self.pas + (self.nombre as f32 - 1.0) / 2.0;
        let i = rang.round();
        (i >= 0.0 && (i as usize) < self.nombre).then_some(i as usize)
    }

    /// Le nom de cette part du premier anneau tomberait-il sous l'arc ? (Il s'efface alors le temps du choix.)
    pub fn couvre(&self, part: usize) -> bool {
        ecart(angle_part(part), self.centre()).abs() <= self.demi_ouverture() + 20.0
    }
}

/// Ce que donne le relâché d'un geste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    /// Relâché au centre : ce que le clic droit faisait avant l'anneau (les réglages de l'outil).
    Centre,
    /// La commande de cette part.
    Part(usize),
    /// Cette pastille du second anneau.
    Pastille(usize),
    /// Relâché sur la part des pastilles : le second anneau reste ouvert, un appui choisira.
    ResteOuvert,
}

/// Un geste d'anneau en cours, de l'appui au relâché (ou au choix d'une pastille quand le second anneau est resté ouvert).
#[derive(Clone, Debug, PartialEq)]
pub struct Geste {
    /// Où l'on a appuyé : le centre de l'anneau, en points d'écran.
    pub centre: Pos2,
    /// Où est le pointeur.
    pub pointeur: Pos2,
    /// L'instant de l'appui, en secondes (horloge d'egui).
    pub debut: f64,
    /// L'anneau est-il dessiné ? (Pas avant le délai : un geste vif ne montre rien.)
    pub affiche: bool,
    /// La part survolée ; aucune au centre.
    pub survol: Option<usize>,
    /// Le second anneau, quand il est déployé.
    pub arc: Option<Arc>,
    /// La pastille survolée.
    pub pastille: Option<usize>,
    /// Le second anneau est resté ouvert après le relâché : plus rien n'est tenu, un appui choisit.
    pub colle: bool,
}

impl Geste {
    /// Le geste qui commence par un appui à cet endroit, à cet instant.
    pub fn nouveau(centre: Pos2, instant: f64) -> Geste {
        Geste { centre, pointeur: centre, debut: instant, affiche: false, survol: None, arc: None, pastille: None, colle: false }
    }

    /// Le temps passe : l'anneau se dessine une fois le délai écoulé. Rend vrai à l'instant où il apparaît.
    pub fn temps(&mut self, instant: f64, delai: f64) -> bool {
        if self.affiche || instant - self.debut < delai {
            return false;
        }
        self.affiche = true;
        true
    }

    /// Combien de secondes avant que l'anneau se dessine (0 s'il l'est déjà).
    pub fn attente(&self, instant: f64, delai: f64) -> f64 {
        if self.affiche {
            0.0
        } else {
            (self.debut + delai - instant).max(0.0)
        }
    }

    /// Le pointeur bouge. `pastilles` : la part qui déploie le second anneau et le nombre de pastilles, s'il y en a une.
    pub fn bouger(&mut self, pointeur: Pos2, pastilles: Option<(usize, usize)>) {
        self.pointeur = pointeur;
        let d = pointeur - self.centre;
        if self.colle {
            self.pastille = self.arc.and_then(|arc| arc.vise(d, R_ARC_APPUI));
            return;
        }
        let part = (d.length() > SEUIL).then(|| direction(d));
        if let Some(arc) = self.arc {
            if let Some(i) = arc.vise(d, f32::INFINITY) {
                self.pastille = Some(i);
                self.survol = Some(arc.part);
                return;
            }
            self.pastille = None;
            if part != Some(arc.part) {
                // On a quitté la part des pastilles : le second anneau se replie.
                self.arc = None;
            }
        }
        self.survol = part;
        if self.arc.is_none() {
            if let (Some(part), Some((part_pastilles, nombre))) = (part, pastilles) {
                if part == part_pastilles {
                    self.arc = Arc::nouveau(part, nombre);
                    // Des couleurs se choisissent en les voyant : le second anneau n'attend pas le délai.
                    self.affiche |= self.arc.is_some();
                }
            }
        }
    }

    /// On relâche. Après [`Issue::ResteOuvert`] le geste continue ([`Geste::colle`]) ; après les autres il est fini.
    pub fn lacher(&mut self) -> Issue {
        if self.arc.is_some() {
            if let Some(i) = self.pastille {
                return Issue::Pastille(i);
            }
            self.colle = true;
            return Issue::ResteOuvert;
        }
        match self.survol {
            Some(part) => Issue::Part(part),
            None => Issue::Centre,
        }
    }

    /// Second anneau resté ouvert : la pastille sous cet appui, s'il y en a une.
    pub fn appui(&self, pointeur: Pos2) -> Option<usize> {
        self.arc.and_then(|arc| arc.vise(pointeur - self.centre, R_ARC_APPUI))
    }
}

// ------------------------------------------------------------------------------------------- les réglages du poste

/// La part des pastilles. Ce n'est pas une commande du registre (`keymap::ACTIONS`) : elle n'existe que dans l'anneau, où
/// elle déploie le second anneau.
pub const PASTILLES: &str = "anneau.pastilles";
/// Le délai d'origine avant que l'anneau se dessine, en millisecondes, et le plus long qu'on puisse régler.
pub const DELAI_D_ORIGINE: u32 = 180;
pub const DELAI_MAX: u32 = 1000;

/// Les commandes proposées dans les listes des Préférences. Toute autre commande s'envoie dans l'anneau par le clic droit
/// sur son icône.
pub const PROPOSEES: &[&str] = &[
    PASTILLES,
    "outil.crayon",
    "outil.pinceau",
    "outil.gomme",
    "outil.remplissage",
    "outil.selection",
    "outil.cutter",
    "outil.pipette",
    "outil.main",
    "edition.annuler",
    "edition.retablir",
    "panneau.precedent",
    "panneau.suivant",
    "panneau.nouveau",
    "panneau.dupliquer",
    "image.precedente",
    "image.suivante",
    "cle.precedente",
    "cle.suivante",
    "lecture.basculer",
    "vue.miroir",
    "vue.ajuster",
    "vue.table_lumineuse",
    "vue.concentration",
    "mode.basculer",
];

/// Les huit commandes d'origine, dans l'ordre des parts : le temps à l'horizontale (plan précédent à gauche, plan suivant à
/// droite, nouveau plan en bas à droite), les outils en haut, la gomme et l'annulation en bas, les pastilles en haut à
/// gauche.
pub fn commandes_d_origine() -> [String; PARTS] {
    ["panneau.suivant", "panneau.nouveau", "outil.gomme", "edition.annuler", "panneau.precedent", PASTILLES, "outil.crayon", "outil.pinceau"].map(String::from)
}

fn oui() -> bool {
    true
}

fn delai_d_origine() -> u32 {
    DELAI_D_ORIGINE
}

/// Lit les huit commandes sans jamais faire échouer la lecture de tous les réglages : une liste trop courte, une valeur qui
/// n'est pas un texte, ou autre chose qu'une liste laissent la commande d'origine à sa place.
fn commandes_tolerantes<'de, D: Deserializer<'de>>(d: D) -> Result<[String; PARTS], D::Error> {
    let mut commandes = commandes_d_origine();
    if let toml::Value::Array(lues) = toml::Value::deserialize(d)? {
        for (commande, lue) in commandes.iter_mut().zip(lues) {
            if let toml::Value::String(texte) = lue {
                *commande = texte;
            }
        }
    }
    Ok(commandes)
}

fn commandes_en_liste<S: Serializer>(commandes: &[String; PARTS], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(commandes.iter())
}

/// Ce que le poste retient de l'anneau (`Settings::anneau`, `reglages.toml`) : jamais le projet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reglages {
    /// L'anneau est en service. Éteint : le clic droit du dessin est celui d'avant, rien de plus.
    #[serde(default = "oui", rename = "actif")]
    pub actif: bool,
    /// Le délai avant que l'anneau se dessine, en millisecondes.
    #[serde(default = "delai_d_origine", rename = "delai_ms")]
    pub delai_ms: u32,
    /// La commande de chaque part (`keymap::ACTIONS`, ou [`PASTILLES`]), dans l'ordre des parts.
    #[serde(default = "commandes_d_origine", rename = "commandes", deserialize_with = "commandes_tolerantes", serialize_with = "commandes_en_liste")]
    pub commandes: [String; PARTS],
}

impl Default for Reglages {
    fn default() -> Reglages {
        Reglages { actif: true, delai_ms: DELAI_D_ORIGINE, commandes: commandes_d_origine() }
    }
}

impl Reglages {
    /// Rien à écrire : l'anneau d'origine.
    pub fn est_d_origine(&self) -> bool {
        *self == Reglages::default()
    }

    /// Le délai en secondes.
    pub fn delai(&self) -> f64 {
        f64::from(self.delai_ms) / 1000.0
    }

    /// Ramène les réglages lus à quelque chose de sûr : un délai dans ses bornes, huit commandes qui existent (`connue`
    /// dit si une action du registre existe) et toutes différentes. Une commande inconnue ou en double cède sa part à une
    /// commande d'origine encore libre (celle de cette part d'abord).
    pub fn nettoyes(mut self, connue: impl Fn(&str) -> bool) -> Reglages {
        self.delai_ms = self.delai_ms.min(DELAI_MAX);
        let mut vues: Vec<String> = Vec::new();
        for commande in self.commandes.iter_mut() {
            if (commande == PASTILLES || connue(commande)) && !vues.contains(commande) {
                vues.push(commande.clone());
            } else {
                commande.clear();
            }
        }
        let origine = commandes_d_origine();
        for part in 0..PARTS {
            if self.commandes[part].is_empty() {
                let libre = std::iter::once(&origine[part]).chain(origine.iter()).find(|o| !self.commandes.contains(o)).cloned();
                self.commandes[part] = libre.unwrap_or_default();
            }
        }
        self
    }
}

/// Pose la commande `action` dans la part. Si elle était déjà dans une autre part, celle-ci reçoit la commande remplacée
/// (un échange : aucune commande n'est perdue, aucune n'est en double). Rend la part échangée, s'il y en a une.
pub fn poser(commandes: &mut [String; PARTS], part: usize, action: &str) -> Option<usize> {
    if commandes[part] == action {
        return None;
    }
    let avant = std::mem::replace(&mut commandes[part], action.to_string());
    let autre = (0..PARTS).find(|&k| k != part && commandes[k] == action)?;
    commandes[autre] = avant;
    Some(autre)
}
