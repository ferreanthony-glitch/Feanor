//! Le modèle d'épisode (pipeline, étape 1) : « Enregistrer comme modèle d'épisode… » garde les réglages d'un épisode, sans un seul dessin ; « Nouvel épisode
//! depuis un modèle » (dans le formulaire Nouveau projet) crée l'épisode suivant avec les mêmes réglages en quelques clics.
//!
//! Ce que le modèle garde : le format (cadre, cadence, papier, couleur des aplats), la numérotation (suivie ou verrouillée), la série, l'épisode (le
//! suivant est proposé), le modèle de noms, l'auteur et le copyright, les calques du premier plan (nom, sorte, visibilité, opacité, personnage), les
//! personnages (nom, couleur du trait, aplat), et les réglages d'export du poste (taille et cadence du film, PDF, images par plan, fiche technique,
//! liste des plans, paquet de livraison).
//!
//! Les brosses importées et celles des décors sont des réglages du poste (`%APPDATA%\SHOT`) : elles servent déjà à tous les projets, le modèle n'a rien
//! à en garder.
//!
//! Les modèles vivent dans `%APPDATA%\SHOT\Modèles\<nom>.episode.json`, jamais dans un projet.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::document::{BackgroundKind, Document, Layer, LayerKind};
use crate::nouveau::Values;
use crate::settings::Settings;

