//! Panneau Pinceaux, la logique sans écran (refonte du 2026-10-18, d'après la demande d'Anthony :
//! « une organisation des brosses plus simple, et la taille du pinceau mise en avant ») :
//! - le rangement des réglages nommés en dossiers (Crayons, Encres et feutres, Peinture, Craie et
//!   fusain, Gommes, Mes brosses, puis un dossier par fichier importé) et la recherche par nom ;
//! - l'état retenu d'une séance à l'autre (onglet, liste ou vignettes, dossiers ouverts) ;
//! - les catégories de l'onglet « Réglages du pinceau », avec la case qui allume ou éteint
//!   celles qui s'allument (dispersion, grain, couleur et mélange, lissage).
//!
//! L'affichage est dans `pinceauxui.rs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::brush::{BrushSettings, Connection, Source};
use crate::brushtex::{Preset, Target, MINE, PAPER_DRAWING};

/// Source des réglages fournis par Pelure.
pub const PELURE: &str = "Pelure";

pub const CRAYONS: &str = "Crayons";
pub const ENCRES: &str = "Encres et feutres";
pub const PEINTURE: &str = "Peinture";
pub const CRAIE: &str = "Craie et fusain";
pub const GOMMES: &str = "Gommes";

/// Les dossiers des réglages fournis, dans l'ordre où ils s'affichent.
pub const DOSSIERS_FOURNIS: [&str; 5] = [CRAYONS, ENCRES, PEINTURE, CRAIE, GOMMES];

/// Minuscules sans accents, pour comparer des noms : « Éponge » se trouve en tapant « epon ».
pub fn plie(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            c => c,
        })
        .collect()
}

/// Le dossier d'un réglage : « Mes brosses » pour ceux de l'utilisateur, le nom du fichier pour
/// un réglage importé ; pour ceux de Pelure, d'après leur nom (la famille de la brosse).
pub fn dossier_de(p: &Preset) -> String {
    if p.source == MINE {
        return MINE.to_string();
    }
    if p.source != PELURE {
        return p.source.clone();
    }
    let n = plie(&p.name);
    let a = |mots: &[&str]| mots.iter().any(|m| n.contains(m));
    if p.target == Target::Eraser || a(&["gomme"]) {
        GOMMES
    } else if a(&["crayon", "graphite", "mine"]) {
        CRAYONS
    } else if a(&["encre", "feutre", "stylo", "plume", "marqueur"]) {
        ENCRES
    } else if a(&["craie", "fusain", "pastel", "sanguine"]) {
        CRAIE
    } else {
        PEINTURE
    }
    .to_string()
}

/// Un dossier de réglages, avec les rangs (dans `BrushLibrary::presets`) de ceux qu'il montre.
#[derive(Clone, Debug, PartialEq)]
pub struct Dossier {
    pub nom: String,
    pub rangs: Vec<usize>,
    /// Importé d'un fichier (vs fourni par Pelure ou à l'utilisateur).
    pub importe: bool,
}

/// Les réglages de `cible` rangés en dossiers, ceux dont le nom contient `requete` (sans tenir
/// compte des majuscules ni des accents). Les dossiers fournis d'abord, « Mes brosses », puis un
/// dossier par fichier importé dans l'ordre où ils apparaissent. Un dossier vide ne se montre pas,
/// sauf « Mes brosses » quand on ne cherche rien (pour savoir où vont les brosses enregistrées).
pub fn ranger(presets: &[Preset], cible: Target, requete: &str) -> Vec<Dossier> {
    let q = plie(requete.trim());
    let mut out: Vec<Dossier> = DOSSIERS_FOURNIS
        .iter()
        .map(|n| Dossier { nom: n.to_string(), rangs: Vec::new(), importe: false })
        .collect();
    out.push(Dossier { nom: MINE.to_string(), rangs: Vec::new(), importe: false });
    for (i, p) in presets.iter().enumerate().filter(|(_, p)| p.target == cible) {
        if !q.is_empty() && !plie(&p.name).contains(&q) {
            continue;
        }
        let nom = dossier_de(p);
        match out.iter_mut().find(|d| d.nom == nom) {
            Some(d) => d.rangs.push(i),
            None => out.push(Dossier { nom, rangs: vec![i], importe: true }),
        }
    }
    out.retain(|d| !d.rangs.is_empty() || (q.is_empty() && d.nom == MINE));
    out
}

