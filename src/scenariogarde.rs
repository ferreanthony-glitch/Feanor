//! Le scénario gardé dans le projet (Anthony, 2026-10-30 : « un panneau Scénario : c'est là qu'apparaît un
//! scénario importé dans SHOT, il est lisible à cet endroit »).
//!
//! - Ce qui est gardé : les éléments lus (`scenario::Script` : intitulés de scène, action, personnages, répliques,
//!   parenthèses, transitions, notes) et, pour les plans créés depuis le scénario (bouton du panneau), leurs
//!   **liens** : le numéro stable du plan (son id) et la tranche d'éléments dont il vient. Pas de numéro de rang :
//!   un plan supprimé, réuni ou déplacé ne fausse rien, son lien n'a plus de plan et on va au plus proche.
//! - Rangement : `Document::scenario` (partagé, `Arc` : une étape d'annulation ne le recopie pas) et
//!   `Structure::scenario` : importer et créer les plans s'annulent avec le reste.
//! - Fichier : `scenario.json` à côté du manifeste, sans changer `VERSION` du projet (même principe que
//!   `commentaires.json`) : un ancien projet n'en a pas, il s'ouvre comme avant ; une version qui ne le connaît
//!   pas le laisse là. Lecture tolérante (fichier abîmé ou plus récent : projet ouvert sans scénario, fichier laissé
//!   en place) ; écriture atomique. Un scénario remplacé par un autre n'est jamais effacé : le fichier d'avant passe à
//!   côté (`scenario.ancien-<identifiant>.json`, `mettre_de_cote`).
//! - L'écran : `scenariopanneau.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::scenario::Script;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "scenario.json";
const FORMAT: &str = "shot-scenario";
const VERSION: u32 = 1;
/// Un fichier qui en a plus n'est pas un scénario que SHOT a écrit (une lecture abîmée ne gèle pas l'interface).
pub(crate) const MAX_ELEMENTS: usize = 400_000;
/// Plus gros fichier lu.
const MAX_OCTETS: u64 = 96 * 1024 * 1024;

/// Un plan créé depuis le scénario : son id, et la tranche d'éléments du scénario (de `de` à `a`, compris) dont il vient.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lien {
    pub plan: u64,
    pub de: usize,
    pub a: usize,
}

/// Un scénario gardé dans le projet.
#[derive(Clone, Debug, PartialEq)]
pub struct Gardee {
    /// Identifiant du texte (empreinte du nom et des éléments) : deux imports du même fichier ont le même.
    pub id: u64,
    /// Nom du fichier d'origine.
    pub nom: String,
    pub script: Script,
    /// Un lien par plan créé depuis le scénario, dans l'ordre du scénario.
    pub liens: Vec<Lien>,
    par_plan: HashMap<u64, usize>,
    /// Pour chaque élément, le lien qui le couvre (`NONE` : aucun).
    couvre: Vec<u32>,
}

const NONE: u32 = u32::MAX;

fn empreinte(nom: &str, script: &Script) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut octet = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    };
    nom.bytes().for_each(&mut octet);
    if let Ok(texte) = serde_json::to_vec(script) {
        texte.into_iter().for_each(&mut octet);
    }
    h
}

impl Gardee {
    pub fn new(nom: &str, script: Script, liens: Vec<Lien>) -> Gardee {
        let id = empreinte(nom, &script);
        Gardee::monter(id, nom.to_string(), script, liens)
    }

    fn monter(id: u64, nom: String, script: Script, mut liens: Vec<Lien>) -> Gardee {
        let n = script.elements.len();
        // (Un lien sans élément, à l'envers, ou dont le plan est déjà pris par un lien d'avant est écarté ; la fin est bornée.)
        liens.retain(|l| l.de < n && l.de <= l.a);
        for l in &mut liens {
            l.a = l.a.min(n.saturating_sub(1));
        }
        liens.sort_by_key(|l| (l.de, l.a));
        let mut vus = std::collections::HashSet::new();
        liens.retain(|l| vus.insert(l.plan));
        let par_plan = liens.iter().enumerate().map(|(i, l)| (l.plan, i)).collect();
        let mut couvre = vec![NONE; n];
        for (i, l) in liens.iter().enumerate() {
            for c in &mut couvre[l.de..=l.a] {
                *c = i as u32;
            }
        }
        Gardee { id, nom, script, liens, par_plan, couvre }
    }

    /// Le même scénario avec d'autres liens (les plans créés depuis lui).
    pub fn avec_liens(&self, liens: Vec<Lien>) -> Gardee {
        Gardee::monter(self.id, self.nom.clone(), self.script.clone(), liens)
    }

    /// Le lien (son rang) du plan `id`.
    pub fn lien_du_plan(&self, id: u64) -> Option<usize> {
        self.par_plan.get(&id).copied()
    }

    /// Le lien (son rang) qui couvre l'élément `i`.
    pub fn lien_de_l_element(&self, i: usize) -> Option<usize> {
        self.couvre.get(i).copied().filter(|&c| c != NONE).map(|c| c as usize)
    }

