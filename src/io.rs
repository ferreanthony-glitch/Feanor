//! Anciens fichiers `.pelure` (formats 1 et 2, un seul fichier) et écriture d'images PNG.
//!
//! Format 1 et 2 : l'en-tête `PELURE\0\x01`, la version (u32), la longueur du manifeste JSON
//! (u64), le manifeste, puis les dessins bitmap encodés en PNG, mis bout à bout. Ces fichiers
//! s'ouvrent toujours ; on les enregistre désormais en dossier projet (voir `project.rs`).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use eframe::egui::Color32;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::ImageEncoder;
use serde::{Deserialize, Serialize};

use crate::document::{Cel, Document, Layer, LayerKind, Panel, Scene, Sequence, Structure};
use crate::raster::{unpremultiply, Raster};
use crate::vector::VectorDrawing;

pub(crate) const LEGACY_MAGIC: &[u8; 8] = b"PELURE\x00\x01";
const LEGACY_VERSION: u32 = 2;
const HEADER: usize = 8 + 4 + 8;

#[derive(Serialize, Deserialize)]
struct LegacyManifest {
    version: u32,
    width: usize,
    height: usize,
    fps: u32,
    paper: [u8; 3],
    /// Format 1 : un seul plan, ses calques directement ici.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layers: Vec<LegacyLayer>,
    /// Format 2 : les plans, chacun avec ses calques.
    #[serde(default)]
    shots: Vec<LegacyShot>,
    cels: Vec<LegacyCel>,
}

#[derive(Serialize, Deserialize)]
struct LegacyShot {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    length: usize,
    #[serde(default)]
    action: String,
    #[serde(default)]
    dialogue: String,
    #[serde(default)]
    notes: String,
    layers: Vec<LegacyLayer>,
}

#[derive(Serialize, Deserialize)]
struct LegacyLayer {
    name: String,
    #[serde(default)]
    kind: LayerKind,
    visible: bool,
    opacity: f32,
    frames: Vec<Option<u64>>,
}

#[derive(Serialize, Deserialize)]
struct LegacyCel {
    id: u64,
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    length: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vector: Option<VectorDrawing>,
}

pub(crate) fn encode_png(rgba: &[u8], width: usize, height: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive)
        .write_image(rgba, width as u32, height as u32, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Le fichier est-il un ancien `.pelure` (un seul fichier binaire) ?
pub fn is_legacy_file(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 8];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok_and(|_| &head == LEGACY_MAGIC)
}

/// Ouvre un ancien `.pelure`. Chaque « plan » d'alors devient une scène d'un panneau, dans une
/// seule séquence ; le nom du plan devient le nom de la scène.
pub fn load_legacy(path: &Path) -> Result<Document, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    if data.len() < HEADER || &data[..8] != LEGACY_MAGIC {
        return Err("ce fichier n'est pas un projet SHOT".into());
    }
    let version = u32::from_le_bytes(data[8..12].try_into().unwrap());
    if version > LEGACY_VERSION {
        return Err(format!("format {version} inconnu"));
    }
    let json_len = u64::from_le_bytes(data[12..20].try_into().unwrap()) as usize;
    let json = data
        .get(HEADER..HEADER.saturating_add(json_len))
        .ok_or("fichier tronqué")?;
    let manifest: LegacyManifest = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    let blob = &data[HEADER + json_len..];
    let (w, h) = (manifest.width, manifest.height);
    // (Comme pour un manifeste d'aujourd'hui, `project.rs` : une feuille vide ou démesurée ne s'ouvre pas.)
    if w == 0 || h == 0 || w > crate::vector::MAX_SIDE * 2 || h > crate::vector::MAX_SIDE * 2 {
        return Err(format!("taille d'image invalide dans le fichier ({w} × {h})"));
    }

    let mut cels = HashMap::new();
    for c in manifest.cels {
        let cel = match c.vector {
            Some(drawing) => Cel {
                raster: drawing.render(w, h),
                vector: Some(drawing),
                version: 0,
                fond: None,
            },
            None => {
                let end = c.offset.checked_add(c.length).ok_or("fichier tronqué")?;
                let bytes = blob.get(c.offset as usize..end as usize).ok_or("fichier tronqué")?;
                let img = image::load_from_memory(bytes)
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                if img.width() as usize != w || img.height() as usize != h {
                    return Err(format!("le dessin {} n'a pas la taille du projet", c.id));
                }
                Cel {
                    raster: Raster::from_rgba(w, h, img.as_raw()),
                    vector: None,
                    version: 0,
                    fond: None,
                }
            }
        };
        cels.insert(c.id, cel);
    }

    let to_layers = |entries: Vec<LegacyLayer>| -> Vec<Layer> {
        let mut layers: Vec<Layer> = entries
            .into_iter()
            .map(|l| Layer {
                visible: l.visible,
                opacity: l.opacity.clamp(0.0, 1.0),
                frames: l
                    .frames
                    .into_iter()
                    .map(|f| f.filter(|id| cels.contains_key(id)))
                    .collect(),
                ..Layer::new(l.name, l.kind)
            })
            .collect();
        if layers.is_empty() {
            layers.push(Layer::new("Dessin", LayerKind::Vector));
        }
        layers
    };

    // Les identifiants de séquence et de scène sont pris au-dessus de tous ceux du fichier.
    let mut next = cels
        .keys()
        .copied()
        .chain(manifest.shots.iter().map(|s| s.id))
        .max()
        .unwrap_or(0)
        + 1;
    let mut alloc = || {
        next += 1;
        next - 1
    };
    let sequence = alloc();
    let mut structure = Structure {
        sequences: vec![Sequence {
            id: sequence,
            name: String::new(),
            number: String::new(),
        }],
        scenes: Vec::new(),
        panels: Vec::new(),
        audio_tracks: Vec::new(),
        guides: Vec::new(),
        paper: manifest.paper,
        fps: manifest.fps.max(1),
        board: Default::default(),
        groups: crate::groups::defaults(),
        start_frame: 1,
        background: Default::default(),
        paper_b: crate::document::DEFAULT_PAPER_B,
        master_volume: 1.0,
        storyboard_locked: false,
        numbering_locked: false,
        properties: Default::default(),
        comments: Vec::new(),
        reserves: Default::default(),
        transitions: Vec::new(),
        references: Vec::new(),
        personnages: Vec::new(),
        versions: Default::default(),
        variantes: Default::default(),
        annotations: Default::default(),
    };
    let mut shots = manifest.shots;
    if shots.is_empty() {
        shots.push(LegacyShot {
            id: alloc(),
            name: String::new(),
            length: 0,
            action: String::new(),
            dialogue: String::new(),
            notes: String::new(),
            layers: manifest.layers,
        });
    }
    for s in shots {
        let scene = alloc();
        structure.scenes.push(Scene {
            id: scene,
            name: s.name,
            number: String::new(),
            sequence,
        });
        structure.panels.push(Panel {
            id: s.id,
            scene,
            length: s.length,
            layers: to_layers(s.layers),
            action: s.action,
            dialogue: s.dialogue,
            notes: s.notes,
            camera: Vec::new(),
            marks: Vec::new(),
            cues: Default::default(),
            color: None,
        });
    }
    Ok(Document::from_parts(w, h, structure, cels, HashMap::new()))
}

