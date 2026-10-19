//! Aide > Diagnostic des performances (`aide.diagnostic`, Anthony, 2026-10-19 : « une option qui
//! détecte le matériel et propose des ajustements de performances qui vont jusqu'à éteindre
//! certaines fonctions gourmandes »).
//!
//! Le diagnostic se fait en quatre temps :
//! 1. **détecter** le matériel (`materiel.rs`) ;
//! 2. **mesurer** pendant une quinzaine de secondes, sur un autre fil (`diagbanc.rs`) ;
//! 3. **proposer** un profil, Qualité, Équilibré ou Performance (`choisir_profil`), avec la liste
//!    des réglages concrets qu'il change : chacun se coche ou se décoche avant d'appliquer ;
//! 4. **appliquer**, ou **revenir comme avant** : les valeurs d'avant sont gardées dans les réglages
//!    (`EtatDiagnostic`), même après un redémarrage.
//!
//! Les dix réglages (`Interrupteur`) sont de vrais réglages de SHOT, tous visibles dans
//! Fichier > Préférences… (section « Performance ») : le diagnostic ne fait que les poser. Leur
//! valeur par défaut est le comportement d'avant le diagnostic. Ils s'appliquent tout de suite,
//! sans redémarrer, sauf le rendu des traits vectoriels texturés, qui joue à l'ouverture d'un
//! projet (le projet déjà ouvert garde son rendu).
//!
//! | Réglage | Ce qu'il change |
//! |---|---|
//! | effets en aperçu réduit | `fx::draft_level` : pendant la lecture, le défilement et les gestes, les effets se calculent au moins à moitié de la résolution, même zoomé |
//! | traits texturés | `project::load_quick_progress` : le rendu exact (rejeu du moteur) des dessins vectoriels texturés attend le repos, ou ne vient que pour l'image affichée |
//! | tremblement à l'arrêt | `tremblement::Settings::of` : « Animer à l'arrêt » ne joue plus |
//! | pelure d'oignon | `onion::neighbours` : au plus N images de chaque côté |
//! | cache de la carte graphique | `GpuCanvas::set_cache_bytes` |
//! | cache des traits | `vector::set_cache_budget` |
//! | cache des effets | `fxcache::set_limit` |
//! | fils de calcul | `parallele::coeurs` (tous les calculs répartis) et fil des effets |
//! | vignettes | `thumbs::thumb_size`, vignettes de calque |
//! | animations de l'interface | `egui::Style::animation_time`, glissement des panneaux |

use serde::{Deserialize, Serialize};

use crate::materiel::{GenreCarte, Materiel};

/// Mémoire du cache des dessins de la carte graphique, avant le diagnostic.
pub const CACHE_CARTE_MO: u32 = 1024;
/// Budget des pixels en cache des traits vectoriels, avant le diagnostic.
pub const CACHE_TRAITS_MO: u32 = 512;
/// Réserve des dessins calculés par les effets, avant le diagnostic.
pub const CACHE_EFFETS_MO: u32 = 768;
/// Durée d'une image à 24 images par seconde, en millisecondes.
pub const BUDGET_IMAGE_MS: f64 = 1000.0 / 24.0;

// ---------------------------------------------------------------------- les réglages

/// Rendu des dessins vectoriels texturés à l'ouverture d'un projet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraitsTextures {
    /// Le rendu rapide s'affiche d'abord, le rendu exact arrive en arrière-plan aussitôt.
    #[default]
    #[serde(rename = "exact")]
    Exact,
    /// Rendu rapide jusqu'au repos : le rendu exact attend que plus rien ne bouge.
    #[serde(rename = "repos")]
    Repos,
    /// Rendu rapide partout : le rendu exact ne vient que pour l'image affichée à l'arrêt (et
    /// avant un export).
    #[serde(rename = "rapide")]
    Rapide,
}

impl TraitsTextures {
    pub const TOUS: [TraitsTextures; 3] = [TraitsTextures::Exact, TraitsTextures::Repos, TraitsTextures::Rapide];

    pub fn nom(self) -> &'static str {
        match self {
            TraitsTextures::Exact => "Rendu exact aussitôt",
            TraitsTextures::Repos => "Rendu rapide jusqu'au repos",
            TraitsTextures::Rapide => "Rendu rapide partout",
        }
    }
}

/// Résolution des vignettes du montage et de la timeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vignettes {
    #[default]
    #[serde(rename = "normales")]
    Normales,
    #[serde(rename = "reduites")]
    Reduites,
    #[serde(rename = "minimales")]
    Minimales,
}

impl Vignettes {
    pub const TOUS: [Vignettes; 3] = [Vignettes::Normales, Vignettes::Reduites, Vignettes::Minimales];