pub const FORMAT: &str = "shot-modele-episode";
pub const EXTENSION: &str = ".episode.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalqueModele {
    pub nom: String,
    pub vectoriel: bool,
    pub visible: bool,
    pub opacite: f32,
    /// Le nom du personnage dont la pastille a fait le calque.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personnage: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub decor: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersonnageModele {
    pub nom: String,
    #[serde(rename = "trait")]
    pub trait_: [u8; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aplat: Option<[u8; 3]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModeleEpisode {
    pub format: String,
    pub version: u32,
    pub nom: String,
    pub cadre: [usize; 2],
    pub cadence: u32,
    pub papier_opaque: bool,
    pub papier: [u8; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aplat: Option<[u8; 3]>,
    pub numerotation_verrouillee: bool,
    pub serie: String,
    pub episode: String,
    pub modele_noms: String,
    pub auteur: String,
    pub copyright: String,
    pub calques: Vec<CalqueModele>,
    pub personnages: Vec<PersonnageModele>,
    pub exports: crate::export::ExportPrefs,
    pub fiche: crate::fiche::Reglages,
    pub liste_plans: crate::listeplans::Reglages,
    pub paquet: crate::paquet::Reglages,
}

/// Le modèle des réglages de `doc` et du poste.
pub fn depuis(doc: &Document, s: &Settings, nom: &str) -> ModeleEpisode {
    let calques = doc
        .panels
        .first()
        .map(|p| {
            p.layers
                .iter()
                .filter(|l| !l.folder && !l.annotation && l.effet.is_none() && l.video.is_none() && l.couleur.is_none() && l.unie.is_none())
                .map(|l| CalqueModele {
                    nom: l.name.clone(),
                    vectoriel: l.kind == LayerKind::Vector,
                    visible: l.visible,
                    opacite: l.opacity,
                    personnage: l.personnage.and_then(|id| doc.personnages.iter().find(|p| p.id == id)).map(|p| p.name.clone()),
                    decor: l.decor,
                })
                .collect()
        })
        .unwrap_or_default();
    let mut paquet = s.paquet.clone();
    paquet.dossier = None;
    ModeleEpisode {
        format: FORMAT.into(),
        version: 1,
        nom: nom.trim().to_string(),
        cadre: doc.frame,
        cadence: doc.fps,
        papier_opaque: doc.background != BackgroundKind::None,
        papier: doc.paper,
        aplat: doc.properties.fond,
        numerotation_verrouillee: doc.numbering_locked,
        serie: doc.properties.serie.clone(),
        episode: doc.properties.episode.clone(),
        modele_noms: doc.properties.modele_noms.clone(),
        auteur: doc.properties.author.clone(),
        copyright: doc.properties.copyright.clone(),
        calques,
        personnages: doc.personnages.iter().map(|p| PersonnageModele { nom: p.name.clone(), trait_: p.line, aplat: p.fill }).collect(),
        exports: s.exports.clone(),
        fiche: s.fiche.clone(),
        liste_plans: s.liste_plans.clone(),
        paquet,
    }
}

impl ModeleEpisode {
    /// Les valeurs du formulaire Nouveau projet : celles d'avant, avec le format du modèle.
    pub fn valeurs(&self, base: &Values) -> Values {
        let mut v = base.clone();
        v.width = self.cadre[0];
        v.height = self.cadre[1];
        v.fps = self.cadence;
        v.paper_on = self.papier_opaque;
        v.paper = self.papier;
        if let Some(a) = self.aplat {
            v.fond = a;
        }
        v.clamp();
        v
    }

    /// L'épisode suivant celui du modèle : « 1 » → « 2 », « 07 » → « 08 » ; un nom qui n'est pas un nombre est gardé.
    pub fn episode_suivant(&self) -> String {
        let e = self.episode.trim();
        match e.parse::<u64>() {
            Ok(n) => {
                let largeur = if e.starts_with('0') { e.len() } else { 0 };
                format!("{:0w$}", n + 1, w = largeur)
            }
            Err(_) => e.to_string(),
        }
    }

    /// Pose les réglages du modèle sur un projet neuf `doc` (un seul plan vide) : jamais un dessin.
    pub fn appliquer_au_projet(&self, doc: &mut Document, episode: &str) {
        doc.numbering_locked = self.numerotation_verrouillee;
        doc.properties.serie = self.serie.clone();
        doc.properties.episode = episode.trim().to_string();
        doc.properties.modele_noms = self.modele_noms.clone();
        doc.properties.author = self.auteur.clone();
        doc.properties.copyright = self.copyright.clone();
        // (Les personnages du modèle, même aucun : le nouvel épisode a ceux de l'ancien, pas ceux d'origine.)
        doc.personnages.clear();
        for p in &self.personnages {
            let id = doc.alloc();
            doc.personnages.push(crate::personnages::Personnage { id, name: p.nom.clone(), line: p.trait_, fill: p.aplat });
        }
        if !self.calques.is_empty() {
            let calques: Vec<Layer> = self
                .calques
                .iter()
                .map(|c| {
                    let mut l = Layer::new(c.nom.clone(), if c.vectoriel { LayerKind::Vector } else { LayerKind::Bitmap });
                    l.visible = c.visible;
                    l.opacity = c.opacite.clamp(0.0, 1.0);
                    l.decor = c.decor;
                    l.personnage = c.personnage.as_ref().and_then(|n| doc.personnages.iter().find(|p| &p.name == n)).map(|p| p.id);
                    l
                })
                .collect();
            if let Some(p) = doc.panels.first_mut() {
                p.layers = calques;
            }
        }
        if doc.numbering_locked {
            doc.assign_numbers();
        }
    }

    /// Les réglages du poste que le modèle porte (exports, fiche, liste des plans, paquet). Le dossier du paquet reste celui du poste.
    pub fn appliquer_aux_reglages(&self, s: &mut Settings) {
        s.exports = self.exports.clone();
        s.fiche = self.fiche.clone();
        s.liste_plans = self.liste_plans.clone();
        let dossier = s.paquet.dossier.clone();
        s.paquet = self.paquet.clone();
        s.paquet.dossier = dossier;
    }

    pub fn en_texte(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }

    pub fn lire(texte: &str) -> Result<ModeleEpisode, String> {
        let m: ModeleEpisode = serde_json::from_str(texte).map_err(|e| format!("modèle d'épisode illisible : {e}"))?;
        if m.format != FORMAT {
            return Err("ce fichier n'est pas un modèle d'épisode de SHOT".into());
        }
        if m.version > 1 {
            return Err("modèle d'épisode d'une version plus récente de SHOT".into());
        }
        Ok(m)
    }
}

/// Le dossier des modèles : `%APPDATA%\SHOT\Modèles`.
pub fn dossier() -> Option<PathBuf> {
    crate::donnees::roaming().map(|d| d.join("Modèles"))
}

fn fichier(nom: &str) -> Option<PathBuf> {
    let stem = crate::nouveau::file_stem(nom);
    (!stem.is_empty()).then(|| dossier().map(|d| d.join(format!("{stem}{EXTENSION}")))).flatten()
}

/// Les noms des modèles enregistrés, par ordre alphabétique.
pub fn lister() -> Vec<String> {
    let Some(d) = dossier() else { return Vec::new() };
    let mut v: Vec<String> = fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.strip_suffix(EXTENSION).map(str::to_string)
        })
        .collect();
    v.sort_by_key(|n| n.to_lowercase());
    v
}

pub fn charger(nom: &str) -> Result<ModeleEpisode, String> {
    let p = fichier(nom).ok_or("nom de modèle vide")?;
    ModeleEpisode::lire(&fs::read_to_string(&p).map_err(|e| format!("{} : {e}", p.display()))?)
}

/// Enregistre le modèle (même nom : il est remplacé, écrit à côté puis renommé).
pub fn enregistrer(m: &ModeleEpisode) -> Result<PathBuf, String> {
    let p = fichier(&m.nom).ok_or("Donne un nom au modèle.")?;
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| format!("{} : {e}", d.display()))?;
    }
    crate::export::write_atomic(&p, m.en_texte().as_bytes())?;
    Ok(p)
}

pub fn supprimer(nom: &str) -> Result<(), String> {
    let p = fichier(nom).ok_or("nom de modèle vide")?;
    fs::remove_file(&p).map_err(|e| format!("{} : {e}", p.display()))
}
