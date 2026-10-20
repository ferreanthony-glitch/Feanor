//! Versions d'un plan : « Nouvelle version du plan » garde le plan tel qu'il est comme version plus
//! ancienne et continue sur une copie ; un petit sélecteur (v1, v2, v3…) fait passer de l'une à l'autre.
//!
//! # Le modèle
//!
//! Un plan a toujours **une version active** : c'est le plan lui-même (`Document::panels`), celui que
//! montrent le montage, la timeline, l'export, les vignettes et les envois. Les **autres versions** sont
//! des plans rangés (`Rangee`) avec leurs propres dessins (des copies : modifier l'une ne touche jamais
//! l'autre ; les tuiles d'un bitmap sont partagées jusqu'à la première retouche, ce qui ne coûte rien en
//! mémoire). Un plan qui n'a jamais eu de version n'a pas d'entrée : c'est la version 1.
//!
//! - **Nouvelle version** : le plan actif reste tel quel (mêmes dessins, mêmes numéros de calques et de
//!   marques : l'historique, les notes de la planche accrochées à ses marques et la sélection ne bougent
//!   pas) ; sa copie est rangée sous le numéro qu'il portait, et il prend le numéro suivant.
//! - **Passer à une version** : le contenu du plan actif et celui de la version choisie s'échangent (tout
//!   sauf le numéro du plan et sa scène : sa place dans le film, ses commentaires, ses transitions, son
//!   contenu rogné restent au plan, pas à une version).
//! - **Restaurer** : une copie de la version choisie devient une nouvelle version active ; l'ancienne
//!   active est rangée. Rien n'est perdu.
//! - **Supprimer** une version rangée (pas l'active).
//!
//! Tout cela est dans `Structure` : une opération = une étape d'annulation nommée (panneau Historique).
//! Les dessins des versions rangées sont de vrais dessins du projet (`dessins/`), comptés par
//! `Document::used_cels` ; ils s'enregistrent avec les autres.
//!
//! # Fichier
//!
//! `plans_versions.json` à côté du manifeste, sans changer `VERSION` : pour chaque plan, le numéro de sa
//! version active, le suivant, et chaque version rangée sous la forme d'une entrée du manifeste (la même
//! que celle d'un plan, `project::snapshot_value`). À l'ouverture, ces entrées passent par la lecture
//! ordinaire des plans (`project::load_manifest`) ; un projet sans versions n'a pas ce fichier. Un fichier
//! illisible ou plus récent n'est jamais effacé (`write_sidecar`), et tant qu'il est là l'enregistrement
//! ne nettoie pas `dessins/` (les dessins des versions qu'on ne sait pas lire y restent).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::document::{relink_copies, CelId, Cues, Document, Layer, LayerKind, Panel};

pub(crate) const FILE: &str = "plans_versions.json";
const FORMAT: &str = "shot-plans-versions";
const VERSION: u32 = 1;
/// Le plus grand nombre de versions d'un plan (un fichier abîmé ne remplit pas la mémoire).
pub const MAX_VERSIONS: usize = 99;

/// Une version rangée : le plan tel qu'il était, sous son numéro.
#[derive(Clone)]
pub struct Rangee {
    pub numero: u32,
    /// Quand cette version a été créée (secondes depuis 1970 ; 0 : inconnue).
    pub date: u64,
    pub panel: Panel,
}

/// Les versions d'un plan.
#[derive(Clone)]
pub struct VersionsDuPlan {
    /// Le numéro de la version active.
    pub actuelle: u32,
    pub date_actuelle: u64,
    /// Le numéro de la prochaine version créée.
    pub prochaine: u32,
    /// Les autres, par numéro.
    pub autres: Vec<Rangee>,
}

impl VersionsDuPlan {
    /// Tous les numéros, dans l'ordre.
    pub fn numeros(&self) -> Vec<u32> {
        let mut n: Vec<u32> = std::iter::once(self.actuelle).chain(self.autres.iter().map(|r| r.numero)).collect();
        n.sort_unstable();
        n
    }

    pub fn nombre(&self) -> usize {
        1 + self.autres.len()
    }