    pub fn nom(self) -> &'static str {
        match self {
            Vignettes::Normales => "Résolution normale",
            Vignettes::Reduites => "Moitié de la résolution",
            Vignettes::Minimales => "Quart de la résolution",
        }
    }

    /// Part de la largeur d'origine, en pour cent.
    pub fn pourcent(self) -> u32 {
        match self {
            Vignettes::Normales => 100,
            Vignettes::Reduites => 50,
            Vignettes::Minimales => 25,
        }
    }
}

/// Les réglages de performance de SHOT (`[performance]` dans reglages.toml). Les valeurs par
/// défaut sont le comportement d'avant le diagnostic : un ancien fichier de réglages, sans cette
/// section, se lit tel quel.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerfReglages {
    /// Effets de calque toujours en aperçu réduit (au moins la moitié) pendant la lecture, le
    /// défilement et les gestes.
    #[serde(rename = "effets_apercu_reduit")]
    pub effets_reduits: bool,
    /// Rendu des traits vectoriels texturés à l'ouverture.
    #[serde(rename = "traits_textures")]
    pub traits_textures: TraitsTextures,
    /// « Animer à l'arrêt » du tremblement du trait forcé éteint.
    #[serde(rename = "tremblement_arret_coupe")]
    pub tremblement_coupe: bool,
    /// Pelure d'oignon : nombre d'images au plus de chaque côté (0 : celui de la pelure).
    #[serde(rename = "pelure_max")]
    pub pelure_max: u32,
    /// Mémoire des dessins gardés par la carte graphique, en Mo.
    #[serde(rename = "cache_carte_mo")]
    pub cache_carte_mo: u32,
    /// Budget des pixels en cache des traits vectoriels, en Mo.
    #[serde(rename = "cache_traits_mo")]
    pub cache_traits_mo: u32,
    /// Réserve des dessins calculés par les effets, en Mo.
    #[serde(rename = "cache_effets_mo")]
    pub cache_effets_mo: u32,
    /// Fils de calcul au plus (0 : tous ceux du processeur).
    #[serde(rename = "fils_calcul")]
    pub fils: u32,
    /// Résolution des vignettes du montage et de la timeline.
    #[serde(rename = "vignettes")]
    pub vignettes: Vignettes,
    /// Interface sans animations (survols, panneaux qui glissent).
    #[serde(rename = "animations_coupees")]
    pub animations_coupees: bool,
}

impl Default for PerfReglages {
    fn default() -> Self {
        PerfReglages {
            effets_reduits: false,
            traits_textures: TraitsTextures::Exact,
            tremblement_coupe: false,
            pelure_max: 0,
            cache_carte_mo: CACHE_CARTE_MO,
            cache_traits_mo: CACHE_TRAITS_MO,
            cache_effets_mo: CACHE_EFFETS_MO,
            fils: 0,
            vignettes: Vignettes::Normales,
            animations_coupees: false,
        }
    }
}

impl PerfReglages {
    /// Les mêmes réglages, bornés (un fichier édité à la main ne doit rien casser).
    pub fn sanitized(mut self) -> PerfReglages {
        self.pelure_max = self.pelure_max.min(crate::onion::MAX_NEIGHBOURS as u32);
        self.cache_carte_mo = self.cache_carte_mo.clamp(128, 32768);
        self.cache_traits_mo = self.cache_traits_mo.clamp(64, 16384);
        self.cache_effets_mo = self.cache_effets_mo.clamp(64, 16384);
        self.fils = self.fils.min(256);
        self
    }

    /// Ce que ces réglages demandent au programme (voir `Cibles`).
    pub fn cibles(&self) -> Cibles {
        let r = self.sanitized();
        Cibles {
            effets_min: if r.effets_reduits { 1 } else { 0 },
            traits: r.traits_textures,
            tremblement_coupe: r.tremblement_coupe,
            pelure_max: if r.pelure_max == 0 { crate::onion::MAX_NEIGHBOURS } else { r.pelure_max as usize },
            cache_carte: r.cache_carte_mo as u64 * (1 << 20),
            cache_traits: r.cache_traits_mo as usize * (1 << 20),
            cache_effets: r.cache_effets_mo as usize * (1 << 20),
            fils: r.fils as usize,
            vignettes_pct: r.vignettes.pourcent(),
            animations_coupees: r.animations_coupees,
        }
    }
}

/// Les valeurs que le programme lit vraiment. Chaque réglage de `PerfReglages` en donne une ;
/// `appliquer_globales` les pose là où chaque module les lit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cibles {
    /// Niveau d'aperçu minimal des effets pendant un mouvement (0 : celui du zoom).
    pub effets_min: u8,
    /// Rendu des traits texturés à l'ouverture d'un projet (`App::finish_open`).
    pub traits: TraitsTextures,
    pub tremblement_coupe: bool,
    /// Images de pelure au plus de chaque côté.
    pub pelure_max: usize,
    pub cache_carte: u64,
    pub cache_traits: usize,
    pub cache_effets: usize,
    /// Fils de calcul au plus, 0 : tous.
    pub fils: usize,
    pub vignettes_pct: u32,
    pub animations_coupees: bool,
}