    /// Le lien dont le plan existe encore (`existe`) le plus proche de l'élément `i` : celui qui le couvre s'il a son plan,
    /// sinon le plus proche dans le scénario (à égalité, celui d'avant).
    pub fn lien_vivant_pres_de(&self, i: usize, existe: &dyn Fn(u64) -> bool) -> Option<usize> {
        let ecart = |l: &Lien| if i < l.de { l.de - i } else { i.saturating_sub(l.a) };
        self.liens
            .iter()
            .enumerate()
            .filter(|(_, l)| existe(l.plan))
            .min_by_key(|(k, l)| (ecart(l), if l.de > i { 1 } else { 0 }, *k))
            .map(|(k, _)| k)
    }
}

// ------------------------------------------------------------------------------------ fichier

#[derive(Serialize, Deserialize)]
struct Fichier {
    format: String,
    version: u32,
    #[serde(default)]
    id: u64,
    #[serde(default)]
    nom: String,
    #[serde(default)]
    scenario: Script,
    #[serde(default)]
    liens: Vec<Lien>,
}

/// Le texte de `scenario.json` pour `doc` ; rien s'il n'y a pas de scénario. Un lien dont le plan n'existe plus n'est pas écrit.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let Some(g) = &doc.scenario else { return Ok(None) };
    // (Un plan écarté existe toujours : son lien l'attend, `planannule.rs`.)
    let existe: std::collections::HashSet<u64> = doc.panels.iter().map(|p| p.id).chain(doc.annules.plans.iter().map(|g| g.panel.id)).collect();
    let liens: Vec<Lien> = g.liens.iter().filter(|l| existe.contains(&l.plan)).copied().collect();
    let f = Fichier { format: FORMAT.into(), version: VERSION, id: g.id, nom: g.nom.clone(), scenario: g.script.clone(), liens };
    serde_json::to_vec(&f).map(Some).map_err(|e| e.to_string())
}

/// Le scénario lu dans le texte de `scenario.json`.
pub(crate) fn parse(text: &str) -> Result<Gardee, String> {
    let f: Fichier = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if f.format != FORMAT {
        return Err("ce n'est pas un fichier de scénario".into());
    }
    if f.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", f.version));
    }
    if f.scenario.elements.len() > MAX_ELEMENTS {
        return Err("trop d'éléments pour un scénario".into());
    }
    let id = if f.id == 0 { empreinte(&f.nom, &f.scenario) } else { f.id };
    Ok(Gardee::monter(id, f.nom, f.scenario, f.liens))
}

/// L'identifiant d'un `scenario.json` lisible.
fn id_du_texte(texte: &str) -> Option<u64> {
    parse(texte).ok().map(|g| g.id)
}

/// Le scénario du dossier du projet `dir` (absent : aucun). Un fichier illisible, trop gros ou d'une version plus récente n'empêche pas
/// d'ouvrir le projet : on le dit, sans scénario, et le fichier reste en place.
pub(crate) fn load(dir: &Path, warnings: &mut Vec<String>) -> Option<std::sync::Arc<Gardee>> {
    let path = dir.join(FILE);
    match std::fs::metadata(&path) {
        Ok(m) if m.len() > MAX_OCTETS => {
            warnings.push(format!("{FILE} : trop gros ; le scénario n'est pas chargé (le fichier reste en place)."));
            return None;
        }
        _ => {}
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => match parse(&text) {
            Ok(g) => Some(std::sync::Arc::new(g)),
            Err(e) => {
                warnings.push(format!("{FILE} : {e} ; le scénario n'est pas chargé (le fichier reste en place)."));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warnings.push(format!("{} : {e} ; le scénario n'est pas chargé.", path.display()));
            None
        }
    }
}

/// Avant d'écrire `bytes` (rien : plus de scénario) dans `path` : un `scenario.json` lisible d'un autre scénario n'est jamais effacé,
/// il passe à côté (`scenario.ancien-<identifiant>.json`).
pub(crate) fn mettre_de_cote(path: &Path, bytes: Option<&[u8]>) {
    let Ok(actuel) = std::fs::read(path) else { return };
    let Ok(texte) = String::from_utf8(actuel) else { return };
    let Some(ancien) = id_du_texte(&texte) else { return };
    let nouveau = bytes.and_then(|b| std::str::from_utf8(b).ok()).and_then(id_du_texte);
    if nouveau == Some(ancien) {
        return;
    }
    let Some(dir) = path.parent() else { return };
    for k in 0..100 {
        let nom = if k == 0 { format!("scenario.ancien-{ancien:016x}.json") } else { format!("scenario.ancien-{ancien:016x}-{k}.json") };
        let cible: PathBuf = dir.join(nom);
        if !cible.exists() {
            let _ = std::fs::rename(path, cible);
            return;
        }
    }
}

// ------------------------------------------------------------------------------------ taille

/// La taille du texte du panneau (réglage du poste, retenu).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Taille {
    Petit,
    #[default]
    Moyen,
    Grand,
}

impl Taille {
    pub fn nom(self) -> &'static str {
        match self {
            Taille::Petit => "Petit",
            Taille::Moyen => "Moyen",
            Taille::Grand => "Grand",
        }
    }

    /// Corps du texte, en points.
    pub fn points(self) -> f32 {
        match self {
            Taille::Petit => 11.0,
            Taille::Moyen => 13.0,
            Taille::Grand => 16.0,
        }
    }

    pub fn est_defaut(&self) -> bool {
        *self == Taille::Moyen
    }
}
