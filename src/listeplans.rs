//! La liste des plans pour le suivi de production (pipeline, étape 1) : une ligne par plan, en CSV ou en JSON, pour un logiciel de suivi
//! (importer des plans dans Kitsu, ou un tableur de production).
//!
//! Deux profils d'en-têtes :
//! - **Kitsu** (logiciel libre de suivi de production) : `Episode, Sequence, Name, Description, Nb Frames, Frame In, Frame Out, FPS`. Source : la
//!   documentation de Kitsu (cgwire) décrit l'import CSV de plans avec ces colonnes : `Sequence` et `Name` obligatoires, `Episode` obligatoire pour une
//!   série (TV show), `Description`, `Nb Frames`, `Frame In`, `Frame Out`, `FPS` facultatives ; `Nb Frames` = `Frame Out` − `Frame In` + 1. **À confirmer**
//!   sur une vraie instance : la page officielle de l'import n'a pas pu être lue (404), les noms viennent de pages qui la citent ; l'aperçu de
//!   l'import de Kitsu permet de rattacher les colonnes une à une si un nom diffère. Détails : `docs/INTERFACE.md` partie 87.
//! - **Générique** : des en-têtes anglais clairs, avec en plus le dialogue, la durée en secondes, le statut (vide), la version, la vignette et
//!   « modifié depuis l'envoi ».
//!
//! L'image d'entrée est réglable (1001 comme en studio, 1, ou 0) ; l'image de sortie est **incluse** : `sortie = entrée + images − 1`.
//! Les nombres s'écrivent avec un point ; le CSV est en UTF-8, sans marque d'ordre des octets (sauf le profil générique en point-virgule, que
//! l'Excel français lit mieux avec elle).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::envois::Etat;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Profil {
    #[default]
    Kitsu,
    Generique,
}