impl Cibles {
    /// Pose les valeurs que les modules lisent (le cache de la carte graphique et le style
    /// d'egui, eux, se posent sur leur objet : `App::appliquer_performance`).
    pub fn appliquer_globales(&self) {
        crate::fx::set_draft_min(self.effets_min);
        crate::tremblement::set_rest_off(self.tremblement_coupe);
        crate::onion::set_cap(self.pelure_max);
        crate::fxcache::set_limit(self.cache_effets);
        crate::vector::set_cache_budget(self.cache_traits);
        crate::parallele::set_plafond(self.fils);
        crate::thumbs::set_scale_pct(self.vignettes_pct);
        crate::ajuste::set_animations_off(self.animations_coupees);
    }
}

/// Un des dix réglages que le diagnostic propose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interrupteur {
    #[serde(rename = "effets_reduits")]
    EffetsReduits,
    #[serde(rename = "traits_textures")]
    TraitsTextures,
    #[serde(rename = "tremblement")]
    Tremblement,
    #[serde(rename = "pelure")]
    Pelure,
    #[serde(rename = "cache_carte")]
    CacheCarte,
    #[serde(rename = "cache_traits")]
    CacheTraits,
    #[serde(rename = "cache_effets")]
    CacheEffets,
    #[serde(rename = "fils")]
    Fils,
    #[serde(rename = "vignettes")]
    Vignettes,
    #[serde(rename = "animations")]
    Animations,
}

fn oui_non(b: bool) -> String {
    if b { "oui" } else { "non" }.into()
}

fn mo(n: u32) -> String {
    if n >= 1024 && n % 1024 == 0 {
        format!("{} Go", n / 1024)
    } else if n >= 1024 {
        format!("{:.1} Go", n as f32 / 1024.0).replace('.', ",")
    } else {
        format!("{n} Mo")
    }
}

impl Interrupteur {
    pub const TOUS: [Interrupteur; 10] = [
        Interrupteur::EffetsReduits,
        Interrupteur::TraitsTextures,
        Interrupteur::Tremblement,
        Interrupteur::Pelure,
        Interrupteur::CacheCarte,
        Interrupteur::CacheTraits,
        Interrupteur::CacheEffets,
        Interrupteur::Fils,
        Interrupteur::Vignettes,
        Interrupteur::Animations,
    ];

