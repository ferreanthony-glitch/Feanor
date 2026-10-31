//! Envois : l'état de chaque plan au moment où le film part chez le réalisateur (« v2 au réal »), pour
//! savoir ce qui a changé depuis, et pour le revoir (`comparer.rs`).
//!
//! # Ce qu'est un envoi
//!
//! Un nom, une date, et pour **chaque plan** : l'empreinte de son contenu (`empreinte.rs`, calculée sur ce
//! qui se voit et ce qui dure, voir `project::panel_entry`), son titre d'alors, sa version (`planvers.rs`) et
//! son nombre d'images. Rien de plus dans `envois.json` : le fichier reste petit (20 octets par plan et par
//! envoi), il s'écrit et se lit à chaque ouverture.
//!
//! Pour **revoir** un plan tel qu'il était, l'envoi garde aussi un instantané (`versions/envoi-N-xxxx.json`) :
//! l'entrée de chaque plan dans le format du manifeste, ses dessins cités par leur empreinte. Les dessins
//! eux-mêmes sont dans `versions/dessins/<empreinte>.png` (ou `.json` pour un dessin vectoriel,
//! `_fond.png` et `_coupe.png` pour une strate Fond) : **un dessin qui n'a pas changé entre deux envois
//! n'est écrit qu'une fois**, et le premier est recopié du dossier `dessins/` (lien dur quand le disque
//! le permet : pas un octet de plus). Un instantané se relit par l'ouverture ordinaire d'un projet
//! (`project::charger_plans`), avec ses protections.
//!
//! ```text
//! MonFilm/
//!   envois.json                         la liste des envois et l'empreinte de chaque plan
//!   versions/
//!     envoi-3-9f2a41c0.json             l'instantané du 3e envoi (écrit une fois, jamais modifié)
//!     dessins/123456789.png             les dessins des instantanés, par empreinte
//!     dessins/987654321.json
//!     textures/pointe_12.png            les textures des traits texturés de ces dessins
//! ```
//!
//! # Fiabilité (`docs/FIABILITE.md`)
//!
//! - **Même chemin d'enregistrement que le reste** : le fichier se prépare avec `project::prepare_save`, se
//!   met en forme et s'écrit sur l'autre fil (`SaveJob`), atomiquement et sur le disque, après les
//!   instantanés et avant le manifeste. Un instantané est écrit avant que `envois.json` le cite.
//! - **Rien n'est jamais effacé** : `envois.json` illisible ou d'une version plus récente reste en place
//!   (`write_sidecar` le met de côté sous `.illisible` si un fichier neuf doit le remplacer) ; aucun
//!   fichier de `versions/` n'est supprimé, même quand un envoi est retiré de la liste ; un fichier
//!   déjà là n'est pas réécrit.
//! - **Un projet sans envois est un projet d'avant** : ni fichier, ni dossier, ni travail de fond.
//! - **Enregistrer sous** un autre nom recopie `versions/` dans le nouveau dossier.
//! - Le format du projet (`project::VERSION`) ne change pas.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::brushtex::TextureKind;
use crate::document::{CelId, Document, Panel};
use crate::empreinte::{self, Contenu, Donnees};
use crate::project::{self, Tete};

/// Nom du fichier de la liste, dans le dossier du projet.
pub(crate) const FILE: &str = "envois.json";
/// Dossier des instantanés et de leurs dessins.
pub(crate) const DIR: &str = "versions";
const DESSINS: &str = "dessins";
const TEXTURES: &str = "textures";
const FORMAT: &str = "shot-envois";
const FORMAT_INSTANTANE: &str = "shot-envoi";
const VERSION: u32 = 1;
/// Plus long nom d'envoi gardé (un fichier abîmé ne remplit pas la liste de pages de texte).
const MAX_NOM: usize = 120;

// ---------------------------------------------------------------------------------- le modèle

/// Un nombre de 64 bits écrit en hexadécimal (JSON ne garantit pas les entiers au-delà de 2^53).
mod hexa {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{v:016x}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let t = String::deserialize(d)?;
        u64::from_str_radix(t.trim(), 16).map_err(serde::de::Error::custom)
    }
}

fn un() -> u32 {
    1
}

fn est_un(v: &u32) -> bool {
    *v == 1
}