/// Le réglage dont les valeurs sont celles de `courant`, pour cette cible (le premier).
pub fn choisi(presets: &[Preset], cible: Target, courant: &BrushSettings) -> Option<usize> {
    presets.iter().position(|p| p.target == cible && p.settings == *courant)
}

/// Un réglage qu'on peut supprimer : ceux de l'utilisateur et les importés, pas ceux de Pelure.
pub fn supprimable(p: &Preset) -> bool {
    p.source != PELURE
}

/// Un dossier est-il ouvert la première fois ? Ceux de Pelure et « Mes brosses » oui ; un dossier
/// importé non (il peut compter des centaines de brosses).
pub fn ouvert_par_defaut(nom: &str) -> bool {
    nom == MINE || DOSSIERS_FOURNIS.contains(&nom)
}

/// Les deux onglets du panneau.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Onglet {
    #[default]
    #[serde(rename = "pinceaux")]
    Pinceaux,
    #[serde(rename = "reglages")]
    Reglages,
}

/// Ce que le panneau retient d'une séance à l'autre, dans les réglages de l'application.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Panneau {
    pub onglet: Onglet,
    /// Liste (vrai) ou vignettes (faux).
    pub liste: bool,
    /// Catégorie de l'onglet « Réglages du pinceau ».
    pub categorie: Categorie,
    /// État de chaque dossier que l'utilisateur a ouvert ou replié (vrai : ouvert) ; ceux qu'il
    /// n'a pas touchés suivent `ouvert_par_defaut`.
    pub dossiers: BTreeMap<String, bool>,
}

impl Default for Panneau {
    fn default() -> Self {
        Panneau { onglet: Onglet::Pinceaux, liste: true, categorie: Categorie::Pointe, dossiers: BTreeMap::new() }
    }
}

impl Panneau {
    pub fn ouvert(&self, nom: &str) -> bool {
        self.dossiers.get(nom).copied().unwrap_or_else(|| ouvert_par_defaut(nom))
    }

    pub fn regler(&mut self, nom: &str, ouvert: bool) {
        if ouvert == ouvert_par_defaut(nom) {
            self.dossiers.remove(nom);
        } else {
            self.dossiers.insert(nom.to_string(), ouvert);
        }
    }
}

// ---------------------------------------------------------------- catégories des réglages

/// Les catégories de l'onglet « Réglages du pinceau ».
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Categorie {
    #[serde(rename = "pointe")]
    Pointe,
    #[serde(rename = "taille")]
    Taille,
    #[serde(rename = "dispersion")]
    Dispersion,
    #[serde(rename = "grain")]
    Grain,
    #[serde(rename = "couleur")]
    Couleur,
    #[serde(rename = "lissage")]
    Lissage,
    #[serde(rename = "toucher")]
    Toucher,
}