    pub fn nom(self) -> &'static str {
        match self {
            Interrupteur::EffetsReduits => "Effets en aperçu réduit pendant la lecture et les gestes",
            Interrupteur::TraitsTextures => "Traits vectoriels texturés",
            Interrupteur::Tremblement => "Tremblement du trait : couper « Animer à l'arrêt »",
            Interrupteur::Pelure => "Pelure d'oignon : images au plus de chaque côté",
            Interrupteur::CacheCarte => "Cache de la carte graphique",
            Interrupteur::CacheTraits => "Cache des traits vectoriels",
            Interrupteur::CacheEffets => "Cache des effets",
            Interrupteur::Fils => "Fils de calcul",
            Interrupteur::Vignettes => "Vignettes du montage et de la timeline",
            Interrupteur::Animations => "Animations de l'interface",
        }
    }

    /// Ce que le réglage change, en une phrase, pour la bulle et la fenêtre.
    pub fn aide(self) -> &'static str {
        match self {
            Interrupteur::EffetsReduits => "Flous, ombres, bruit… se calculent à moitié de la résolution (ou moins) tant que l'image bouge ; la pleine résolution revient au repos. Gain surtout quand on est zoomé.",
            Interrupteur::TraitsTextures => "À l'ouverture d'un projet, les traits texturés (gouache, fusain…) s'affichent en rendu rapide. « Jusqu'au repos » : le rendu exact attend que plus rien ne bouge. « Partout » : il ne vient que pour l'image affichée à l'arrêt, et avant un export. S'applique aux projets ouverts ensuite.",
            Interrupteur::Tremblement => "Les calques au tremblement qui « bouillent » à l'arrêt restent fixes (l'export garde le tremblement de chaque image). Plus aucun redessin à 12 images par seconde au repos.",
            Interrupteur::Pelure => "Plafonne le nombre d'images de pelure d'oignon affichées de chaque côté, quel que soit le réglage de la pelure (qui n'est pas modifié).",
            Interrupteur::CacheCarte => "Mémoire de la carte graphique gardée pour les dessins déjà envoyés. Plus grand : lecture sans renvoi ; plus petit : moins de mémoire vidéo occupée.",
            Interrupteur::CacheTraits => "Mémoire des pixels déjà rendus des traits vectoriels. Plus petit : plus de rejeux du moteur de pinceau en arrière-plan.",
            Interrupteur::CacheEffets => "Mémoire des dessins déjà passés par les effets. Plus petit : les effets se recalculent plus souvent.",
            Interrupteur::Fils => "Nombre de cœurs au plus pour les calculs répartis (ouverture, enregistrement, effets, rendus). Automatique : tous. Moins de fils laisse de la place à l'interface.",
            Interrupteur::Vignettes => "Les vignettes se calculent à une résolution plus basse (agrandies à l'affichage) : moins de calcul et de mémoire.",
            Interrupteur::Animations => "Pas de transitions sur les survols et les menus, et les panneaux s'ajustent d'un coup au lieu de glisser.",
        }
    }

    /// Le réglage joue-t-il tout de suite (sans rouvrir le projet) ?
    pub fn immediat(self) -> bool {
        self != Interrupteur::TraitsTextures
    }

    /// La valeur du réglage dans `r`, en toutes lettres.
    pub fn texte(self, r: &PerfReglages) -> String {
        match self {
            Interrupteur::EffetsReduits => oui_non(r.effets_reduits),
            Interrupteur::TraitsTextures => r.traits_textures.nom().into(),
            Interrupteur::Tremblement => oui_non(r.tremblement_coupe),
            Interrupteur::Pelure => match r.pelure_max {
                0 => "pas de plafond".into(),
                1 => "1 image".into(),
                n => format!("{n} images"),
            },
            Interrupteur::CacheCarte => mo(r.cache_carte_mo),
            Interrupteur::CacheTraits => mo(r.cache_traits_mo),
            Interrupteur::CacheEffets => mo(r.cache_effets_mo),
            Interrupteur::Fils => match r.fils {
                0 => "automatique (tous les cœurs)".into(),
                n => format!("{n} au plus"),
            },
            Interrupteur::Vignettes => r.vignettes.nom().into(),
            Interrupteur::Animations => oui_non(r.animations_coupees),
        }
    }

    /// Recopie ce réglage de `depuis` dans `vers`.
    pub fn copier(self, vers: &mut PerfReglages, depuis: &PerfReglages) {
        match self {
            Interrupteur::EffetsReduits => vers.effets_reduits = depuis.effets_reduits,
            Interrupteur::TraitsTextures => vers.traits_textures = depuis.traits_textures,
            Interrupteur::Tremblement => vers.tremblement_coupe = depuis.tremblement_coupe,
            Interrupteur::Pelure => vers.pelure_max = depuis.pelure_max,
            Interrupteur::CacheCarte => vers.cache_carte_mo = depuis.cache_carte_mo,
            Interrupteur::CacheTraits => vers.cache_traits_mo = depuis.cache_traits_mo,
            Interrupteur::CacheEffets => vers.cache_effets_mo = depuis.cache_effets_mo,
            Interrupteur::Fils => vers.fils = depuis.fils,
            Interrupteur::Vignettes => vers.vignettes = depuis.vignettes,
            Interrupteur::Animations => vers.animations_coupees = depuis.animations_coupees,
        }
    }

    /// Ce réglage est-il le même dans `a` et `b` ?
    pub fn egal(self, a: &PerfReglages, b: &PerfReglages) -> bool {
        let mut x = *a;
        self.copier(&mut x, b);
        x == *a
    }

    /// La valeur que le programme lit pour ce réglage, en texte (pour vérifier que le réglage
    /// change bien quelque chose : deux valeurs différentes donnent deux lectures différentes).
    #[cfg(test)]
    pub fn lecture(self, r: &PerfReglages) -> String {
        let c = r.cibles();
        match self {
            Interrupteur::EffetsReduits => format!("niveau d'aperçu minimal {}", c.effets_min),
            Interrupteur::TraitsTextures => format!("{:?}", r.traits_textures),
            Interrupteur::Tremblement => format!("repos coupé : {}", c.tremblement_coupe),
            Interrupteur::Pelure => format!("{} images au plus", c.pelure_max),
            Interrupteur::CacheCarte => format!("{} octets", c.cache_carte),
            Interrupteur::CacheTraits => format!("{} octets", c.cache_traits),
            Interrupteur::CacheEffets => format!("{} octets", c.cache_effets),
            Interrupteur::Fils => format!("{} fils", c.fils),
            Interrupteur::Vignettes => format!("{} %", c.vignettes_pct),
            Interrupteur::Animations => format!("animations coupées : {}", c.animations_coupees),
        }
    }
}

// ---------------------------------------------------------------------- les mesures

