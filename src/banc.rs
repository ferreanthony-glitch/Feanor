//! Banc de traits (moteur de dessin, étape 1) : Aide > « Enregistrer mes traits » garde chaque
//! trait tel que le stylet l'a donné (positions, appui, instants) avec son réglage, une ligne JSON
//! par trait dans `%APPDATA%\SHOT\traits\traits.jsonl`. On les rejoue ensuite dans le moteur
//! (essai ignoré `PELURE_TRAITS`) pour régler la sensation sur les vrais gestes d'Anthony et
//! comparer avec TVPaint, sans deviner.

use std::io::Write;
use std::path::{Path, PathBuf};

use eframe::egui::pos2;
use serde::{Deserialize, Serialize};

use crate::brush::{BrushSettings, StrokeMode, StrokeState, Toucher};
use crate::brushtex::BrushTextures;
use crate::raster::Raster;

/// Un trait enregistré.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recorded {
    #[serde(rename = "reglage")]
    pub settings: BrushSettings,
    #[serde(rename = "couleur")]
    pub color: [u8; 3],
    #[serde(rename = "gomme")]
    pub eraser: bool,
    /// x, y (pixels du dessin), appui tel que lu sur la tablette (0 à 1), instant (secondes ;
    /// −1 : inconnu).
    pub points: Vec<[f64; 4]>,
    /// Le toucher du stylet au moment du trait (les traits d'avant l'étude : celui par défaut).
    #[serde(default, rename = "toucher")]
    pub feel: Toucher,
}

impl Recorded {
    pub fn of(stroke: &StrokeState) -> Recorded {
        let (color, eraser) = match stroke.mode() {
            StrokeMode::Paint(rgb) => (rgb, false),
            StrokeMode::Erase => ([0, 0, 0], true),
        };
        Recorded { settings: stroke.settings(), color, eraser, points: stroke.raw_log.clone(), feel: stroke.feel() }
    }
}

/// Le dossier des traits enregistrés.
pub fn dir() -> Option<PathBuf> {
    crate::donnees::roaming().map(|d| d.join("traits"))
}

/// Ajoute un trait au fichier `traits.jsonl` du dossier `dir` ; renvoie le fichier.
pub fn append(dir: &Path, r: &Recorded) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join("traits.jsonl");
    let line = serde_json::to_string(r).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("{} : {e}", path.display()))?;
    writeln!(f, "{line}").map_err(|e| e.to_string())?;
    Ok(path)
}

/// Les traits d'un fichier (les lignes illisibles sont sautées).
pub fn read(path: &Path) -> Result<Vec<Recorded>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
}

/// Rejoue un trait dans le moteur, sur une feuille vide de la taille de son étendue (plus une
/// marge) : la feuille et le décalage appliqué aux points.
pub fn replay(r: &Recorded, textures: BrushTextures) -> (Raster, [f32; 2]) {
    let margin = r.settings.size as f64 + 4.0;
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &r.points {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    if r.points.is_empty() {
        return (Raster::new(1, 1), [0.0, 0.0]);
    }
    let (w, h) = (((x1 - x0) + 2.0 * margin).ceil().max(1.0) as usize, ((y1 - y0) + 2.0 * margin).ceil().max(1.0) as usize);
    let off = [(margin - x0) as f32, (margin - y0) as f32];
    let mut raster = Raster::new(w.min(8192), h.min(8192));
    let mode = if r.eraser { StrokeMode::Paint([0, 0, 0]) } else { StrokeMode::Paint(r.color) };
    let mut st = StrokeState::new(0, mode, r.settings, None, true).with_textures(textures).with_feel(r.feel);
    let mut dirty = Vec::new();
    for p in &r.points {
        let pos = pos2(p[0] as f32 + off[0], p[1] as f32 + off[1]);
        if p[3] >= 0.0 {
            st.add_point_at(&mut raster, pos, p[2] as f32, p[3], &mut dirty);
        } else {
            st.add_point(&mut raster, pos, p[2] as f32, &mut dirty);
        }
    }
    st.finish(&mut raster, &mut dirty);
    (raster, off)
}