/// Un plan tel qu'il était à un envoi.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanEnvoye {
    /// Le numéro du plan (`Panel::id`) ; il survit aux déplacements et aux renumérotations.
    pub plan: u64,
    /// Son titre d'alors (« Plan 3 ») : un plan supprimé depuis se nomme encore.
    #[serde(default)]
    pub titre: String,
    /// L'empreinte de son contenu (`project::digest_bytes`).
    #[serde(with = "hexa")]
    pub empreinte: u64,
    /// Le numéro de sa version quand l'envoi est parti (`planvers.rs`) ; 1 sans versions.
    #[serde(default = "un", skip_serializing_if = "est_un")]
    pub version: u32,
    #[serde(default)]
    pub images: usize,
    /// Le nom du plan d'alors, selon le modèle de noms (« SQ01_PL012A ») ; vide pour un envoi d'avant « petits livrables » (`renumerotation.rs`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nom: String,
    /// Le nom ou numéro de sa séquence d'alors (« 2 », « Poursuite ») ; vide : inconnu.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sequence: String,
}

/// La variante choisie d'une séquence quand l'envoi est parti (`variantes.rs`) : seulement pour les séquences qui en
/// ont plusieurs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VarianteEnvoyee {
    /// Le numéro de la séquence.
    pub sequence: u64,
    pub numero: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nom: String,
}

/// Un envoi : un nom, une date, l'état de chaque plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envoi {
    pub id: u64,
    pub nom: String,
    /// La date de l'envoi, en secondes depuis 1970 (le jour choisi, à midi, temps universel).
    pub date: u64,
    /// Quand SHOT l'a noté (secondes depuis 1970).
    #[serde(default)]
    pub note_le: u64,
    /// Le fichier de l'instantané, dans `versions/` (vide : il n'y en a pas, pas de comparaison possible).
    #[serde(default)]
    pub fichier: String,
    pub plans: Vec<PlanEnvoye>,
    /// La variante choisie de chaque séquence qui en a plusieurs (`variantes.rs`) ; vide : aucune séquence n'en avait.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variantes: Vec<VarianteEnvoyee>,
}

impl Envoi {
    pub fn plan(&self, id: u64) -> Option<&PlanEnvoye> {
        self.plans.iter().find(|p| p.plan == id)
    }

    /// « v2 au réal, 20/10/2026 ».
    pub fn libelle(&self) -> String {
        format!("{}, {}", self.nom, date_courte(self.date))
    }
}

/// Un dessin d'un instantané, avant d'être écrit.
pub struct DessinInstantane {
    pub empreinte: u64,
    pub donnees: Donnees,
    /// D'où il vient dans le projet (numéro et version du dessin) : si le fichier de `dessins/` est
    /// celui de cette version, il est recopié au lieu d'être encodé de nouveau.
    pub source: Option<(CelId, u64)>,
}

/// Un instantané qui attend d'être écrit (jusqu'au prochain enregistrement réussi, qui le libère).
pub struct Instantane {
    pub id: u64,
    pub fichier: String,
    /// Le fichier tel qu'il sera écrit.
    pub octets: Vec<u8>,
    pub dessins: Vec<DessinInstantane>,
    pub textures: Vec<(TextureKind, u32)>,
    /// Les images collées (`imagevec.rs`) que citent ces dessins : écrites dans `versions/images/` si elles manquent.
    pub images: Vec<u64>,
}

/// Les envois d'un projet.
#[derive(Default)]
pub struct Envois {
    pub list: Vec<Envoi>,
    /// Les instantanés pas encore écrits dans le dossier (`project::prepare_save` les prend).
    pub(crate) pending: Vec<Arc<Instantane>>,
    /// Le dossier du projet où `versions/` est à jour (ouverture, dernier enregistrement) : un
    /// « Enregistrer sous » y reprend les instantanés déjà écrits.
    pub dossier: Option<PathBuf>,
}

impl Envois {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Le dernier envoi : le dernier marqué (les numéros ne font que croître). Pas le plus tardif par sa date :
    /// la date se tape, elle peut être celle d'un jour qui vient, et un envoi marqué aujourd'hui est toujours
    /// l'état le plus récent du film.
    pub fn dernier(&self) -> Option<&Envoi> {
        self.list.iter().max_by_key(|e| e.id)
    }

    pub fn trouve(&self, id: u64) -> Option<&Envoi> {
        self.list.iter().find(|e| e.id == id)
    }

    pub fn prochain_numero(&self) -> u64 {
        self.list.iter().map(|e| e.id).max().unwrap_or(0) + 1
    }