impl Categorie {
    pub const ALL: [Categorie; 7] = [
        Categorie::Pointe,
        Categorie::Taille,
        Categorie::Dispersion,
        Categorie::Grain,
        Categorie::Couleur,
        Categorie::Lissage,
        Categorie::Toucher,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Categorie::Pointe => "Forme de la pointe",
            Categorie::Taille => "Taille et appui",
            Categorie::Dispersion => "Dispersion",
            Categorie::Grain => "Grain et papier",
            Categorie::Couleur => "Couleur et mélange",
            Categorie::Lissage => "Lissage",
            Categorie::Toucher => "Toucher du stylet",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Categorie::Pointe => "La pointe (ronde, ou une image), sa dureté, sa rondeur, son angle et l'espacement des empreintes.",
            Categorie::Taille => "La taille et l'opacité, ce que l'appui du stylet en fait, et les connexions (vitesse, direction, fondu, hasard).",
            Categorie::Dispersion => "Chaque empreinte s'écarte, grandit ou tourne au hasard. La case l'éteint sans perdre les valeurs.",
            Categorie::Grain => "Le papier et son grain, le grain propre du crayon. La case les éteint sans perdre les valeurs.",
            Categorie::Couleur => "Le mélange avec la couleur déjà posée (peinture humide), la charge de la brosse, la couleur A→B. La case les éteint.",
            Categorie::Lissage => "Le trait suit la pointe en douceur (suivi, corde, moyennes). La case l'éteint sans perdre les valeurs.",
            Categorie::Toucher => "La réponse du stylet à la pression, commune à tous les pinceaux.",
        }
    }

    /// Cette catégorie a-t-elle une case qui l'allume et l'éteint ?
    pub fn a_une_case(self) -> bool {
        matches!(self, Categorie::Dispersion | Categorie::Grain | Categorie::Couleur | Categorie::Lissage)
    }

    /// La catégorie agit-elle dans ce réglage ? (Sans case : toujours.)
    pub fn allumee(self, s: &BrushSettings) -> bool {
        match self {
            Categorie::Dispersion => s.scatter > 0.0 || s.scatter_along > 0.0 || s.count > 1 || s.size_jitter > 0.0 || s.angle_jitter > 0.0,
            Categorie::Grain => (s.paper.is_some() && s.grain > 0.0) || s.pencil_grain != crate::brush::PencilGrain::None,
            Categorie::Couleur => s.mix > 0.0 || s.charge < 1.0 || s.dynamics.color.active(),
            Categorie::Lissage => match s.smooth_kind {
                crate::brush::SmoothKind::Follow => s.smoothing > 0.0,
                crate::brush::SmoothKind::PulledString | crate::brush::SmoothKind::AverageDistance => s.smooth_px > 0.0,
                crate::brush::SmoothKind::AveragePoints => s.smooth_points > 1,
            },
            _ => true,
        }
    }

    /// Éteint la catégorie : les réglages qu'elle commande reviennent à « sans effet ».
    pub fn eteindre(self, s: &mut BrushSettings) {
        match self {
            Categorie::Dispersion => {
                (s.scatter, s.scatter_along, s.size_jitter, s.angle_jitter, s.count) = (0.0, 0.0, 0.0, 0.0, 1);
            }
            Categorie::Grain => {
                s.paper = None;
                s.pencil_grain = crate::brush::PencilGrain::None;
            }
            Categorie::Couleur => {
                (s.mix, s.charge) = (0.0, 1.0);
                s.dynamics.color = Connection { source: Source::None, ..s.dynamics.color };
            }
            Categorie::Lissage => {
                (s.smoothing, s.smooth_px, s.smooth_points) = (0.0, 0.0, 1);
            }
            _ => {}
        }
    }

    /// Allume la catégorie. `avant` : le réglage tel qu'il était quand on l'a éteinte, dont on
    /// reprend les valeurs ; sans lui (ou s'il n'avait rien d'allumé), une valeur de départ.
    pub fn allumer(self, s: &mut BrushSettings, avant: Option<&BrushSettings>) {
        let avant = avant.filter(|b| self.allumee(b));
        match self {
            Categorie::Dispersion => match avant {
                Some(b) => (s.scatter, s.scatter_along, s.size_jitter, s.angle_jitter, s.count) = (b.scatter, b.scatter_along, b.size_jitter, b.angle_jitter, b.count),
                None => s.scatter = 0.5,
            },
            Categorie::Grain => {
                if let Some(b) = avant {
                    (s.paper, s.grain, s.pencil_grain, s.paper_scale, s.paper_angle) = (b.paper, b.grain, b.pencil_grain, b.paper_scale, b.paper_angle);
                    (s.paper_invert, s.paper_softness, s.paper_follow) = (b.paper_invert, b.paper_softness, b.paper_follow);
                }
                if s.paper.is_none() && s.pencil_grain == crate::brush::PencilGrain::None {
                    s.paper = Some(PAPER_DRAWING);
                }
                if s.paper.is_some() && s.grain <= 0.0 {
                    s.grain = 0.6;
                }
            }
            Categorie::Couleur => match avant {
                Some(b) => (s.mix, s.charge, s.dynamics.color) = (b.mix, b.charge, b.dynamics.color),
                None => s.mix = 0.4,
            },
            Categorie::Lissage => match avant {
                Some(b) => (s.smoothing, s.smooth_px, s.smooth_points) = (b.smoothing, b.smooth_px, b.smooth_points),
                None => (s.smoothing, s.smooth_px, s.smooth_points) = (0.3, 8.0, 6),
            },
            _ => {}
        }
    }
}