    pub fn rangee(&self, numero: u32) -> Option<&Rangee> {
        self.autres.iter().find(|r| r.numero == numero)
    }
}

/// « V3 » : le nom d'une version, en majuscule, partout (bande, cartes, fiche, planche, comparaison,
/// messages, noms de clips de l'export pour le montage).
pub fn etiquette(numero: u32) -> String {
    format!("V{numero}")
}

/// Échange le contenu de deux plans : tout, sauf leur numéro et leur scène.
fn echanger(a: &mut Panel, b: &mut Panel) {
    std::mem::swap(&mut a.layers, &mut b.layers);
    std::mem::swap(&mut a.length, &mut b.length);
    std::mem::swap(&mut a.action, &mut b.action);
    std::mem::swap(&mut a.dialogue, &mut b.dialogue);
    std::mem::swap(&mut a.notes, &mut b.notes);
    std::mem::swap(&mut a.camera, &mut b.camera);
    std::mem::swap(&mut a.marks, &mut b.marks);
    std::mem::swap(&mut a.cues, &mut b.cues);
    std::mem::swap(&mut a.color, &mut b.color);
}

impl Document {
    /// Les versions du plan `plan`, s'il en a plusieurs.
    pub fn versions_du_plan(&self, plan: u64) -> Option<&VersionsDuPlan> {
        self.versions.get(&plan)
    }

    /// Le numéro de la version active du plan `plan` (1 tant qu'il n'a qu'une version).
    pub fn numero_de_version(&self, plan: u64) -> u32 {
        self.versions.get(&plan).map_or(1, |v| v.actuelle)
    }

    pub fn nombre_de_versions(&self, plan: u64) -> usize {
        self.versions.get(&plan).map_or(1, VersionsDuPlan::nombre)
    }

    /// Les versions des plans qui existent encore (les autres ne s'enregistrent pas).
    pub fn versions_vivantes(&self) -> impl Iterator<Item = (usize, &VersionsDuPlan)> {
        // (Sans aucune version, on ne parcourt pas les plans : cette liste se demande à chaque enregistrement.)
        let n = if self.versions.is_empty() { 0 } else { self.panels.len() };
        self.panels.iter().enumerate().take(n).filter_map(|(i, p)| self.versions.get(&p.id).map(|v| (i, v)))
    }

    /// Les plans rangés des versions des plans qui existent encore.
    pub fn plans_ranges(&self) -> impl Iterator<Item = &Panel> {
        self.versions_vivantes().flat_map(|(_, v)| v.autres.iter().map(|r| &r.panel))
    }

    /// Une copie de `source` avec ses propres dessins, numéros de plan, de marques et de calques neufs.
    pub(crate) fn copie_de(&mut self, source: &Panel) -> Panel {
        let mut panel = source.clone();
        panel.id = self.alloc();
        for m in &mut panel.marks {
            m.id = self.alloc();
        }
        let old_ids: Vec<u64> = panel.layers.iter().map(|l| l.id).collect();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for layer in &mut panel.layers {
            layer.id = crate::document::fresh_layer_id();
            if layer.scene_wide.is_some() {
                continue;
            }
            for f in layer.frames.iter_mut().flatten() {
                *f = match copies.get(f) {
                    Some(&c) => c,
                    None => {
                        let c = self.duplicate_cel(*f);
                        copies.insert(*f, c);
                        c
                    }
                };
            }
        }
        relink_copies(&mut panel.layers, &old_ids, &copies);
        panel
    }

    /// Un plan vide qui a un numéro neuf, de la scène du plan `index`.
    fn coquille(&mut self, index: usize) -> Panel {
        let scene = self.panels[index].scene;
        let mut p = self.make_panel(scene);
        p.layers.clear();
        p.length = 1;
        p.cues = Cues::default();
        p
    }

