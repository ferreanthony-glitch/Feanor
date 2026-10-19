//! Enregistrement des scènes 3D : `scenes3d.json`, à côté du manifeste (comme `commentaires.json`),
//! sans changer `VERSION` du projet.
//!
//! - Absent : aucune scène. Une version de SHOT qui ne connaît pas ce fichier le laisse en place (elle
//!   n'efface que le contenu de `dessins/`, `sons/`, `modeles/`, `textures/`) ; le calque « Fond 3D »
//!   est un calque bitmap ordinaire, que toute version relit.
//! - Écriture : atomique et sur le disque (`write_atomic_synced`), par le même `SaveJob` que le reste
//!   (`project::prepare_save`).
//! - Protection (`project::write_sidecar`) : un fichier écrit par une version plus récente, ou abîmé,
//!   s'ouvre sans ses scènes (avec un avertissement) mais n'est jamais effacé ; si une scène neuve
//!   est à écrire, il est mis de côté sous le nom `scenes3d.json.illisible`.
//! - Pourquoi un fichier à part et pas un champ du manifeste : sans lui, un projet qui contient une
//!   scène 3D s'ouvre dans toutes les versions précédentes (qui ne montrent simplement pas la
//!   scène), au lieu d'être refusé comme le veut tout changement de format.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::scene3d::{Obj3d, PlanScene, Scenes3d};

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "scenes3d.json";
const FORMAT: &str = "shot-scenes3d";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    #[serde(rename = "plans", default)]
    plans: Vec<PlanEntry>,
    #[serde(rename = "decors", default)]
    decors: Vec<DecorEntry>,
}

#[derive(Serialize, Deserialize)]
struct PlanEntry {
    plan: u64,
    scene: PlanScene,
}

#[derive(Serialize, Deserialize)]
struct DecorEntry {
    #[serde(rename = "sequence")]
    sequence: u64,
    #[serde(rename = "objets")]
    objects: Vec<Obj3d>,
}

/// Le contenu du fichier pour `doc` ; rien s'il n'y a aucune scène à garder (le fichier part).
/// Seuls les plans et les séquences qui existent sont écrits, dans l'ordre du film.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let scenes = &doc.scenes3d;
    let mut plans = Vec::new();
    for p in &doc.panels {
        if let Some(scene) = scenes.plans.get(&p.id).filter(|s| **s != PlanScene::default()) {
            plans.push(PlanEntry { plan: p.id, scene: scene.clone() });
        }
    }
    let mut decors = Vec::new();
    for seq in doc.sequence_order() {
        if let Some(objects) = scenes.decors.get(&seq).filter(|d| !d.is_empty()) {
            decors.push(DecorEntry { sequence: seq, objects: objects.clone() });
        }
    }
    if plans.is_empty() && decors.is_empty() {
        return Ok(None);
    }
    let mut sidecar = Sidecar { format: FORMAT.into(), version: VERSION, plans, decors };
    // JSON n'a ni NaN ni infini : une valeur abîmée ne doit pas faire échouer tout l'enregistrement.
    let mut clean = Scenes3d::default();
    for e in &sidecar.plans {
        clean.plans.insert(e.plan, e.scene.clone());
    }
    for e in &sidecar.decors {
        clean.decors.insert(e.sequence, e.objects.clone());
    }
    clean.sanitize();
    for e in &mut sidecar.plans {
        if let Some(s) = clean.plans.remove(&e.plan) {
            e.scene = s;
        }
    }
    for e in &mut sidecar.decors {
        if let Some(d) = clean.decors.remove(&e.sequence) {
            e.objects = d;
        }
    }
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

/// Les scènes lues dans le texte de `scenes3d.json`. Numéros d'objets uniques (un doublon en reçoit
/// un neuf), valeurs ramenées dans des bornes sûres.
pub(crate) fn parse(text: &str) -> Result<Scenes3d, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier de scènes 3D".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let mut scenes = Scenes3d {
        plans: sidecar.plans.into_iter().map(|e| (e.plan, e.scene)).collect::<BTreeMap<_, _>>(),
        decors: sidecar.decors.into_iter().map(|e| (e.sequence, e.objects)).collect(),
        next_id: 0,
        rev: crate::scene3d::next_rev(),
    };
    // Numéros uniques dans tout le projet.
    let mut seen = HashSet::new();
    let mut dup = Vec::new();
    for o in scenes.plans.values().flat_map(|p| p.objects.iter()).chain(scenes.decors.values().flatten()) {
        if o.id == 0 || !seen.insert(o.id) {
            dup.push(o.id);
        }
    }
    scenes.sanitize();
    if !dup.is_empty() {
        let mut taken: HashSet<u64> = HashSet::new();
        let mut next = scenes.next_id;
        let fix = |o: &mut Obj3d, taken: &mut HashSet<u64>, next: &mut u64| {
            if o.id == 0 || !taken.insert(o.id) {
                o.id = *next;
                *next += 1;
                taken.insert(o.id);
            }
        };
        for o in scenes.plans.values_mut().flat_map(|p| p.objects.iter_mut()).chain(scenes.decors.values_mut().flatten()) {
            fix(o, &mut taken, &mut next);
        }
        scenes.next_id = next;
    }
    Ok(scenes)
}
