//! Projet en dossier (format 3).
//!
//! ```text
//! MonFilm/
//!   MonFilm.pelure        manifeste JSON lisible : réglages, séquences > scènes > panneaux > calques,
//!                         pistes son
//!   dessins/<id>.png      dessins bitmap
//!   dessins/<id>.json     dessins vectoriels
//!   sons/<id>.<ext>       fichiers son importés, copiés tels quels
//!   modeles/<id>.pelure3d modèles 3D de repère (format 5), maillage soudé (voir `model3d`)
//! ```
//!
//! Un enregistrement n'écrit que les dessins modifiés depuis le précédent, puis le manifeste.
//! Chaque fichier est écrit à côté puis renommé : un plantage en cours d'enregistrement laisse
//! l'ancienne version intacte. Les dessins qui ne servent plus sont effacés en dernier.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::document::{
    fresh_layer_id, AudioTrack, Cel, CelId, Document, Layer, LayerKind, LayerTransform, Panel, Ref3d, Scene, Sequence,
    Sound, SoundClip, Structure,
};
use crate::camera::{Camera, CameraKey};
use crate::guides::Guide;
use crate::io::encode_png;
use crate::model3d::Model3d;
use crate::view3d::{Cam3d, MAX_ELEVATION};
use crate::raster::Raster;
use crate::vector::VectorDrawing;

pub const FORMAT: &str = "pelure-projet";
/// 4 (phase 2b) : la feuille de dessin est plus grande que le cadre de la caméra (`cadre`).
/// 5 (phase 2d) : modèles 3D (`modeles`) et repère 3D par panneau (`repere3d`).
pub const VERSION: u32 = 5;
const DRAWINGS: &str = "dessins";
const SOUNDS: &str = "sons";
/// Modèles 3D de repère (2d), dans le format de Pelure.
const MODELS: &str = "modeles";
/// Formats de son acceptés à l'import.
pub const SOUND_EXTS: [&str; 5] = ["wav", "mp3", "flac", "ogg", "oga"];

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    /// Taille de la feuille de dessin (et de chaque fichier de `dessins/`).
    largeur: usize,
    hauteur: usize,
    /// Cadre de la caméra = taille de l'image finale. Absent (format 3) : la feuille était le
    /// cadre ; elle est alors doublée à l'ouverture, dessin au centre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cadre: Option<[usize; 2]>,
    cadence: u32,
    papier: [u8; 3],
    sequences: Vec<SequenceEntry>,
    /// Fichiers son, copiés tels quels dans `sons/` (ajouté en 1b ; absent = aucun son).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sons: Vec<SoundEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pistes_son: Vec<TrackEntry>,
    /// Guides de perspective (ajouté en 1c).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    guides: Vec<Guide>,
    /// Modèles 3D de repère (format 5).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    modeles: Vec<ModelEntry>,
}

#[derive(Serialize, Deserialize)]
struct ModelEntry {
    id: u64,
    /// Nom du fichier d'origine (affiché).
    #[serde(default)]
    nom: String,
}

/// Repère 3D d'un panneau (format 5) : modèle, caméra en orbite, affichage.
#[derive(Serialize, Deserialize)]
struct Ref3dEntry {
    modele: u64,
    cible: [f32; 3],
    azimut: f32,
    elevation: f32,
    distance: f32,
    focale: f32,
    #[serde(default = "yes")]
    visible: bool,
    #[serde(default = "yes")]
    aretes_cachees: bool,
    #[serde(default = "default_ref_color")]
    couleur: [u8; 3],
    #[serde(default = "one")]
    opacite: f32,
}

fn default_ref_color() -> [u8; 3] {
    [40, 150, 230]
}

#[derive(Serialize, Deserialize)]
struct SoundEntry {
    id: u64,
    /// Nom du fichier d'origine (affiché).
    #[serde(default)]
    nom: String,
    /// Fichier dans `sons/`.
    fichier: String,
}