/// Écrit un ancien `.pelure` (format 2) : sert seulement aux tests de compatibilité.
#[cfg(test)]
pub fn save_legacy_v2(doc: &Document, path: &Path) -> Result<(), String> {
    let mut blob = Vec::new();
    let mut cels = Vec::new();
    for id in doc.used_cels() {
        let cel = &doc.cels[&id];
        if let Some(v) = &cel.vector {
            cels.push(LegacyCel {
                id,
                offset: 0,
                length: 0,
                vector: Some(v.clone()),
            });
            continue;
        }
        let png = encode_png(&cel.raster.to_rgba(), cel.raster.width, cel.raster.height)?;
        cels.push(LegacyCel {
            id,
            offset: blob.len() as u64,
            length: png.len() as u64,
            vector: None,
        });
        blob.extend_from_slice(&png);
    }
    let manifest = LegacyManifest {
        version: 2,
        width: doc.width,
        height: doc.height,
        fps: doc.fps,
        paper: doc.paper,
        layers: Vec::new(),
        shots: doc
            .panels
            .iter()
            .map(|p| LegacyShot {
                id: p.id,
                name: doc.scene(p.scene).map(|s| s.name.clone()).unwrap_or_default(),
                length: p.length,
                action: p.action.clone(),
                dialogue: p.dialogue.clone(),
                notes: p.notes.clone(),
                layers: p
                    .layers
                    .iter()
                    .map(|l| LegacyLayer {
                        name: l.name.clone(),
                        kind: l.kind,
                        visible: l.visible,
                        opacity: l.opacity,
                        frames: l.frames.clone(),
                    })
                    .collect(),
            })
            .collect(),
        cels,
    };
    let json = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    out.extend_from_slice(LEGACY_MAGIC);
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(json.len() as u64).to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&blob);
    fs::write(path, out).map_err(|e| e.to_string())
}

pub fn write_png(path: &Path, pixels: &[Color32], width: usize, height: usize) -> Result<(), String> {
    let rgba: Vec<u8> = pixels.iter().flat_map(|&c| unpremultiply(c)).collect();
    let png = encode_png(&rgba, width, height)?;
    fs::write(path, png).map_err(|e| e.to_string())
}

/// Comme `write_png`, réduite à `max_width` pixels de large au plus.
pub fn write_png_scaled(
    path: &Path,
    pixels: &[Color32],
    width: usize,
    height: usize,
    max_width: usize,
) -> Result<(), String> {
    if width <= max_width {
        return write_png(path, pixels, width, height);
    }
    let rgba: Vec<u8> = pixels.iter().flat_map(|&c| unpremultiply(c)).collect();
    let img = image::RgbaImage::from_raw(width as u32, height as u32, rgba).ok_or("image invalide")?;
    let nh = ((height * max_width) as f32 / width as f32).round().max(1.0) as u32;
    let small = image::imageops::resize(&img, max_width as u32, nh, image::imageops::FilterType::Triangle);
    let png = encode_png(small.as_raw(), small.width() as usize, small.height() as usize)?;
    fs::write(path, png).map_err(|e| e.to_string())
}

pub fn load_image(path: &Path) -> Result<image::RgbaImage, String> {
    image::open(path)
        .map(|i| i.to_rgba8())
        .map_err(|e| format!("impossible de lire l'image : {e}"))
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "<br>")
}