    /// « Nouvelle version » : le plan `index` reste le plan actif, sa copie est rangée sous le numéro qu'il
    /// portait, et il prend le suivant. Rend le numéro de la nouvelle version active.
    pub fn nouvelle_version(&mut self, index: usize, now: u64) -> Option<u32> {
        let host = self.panels.get(index)?.id;
        if self.nombre_de_versions(host) >= MAX_VERSIONS {
            return None;
        }
        let source = self.panels[index].clone();
        let copie = self.copie_de(&source);
        let v = self.versions.entry(host).or_insert(VersionsDuPlan { actuelle: 1, date_actuelle: 0, prochaine: 2, autres: Vec::new() });
        v.autres.push(Rangee { numero: v.actuelle, date: v.date_actuelle, panel: copie });
        v.autres.sort_by_key(|r| r.numero);
        v.actuelle = v.prochaine;
        v.prochaine += 1;
        v.date_actuelle = now;
        Some(v.actuelle)
    }

    /// « Nouvelle version, en partant d'un plan vide » : comme `nouvelle_version` (le plan tel qu'il est est rangé
    /// sous le numéro qu'il portait), mais la version active qui suit repart à neuf : **même durée**, **même
    /// taille de cadre** (celle du film), **un seul calque de dessin vide** comme dans un plan tout neuf, posé
    /// au-dessus des calques de toute la scène (le décor, qui est à la scène et pas à la version).
    ///
    /// Ce qui est gardé, parce que c'est le cadrage et le texte du plan et pas son dessin : la **caméra** (ses
    /// clés), le **repère 3D**, les textes d'action, de dialogue et de notes, la couleur de repère. Ce qui repart
    /// à zéro : les calques de dessin, les marques d'image, les signets et points d'entrée / sortie.
    /// Rend le numéro de la nouvelle version active.
    pub fn nouvelle_version_vide(&mut self, index: usize, now: u64) -> Option<u32> {
        let numero = self.nouvelle_version(index, now)?;
        let p = &mut self.panels[index];
        let duree = p.frame_count();
        p.layers.retain(|l| l.scene_wide.is_some());
        // (Un calque de scène rangé dans un dossier du plan qui part avec lui n'a plus de dossier.)
        let gardes: Vec<u64> = p.layers.iter().map(|l| l.id).collect();
        for l in &mut p.layers {
            if l.parent.is_some_and(|d| !gardes.contains(&d)) {
                l.parent = None;
            }
        }
        p.layers.push(Layer::new("Dessin", LayerKind::Vector));
        p.length = duree;
        p.marks.clear();
        p.cues = Cues::default();
        Some(numero)
    }

    /// Fait de la version rangée `numero` la version active du plan `index` : les deux échangent leur
    /// contenu. Rend faux si ce numéro n'est pas rangé.
    pub fn changer_de_version(&mut self, index: usize, numero: u32) -> bool {
        let Some(host) = self.panels.get(index).map(|p| p.id) else { return false };
        let Some(v) = self.versions.get_mut(&host) else { return false };
        let Some(k) = v.autres.iter().position(|r| r.numero == numero) else { return false };
        let rangee = &mut v.autres[k];
        echanger(&mut self.panels[index], &mut rangee.panel);
        std::mem::swap(&mut rangee.numero, &mut v.actuelle);
        std::mem::swap(&mut rangee.date, &mut v.date_actuelle);
        v.autres.sort_by_key(|r| r.numero);
        true
    }

    /// « Restaurer » : une copie de la version rangée `numero` devient la version active (sous un numéro
    /// neuf) ; la version active d'avant est rangée telle quelle.
    pub fn restaurer_version(&mut self, index: usize, numero: u32, now: u64) -> Option<u32> {
        let host = self.panels.get(index)?.id;
        let v = self.versions.get(&host)?;
        if v.autres.len() + 1 >= MAX_VERSIONS {
            return None;
        }
        let source = v.rangee(numero)?.panel.clone();
        let mut copie = self.copie_de(&source);
        let mut rangee = self.coquille(index);
        echanger(&mut self.panels[index], &mut rangee);
        echanger(&mut self.panels[index], &mut copie);
        let v = self.versions.get_mut(&host)?;
        v.autres.push(Rangee { numero: v.actuelle, date: v.date_actuelle, panel: rangee });
        v.autres.sort_by_key(|r| r.numero);
        v.actuelle = v.prochaine;
        v.prochaine += 1;
        v.date_actuelle = now;
        Some(v.actuelle)
    }