/// Ce que le banc a mesuré (millisecondes, ou Mo par seconde pour l'envoi à la carte). `None` :
/// l'étape n'a pas pu se faire.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesures {
    /// Composition de 12 calques de 3840 × 2160 sur le processeur.
    pub composition_ms: Option<f64>,
    /// Rejeu d'un trait texturé large par le moteur de pinceau.
    pub trait_ms: Option<f64>,
    /// Flou gaussien d'une feuille à pleine résolution.
    pub flou_plein_ms: Option<f64>,
    /// Le même flou en aperçu réduit de moitié (réduction comprise).
    pub flou_apercu_ms: Option<f64>,
    /// Image filmée de ce plan chargé, en moyenne sur une lecture à 24 images par seconde.
    pub lecture_ms: Option<f64>,
    /// La plus lente des images de cette lecture.
    pub lecture_pire_ms: Option<f64>,
    /// Envoi de dessins à la carte graphique, Mo par seconde.
    pub envoi_carte_mo_s: Option<f64>,
    /// Durée totale du banc, en secondes.
    pub duree_s: f64,
}

/// Temps de référence : le poste d'Anthony (Core i7-10700K, RTX 2080 SUPER), mesuré par ce banc
/// (deux séries, 2026-10-19, arrondies un peu au-dessus de la meilleure : 19,5 / 106 / 43 / 12,7 ms
/// et 6 800 Mo/s au mieux, 36 / 155 / 63 / 17 ms quand la machine était occupée). Un temps de plus
/// de `LENT` fois la référence pèse sur le profil, de plus de `TRES_LENT` fois il pèse lourd.
pub const REF_COMPOSITION_MS: f64 = 25.0;
pub const REF_TRAIT_MS: f64 = 120.0;
pub const REF_FLOU_MS: f64 = 50.0;
pub const REF_LECTURE_MS: f64 = 15.0;
pub const REF_ENVOI_MO_S: f64 = 6000.0;
pub const LENT: f64 = 2.2;
pub const TRES_LENT: f64 = 4.5;

// ---------------------------------------------------------------------- les profils

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profil {
    #[serde(rename = "qualite")]
    Qualite,
    #[serde(rename = "equilibre")]
    Equilibre,
    #[serde(rename = "performance")]
    Performance,
}

impl Profil {
    pub const TOUS: [Profil; 3] = [Profil::Qualite, Profil::Equilibre, Profil::Performance];

    pub fn nom(self) -> &'static str {
        match self {
            Profil::Qualite => "Qualité",
            Profil::Equilibre => "Équilibré",
            Profil::Performance => "Performance",
        }
    }

    pub fn resume(self) -> &'static str {
        match self {
            Profil::Qualite => "Tout en pleine qualité, les caches agrandis si la mémoire le permet.",
            Profil::Equilibre => "Aperçu réduit pendant les mouvements, rendus exacts au repos, caches mesurés.",
            Profil::Performance => "Les fonctions gourmandes s'éteignent : rendu rapide des traits, pas de tremblement à l'arrêt, pelure et vignettes réduites.",
        }
    }
}

/// Un constat sur la machine : son poids (0 : information, 1 : pèse, 2 : pèse lourd) et la phrase.
#[derive(Clone, Debug, PartialEq)]
pub struct Constat {
    pub poids: u8,
    pub texte: String,
}

/// Le profil proposé et pourquoi.
#[derive(Clone, Debug, PartialEq)]
pub struct Choix {
    pub profil: Profil,
    pub constats: Vec<Constat>,
}

impl Choix {
    /// L'explication, en une ou deux phrases par constat qui compte.
    pub fn raisons(&self) -> Vec<String> {
        let importants: Vec<String> = self.constats.iter().filter(|c| c.poids > 0).map(|c| c.texte.clone()).collect();
        if importants.is_empty() {
            self.constats.iter().filter(|c| c.poids == 0).map(|c| c.texte.clone()).take(3).collect()
        } else {
            importants
        }
    }
}

fn virgule(x: f64, decimales: usize) -> String {
    format!("{x:.decimales$}").replace('.', ",")
}

/// Compare un temps mesuré à sa référence ; un constat s'il est lent.
fn lenteur(constats: &mut Vec<Constat>, nom: &str, mesure: Option<f64>, reference: f64, inverse: bool) {
    let Some(v) = mesure.filter(|v| v.is_finite() && *v > 0.0) else { return };
    // `inverse` : un débit (plus grand = mieux), sinon un temps.
    let rapport = if inverse { reference / v } else { v / reference };
    let poids = if rapport >= TRES_LENT {
        2
    } else if rapport >= LENT {
        1
    } else {
        0
    };
    if poids > 0 {
        let unite = if inverse { "Mo/s" } else { "ms" };
        constats.push(Constat {
            poids,
            texte: format!("{nom} : {} {unite} ({}× plus lent que sur le poste de référence, {} {unite}).", virgule(v, 0), virgule(rapport, 1), virgule(reference, 0)),
        });
    }
}

