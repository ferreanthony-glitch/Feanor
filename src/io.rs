//! Fichiers projet `.pelure` et export d'images.
//!
//! Format `.pelure` : l'en-tête `PELURE\0\x01`, la version (u32), la longueur du manifeste
//! JSON (u64), le manifeste, puis les dessins bitmap encodés en PNG, mis bout à bout.
//! Les dessins vectoriels sont écrits dans le manifeste lui-même.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use eframe::egui::Color32;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::ImageEncoder;
use serde::{Deserialize, Serialize};

use crate::document::{fresh_layer_id, Cel, Document, Layer, LayerKind, Shot};
use crate::raster::{unpremultiply, Raster};
use crate::vector::VectorDrawing;

const MAGIC: &[u8; 8] = b"PELURE\x00\x01";
const VERSION: u32 = 2;
const HEADER: usize = 8 + 4 + 8;

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    width: usize,
    height: usize,
    fps: u32,
    paper: [u8; 3],
    /// Format 1 : un seul plan, ses calques directement ici.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layers: Vec<LayerEntry>,
    #[serde(default)]
    shots: Vec<ShotEntry>,
    cels: Vec<CelEntry>,
}

#[derive(Serialize, Deserialize)]
struct ShotEntry {
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
    layers: Vec<LayerEntry>,
}

#[derive(Serialize, Deserialize)]
struct LayerEntry {
    name: String,
    #[serde(default)]
    kind: LayerKind,
    visible: bool,
    opacity: f32,
    frames: Vec<Option<u64>>,
}

#[derive(Serialize, Deserialize)]
struct CelEntry {
    id: u64,
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    length: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vector: Option<VectorDrawing>,
}

fn layer_entry(l: &Layer) -> LayerEntry {
    LayerEntry {
        name: l.name.clone(),
        kind: l.kind,
        visible: l.visible,
        opacity: l.opacity,
        frames: l.frames.clone(),
    }
}

fn encode_png(rgba: &[u8], width: usize, height: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive)
        .write_image(rgba, width as u32, height as u32, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn save_project(doc: &Document, path: &Path) -> Result<(), String> {
    let mut blob = Vec::new();
    let mut cels = Vec::new();
    for id in doc.used_cels() {
        let Some(cel) = doc.cels.get(&id) else { continue };
        if let Some(v) = &cel.vector {
            cels.push(CelEntry {
                id,
                offset: 0,
                length: 0,
                vector: Some(v.clone()),
            });
            continue;
        }
        let png = encode_png(&cel.raster.to_rgba(), cel.raster.width, cel.raster.height)?;
        cels.push(CelEntry {
            id,
            offset: blob.len() as u64,
            length: png.len() as u64,
            vector: None,
        });
        blob.extend_from_slice(&png);
    }
    let manifest = Manifest {
        version: VERSION,
        width: doc.width,
        height: doc.height,
        fps: doc.fps,
        paper: doc.paper,
        layers: Vec::new(),
        shots: doc
            .shots
            .iter()
            .map(|s| ShotEntry {
                id: s.id,
                name: s.name.clone(),
                length: s.length,
                action: s.action.clone(),
                dialogue: s.dialogue.clone(),
                notes: s.notes.clone(),
                layers: s.layers.iter().map(layer_entry).collect(),
            })
            .collect(),
        cels,
    };
    let json = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(HEADER + json.len() + blob.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(json.len() as u64).to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&blob);

    // On écrit à côté puis on remplace : un plantage en cours d'écriture ne détruit pas l'ancien fichier.
    let tmp = path.with_extension("pelure.tmp");
    fs::write(&tmp, &out).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn load_project(path: &Path) -> Result<Document, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    if data.len() < HEADER || &data[..8] != MAGIC {
        return Err("ce fichier n'est pas un projet Pelure".into());
    }
    let version = u32::from_le_bytes(data[8..12].try_into().unwrap());
    if version > VERSION {
        return Err(format!("projet créé par une version plus récente (format {version})"));
    }
    let json_len = u64::from_le_bytes(data[12..20].try_into().unwrap()) as usize;
    let json = data
        .get(HEADER..HEADER + json_len)
        .ok_or("fichier tronqué")?;
    let manifest: Manifest = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    let blob = &data[HEADER + json_len..];
    let (w, h) = (manifest.width, manifest.height);

    let mut cels = HashMap::new();
    for c in manifest.cels {
        let cel = match c.vector {
            Some(drawing) => Cel {
                raster: drawing.render(w, h),
                vector: Some(drawing),
                version: 0,
            },
            None => {
                let bytes = blob
                    .get(c.offset as usize..(c.offset + c.length) as usize)
                    .ok_or("fichier tronqué")?;
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
                }
            }
        };
        cels.insert(c.id, cel);
    }

    let to_layers = |entries: Vec<LayerEntry>| -> Vec<Layer> {
        let mut layers: Vec<Layer> = entries
            .into_iter()
            .map(|l| Layer {
                id: fresh_layer_id(),
                name: l.name,
                kind: l.kind,
                visible: l.visible,
                opacity: l.opacity.clamp(0.0, 1.0),
                frames: l
                    .frames
                    .into_iter()
                    .map(|f| f.filter(|id| cels.contains_key(id)))
                    .collect(),
            })
            .collect();
        if layers.is_empty() {
            layers.push(Layer::new("Dessin", LayerKind::Vector));
        }
        layers
    };

    let mut shots: Vec<Shot> = manifest
        .shots
        .into_iter()
        .map(|s| Shot {
            id: s.id,
            name: s.name,
            length: s.length,
            layers: to_layers(s.layers),
            action: s.action,
            dialogue: s.dialogue,
            notes: s.notes,
        })
        .collect();
    if shots.is_empty() {
        // Format 1 : un seul plan.
        let id = cels.keys().max().copied().unwrap_or(0) + 1;
        shots.push(Shot {
            id,
            name: String::new(),
            length: 0,
            layers: to_layers(manifest.layers),
            action: String::new(),
            dialogue: String::new(),
            notes: String::new(),
        });
    }
    Ok(Document::from_parts(
        w,
        h,
        manifest.fps.max(1),
        manifest.paper,
        shots,
        cels,
    ))
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