    /// Un nouvel envoi et son instantané (à écrire au prochain enregistrement).
    pub fn ajouter(&mut self, envoi: Envoi, instantane: Instantane) {
        self.list.push(envoi);
        self.pending.push(Arc::new(instantane));
    }

    /// Retire un envoi de la liste. Ses fichiers de `versions/` restent (jamais d'effacement) ; ceux qui
    /// n'étaient pas encore écrits sont oubliés.
    pub fn retirer(&mut self, id: u64) -> bool {
        let n = self.list.len();
        self.list.retain(|e| e.id != id);
        self.pending.retain(|p| p.id != id);
        self.list.len() != n
    }

    /// Les instantanés `ids` sont écrits : la mémoire qu'ils tenaient est rendue.
    pub fn liberer(&mut self, ids: &[u64]) {
        self.pending.retain(|p| !ids.contains(&p.id));
    }
}

/// Où en est un plan par rapport au dernier envoi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etat {
    /// Son contenu est celui de l'envoi.
    Inchange,
    /// Son contenu a changé depuis.
    Modifie,
    /// Il n'y était pas : le plan est neuf (ou rangé depuis ailleurs).
    Nouveau,
}

/// L'état d'un plan d'empreinte `empreinte` au regard de `envoi`.
pub fn etat_du_plan(envoi: &Envoi, plan: u64, empreinte: u64) -> Etat {
    match envoi.plan(plan) {
        None => Etat::Nouveau,
        Some(p) if p.empreinte == empreinte => Etat::Inchange,
        Some(_) => Etat::Modifie,
    }
}

/// Comme `etat_du_plan`, pour un plan du film d'aujourd'hui : un plan copié par une variante de séquence
/// (`variantes.rs`) se compare au plan dont il vient quand l'envoi ne le connaît pas lui-même.
pub fn etat_du_plan_dans(doc: &Document, envoi: &Envoi, plan: u64, empreinte: u64) -> Etat {
    match doc.plan_dans_envoi(envoi, plan) {
        Some(connu) => etat_du_plan(envoi, connu, empreinte),
        None => Etat::Nouveau,
    }
}

// ---------------------------------------------------------------------------------- les dates

/// Jours depuis le 1er janvier 1970 de la date (`y`, `m`, `j`), calendrier grégorien.
pub fn jours_depuis_1970(y: i64, m: u32, j: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + j as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// « 20/10/2026 » d'une date en secondes depuis 1970 (temps universel).
pub fn date_courte(secs: u64) -> String {
    let (y, m, d) = crate::projetui::civil_from_days((secs / 86_400) as i64);
    format!("{d:02}/{m:02}/{y}")
}

/// Une date tapée (« 20/10/2026 », « 20-10-2026 », « 20.10.2026 », ou « 2026-10-20 »), à midi, en
/// secondes depuis 1970 ; rien si ce n'est pas une date.
pub fn lire_date(texte: &str) -> Option<u64> {
    let parts: Vec<&str> = texte.trim().split(['/', '-', '.', ' ']).filter(|s| !s.is_empty()).collect();
    let [a, b, c] = parts.as_slice() else { return None };
    let (j, m, y): (u32, u32, i64) = if a.len() == 4 { (c.parse().ok()?, b.parse().ok()?, a.parse().ok()?) } else { (a.parse().ok()?, b.parse().ok()?, c.parse().ok()?) };
    if !(1..=12).contains(&m) || !(1970..=9999).contains(&y) {
        return None;
    }
    let days_in_month = [31, if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][(m - 1) as usize];
    if j == 0 || j > days_in_month {
        return None;
    }
    Some(jours_depuis_1970(y, m, j) as u64 * 86_400 + 43_200)
}

// ---------------------------------------------------------------------------------- le fichier

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    #[serde(default)]
    envois: Vec<Envoi>,
}

/// Le contenu de `envois.json` pour `doc` ; rien s'il n'y a aucun envoi (le fichier n'existe pas, ou, s'il
/// est illisible, reste là : `project::write_sidecar`).
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    if doc.envois.list.is_empty() {
        return Ok(None);
    }
    let mut envois = doc.envois.list.clone();
    envois.sort_by_key(|e| e.id);
    serde_json::to_vec_pretty(&Sidecar { format: FORMAT.into(), version: VERSION, envois }).map(Some).map_err(|e| e.to_string())
}

