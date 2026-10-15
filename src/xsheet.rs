//! Feuille d'exposition (13g-1), la « Timesheet » de TVPaint 12 en affichage occidental : une
//! ligne par image du plan, une colonne par calque, plus les colonnes Image, Action,
//! Dialogues/Notes, Audio et Caméra. Ce module calcule ce que montre chaque case ; l'affichage
//! est dans `xsheetui.rs`.
//!
//! Tout vient de ce que le plan contient déjà : un dessin est écrit à la première image de son
//! exposition (son nom d'instance, sinon son numéro dans l'ordre du calque), un trait descend
//! tant qu'il est tenu, une croix marque le début d'images vides ; Action et Dialogues/Notes
//! reprennent les textes de chaque image de storyboard (début du plan et marques d'image).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::document::{CelId, Document, Layer, Panel};

/// Ce que montre une case de calque.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    /// Rien (images vides qui suivent une croix).
    Blank,
    /// Première image d'une exposition : le nom du dessin, et sa marque de couleur.
    Start { cel: CelId, label: String, mark: Option<u8> },
    /// Le dessin d'au-dessus est tenu.
    Hold,
    /// Début d'images vides : une croix.
    EmptyStart,
    /// Image montrée par le pré- ou post-comportement (le dessin n'est pas exposé ici).
    Behaviour { label: String },
}

/// Nom de chaque dessin du calque : son nom d'instance, sinon son rang dans l'ordre du calque
/// (1, 2, 3…), comme « Tout renommer ».
pub fn drawing_labels(layer: &Layer) -> HashMap<CelId, String> {
    let order = crate::images::drawings_in_order(&layer.frames);
    order
        .iter()
        .enumerate()
        .map(|(k, id)| (*id, layer.names.get(id).cloned().unwrap_or_else(|| (k + 1).to_string())))
        .collect()
}

/// Les `len` cases d'un calque. `behaviours` : montrer ce que les pré- et post-comportements
/// font voir là où rien n'est exposé.
pub fn layer_cells(layer: &Layer, len: usize, behaviours: bool) -> Vec<Cell> {
    let labels = drawing_labels(layer);
    let raw = |f: usize| layer.frames.get(f).copied().flatten();
    (0..len)
        .map(|f| match raw(f) {
            Some(id) if f > 0 && raw(f - 1) == Some(id) => Cell::Hold,
            Some(id) => Cell::Start { cel: id, label: labels.get(&id).cloned().unwrap_or_default(), mark: layer.color_marks.get(&id).copied() },
            None => {
                let shown = if behaviours { layer.shown_at(f) } else { None };
                match shown {
                    Some(id) => Cell::Behaviour { label: labels.get(&id).cloned().unwrap_or_default() },
                    None if f == 0 || raw(f - 1).is_some() => Cell::EmptyStart,
                    None => Cell::Blank,
                }
            }
        })
        .collect()
}

/// Ce que montre une case de la colonne Caméra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CamCell {
    None,
    /// Une image clé de la caméra.
    Key,
    /// Entre deux clés : la caméra bouge.
    Between,
}

/// Les `len` cases de la colonne Caméra du plan.
pub fn camera_cells(panel: &Panel, len: usize) -> Vec<CamCell> {
    let keys: Vec<usize> = panel.camera.iter().map(|k| k.frame).collect();
    let (first, last) = (keys.iter().min().copied(), keys.iter().max().copied());
    (0..len)
        .map(|f| {
            if keys.contains(&f) {
                CamCell::Key
            } else if first.is_some_and(|a| a < f) && last.is_some_and(|b| f < b) {
                CamCell::Between
            } else {
                CamCell::None
            }
        })
        .collect()
}

/// Textes d'une image de storyboard, à l'image où elle commence.
#[derive(Clone, Debug, PartialEq)]
pub struct StoryRow {
    pub frame: usize,
    pub action: String,
    pub dialogue: String,
    pub notes: String,
}

/// Les textes du storyboard du plan : son début, puis chaque marque d'image, dans l'ordre.
pub fn story_rows(panel: &Panel) -> Vec<StoryRow> {
    let mut rows = vec![StoryRow { frame: 0, action: panel.action.clone(), dialogue: panel.dialogue.clone(), notes: panel.notes.clone() }];
    rows.extend(panel.marks.iter().map(|m| StoryRow { frame: m.frame, action: m.action.clone(), dialogue: m.dialogue.clone(), notes: m.notes.clone() }));
    rows.sort_by_key(|r| r.frame);
    rows
}

/// Niveau du son (0 à 1) à chaque image du plan `index` : le plus fort des extraits entendus
/// (pistes non muettes), volume de la piste et volume général compris.
pub fn audio_levels(doc: &Document, index: usize) -> Vec<f32> {
    let len = doc.panels.get(index).map_or(0, Panel::frame_count);
    let start = doc.panel_start(index);
    let spf = crate::audio::samples_per_frame(doc.fps);
    let mut levels = vec![0.0f32; len];
    for track in doc.audio_tracks.iter().filter(|t| !t.muted) {
        for clip in &track.clips {
            let Some(sound) = doc.sounds.get(&clip.sound) else { continue };
            for (f, level) in levels.iter_mut().enumerate() {
                let g = start + f;
                if g < clip.start || g >= clip.end() {
                    continue;
                }
                let s = g - clip.start + clip.offset;
                let a = sound.pcm.peak_between((s as f64 * spf) as usize, ((s + 1) as f64 * spf) as usize);
                *level = level.max((a * track.volume * doc.master_volume).min(1.0));
            }
        }
    }
    levels
}

/// Ce que la feuille montre (« Modifier infos affichées » de TVPaint) ; réglage de
/// l'application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct XSheetSettings {
    pub action: bool,
    pub dialogue: bool,
    pub audio: bool,
    pub camera: bool,
    pub layers: bool,
    /// Aussi les calques cachés (« calques inactifs »).
    pub hidden_layers: bool,
    /// Ce que montrent les pré- et post-comportements.
    pub behaviours: bool,
    /// Ligne de l'image courante surlignée (« position de la tête de lecture »).
    pub playhead: bool,
    /// Points d'entrée et de sortie, et signets.
    pub cues: bool,
    /// Un trait épais toutes les `separator` images (0 : chaque seconde).
    pub separator: usize,
}

impl Default for XSheetSettings {
    fn default() -> Self {
        Self {
            action: true,
            dialogue: true,
            audio: true,
            camera: true,
            layers: true,
            hidden_layers: false,
            behaviours: true,
            playhead: true,
            cues: true,
            separator: 0,
        }
    }
}

impl XSheetSettings {
    /// Images entre deux traits épais : `separator`, ou une seconde si 0.
    pub fn separator_every(&self, fps: u32) -> usize {
        if self.separator > 0 {
            self.separator
        } else {
            fps.max(1) as usize
        }
    }
}