impl Profil {
    pub fn nom(self) -> &'static str {
        match self {
            Profil::Kitsu => "Kitsu",
            Profil::Generique => "Générique",
        }
    }

    pub fn en_tetes(self) -> &'static [&'static str] {
        match self {
            Profil::Kitsu => &["Episode", "Sequence", "Name", "Description", "Nb Frames", "Frame In", "Frame Out", "FPS"],
            Profil::Generique => &["episode", "sequence", "shot", "description", "dialogue", "frames", "frame_in", "frame_out", "duration_seconds", "fps", "status", "version", "thumbnail", "modified_since_send"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Format {
    #[default]
    Csv,
    Json,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Json => "json",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Separateur {
    #[default]
    Virgule,
    PointVirgule,
}

impl Separateur {
    pub fn car(self) -> char {
        match self {
            Separateur::Virgule => ',',
            Separateur::PointVirgule => ';',
        }
    }
}

/// Les choix de la fenêtre, retenus d'une séance à l'autre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reglages {
    pub profil: Profil,
    pub format: Format,
    pub separateur: Separateur,
    /// Numéro de la première image du premier plan (1001, 1 ou 0).
    pub depart: i64,
    /// Écrire aussi une vignette par plan, dans un dossier « vignettes » à côté.
    pub vignettes: bool,
}

impl Default for Reglages {
    fn default() -> Self {
        Reglages { profil: Profil::Kitsu, format: Format::Csv, separateur: Separateur::Virgule, depart: 1001, vignettes: false }
    }
}

/// Une ligne : un plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Ligne {
    pub panel: usize,
    pub episode: String,
    pub sequence: String,
    pub plan: String,
    pub description: String,
    pub dialogue: String,
    pub images: usize,
    pub entree: i64,
    pub sortie: i64,
    pub secondes: f64,
    pub cadence: u32,
    pub statut: String,
    pub version: u32,
    /// Chemin relatif de la vignette (vide : sans vignette).
    pub vignette: String,
    pub modifie: String,
}

/// Les lignes des plans `plans` (rangs de panneaux), dans l'ordre du film. `depart` : numéro de la première image du premier plan **du film**
/// (un plan garde son numéro même si l'on n'écrit que quelques plans). `etats` : l'état des plans depuis l'envoi de référence.
pub fn construire(doc: &Document, plans: &[usize], depart: i64, etats: Option<&HashMap<u64, Etat>>, vignettes: bool) -> Vec<Ligne> {
    let mut debuts = Vec::with_capacity(doc.panels.len());
    let mut at = 0usize;
    for p in &doc.panels {
        debuts.push(at);
        at += p.frame_count();
    }
    let mut rangs: Vec<usize> = plans.iter().copied().filter(|&p| p < doc.panels.len()).collect();
    rangs.sort_unstable();
    rangs.dedup();
    let fps = doc.fps.max(1);
    let mut vus: Vec<String> = Vec::new();
    rangs
        .into_iter()
        .map(|p| {
            let panel = &doc.panels[p];
            let images = panel.frame_count();
            let entree = depart + debuts[p] as i64;
            let nom = doc.nom_de_plan(p);
            let vignette = if vignettes { format!("vignettes/{}.png", vignette_nom(&nom, &mut vus)) } else { String::new() };
            Ligne {
                panel: p,
                episode: doc.properties.episode.trim().to_string(),
                sequence: match doc.sequences.iter().find(|s| s.id == doc.sequence_of(p)) {
                    Some(s) if !s.name.trim().is_empty() => s.name.trim().to_string(),
                    _ => doc.sequence_ref(doc.sequence_of(p)),
                },
                plan: nom,
                description: panel.action.trim().to_string(),
                dialogue: panel.dialogue.trim().to_string(),
                images,
                entree,
                sortie: entree + images as i64 - 1,
                secondes: (images as f64 / fps as f64 * 1000.0).round() / 1000.0,
                cadence: fps,
                statut: String::new(),
                version: doc.numero_de_version(panel.id),
                vignette,
                modifie: match etats.and_then(|m| m.get(&panel.id)) {
                    Some(Etat::Modifie) => "Modifié".into(),
                    Some(Etat::Nouveau) => "Ajouté".into(),
                    _ => String::new(),
                },
            }
        })
        .collect()
}

/// Le nom du fichier de vignette d'un plan (sans extension) : le nom du plan en caractères sûrs, jamais deux fois le même (`vus`).
/// (Doit rester d'accord avec `exportstills::items`, qui nomme les images de la même façon.)
fn vignette_nom(nom: &str, vus: &mut Vec<String>) -> String {
    let base = crate::editorial::file_safe(nom);
    let mut name = base.clone();
    let mut n = 1;
    while vus.contains(&name) {
        n += 1;
        name = format!("{base}_{n}");
    }
    vus.push(name.clone());
    name
}

fn plat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Les cellules d'une ligne, dans l'ordre des en-têtes du profil.
pub fn cellules(l: &Ligne, profil: Profil) -> Vec<String> {
    match profil {
        Profil::Kitsu => vec![l.episode.clone(), l.sequence.clone(), l.plan.clone(), plat(&l.description), l.images.to_string(), l.entree.to_string(), l.sortie.to_string(), l.cadence.to_string()],
        Profil::Generique => vec![
            l.episode.clone(),
            l.sequence.clone(),
            l.plan.clone(),
            plat(&l.description),
            plat(&l.dialogue),
            l.images.to_string(),
            l.entree.to_string(),
            l.sortie.to_string(),
            format!("{}", l.secondes),
            l.cadence.to_string(),
            l.statut.clone(),
            l.version.to_string(),
            l.vignette.clone(),
            l.modifie.clone(),
        ],
    }
}

/// Une cellule CSV : entre guillemets si elle contient le séparateur, un guillemet ou un saut de ligne (les guillemets sont doublés).
pub fn cellule_csv(s: &str, sep: char) -> String {
    if s.contains(sep) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Le texte du CSV (UTF-8, lignes terminées par CRLF comme le veut la RFC 4180).
pub fn csv(lignes: &[Ligne], profil: Profil, sep: Separateur) -> String {
    let s = sep.car();
    let mut out = String::new();
    if profil == Profil::Generique && sep == Separateur::PointVirgule {
        out.push('\u{feff}');
    }
    let join = |v: Vec<String>| v.iter().map(|c| cellule_csv(c, s)).collect::<Vec<_>>().join(&s.to_string());
    out.push_str(&join(profil.en_tetes().iter().map(|h| h.to_string()).collect()));
    out.push_str("\r\n");
    for l in lignes {
        out.push_str(&join(cellules(l, profil)));
        out.push_str("\r\n");
    }
    out
}

/// Le texte du JSON : un objet qui décrit la production, puis `shots`, un objet par plan aux clés des en-têtes du profil (les nombres sont des
/// nombres).
pub fn json(doc: &Document, lignes: &[Ligne], profil: Profil, depart: i64) -> String {
    use serde_json::{json, Map, Value};
    let heads = profil.en_tetes();
    let shots: Vec<Value> = lignes
        .iter()
        .map(|l| {
            let mut m = Map::new();
            let vals: Vec<Value> = match profil {
                Profil::Kitsu => vec![json!(l.episode), json!(l.sequence), json!(l.plan), json!(plat(&l.description)), json!(l.images), json!(l.entree), json!(l.sortie), json!(l.cadence)],
                Profil::Generique => vec![
                    json!(l.episode),
                    json!(l.sequence),
                    json!(l.plan),
                    json!(l.description),
                    json!(l.dialogue),
                    json!(l.images),
                    json!(l.entree),
                    json!(l.sortie),
                    json!(l.secondes),
                    json!(l.cadence),
                    json!(l.statut),
                    json!(l.version),
                    json!(l.vignette),
                    json!(l.modifie),
                ],
            };
            for (h, v) in heads.iter().zip(vals) {
                m.insert((*h).to_string(), v);
            }
            Value::Object(m)
        })
        .collect();
    let racine = json!({
        "format": "shot-liste-des-plans",
        "profil": profil.nom(),
        "projet": if doc.properties.serie.trim().is_empty() { doc.properties.title.trim() } else { doc.properties.serie.trim() },
        "episode": doc.properties.episode.trim(),
        "cadence": doc.fps,
        "image_de_depart": depart,
        "shots": shots,
    });
    serde_json::to_string_pretty(&racine).unwrap_or_default() + "\n"
}

/// Le contenu du fichier pour `reglages`.
pub fn texte(doc: &Document, lignes: &[Ligne], r: &Reglages) -> String {
    match r.format {
        Format::Csv => csv(lignes, r.profil, r.separateur),
        Format::Json => json(doc, lignes, r.profil, r.depart),
    }
}

/// Le nom du fichier proposé : `<projet>_EP02_plans.csv`.
pub fn nom_de_fichier(doc: &Document, base: &str, format: Format) -> String {
    let ep = doc.properties.episode.trim();
    let ep = if ep.is_empty() { String::new() } else { format!("_EP{}", crate::editorial::file_safe(ep)) };
    format!("{}{ep}_plans.{}", crate::editorial::file_safe(base), format.extension())
}