/// Le nom d'un fichier de `versions/` qui ne sort pas du dossier.
fn nom_sur(nom: &str) -> bool {
    !nom.is_empty() && Path::new(nom).components().count() == 1 && !nom.contains(['/', '\\']) && nom != "." && nom != ".."
}

/// Les envois lus dans `envois.json`. Numéros uniques et non nuls, noms bornés, un nom de fichier qui sort
/// de `versions/` est oublié (l'envoi reste, sans instantané), un plan en double n'est gardé qu'une fois.
pub(crate) fn parse(text: &str) -> Result<Vec<Envoi>, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier d'envois".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let mut next = sidecar.envois.iter().map(|e| e.id).max().unwrap_or(0) + 1;
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(sidecar.envois.len());
    for mut e in sidecar.envois {
        if e.id == 0 || !seen.insert(e.id) {
            e.id = next;
            next += 1;
            seen.insert(e.id);
        }
        e.nom = e.nom.chars().take(MAX_NOM).collect();
        if !nom_sur(&e.fichier) {
            e.fichier.clear();
        }
        let mut plans = HashSet::new();
        e.plans.retain(|p| plans.insert(p.plan));
        out.push(e);
    }
    Ok(out)
}

/// Les envois du projet de `dir`, lus dans `envois.json` (absent : aucun). Un fichier illisible ou d'une
/// version plus récente n'empêche pas d'ouvrir le projet : on le dit, sans envois, et il reste en place.
pub(crate) fn load(dir: &Path, warnings: &mut Vec<String>) -> Envois {
    let path = dir.join(FILE);
    let list = match std::fs::read(&path) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| "le fichier n'est pas du texte (UTF-8)".to_string()).and_then(|t| parse(&t)).unwrap_or_else(|e| {
            warnings.push(format!("{FILE} : {e} ; les envois ne sont pas chargés."));
            Vec::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            warnings.push(format!("{} : {e} ; les envois ne sont pas chargés.", path.display()));
            Vec::new()
        }
    };
    Envois { list, pending: Vec::new(), dossier: Some(dir.to_path_buf()) }
}

// ---------------------------------------------------------------------------------- l'instantané

#[derive(Serialize, Deserialize)]
struct FichierInstantane {
    format: String,
    version: u32,
    id: u64,
    tete: Tete,
    plans: Vec<PlanInstantane>,
}

#[derive(Serialize, Deserialize)]
struct PlanInstantane {
    plan: u64,
    scene: u64,
    /// L'entrée du plan dans le format du manifeste, ses dessins cités par empreinte.
    entree: serde_json::Value,
    /// Les dessins qu'elle cite.
    dessins: Vec<u64>,
}

/// Le plan `plan` de `envoi`, relu dans son instantané : un petit projet à un plan, dont les dessins sont
/// ceux de `dossier/versions/dessins/`. Lecture complète (disque et rendu des traits) : sur un autre fil.
pub fn charger_plan(dossier: &Path, envoi: &Envoi, plan: u64) -> Result<project::Loaded, String> {
    if envoi.fichier.is_empty() {
        return Err(format!("l'envoi « {} » n'a pas d'instantané : rien à comparer.", envoi.nom));
    }
    let chemin = dossier.join(DIR).join(&envoi.fichier);
    let octets = std::fs::read(&chemin).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("l'instantané de l'envoi « {} » est introuvable ({}) ; le dossier « {DIR} » du projet a-t-il été déplacé ?", envoi.nom, chemin.display())
        } else {
            format!("l'instantané de l'envoi « {} » est illisible : {e}", envoi.nom)
        }
    })?;
    let texte = String::from_utf8(octets).map_err(|_| format!("l'instantané de l'envoi « {} » n'est pas du texte (UTF-8)", envoi.nom))?;
    let f: FichierInstantane = serde_json::from_str(texte.trim_start_matches('\u{feff}')).map_err(|e| format!("l'instantané de l'envoi « {} » est illisible : {e}", envoi.nom))?;
    if f.format != FORMAT_INSTANTANE {
        return Err(format!("« {} » n'est pas un instantané d'envoi", envoi.fichier));
    }
    if f.version > VERSION {
        return Err(format!("l'instantané de l'envoi « {} » vient d'une version plus récente de SHOT", envoi.nom));
    }
    let p = f.plans.into_iter().find(|p| p.plan == plan).ok_or_else(|| format!("le plan n'était pas dans l'envoi « {} »", envoi.nom))?;
    project::charger_plans(&dossier.join(DIR), &f.tete, p.scene, vec![p.entree], false)
}

