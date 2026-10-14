//! Projet en dossier (format 3).
//!
//! ```text
//! MonFilm/
//!   MonFilm.pelure        manifeste JSON lisible : réglages, séquences > scènes > panneaux > calques
//!   dessins/<id>.png      dessins bitmap
//!   dessins/<id>.json     dessins vectoriels
//! ```
//!
//! Un enregistrement n'écrit que les dessins modifiés depuis le précédent, puis le manifeste.
//! Chaque fichier est écrit à côté puis renommé : un plantage en cours d'enregistrement laisse
//! l'ancienne version intacte. Les dessins qui ne servent plus sont effacés en dernier.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document::{fresh_layer_id, Cel, CelId, Document, Layer, LayerKind, Panel, Scene, Sequence, Structure};
use crate::io::encode_png;
use crate::raster::Raster;
use crate::vector::VectorDrawing;

pub const FORMAT: &str = "pelure-projet";
pub const VERSION: u32 = 3;
const DRAWINGS: &str = "dessins";

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    largeur: usize,
    hauteur: usize,
    cadence: u32,
    papier: [u8; 3],
    sequences: Vec<SequenceEntry>,
}

#[derive(Serialize, Deserialize)]
struct SequenceEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    scenes: Vec<SceneEntry>,
}

#[derive(Serialize, Deserialize)]
struct SceneEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    panneaux: Vec<PanelEntry>,
}

#[derive(Serialize, Deserialize)]
struct PanelEntry {
    id: u64,
    #[serde(default)]
    duree: usize,
    #[serde(default)]
    action: String,
    #[serde(default)]
    dialogue: String,
    #[serde(default)]
    notes: String,
    calques: Vec<LayerEntry>,
}

#[derive(Serialize, Deserialize)]
struct LayerEntry {
    nom: String,
    /// « vectoriel » ou « bitmap ».
    #[serde(rename = "type")]
    kind: String,
    #[serde(default = "yes")]
    visible: bool,
    #[serde(default = "one")]
    opacite: f32,
    /// Les expositions dans l'ordre : quel dessin (fichier `dessins/<id>.*`, ou aucun) et pendant
    /// combien d'images.
    expositions: Vec<Exposure>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
struct Exposure {
    dessin: Option<u64>,
    images: usize,
}

fn to_exposures(frames: &[Option<u64>]) -> Vec<Exposure> {
    let mut out: Vec<Exposure> = Vec::new();
    for &f in frames {
        match out.last_mut() {
            Some(last) if last.dessin == f => last.images += 1,
            _ => out.push(Exposure { dessin: f, images: 1 }),
        }
    }
    out
}

fn from_exposures(runs: &[Exposure]) -> Vec<Option<u64>> {
    runs.iter()
        .flat_map(|r| std::iter::repeat_n(r.dessin, r.images.min(1_000_000)))
        .collect()
}

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

/// Dossier et manifeste correspondant au nom choisi dans la boîte « Enregistrer sous ».
/// « C:\Films\Bus.pelure » → dossier « C:\Films\Bus\ », manifeste « C:\Films\Bus\Bus.pelure »
/// (ou directement dans « C:\Films\Bus\ » si l'on a choisi un fichier qui s'y trouve déjà).
pub fn target_for(chosen: &Path) -> (PathBuf, PathBuf) {
    let stem = chosen
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "projet".into());
    let parent = chosen.parent().map(Path::to_path_buf).unwrap_or_default();
    let dir = if parent.file_name().is_some_and(|n| n.to_string_lossy() == stem) {
        parent
    } else {
        parent.join(&stem)
    };
    let manifest = dir.join(format!("{stem}.pelure"));
    (dir, manifest)
}

/// Écrire là est-il sans danger ? (pas d'écrasement d'un autre projet ni de données étrangères)
pub fn check_target(manifest: &Path, current: Option<&Path>) -> Result<(), String> {
    if current == Some(manifest) {
        return Ok(());
    }
    let dir = manifest.parent().unwrap_or(Path::new("."));
    if manifest.exists() {
        return Err(format!(
            "un projet existe déjà à cet endroit ({}). Choisis un autre nom.",
            manifest.display()
        ));
    }
    if dir.join(DRAWINGS).exists() {
        return Err(format!(
            "le dossier {} contient déjà un dossier « {DRAWINGS} ». Choisis un autre nom.",
            dir.display()
        ));
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes).map_err(|e| format!("{} : {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{} : {e}", path.display()))
}