#[derive(Serialize, Deserialize)]
struct TrackEntry {
    id: u64,
    #[serde(default)]
    nom: String,
    #[serde(default)]
    muette: bool,
    #[serde(default = "one")]
    volume: f32,
    #[serde(default)]
    extraits: Vec<ClipEntry>,
}

/// Un extrait de son, en images du montage.
#[derive(Serialize, Deserialize)]
struct ClipEntry {
    id: u64,
    son: u64,
    debut: usize,
    /// Images sautées au début du son.
    #[serde(default)]
    decalage: usize,
    duree: usize,
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
    /// Images clés de la caméra (absent : caméra au centre de la feuille).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    camera: Vec<CameraEntry>,
    /// Repère 3D (format 5 ; absent : aucun).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repere3d: Option<Ref3dEntry>,
}

#[derive(Serialize, Deserialize)]
struct TransformEntry {
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default = "one")]
    echelle: f32,
    #[serde(default)]
    rotation: f32,
}

/// Une image clé de caméra : à l'image `image` du panneau, centre (`x`, `y`) en pixels de la
/// feuille, zoom, rotation en degrés.
#[derive(Serialize, Deserialize)]
struct CameraEntry {
    image: usize,
    x: f32,
    y: f32,
    #[serde(default = "one")]
    zoom: f32,
    #[serde(default)]
    rotation: f32,
    /// Mouvement adouci à cette clé.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    doux: bool,
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
    /// Ajouté en 1e (absent = non verrouillé).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    verrouille: bool,
    /// Transformation du calque (2c ; absent = aucune) : décalage en pixels de la feuille,
    /// échelle, rotation en degrés, autour du centre de la feuille.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transformation: Option<TransformEntry>,
    /// Profondeur multiplan (2c ; absent = 0, plan de référence).
    #[serde(default, skip_serializing_if = "is_zero")]
    profondeur: f32,
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

fn is_zero(v: &f32) -> bool {
    *v == 0.0
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
    for sub in [DRAWINGS, SOUNDS, MODELS] {
        if dir.join(sub).exists() {
            return Err(format!(
                "le dossier {} contient déjà un dossier « {sub} ». Choisis un autre nom.",
                dir.display()
            ));
        }
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

fn sound_file(sound: &Sound) -> String {
    format!("{}.{}", sound.id, sound.ext)
}

/// Sons utilisés par au moins un extrait.
fn used_sounds(doc: &Document) -> BTreeSet<u64> {
    doc.audio_tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(|c| c.sound))
        .filter(|id| doc.sounds.contains_key(id))
        .collect()
}

pub struct SaveReport {
    pub written: usize,
    pub removed: usize,
}

/// Enregistre le projet. `written` retient la version de chaque dessin déjà écrit dans ce dossier.
/// En cas d'échec, `written` est vidé : le prochain enregistrement réécrira tout.
pub fn save(doc: &Document, manifest_path: &Path, written: &mut HashMap<CelId, u64>) -> Result<SaveReport, String> {
    let job = prepare_save(doc, manifest_path, written, None)?;
    job.run().inspect_err(|_| written.clear())
}

/// Un dessin à écrire : déjà converti (vectoriel), copié tel quel (bitmap, converti en PNG à
/// l'écriture, la partie lente), ou à recopier d'un autre dossier où il est déjà écrit.
enum DrawingData {
    Bytes(Vec<u8>),
    Raster(Raster),
    CopyFrom(PathBuf),
}

/// Dossier d'un projet déjà enregistré, et versions des dessins qui y sont écrits : un dessin
/// inchangé depuis s'y recopie au lieu d'être réencodé.
pub struct Reuse<'a> {
    pub manifest_path: &'a Path,
    pub written: &'a HashMap<CelId, u64>,
}

/// Ce qu'un enregistrement doit écrire, préparé sur le fil de l'interface (copie des dessins
/// modifiés seulement) : l'écriture, elle, peut se faire sur un autre fil (sauvegarde de
/// secours) pendant qu'on continue à dessiner.
pub struct SaveJob {
    manifest_path: PathBuf,
    drawings: Vec<(PathBuf, DrawingData)>,
    sounds: Vec<(PathBuf, Arc<Vec<u8>>)>,
    models: Vec<(PathBuf, Arc<Model3d>)>,
    manifest: Vec<u8>,
    used: BTreeSet<CelId>,
    used_sounds: BTreeSet<u64>,
    used_models: BTreeSet<u64>,
}

/// Prépare l'enregistrement de `doc` dans `manifest_path`. `written` est mis à jour comme si
/// l'écriture allait réussir (à vider si elle échoue).
pub fn prepare_save(
    doc: &Document,
    manifest_path: &Path,
    written: &mut HashMap<CelId, u64>,
    reuse: Option<Reuse>,
) -> Result<SaveJob, String> {
    let dir = manifest_path.parent().ok_or("chemin de projet invalide")?;
    let reuse_dir = reuse.as_ref().and_then(|r| r.manifest_path.parent());
    let used = doc.used_cels();
    written.retain(|id, _| used.contains(id));
    let mut drawings = Vec::new();
    for &id in &used {
        let Some(cel) = doc.cels.get(&id) else { continue };
        let vector = cel.vector.is_some();
        let path = drawing_path(dir, id, vector);
        if written.get(&id) == Some(&cel.version) && path.exists() {
            continue;
        }
        let elsewhere = reuse_dir
            .zip(reuse.as_ref())
            .filter(|(_, r)| r.written.get(&id) == Some(&cel.version))
            .map(|(d, _)| drawing_path(d, id, vector))
            .filter(|p| p.exists());
        let data = match (&cel.vector, elsewhere) {
            (_, Some(from)) => DrawingData::CopyFrom(from),
            (Some(v), None) => DrawingData::Bytes(serde_json::to_vec(v).map_err(|e| e.to_string())?),
            (None, None) => DrawingData::Raster(cel.raster.clone()),
        };
        drawings.push((path, data));
        written.insert(id, cel.version);
    }
    // Un son ne change jamais : on ne l'écrit que s'il n'est pas déjà dans le dossier.
    let used_sounds = used_sounds(doc);
    let sounds = used_sounds
        .iter()
        .map(|id| &doc.sounds[id])
        .map(|s| (dir.join(SOUNDS).join(sound_file(s)), s.bytes.clone()))
        .filter(|(path, _)| !path.exists())
        .collect();
    // Un modèle 3D non plus ; il est converti à l'écriture (sur l'autre fil), s'il manque.
    let used_models = used_models(doc);
    let models = used_models
        .iter()
        .map(|id| (model_file(dir, *id), doc.models[id].clone()))
        .filter(|(path, _)| !path.exists())
        .collect();
    let manifest = serde_json::to_vec_pretty(&manifest_of(doc)).map_err(|e| e.to_string())?;
    Ok(SaveJob {
        manifest_path: manifest_path.to_path_buf(),
        drawings,
        sounds,
        models,
        manifest,
        used,
        used_sounds,
        used_models,
    })
}

impl SaveJob {
    /// Écrit les dessins, puis les sons, puis le manifeste (en dernier : un manifeste ne cite
    /// jamais un dessin pas encore écrit), puis efface ce qui ne sert plus.
    pub fn run(self) -> Result<SaveReport, String> {
        let dir = self.manifest_path.parent().ok_or("chemin de projet invalide")?;
        fs::create_dir_all(dir.join(DRAWINGS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        let mut count = 0;
        for (path, data) in &self.drawings {
            let bytes = match data {
                DrawingData::Bytes(b) => b.clone(),
                DrawingData::Raster(r) => encode_png(&r.to_rgba(), r.width, r.height)?,
                DrawingData::CopyFrom(from) => fs::read(from).map_err(|e| format!("{} : {e}", from.display()))?,
            };
            write_atomic(path, &bytes)?;
            count += 1;
        }
        if !self.sounds.is_empty() {
            fs::create_dir_all(dir.join(SOUNDS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, bytes) in &self.sounds {
            write_atomic(path, bytes)?;
            count += 1;
        }
        if !self.models.is_empty() {
            fs::create_dir_all(dir.join(MODELS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, model) in &self.models {
            write_atomic(path, &crate::model3d::to_bytes(&model.mesh))?;
            count += 1;
        }
        write_atomic(&self.manifest_path, &self.manifest)?;
        let removed = remove_unused(dir, &self.used)
            + remove_unused_sounds(dir, &self.used_sounds)
            + remove_unused_models(dir, &self.used_models);
        Ok(SaveReport { written: count, removed })
    }
}

/// Efface les modèles 3D du dossier qui ne servent plus (seulement « <nombre>.pelure3d »).
fn remove_unused_models(dir: &Path, used: &BTreeSet<u64>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(MODELS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let id = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u64>().ok());
        if let (Some(id), "pelure3d") = (id, ext) {
            if !used.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// Efface les sons du dossier qui ne servent plus (seulement les fichiers « <nombre>.<ext> »).
fn remove_unused_sounds(dir: &Path, used: &BTreeSet<u64>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(SOUNDS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let id = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u64>().ok());
        if let Some(id) = id {
            if SOUND_EXTS.contains(&ext.as_str()) && !used.contains(&id) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// Efface les dessins du dossier qui ne servent plus (seulement les fichiers « <nombre>.png/json »).
fn remove_unused(dir: &Path, used: &BTreeSet<CelId>) -> usize {
    let Ok(entries) = fs::read_dir(dir.join(DRAWINGS)) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let id = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<CelId>().ok());
        if let (Some(id), "png" | "json") = (id, ext) {
            if !used.contains(&id) && fs::remove_file(&path).is_ok() {
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
                    verrouille: l.locked,
                    transformation: (!l.transform.is_identity()).then(|| TransformEntry {
                        x: l.transform.x,
                        y: l.transform.y,
                        echelle: l.transform.scale,
                        rotation: l.transform.angle,
                    }),
                    profondeur: l.depth,
                    expositions: to_exposures(&l.frames),
                })
                .collect(),
            camera: panel
                .camera
                .iter()
                .map(|k| CameraEntry {
                    image: k.frame,
                    x: k.cam.x,
                    y: k.cam.y,
                    zoom: k.cam.zoom,
                    rotation: k.cam.angle,
                    doux: k.smooth,
                })
                .collect(),
            repere3d: panel.ref3d.as_ref().map(|r| Ref3dEntry {
                modele: r.model,
                cible: r.cam.target,
                azimut: r.cam.azimuth,
                elevation: r.cam.elevation,
                distance: r.cam.distance,
                focale: r.cam.focal,
                visible: r.visible,
                aretes_cachees: r.hidden_lines,
                couleur: r.color,
                opacite: r.opacity,
            }),
        });
    }
    let used = used_sounds(doc);
    Manifest {
        format: FORMAT.into(),
        version: VERSION,
        largeur: doc.width,
        hauteur: doc.height,
        cadre: Some(doc.frame),
        cadence: doc.fps,
        papier: doc.paper,
        sequences,
        sons: used
            .iter()
            .map(|id| {
                let s = &doc.sounds[id];
                SoundEntry {
                    id: s.id,
                    nom: s.name.clone(),
                    fichier: sound_file(s),
                }
            })
            .collect(),
        pistes_son: doc
            .audio_tracks
            .iter()
            .map(|t| TrackEntry {
                id: t.id,
                nom: t.name.clone(),
                muette: t.muted,
                volume: t.volume,
                extraits: t
                    .clips
                    .iter()
                    .filter(|c| used.contains(&c.sound))
                    .map(|c| ClipEntry {
                        id: c.id,
                        son: c.sound,
                        debut: c.start,
                        decalage: c.offset,
                        duree: c.length,
                    })
                    .collect(),
            })
            .collect(),
        guides: doc.guides.clone(),
        modeles: used_models(doc)
            .iter()
            .map(|id| ModelEntry {
                id: *id,
                nom: doc.models[id].name.clone(),
            })
            .collect(),
    }
}

/// Modèles 3D cités par au moins un panneau.
fn used_models(doc: &Document) -> BTreeSet<u64> {
    doc.panels
        .iter()
        .filter_map(|p| p.ref3d.as_ref().map(|r| r.model))
        .filter(|id| doc.models.contains_key(id))
        .collect()
}

fn model_file(dir: &Path, id: u64) -> PathBuf {
    dir.join(MODELS).join(format!("{id}.pelure3d"))
}

pub struct Loaded {
    pub doc: Document,
    /// Dessins présents dans le dossier, à la version chargée (rien à réécrire au prochain enregistrement).
    pub written: HashMap<CelId, u64>,
    /// Problèmes non bloquants (dessin manquant ou illisible…).
    pub warnings: Vec<String>,
    /// Ancien format (3) converti à l'ouverture : feuille doublée autour du cadre.
    pub converted: bool,
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

    let mut sounds: HashMap<u64, Sound> = HashMap::new();
    for entry in &manifest.sons {
        match load_sound(dir, entry) {
            Ok(sound) => {
                sounds.insert(sound.id, sound);
            }
            Err(e) => warnings.push(e),
        }
    }
    let mut models: HashMap<u64, Model3d> = HashMap::new();
    for entry in &manifest.modeles {
        match fs::read(model_file(dir, entry.id))
            .map_err(|e| e.to_string())
            .and_then(|b| crate::model3d::from_bytes(&b))
        {
            Ok(mesh) => {
                models.insert(entry.id, Model3d::new(entry.id, entry.nom.clone(), mesh));
            }
            Err(e) => warnings.push(format!("modèle 3D « {} » illisible ({e})", entry.nom)),
        }
    }
    let audio_tracks = manifest
        .pistes_son
        .iter()
        .map(|t| AudioTrack {
            id: t.id,
            name: t.nom.clone(),
            muted: t.muette,
            volume: t.volume.clamp(0.0, 4.0),
            clips: t
                .extraits
                .iter()
                .filter(|c| sounds.contains_key(&c.son) && c.duree > 0)
                .map(|c| SoundClip {
                    id: c.id,
                    sound: c.son,
                    start: c.debut,
                    offset: c.decalage,
                    length: c.duree,
                })
                .collect(),
        })
        .collect();

    let mut structure = Structure {
        sequences: Vec::new(),
        scenes: Vec::new(),
        panels: Vec::new(),
        audio_tracks,
        guides: manifest
            .guides
            .into_iter()
            .filter(|g| (1..=3).contains(&g.points.len()) && g.points.iter().flatten().all(|v| v.is_finite()))
            .collect(),
        paper: manifest.papier,
        fps: manifest.cadence.max(1),
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
                        locked: l.verrouille,
                        transform: l
                            .transformation
                            .as_ref()
                            .filter(|t| [t.x, t.y, t.echelle, t.rotation].iter().all(|v| v.is_finite()))
                            .map(|t| LayerTransform {
                                x: t.x,
                                y: t.y,
                                scale: t.echelle.clamp(0.01, 100.0),
                                angle: t.rotation,
                            })
                            .unwrap_or_default(),
                        depth: if l.profondeur.is_finite() {
                            l.profondeur.clamp(Layer::MIN_DEPTH, 1000.0)
                        } else {
                            0.0
                        },
                        frames: from_exposures(&l.expositions)
                            .into_iter()
                            .map(|f| f.filter(|id| !missing.contains(id)))
                            .collect(),
                    })
                    .collect();
                if layers.is_empty() {
                    layers.push(Layer::new("Dessin", LayerKind::Vector));
                }
                let mut camera: Vec<CameraKey> = Vec::new();
                for k in &p.camera {
                    if k.x.is_finite() && k.y.is_finite() && k.zoom.is_finite() && k.rotation.is_finite() {
                        let cam = Camera {
                            x: k.x,
                            y: k.y,
                            zoom: k.zoom.clamp(0.05, 50.0),
                            angle: k.rotation,
                        };
                        crate::camera::set_key(&mut camera, k.image, cam);
                        if let Some(x) = camera.iter_mut().find(|x| x.frame == k.image) {
                            x.smooth = k.doux;
                        }
                    }
                }
                structure.panels.push(Panel {
                    id: p.id,
                    scene: scene.id,
                    length: p.duree,
                    layers,
                    action: p.action,
                    dialogue: p.dialogue,
                    notes: p.notes,
                    camera,
                    ref3d: p.repere3d.as_ref().filter(|r| models.contains_key(&r.modele)).and_then(ref3d_of),
                });
            }
        }
    }
    if structure.panels.is_empty() {
        return Err("le projet ne contient aucun panneau".into());
    }
    let frame = manifest.cadre;
    let mut written: HashMap<CelId, u64> = cels.keys().map(|&id| (id, 0)).collect();
    let mut doc = Document::from_parts(w, h, structure, cels, sounds);
    for model in models.into_values() {
        doc.insert_model(model);
    }
    let converted = match frame {
        Some([fw, fh]) if fw > 0 && fh > 0 && fw <= w && fh <= h => {
            doc.frame = [fw, fh];
            false
        }
        // Format 3 : la feuille était le cadre. On la double, dessin au centre ; les fichiers de
        // dessins seront tous réécrits à la nouvelle taille au prochain enregistrement.
        _ => {
            doc.enlarge_stage(w * 2, h * 2);
            doc.frame = [w, h];
            written.clear();
            true
        }
    };
    Ok(Loaded {
        doc,
        written,
        warnings,
        converted,
    })
}

/// Repère 3D relu du manifeste ; `None` si ses valeurs sont absurdes.
fn ref3d_of(r: &Ref3dEntry) -> Option<Ref3d> {
    let values = [r.azimut, r.elevation, r.distance, r.focale, r.opacite];
    if !values.iter().chain(&r.cible).all(|v| v.is_finite()) || r.distance <= 0.0 || r.focale <= 0.0 {
        return None;
    }
    Some(Ref3d {
        model: r.modele,
        cam: Cam3d {
            target: r.cible,
            azimuth: r.azimut,
            elevation: r.elevation.clamp(-MAX_ELEVATION, MAX_ELEVATION),
            distance: r.distance,
            focal: r.focale,
        },
        visible: r.visible,
        hidden_lines: r.aretes_cachees,
        color: r.couleur,
        opacity: r.opacite.clamp(0.0, 1.0),
    })
}

fn load_sound(dir: &Path, entry: &SoundEntry) -> Result<Sound, String> {
    let shown = if entry.nom.is_empty() { &entry.fichier } else { &entry.nom };
    // Le nom de fichier vient du manifeste : on refuse tout ce qui sortirait du dossier `sons`.
    let file = Path::new(&entry.fichier);
    if file.components().count() != 1 || file.file_name().is_none() {
        return Err(format!("son « {shown} » : nom de fichier invalide"));
    }
    let bytes = fs::read(dir.join(SOUNDS).join(file)).map_err(|e| format!("son « {shown} » introuvable ({e})"))?;
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let pcm = crate::audio::decode(&bytes, &ext).map_err(|e| format!("son « {shown} » : {e}"))?;
    Ok(Sound {
        id: entry.id,
        name: entry.nom.clone(),
        ext,
        bytes: Arc::new(bytes),
        pcm: Arc::new(pcm),
    })
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