// ---------------------------------------------------------------------------------- le calcul

/// Un plan à examiner : sa copie, son titre et sa version.
pub struct PlanCopie {
    pub panel: Panel,
    pub titre: String,
    pub version: u32,
    /// Le nom du plan selon le modèle de noms, et sa séquence, au moment de la copie (gardés dans l'envoi).
    pub nom: String,
    pub sequence: String,
}

/// Un dessin copié, avec la version qu'il avait.
pub struct Copie {
    pub version: u64,
    pub donnees: Donnees,
}

/// Ce que le fil de fond calcule (`construire`).
pub struct Demande {
    pub plans: Vec<PlanCopie>,
    /// Les dessins à hacher ou à écrire. (Un dessin absent ici est cherché dans `connus`.)
    pub cels: HashMap<CelId, Copie>,
    /// Les empreintes déjà calculées : numéro du dessin → (version, empreinte).
    pub connus: HashMap<CelId, (u64, u64)>,
    pub tete: Tete,
    /// Pour un envoi : son numéro, son nom, sa date, la date où SHOT le note, et de quoi nommer son fichier.
    pub envoi: Option<NouvelEnvoi>,
}

pub struct NouvelEnvoi {
    pub id: u64,
    pub nom: String,
    pub date: u64,
    pub note_le: u64,
    pub unique: u32,
}

pub struct Resultat {
    /// Numéro du dessin → (version, empreinte), pour tous ceux qui ont été hachés.
    pub empreintes: HashMap<CelId, (u64, u64)>,
    /// Plan → empreinte de son contenu.
    pub plans: Vec<(u64, u64)>,
    pub envoi: Option<(Envoi, Instantane)>,
}

/// L'empreinte d'un dessin qu'on ne trouve pas : ce que dit son numéro (le plan reste comparable).
fn empreinte_absente(id: CelId) -> u64 {
    empreinte::de_octets(&id.to_le_bytes())
}

/// Les textures que citent les traits d'un dessin vectoriel.
fn textures_de(d: &Donnees) -> Vec<(TextureKind, u32)> {
    let mut out = Vec::new();
    if let Contenu::Vectoriel(v) = &d.contenu {
        for e in &v.elements {
            if let crate::vector::Element::Stroke { brush: Some(b), .. } = e {
                out.extend(b.tip.map(|k| (TextureKind::Tip, k)));
                out.extend(b.paper.map(|k| (TextureKind::Paper, k)));
            }
        }
    }
    out
}