    /// Supprime la version rangée `numero` du plan `plan` (jamais l'active). Quand il ne reste que
    /// l'active, le plan n'a plus de versions.
    pub fn supprimer_version(&mut self, plan: u64, numero: u32) -> bool {
        let Some(v) = self.versions.get_mut(&plan) else { return false };
        let n = v.autres.len();
        v.autres.retain(|r| r.numero != numero);
        if v.autres.len() == n {
            return false;
        }
        if v.autres.is_empty() {
            self.versions.remove(&plan);
        }
        true
    }

    /// Les versions « non affichées » : toutes les versions rangées (pas les actives) des plans qui existent
    /// encore, ou de ce seul plan (`seulement`). Rend (versions, plans). Les états gardés par les envois
    /// (`versions/`) n'en font pas partie : ils ne sont pas des versions du plan.
    pub fn versions_non_affichees(&self, seulement: Option<u64>) -> (usize, usize) {
        let mut plans = 0;
        let mut versions = 0;
        for (i, v) in self.versions_vivantes() {
            if seulement.is_some_and(|p| p != self.panels[i].id) || v.autres.is_empty() {
                continue;
            }
            plans += 1;
            versions += v.autres.len();
        }
        (versions, plans)
    }

    /// Supprime les versions non affichées (voir `versions_non_affichees`) ; rend (versions, plans) supprimées.
    pub fn supprimer_versions_non_affichees(&mut self, seulement: Option<u64>) -> (usize, usize) {
        let compte = self.versions_non_affichees(seulement);
        let vivants: Vec<u64> = self.panels.iter().map(|p| p.id).collect();
        self.versions.retain(|plan, _| !(vivants.contains(plan) && seulement.is_none_or(|p| p == *plan)));
        compte
    }

    /// Le numéro de la version avant / après l'active (en tournant), pour passer de l'une à l'autre au clavier.
    pub fn version_voisine(&self, plan: u64, sens: i32) -> Option<u32> {
        let v = self.versions.get(&plan)?;
        let numeros = v.numeros();
        let k = numeros.iter().position(|&n| n == v.actuelle)?;
        let n = numeros.len() as i32;
        Some(numeros[(k as i32 + sens).rem_euclid(n) as usize])
    }
}

// ---------------------------------------------------------------------------------- le fichier

#[derive(Serialize, Deserialize)]
pub(crate) struct Fichier {
    format: String,
    version: u32,
    #[serde(default)]
    pub plans: Vec<PlanFichier>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PlanFichier {
    pub plan: u64,
    pub actuelle: u32,
    #[serde(default)]
    pub date_actuelle: u64,
    pub prochaine: u32,
    #[serde(default)]
    pub autres: Vec<VersionFichier>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct VersionFichier {
    pub numero: u32,
    #[serde(default)]
    pub date: u64,
    /// L'entrée du plan dans le format du manifeste (`project::snapshot_value`), dessins cités par leur
    /// numéro dans `dessins/`.
    pub entree: serde_json::Value,
}

/// Les versions d'un plan sous la forme du fichier (un plan partagé les porte dans son propre fichier).
pub(crate) fn plan_fichier(host: u64, v: &VersionsDuPlan) -> Result<PlanFichier, String> {
    let mut autres = Vec::new();
    for r in &v.autres {
        autres.push(VersionFichier { numero: r.numero, date: r.date, entree: crate::project::snapshot_value(&r.panel, &|c| c)? });
    }
    Ok(PlanFichier { plan: host, actuelle: v.actuelle, date_actuelle: v.date_actuelle, prochaine: v.prochaine, autres })
}

/// Un fichier fait des versions de plusieurs plans, nettoyé comme celui qu'on lit (numéros uniques et non nuls…).
pub(crate) fn fichier_de(plans: Vec<PlanFichier>) -> Result<Fichier, String> {
    let texte = serde_json::to_string(&Fichier { format: FORMAT.into(), version: VERSION, plans }).map_err(|e| e.to_string())?;
    parse(&texte)
}

/// Le contenu de `plans_versions.json` ; rien s'il n'y a aucune version rangée (le fichier part, ou, s'il
/// est illisible, reste là).
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let mut plans = Vec::new();
    for (i, v) in doc.versions_vivantes() {
        plans.push(plan_fichier(doc.panels[i].id, v)?);
    }
    plans.retain(|p| !p.autres.is_empty());
    if plans.is_empty() {
        return Ok(None);
    }
    serde_json::to_vec_pretty(&Fichier { format: FORMAT.into(), version: VERSION, plans }).map(Some).map_err(|e| e.to_string())
}

/// Les versions lues dans le texte de `plans_versions.json` : numéros uniques et non nuls, l'active n'est pas
/// aussi rangée, la suivante est au-dessus de toutes, pas plus de `MAX_VERSIONS` par plan.
pub(crate) fn parse(text: &str) -> Result<Fichier, String> {
    let mut f: Fichier = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if f.format != FORMAT {
        return Err("ce n'est pas un fichier de versions de plans".into());
    }
    if f.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", f.version));
    }
    let mut vus = std::collections::HashSet::new();
    f.plans.retain(|p| vus.insert(p.plan));
    for p in &mut f.plans {
        p.actuelle = p.actuelle.max(1);
        let mut numeros = std::collections::HashSet::from([p.actuelle]);
        p.autres.retain(|v| v.numero >= 1 && numeros.insert(v.numero));
        p.autres.truncate(MAX_VERSIONS - 1);
        p.autres.sort_by_key(|v| v.numero);
        p.prochaine = p.prochaine.max(p.autres.iter().map(|v| v.numero).max().unwrap_or(0).max(p.actuelle) + 1);
    }
    Ok(f)
}