/// Les constats sur le matériel et les mesures, du plus lourd au plus léger.
pub fn constats(m: &Materiel, x: &Mesures) -> Vec<Constat> {
    let mut v: Vec<Constat> = Vec::new();
    let c = |poids, texte: String| Constat { poids, texte };
    // Mémoire vive.
    if let Some(r) = m.memoire {
        let go = r.totale as f64 / (1u64 << 30) as f64;
        let dispo = r.disponible as f64 / (1u64 << 30) as f64;
        if go < 8.0 {
            v.push(c(2, format!("Mémoire vive : {} Go seulement (SHOT garde des dessins en mémoire).", virgule(go, 1))));
        } else if go < 16.0 {
            v.push(c(1, format!("Mémoire vive : {} Go, juste pour de gros projets.", virgule(go, 1))));
        }
        if dispo < 2.0 {
            v.push(c(2, format!("Seulement {} Go de mémoire vive disponibles en ce moment.", virgule(dispo, 1))));
        } else if dispo < 4.0 {
            v.push(c(1, format!("{} Go de mémoire vive disponibles en ce moment : peu de marge.", virgule(dispo, 1))));
        }
    }
    // Processeur.
    let n = m.processeur.logiques;
    if n < 4 {
        v.push(c(2, format!("Processeur : {n} fils seulement, les calculs de fond se disputent l'interface.")));
    } else if n < 8 {
        v.push(c(1, format!("Processeur : {n} fils, les effets et les rendus de fond laissent peu de marge à l'interface.")));
    }
    if !m.processeur.x86_64_v3() {
        v.push(c(0, "Le processeur n'a pas toutes les instructions x86-64-v3 (AVX2, FMA, BMI2) pour lesquelles SHOT est compilé.".into()));
    }
    // Carte graphique.
    match &m.carte {
        None => v.push(c(2, "Pas de carte graphique pour l'affichage (OpenGL de secours) : les calques se composent sur le processeur.".into())),
        Some(carte) => {
            match carte.genre {
                GenreCarte::Logicielle => v.push(c(2, format!("{} : rendu logiciel, sans vraie carte graphique.", carte.nom))),
                GenreCarte::Integree => v.push(c(1, format!("{} : graphisme intégré au processeur, la mémoire est partagée avec le système.", carte.nom))),
                GenreCarte::Virtuelle => v.push(c(1, format!("{} : carte virtuelle.", carte.nom))),
                GenreCarte::Dediee | GenreCarte::Autre => {}
            }
            if let Some(vram) = carte.vram {
                let go = vram as f64 / (1u64 << 30) as f64;
                if go < 2.0 {
                    v.push(c(2, format!("Mémoire vidéo : {} Go seulement.", virgule(go, 1))));
                } else if go < 4.0 {
                    v.push(c(1, format!("Mémoire vidéo : {} Go, le cache de dessins doit rester modeste.", virgule(go, 1))));
                }
            }
        }
    }
    // Les mesures, contre le poste de référence.
    lenteur(&mut v, "Lecture d'un plan chargé (12 calques)", x.lecture_ms, REF_LECTURE_MS, false);
    lenteur(&mut v, "Composition de 12 calques", x.composition_ms, REF_COMPOSITION_MS, false);
    lenteur(&mut v, "Rejeu d'un trait texturé", x.trait_ms, REF_TRAIT_MS, false);
    lenteur(&mut v, "Flou à pleine résolution", x.flou_plein_ms, REF_FLOU_MS, false);
    lenteur(&mut v, "Envoi de dessins à la carte graphique", x.envoi_carte_mo_s, REF_ENVOI_MO_S, true);
    // Le disque ne change pas de réglage, mais il se dit.
    if let Some(d) = &m.disque {
        if d.ecriture < 60.0 {
            v.push(c(0, format!("Disque lent en écriture ({} Mo/s) : l'enregistrement et la copie de secours prendront du temps.", virgule(d.ecriture, 0))));
        }
    }
    v.sort_by_key(|c| std::cmp::Reverse(c.poids));
    v
}