/// Hache les dessins de `demande`, puis chaque plan ; pour un envoi, bâtit aussi son instantané. Rien ici ne
/// touche au document : tout vient de copies (`Panel`, tuiles partagées), c'est un travail de fil de fond.
pub fn construire(demande: Demande) -> Result<Resultat, String> {
    let Demande { plans, cels, connus, tete, envoi } = demande;
    // Les dessins : l'empreinte connue si la version n'a pas bougé, sinon on la calcule (en parallèle).
    let ids: Vec<CelId> = cels.keys().copied().collect();
    let hachees: Vec<u64> = crate::parallele::par_map(&ids, 8, |_, id| {
        let c = &cels[id];
        match connus.get(id) {
            Some((v, fp)) if *v == c.version => *fp,
            _ => c.donnees.empreinte(),
        }
    });
    let mut empreintes: HashMap<CelId, (u64, u64)> = HashMap::new();
    for (id, fp) in ids.iter().zip(&hachees) {
        empreintes.insert(*id, (cels[id].version, *fp));
    }
    let fp_de = |id: CelId| -> CelId {
        empreintes.get(&id).or_else(|| connus.get(&id)).map(|(_, fp)| *fp).unwrap_or_else(|| empreinte_absente(id))
    };
    let mut resultats = Vec::with_capacity(plans.len());
    let mut instantane_plans = Vec::new();
    for p in &plans {
        let octets = project::digest_bytes(&p.panel, &fp_de);
        resultats.push((p.panel.id, empreinte::de_octets(&octets)));
        if envoi.is_some() {
            let entree = project::snapshot_value(&p.panel, &fp_de)?;
            let mut dessins: Vec<u64> = empreinte::dessins_du_plan(&p.panel).into_iter().map(fp_de).collect();
            dessins.sort_unstable();
            dessins.dedup();
            instantane_plans.push(PlanInstantane { plan: p.panel.id, scene: p.panel.scene, entree, dessins });
        }
    }
    let Some(nouveau) = envoi else {
        return Ok(Resultat { empreintes, plans: resultats, envoi: None });
    };
    // Les dessins à écrire : un par empreinte (deux dessins identiques ne s'écrivent qu'une fois).
    let mut vus = HashSet::new();
    let mut dessins = Vec::new();
    let mut textures: Vec<(TextureKind, u32)> = Vec::new();
    let mut cels = cels;
    let mut ordre: Vec<CelId> = ids;
    ordre.sort_unstable();
    for id in ordre {
        let (version, fp) = empreintes[&id];
        if !vus.insert(fp) {
            continue;
        }
        if let Some(c) = cels.remove(&id) {
            textures.extend(textures_de(&c.donnees));
            dessins.push(DessinInstantane { empreinte: fp, donnees: c.donnees, source: Some((id, version)) });
        }
    }
    textures.sort_unstable();
    textures.dedup();
    let images: Vec<u64> = dessins
        .iter()
        .filter_map(|d| if let Contenu::Vectoriel(v) = &d.donnees.contenu { Some(crate::imagevec::citees(&v.elements)) } else { None })
        .flatten()
        .collect::<std::collections::BTreeSet<u64>>()
        .into_iter()
        .collect();
    let fichier = format!("envoi-{}-{:08x}.json", nouveau.id, nouveau.unique);
    let octets = serde_json::to_vec_pretty(&FichierInstantane { format: FORMAT_INSTANTANE.into(), version: VERSION, id: nouveau.id, tete, plans: instantane_plans })
        .map_err(|e| e.to_string())?;
    let envoi = Envoi {
        id: nouveau.id,
        nom: nouveau.nom.chars().take(MAX_NOM).collect(),
        date: nouveau.date,
        note_le: nouveau.note_le,
        fichier: fichier.clone(),
        plans: plans
            .iter()
            .zip(&resultats)
            .map(|(p, (_, e))| PlanEnvoye { plan: p.panel.id, titre: p.titre.clone(), empreinte: *e, version: p.version.max(1), images: p.panel.frame_count(), nom: p.nom.clone(), sequence: p.sequence.clone() })
            .collect(),
        // (Les variantes choisies : notées par l'appelant, qui a le document.)
        variantes: Vec::new(),
    };
    Ok(Resultat { empreintes, plans: resultats, envoi: Some((envoi, Instantane { id: nouveau.id, fichier, octets, dessins, textures, images })) })
}

// ---------------------------------------------------------------------------------- l'écriture

/// Un dessin d'instantané dans `versions/dessins/` : son fichier, et sa strate Fond s'il en a une. Ce qui est
/// déjà là n'est pas réécrit. `reuse` : le fichier de `dessins/` qui a ce contenu (recopié plutôt
/// qu'encodé).
fn ecrire_dessin(dessins: &Path, d: &DessinInstantane, reuse: Option<&PathBuf>) -> Result<usize, String> {
    let mut ecrits = 0;
    let fichier = match &d.donnees.contenu {
        Contenu::Vectoriel(_) => dessins.join(format!("{}.json", d.empreinte)),
        Contenu::Bitmap(_) => dessins.join(format!("{}.png", d.empreinte)),
    };
    if !fichier.exists() {
        match (&d.donnees.contenu, reuse) {
            (Contenu::Bitmap(_), Some(from)) if from.exists() => copier(from, &fichier)?,
            (Contenu::Bitmap(r), _) => project::write_atomic(&fichier, &crate::io::encode_png(&r.to_rgba(), r.width, r.height)?)?,
            (Contenu::Vectoriel(v), _) => project::write_atomic(&fichier, &serde_json::to_vec(v).map_err(|e| e.to_string())?)?,
        }
        ecrits += 1;
    }
    if let Some((peinture, coupe)) = &d.donnees.fond {
        for (suffixe, r) in [("fond", peinture), ("coupe", coupe)] {
            let f = dessins.join(format!("{}_{suffixe}.png", d.empreinte));
            if !f.exists() {
                project::write_atomic(&f, &crate::io::encode_png(&r.to_rgba(), r.width, r.height)?)?;
                ecrits += 1;
            }
        }
    }
    Ok(ecrits)
}