/// Le fichier du dossier `dir` : rien s'il n'existe pas, et un avertissement s'il est illisible (le projet
/// s'ouvre sans versions, le fichier reste en place).
pub(crate) fn lire(dir: &Path, warnings: &mut Vec<String>) -> Option<Fichier> {
    let path = dir.join(FILE);
    match std::fs::read(&path) {
        Ok(bytes) => match String::from_utf8(bytes).map_err(|_| "le fichier n'est pas du texte (UTF-8)".to_string()).and_then(|t| parse(&t)) {
            Ok(f) => Some(f),
            Err(e) => {
                warnings.push(format!("{FILE} : {e} ; les versions des plans ne sont pas chargées."));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warnings.push(format!("{} : {e} ; les versions des plans ne sont pas chargées.", path.display()));
            None
        }
    }
}

/// Le fichier existe mais cette version ne sait pas le lire : l'enregistrement n'efface alors aucun dessin de
/// `dessins/` (ceux des versions qu'il décrit y sont encore).
pub(crate) fn illisible(dir: &Path) -> bool {
    match std::fs::read(dir.join(FILE)) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| ()).and_then(|t| parse(&t).map(|_| ()).map_err(|_| ())).is_err(),
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

/// Met en place les versions lues : `cles` dit, pour chaque plan de `rangees` (même ordre), à quel plan et à
/// quel numéro il appartient. Les versions d'un plan qui n'existe plus sont oubliées.
pub(crate) fn installer(doc: &mut Document, lu: Fichier, cles: Vec<(u64, u32)>, rangees: Vec<Panel>, warnings: &mut Vec<String>) {
    let mut par_cle: HashMap<(u64, u32), Panel> = cles.into_iter().zip(rangees).collect();
    let mut out: BTreeMap<u64, VersionsDuPlan> = BTreeMap::new();
    for p in lu.plans {
        if !doc.panels.iter().any(|x| x.id == p.plan) {
            continue;
        }
        let mut autres = Vec::new();
        for v in p.autres {
            match par_cle.remove(&(p.plan, v.numero)) {
                Some(panel) => autres.push(Rangee { numero: v.numero, date: v.date, panel }),
                None => warnings.push(format!("version {} d'un plan : illisible, ignorée.", etiquette(v.numero))),
            }
        }
        if autres.is_empty() {
            continue;
        }
        for r in &autres {
            doc.reserve_id(r.panel.id);
            for m in &r.panel.marks {
                doc.reserve_id(m.id);
            }
        }
        out.insert(p.plan, VersionsDuPlan { actuelle: p.actuelle, date_actuelle: p.date_actuelle, prochaine: p.prochaine, autres });
    }
    doc.versions = out;
}