/// Le profil que la machine mérite. Règles : un constat qui pèse lourd (poids 2) donne
/// Performance ; sinon un constat qui pèse (poids 1) donne Équilibré ; sinon Qualité.
pub fn choisir_profil(m: &Materiel, x: &Mesures) -> Choix {
    let constats = constats(m, x);
    let max = constats.iter().map(|c| c.poids).max().unwrap_or(0);
    let profil = match max {
        2 => Profil::Performance,
        1 => Profil::Equilibre,
        _ => Profil::Qualite,
    };
    let mut constats = constats;
    if max == 0 {
        // Rien ne pèse : on dit ce qui rassure.
        let mut bonnes = Vec::new();
        if let Some(r) = m.memoire {
            bonnes.push(format!("{} de mémoire vive", crate::materiel::en_go(r.totale)));
        }
        bonnes.push(format!("{} fils", m.processeur.logiques));
        if let Some(c) = &m.carte {
            bonnes.push(c.nom.clone());
        }
        constats.push(Constat { poids: 0, texte: format!("Cette machine a de la marge : {}.", bonnes.join(", ")) });
        if let Some(l) = x.lecture_ms {
            constats.push(Constat { poids: 0, texte: format!("Une image d'un plan chargé se compose en {} ms en moyenne (une image à 24 par seconde dure {} ms).", virgule(l, 0), virgule(BUDGET_IMAGE_MS, 0)) });
        }
    }
    Choix { profil, constats }
}

fn go_de(octets: u64) -> f64 {
    octets as f64 / (1u64 << 30) as f64
}

/// Les réglages que `profil` propose pour cette machine.
pub fn reglages_du_profil(profil: Profil, m: &Materiel) -> PerfReglages {
    let ram = m.memoire.map(|r| go_de(r.totale));
    let vram_mo = m.carte.as_ref().and_then(|c| c.vram).map(|v| (v >> 20) as u32);
    let logiques = m.processeur.logiques;
    let peu_de_ram = ram.is_some_and(|g| g < 16.0);
    match profil {
        Profil::Qualite => {
            let mut r = PerfReglages::default();
            // Beaucoup de mémoire vidéo : un cache de dessins plus grand (le quart, de 1 à 3 Go).
            if let Some(v) = vram_mo.filter(|v| *v >= 6144) {
                r.cache_carte_mo = (v / 4).clamp(CACHE_CARTE_MO, 3072);
            }
            if ram.is_some_and(|g| g >= 32.0) {
                r.cache_traits_mo = 1024;
                r.cache_effets_mo = 1536;
            }
            r
        }
        Profil::Equilibre => PerfReglages {
            effets_reduits: true,
            traits_textures: TraitsTextures::Repos,
            pelure_max: 6,
            cache_carte_mo: vram_mo.map_or(768, |v| (v / 8).clamp(512, CACHE_CARTE_MO)),
            cache_traits_mo: if peu_de_ram { 384 } else { CACHE_TRAITS_MO },
            cache_effets_mo: if peu_de_ram { 512 } else { CACHE_EFFETS_MO },
            ..PerfReglages::default()
        },
        Profil::Performance => {
            let tres_peu = ram.is_some_and(|g| g < 8.0) || logiques <= 4;
            PerfReglages {
                effets_reduits: true,
                traits_textures: TraitsTextures::Rapide,
                tremblement_coupe: true,
                pelure_max: 2,
                cache_carte_mo: vram_mo.map_or(384, |v| (v / 8).clamp(256, 512)),
                cache_traits_mo: 256,
                cache_effets_mo: if ram.is_some_and(|g| g < 8.0) { 256 } else { 384 },
                // Un cœur au moins reste à l'interface sur un petit processeur.
                fils: if (3..=4).contains(&logiques) { logiques as u32 - 1 } else { 0 },
                vignettes: if tres_peu { Vignettes::Minimales } else { Vignettes::Reduites },
                animations_coupees: true,
            }
        }
    }
}

/// Les réglages qui changent si on pose `cible` à la place de `courant`.
pub fn differences(courant: &PerfReglages, cible: &PerfReglages) -> Vec<Interrupteur> {
    Interrupteur::TOUS.into_iter().filter(|i| !i.egal(courant, cible)).collect()
}

// ---------------------------------------------------------------------- appliquer, revenir

/// Ce que le diagnostic garde dans les réglages (`[diagnostic]`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EtatDiagnostic {
    /// Le diagnostic a été proposé au premier lancement (une réponse a été donnée).
    #[serde(rename = "propose")]
    pub propose: bool,
    /// Date du dernier diagnostic, en secondes depuis 1970.
    #[serde(rename = "date")]
    pub date: Option<u64>,
    /// Le profil appliqué la dernière fois.
    #[serde(rename = "profil")]
    pub profil: Option<Profil>,
    /// Les réglages d'avant la dernière application.
    #[serde(rename = "avant")]
    pub avant: Option<PerfReglages>,
    /// Les réglages posés par la dernière application.
    #[serde(rename = "appliques")]
    pub appliques: Vec<Interrupteur>,
}

impl EtatDiagnostic {
    /// Faut-il proposer le diagnostic au lancement ? Seulement sur une machine qui n'en a jamais
    /// eu, et qu'on n'a pas déjà sollicitée.
    pub fn a_proposer(&self) -> bool {
        !self.propose && self.date.is_none()
    }