fn drawing_path(dir: &Path, id: CelId, vector: bool) -> PathBuf {
    dir.join(DRAWINGS).join(format!("{id}.{}", if vector { "json" } else { "png" }))
}

pub struct SaveReport {
    pub written: usize,
    pub removed: usize,
}

/// Enregistre le projet. `written` retient la version de chaque dessin déjà écrit dans ce dossier.
pub fn save(doc: &Document, manifest_path: &Path, written: &mut HashMap<CelId, u64>) -> Result<SaveReport, String> {
    let dir = manifest_path.parent().ok_or("chemin de projet invalide")?;
    fs::create_dir_all(dir.join(DRAWINGS)).map_err(|e| format!("{} : {e}", dir.display()))?;
    let used = doc.used_cels();
    let mut count = 0;
    for &id in &used {
        let Some(cel) = doc.cels.get(&id) else { continue };
        let path = drawing_path(dir, id, cel.vector.is_some());
        if written.get(&id) == Some(&cel.version) && path.exists() {
            continue;
        }
        let bytes = match &cel.vector {
            Some(v) => serde_json::to_vec(v).map_err(|e| e.to_string())?,
            None => encode_png(&cel.raster.to_rgba(), cel.raster.width, cel.raster.height)?,
        };
        write_atomic(&path, &bytes)?;
        written.insert(id, cel.version);
        count += 1;
    }
    let manifest = manifest_of(doc);
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    write_atomic(manifest_path, &json)?;
    let removed = remove_unused(dir, &used, written);
    Ok(SaveReport { written: count, removed })
}

/// Efface les dessins du dossier qui ne servent plus (seulement les fichiers « <nombre>.png/json »).
fn remove_unused(dir: &Path, used: &BTreeSet<CelId>, written: &mut HashMap<CelId, u64>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(DRAWINGS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let id = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<CelId>().ok());
        if let (Some(id), "png" | "json") = (id, ext) {
            if !used.contains(&id) && fs::remove_file(&path).is_ok() {
                written.remove(&id);
                removed += 1;
            }
        }
    }
    removed
}

fn manifest_of(doc: &Document) -> Manifest {
    let mut sequences: Vec<SequenceEntry> = Vec::new();
    for (i, panel) in doc.panels.iter().enumerate() {
        let seq = doc.sequence_of(i);
        if sequences.last().is_none_or(|s| s.id != seq) {
            let nom = doc.sequences.iter().find(|s| s.id == seq).map(|s| s.name.clone()).unwrap_or_default();
            sequences.push(SequenceEntry {
                id: seq,
                nom,
                scenes: Vec::new(),
            });
        }
        let scenes = &mut sequences.last_mut().expect("séquence ajoutée juste avant").scenes;
        if scenes.last().is_none_or(|s| s.id != panel.scene) {
            let nom = doc.scene(panel.scene).map(|s| s.name.clone()).unwrap_or_default();
            scenes.push(SceneEntry {
                id: panel.scene,
                nom,
                panneaux: Vec::new(),
            });
        }
        scenes.last_mut().expect("scène ajoutée juste avant").panneaux.push(PanelEntry {
            id: panel.id,
            duree: panel.length,
            action: panel.action.clone(),
            dialogue: panel.dialogue.clone(),
            notes: panel.notes.clone(),
            calques: panel
                .layers
                .iter()
                .map(|l| LayerEntry {
                    nom: l.name.clone(),
                    kind: match l.kind {
                        LayerKind::Vector => "vectoriel".into(),
                        LayerKind::Bitmap => "bitmap".into(),
                    },
                    visible: l.visible,
                    opacite: l.opacity,
                    expositions: to_exposures(&l.frames),
                })
                .collect(),
        });
    }
    Manifest {
        format: FORMAT.into(),
        version: VERSION,
        largeur: doc.width,
        hauteur: doc.height,
        cadence: doc.fps,
        papier: doc.paper,
        sequences,
    }
}

pub struct Loaded {
    pub doc: Document,
    /// Dessins présents dans le dossier, à la version chargée (rien à réécrire au prochain enregistrement).
    pub written: HashMap<CelId, u64>,
    /// Problèmes non bloquants (dessin manquant ou illisible…).
    pub warnings: Vec<String>,
}