/// Copie `from` en `to` sans jamais laisser un fichier à moitié écrit : un lien dur si le disque le permet
/// (aucun octet de plus), sinon une copie atomique.
fn copier(from: &Path, to: &Path) -> Result<(), String> {
    if std::fs::hard_link(from, to).is_ok() {
        return Ok(());
    }
    let octets = std::fs::read(from).map_err(|e| format!("{} : {e}", from.display()))?;
    project::write_atomic(to, &octets)
}

/// Recopie les fichiers de `src` (un dossier `versions/`) qui manquent dans `dest`.
fn copier_arbre(src: &Path, dest: &Path) -> Result<usize, String> {
    let mut n = 0;
    let Ok(entries) = std::fs::read_dir(src) else { return Ok(0) };
    std::fs::create_dir_all(dest).map_err(|e| format!("{} : {e}", dest.display()))?;
    for entry in entries.flatten() {
        let (from, to) = (entry.path(), dest.join(entry.file_name()));
        if from.is_dir() {
            n += copier_arbre(&from, &to)?;
        } else if !to.exists() && from.extension().is_none_or(|e| e != "tmp") {
            copier(&from, &to)?;
            n += 1;
        }
    }
    Ok(n)
}

/// L'écriture de `SaveJob` : les instantanés en attente, dessins puis textures puis fichier de
/// l'instantané (l'instantané est écrit en dernier : il ne cite jamais un dessin absent), après avoir
/// recopié `versions/` d'un ancien dossier (« Enregistrer sous »). Rend le nombre de fichiers écrits.
pub(crate) fn ecrire(dir: &Path, snaps: &[Arc<Instantane>], reuse: &HashMap<u64, PathBuf>, copy_from: Option<&Path>) -> Result<usize, String> {
    if snaps.is_empty() && copy_from.is_none() {
        return Ok(0);
    }
    let racine = dir.join(DIR);
    let dessins = racine.join(DESSINS);
    std::fs::create_dir_all(&dessins).map_err(|e| format!("{} : {e}", dessins.display()))?;
    let mut n = 0;
    if let Some(src) = copy_from {
        n += copier_arbre(src, &racine)?;
    }
    for s in snaps {
        for d in &s.dessins {
            n += ecrire_dessin(&dessins, d, reuse.get(&d.empreinte))?;
        }
        if !s.textures.is_empty() {
            std::fs::create_dir_all(racine.join(TEXTURES)).map_err(|e| format!("{} : {e}", racine.display()))?;
        }
        for &(kind, key) in &s.textures {
            let f = racine.join(TEXTURES).join(format!("{}_{key}.png", kind.file_word()));
            if f.exists() {
                continue;
            }
            if let Some(img) = crate::brushtex::shared_image(kind, key) {
                project::write_atomic(&f, &project::texture_png(&img)?)?;
                n += 1;
            }
        }
        for &key in &s.images {
            let f = crate::imagevec::fichier(&racine, key);
            if f.exists() {
                continue;
            }
            if let Some(px) = crate::imagevec::obtenir(key) {
                std::fs::create_dir_all(racine.join(crate::imagevec::DOSSIER)).map_err(|e| format!("{} : {e}", racine.display()))?;
                project::write_atomic(&f, &crate::imagevec::png(&px)?)?;
                n += 1;
            }
        }
        let f = racine.join(&s.fichier);
        if !f.exists() {
            project::write_atomic_synced(&f, &s.octets)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Les dessins de `dessins/` qui ont déjà le contenu d'un dessin d'instantané : (empreinte → fichier). Calculé
/// sur le fil de l'interface avec `written`, qui dit quelle version de chaque dessin est dans le dossier.
pub(crate) fn dessins_reutilisables(
    snaps: &[Arc<Instantane>],
    written: &HashMap<CelId, u64>,
    existe: &dyn Fn(CelId) -> Option<PathBuf>,
) -> HashMap<u64, PathBuf> {
    let mut out = HashMap::new();
    for s in snaps {
        for d in &s.dessins {
            if let (Some((id, version)), Contenu::Bitmap(_)) = (d.source, &d.donnees.contenu) {
                if written.get(&id) == Some(&version) {
                    if let Some(p) = existe(id) {
                        out.insert(d.empreinte, p);
                    }
                }
            }
        }
    }
    out
}