    pub fn peut_revenir(&self) -> bool {
        self.avant.is_some()
    }

    /// Pose, parmi `cible`, les réglages de `coches` dans `courant`, en gardant ceux d'avant.
    pub fn appliquer(&mut self, courant: &mut PerfReglages, cible: &PerfReglages, coches: &[Interrupteur], profil: Profil, date: u64) {
        self.avant = Some(*courant);
        self.appliques = coches.to_vec();
        for i in coches {
            i.copier(courant, cible);
        }
        *courant = courant.sanitized();
        self.profil = Some(profil);
        self.date = Some(date);
        self.propose = true;
    }

    /// Remet les valeurs d'avant pour les réglages que la dernière application a posés (ceux
    /// qu'on a changés à la main depuis, et les autres, ne bougent pas). `false` : rien à remettre.
    pub fn revenir(&mut self, courant: &mut PerfReglages) -> bool {
        let Some(avant) = self.avant.take() else { return false };
        for i in std::mem::take(&mut self.appliques) {
            i.copier(courant, &avant);
        }
        self.profil = None;
        true
    }

    /// Le diagnostic a été fait (sans rien appliquer) : plus de proposition au lancement.
    pub fn note_fait(&mut self, date: u64) {
        self.propose = true;
        self.date = Some(date);
    }
}

// ---------------------------------------------------------------------- le rapport

/// « 2026-10-19 14:05 (UTC) » pour des secondes depuis 1970.
pub fn date_texte(secs: u64) -> String {
    let jours = (secs / 86_400) as i64;
    let reste = secs % 86_400;
    // Jours depuis 1970 -> date civile (algorithme de Howard Hinnant).
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + ere * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let an = if mois <= 2 { y + 1 } else { y };
    format!("{an}-{mois:02}-{d:02} {:02}:{:02} (UTC)", reste / 3600, (reste % 3600) / 60)
}

pub fn maintenant() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn mesure_ligne(nom: &str, v: Option<f64>, unite: &str) -> String {
    match v {
        Some(v) => format!("{nom} : {} {unite}", virgule(v, if v >= 100.0 { 0 } else { 1 })),
        None => format!("{nom} : non mesuré"),
    }
}

/// Les mesures en lignes lisibles.
pub fn lignes_mesures(x: &Mesures) -> Vec<String> {
    vec![
        mesure_ligne("Composition de 12 calques (processeur)", x.composition_ms, "ms"),
        mesure_ligne("Rejeu d'un trait texturé", x.trait_ms, "ms"),
        mesure_ligne("Flou, pleine résolution", x.flou_plein_ms, "ms"),
        mesure_ligne("Flou, aperçu réduit de moitié", x.flou_apercu_ms, "ms"),
        match (x.lecture_ms, x.lecture_pire_ms) {
            (Some(m), Some(p)) => format!("Lecture d'un plan chargé : {} ms par image en moyenne, {} ms au pire (budget {} ms)", virgule(m, 1), virgule(p, 1), virgule(BUDGET_IMAGE_MS, 1)),
            _ => "Lecture d'un plan chargé : non mesurée".into(),
        },
        mesure_ligne("Envoi de dessins à la carte graphique", x.envoi_carte_mo_s, "Mo/s"),
    ]
}

/// Le rapport en texte brut, à copier ou à enregistrer.
pub fn rapport(m: &Materiel, x: &Mesures, choix: &Choix, courant: &PerfReglages, cible: &PerfReglages, date: u64) -> String {
    let mut s = String::new();
    s.push_str("SHOT — Rapport du diagnostic des performances\n");
    s.push_str(&format!("Version {} — {}\n\n", env!("CARGO_PKG_VERSION"), date_texte(date)));
    s.push_str("MATÉRIEL\n");
    for (nom, valeur) in m.lignes() {
        s.push_str(&format!("  {nom} : {valeur}\n"));
    }
    s.push_str("\nMESURES\n");
    for l in lignes_mesures(x) {
        s.push_str(&format!("  {l}\n"));
    }
    s.push_str(&format!("  Durée du banc : {} s\n", virgule(x.duree_s, 1)));
    s.push_str(&format!("\nPROFIL PROPOSÉ : {}\n", choix.profil.nom()));
    for r in choix.raisons() {
        s.push_str(&format!("  - {r}\n"));
    }
    s.push_str("\nRÉGLAGES (actuel -> proposé)\n");
    for i in Interrupteur::TOUS {
        let marque = if i.egal(courant, cible) { " " } else { "*" };
        s.push_str(&format!("  {marque} {} : {} -> {}\n", i.nom(), i.texte(courant), i.texte(cible)));
    }
    s.push_str("\n(* : réglage qui changerait)\n");
    s
}