pub fn load(manifest_path: &Path) -> Result<Loaded, String> {
    let text = fs::read_to_string(manifest_path).map_err(|e| e.to_string())?;
    let manifest: Manifest =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("manifeste illisible : {e}"))?;
    if manifest.format != FORMAT {
        return Err("ce fichier n'est pas un projet Pelure".into());
    }
    if manifest.version > VERSION {
        return Err(format!(
            "projet créé par une version plus récente de Pelure (format {})",
            manifest.version
        ));
    }
    let dir = manifest_path.parent().unwrap_or(Path::new("."));
    let (w, h) = (manifest.largeur, manifest.hauteur);
    if w == 0 || h == 0 {
        return Err("taille d'image invalide dans le manifeste".into());
    }
    let mut warnings = Vec::new();
    let mut cels: HashMap<CelId, Cel> = HashMap::new();
    let mut missing: BTreeSet<CelId> = BTreeSet::new();
    let referenced: BTreeSet<CelId> = manifest
        .sequences
        .iter()
        .flat_map(|s| &s.scenes)
        .flat_map(|s| &s.panneaux)
        .flat_map(|p| &p.calques)
        .flat_map(|l| l.expositions.iter().filter_map(|r| r.dessin))
        .collect();
    for id in referenced {
        match load_drawing(dir, id, w, h) {
            Ok(cel) => {
                cels.insert(id, cel);
            }
            Err(e) => {
                warnings.push(e);
                missing.insert(id);
            }
        }
    }

    let mut structure = Structure {
        sequences: Vec::new(),
        scenes: Vec::new(),
        panels: Vec::new(),
    };
    for seq in manifest.sequences {
        structure.sequences.push(Sequence {
            id: seq.id,
            name: seq.nom,
        });
        for scene in seq.scenes {
            structure.scenes.push(Scene {
                id: scene.id,
                name: scene.nom,
                sequence: seq.id,
            });
            for p in scene.panneaux {
                let mut layers: Vec<Layer> = p
                    .calques
                    .into_iter()
                    .map(|l| Layer {
                        id: fresh_layer_id(),
                        name: l.nom,
                        kind: if l.kind == "bitmap" {
                            LayerKind::Bitmap
                        } else {
                            LayerKind::Vector
                        },
                        visible: l.visible,
                        opacity: l.opacite.clamp(0.0, 1.0),
                        frames: from_exposures(&l.expositions)
                            .into_iter()
                            .map(|f| f.filter(|id| !missing.contains(id)))
                            .collect(),
                    })
                    .collect();
                if layers.is_empty() {
                    layers.push(Layer::new("Dessin", LayerKind::Vector));
                }
                structure.panels.push(Panel {
                    id: p.id,
                    scene: scene.id,
                    length: p.duree,
                    layers,
                    action: p.action,
                    dialogue: p.dialogue,
                    notes: p.notes,
                });
            }
        }
    }
    if structure.panels.is_empty() {
        return Err("le projet ne contient aucun panneau".into());
    }
    let written = cels.keys().map(|&id| (id, 0)).collect();
    let doc = Document::from_parts(w, h, manifest.cadence.max(1), manifest.papier, structure, cels);
    Ok(Loaded { doc, written, warnings })
}

fn load_drawing(dir: &Path, id: CelId, w: usize, h: usize) -> Result<Cel, String> {
    let vector_path = drawing_path(dir, id, true);
    if vector_path.exists() {
        let text = fs::read_to_string(&vector_path).map_err(|e| format!("dessin {id} illisible : {e}"))?;
        let drawing: VectorDrawing =
            serde_json::from_str(&text).map_err(|e| format!("dessin {id} illisible : {e}"))?;
        return Ok(Cel {
            raster: drawing.render(w, h),
            vector: Some(drawing),
            version: 0,
        });
    }
    let png_path = drawing_path(dir, id, false);
    if png_path.exists() {
        let img = image::open(&png_path)
            .map_err(|e| format!("dessin {id} illisible : {e}"))?
            .to_rgba8();
        if img.width() as usize != w || img.height() as usize != h {
            return Err(format!("dessin {id} : taille différente de celle du projet, ignoré"));
        }
        return Ok(Cel {
            raster: Raster::from_rgba(w, h, img.as_raw()),
            vector: None,
            version: 0,
        });
    }
    Err(format!("dessin {id} introuvable dans « {DRAWINGS} »"))
}
