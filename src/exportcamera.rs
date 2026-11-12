//! « Clés de caméra » (exports V1, 2026-10-30) : pour chaque plan de l'étendue choisie, les clés de la caméra (image, position, zoom,
//! rotation, avancée, bascule, panoramique, interpolation), en **JSON** (le format décrit dans `docs/EXPORT_MONTAGE.md`, à relire par un
//! script After Effects, Blender ou autre) et en **CSV** (une ligne par clé, ouvrable dans un tableur) ; avec, si on le veut, la caméra de
//! **chaque image** (déjà interpolée : un script n'a pas à refaire les adoucis).
//!
//! # Le format JSON
//!
//! ```text
//! { "format": "shot-camera", "version": 1, "projet": "...", "cadence": 24,
//!   "cadre": { "largeur": 1920, "hauteur": 1080 }, "feuille": { "largeur": 3840, "hauteur": 2160 },
//!   "unites": { ... phrases qui disent l'unité de chaque valeur ... },
//!   "plans": [ { "plan": "SQ01_PL012A", "titre": "...", "image_debut": 0, "images": 24,
//!                "cles": [ { "image": 0, "x": 960.0, "y": 540.0, "zoom": 1.0, "rotation": 0.0, "avancee": 0.0, "bascule": 0.0,
//!                            "panoramique": 0.0, "interpolation": "lineaire", "adouci_entree": 0.0, "adouci_sortie": 0.0 } ],
//!                "par_image": [ { "image": 0, "x": ..., "y": ..., "zoom": ..., ... } ] } ] }
//! ```
//!
//! Les numéros d'image des clés et de `par_image` comptent depuis 0 dans le plan ; `image_debut` est le rang de la première image du plan
//! dans le film. Un plan sans clé est filmé par la caméra du projet : `cles` est vide et `par_image` (si demandé) donne cette caméra.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::camera::{Camera, CameraKey};
use crate::document::Document;

/// Les choix de la fenêtre « Clés de caméra ».
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraSettings {
    /// Écrit aussi les CSV (`<nom>_cles.csv`, et `<nom>_images.csv` si la caméra de chaque image est demandée).
    pub csv: bool,
    /// La caméra de chaque image, déjà interpolée.
    pub per_frame: bool,
}

impl Default for CameraSettings {
    fn default() -> Self {
        CameraSettings { csv: true, per_frame: true }
    }
}

/// Ce que l'export écrit.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraExport {
    pub json: String,
    /// Une ligne par clé.
    pub keys_csv: String,
    /// Une ligne par image (seulement si `per_frame`).
    pub frames_csv: Option<String>,
}

fn camera_fields(c: &Camera) -> Vec<(&'static str, f32)> {
    vec![("x", c.x), ("y", c.y), ("zoom", c.zoom), ("rotation", c.angle), ("avancee", c.dolly), ("bascule", c.tilt), ("panoramique", c.pan)]
}

fn key_json(k: &CameraKey) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("image".into(), json!(k.frame));
    for (name, v) in camera_fields(&k.cam) {
        o.insert(name.into(), json!(v));
    }
    let eased = k.smooth || k.ease_in > 0.0 || k.ease_out > 0.0;
    o.insert("interpolation".into(), json!(if eased { "adoucie" } else { "lineaire" }));
    o.insert("adouci_entree".into(), json!(k.ease_in));
    o.insert("adouci_sortie".into(), json!(k.ease_out));
    // L'ancien réglage « adoucir » (avant les adoucis en images) : un demi-intervalle de chaque côté.
    o.insert("adouci_ancien".into(), json!(k.smooth));
    Value::Object(o)
}

fn frame_json(f: usize, c: &Camera) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("image".into(), json!(f));
    for (name, v) in camera_fields(c) {
        o.insert(name.into(), json!(v));
    }
    Value::Object(o)
}

/// Une valeur de CSV : point décimal (les scripts, pas le tableur français), texte entre guillemets.
fn csv_text(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Les clés de caméra des plans `plans`.
pub fn build(doc: &Document, plans: &[usize], title: &str, s: &CameraSettings) -> CameraExport {
    let mut out_plans: Vec<Value> = Vec::new();
    let mut keys_csv = String::from("plan,titre,image,x,y,zoom,rotation,avancee,bascule,panoramique,interpolation,adouci_entree,adouci_sortie\n");
    let mut frames_csv = String::from("plan,image_du_plan,image_du_film,x,y,zoom,rotation,avancee,bascule,panoramique\n");
    for &p in plans.iter().filter(|&&p| p < doc.panels.len()) {
        let panel = &doc.panels[p];
        let name = crate::editorial::clip_name(doc, p);
        let long = crate::storyboard::panel_long_title(doc, p);
        let len = panel.frame_count();
        let start = doc.panel_start(p);
        let mut plan = serde_json::Map::new();
        plan.insert("plan".into(), json!(name));
        plan.insert("titre".into(), json!(long));
        plan.insert("image_debut".into(), json!(start));
        plan.insert("images".into(), json!(len));
        // Un effet de caméra vivant (partie 129) s'exporte comme il se voit : ses clés cuites (celles que donnerait « Convertir
        // en clés »), et les valeurs image par image même si elles ne sont pas demandées. Le nom de l'effet est dit.
        let mut keys = doc.cles_montrees(p).into_owned();
        keys.sort_by_key(|k| k.frame);
        if let Some(v) = panel.effet_camera {
            plan.insert("effet_vivant".into(), json!(v.params.preset.label()));
        }
        for k in &keys {
            let v = key_json(k);
            keys_csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                csv_text(&name),
                csv_text(&long),
                k.frame,
                k.cam.x,
                k.cam.y,
                k.cam.zoom,
                k.cam.angle,
                k.cam.dolly,
                k.cam.tilt,
                k.cam.pan,
                v["interpolation"].as_str().unwrap_or(""),
                k.ease_in,
                k.ease_out
            ));
        }
        plan.insert("cles".into(), Value::Array(keys.iter().map(key_json).collect()));
        // Plan en Espace 3D : la caméra est libre dans l'espace (ses valeurs ne sont pas bridées à ce qui voit la feuille).
        // Le champ n'est écrit que pour ces plans : rien ne change pour les autres.
        if panel.espace {
            plan.insert("espace_3d".into(), Value::Bool(true));
        }
        // Une trajectoire de caméra (partie 149) ne pose aucune clé : comme l'effet vivant, elle s'exporte comme elle se voit, image
        // par image, même si ces valeurs ne sont pas demandées ; et le plan le dit.
        if panel.espace && panel.trajectoire_camera.is_some() {
            plan.insert("trajectoire_dans_l_espace".into(), Value::Bool(true));
        }
        if s.per_frame || panel.effet_camera.is_some() || (panel.espace && panel.trajectoire_camera.is_some()) {
            let mut frames = Vec::with_capacity(len);
            for f in 0..len {
                let c = doc.panel_camera_reelle(p, f);
                frames_csv.push_str(&format!("{},{f},{},{},{},{},{},{},{},{}\n", csv_text(&name), start + f, c.x, c.y, c.zoom, c.angle, c.dolly, c.tilt, c.pan));
                frames.push(frame_json(f, &c));
            }
            plan.insert("par_image".into(), Value::Array(frames));
        }
        out_plans.push(Value::Object(plan));
    }
    let doc_json = json!({
        "format": "shot-camera",
        "version": 1,
        "projet": title,
        "cadence": doc.fps,
        "cadre": { "largeur": doc.frame[0], "hauteur": doc.frame[1] },
        "feuille": { "largeur": doc.width, "hauteur": doc.height },
        "unites": {
            "x": "centre du cadre, en pixels de la feuille ; origine en haut à gauche de la feuille, y vers le bas",
            "y": "voir x",
            "zoom": "1 : le cadre fait la taille de l'image finale ; 2 : deux fois plus serré (focale doublée)",
            "rotation": "degrés, sens horaire à l'écran",
            "avancee": "pixels, travelling avant (négatif : recul)",
            "bascule": "degrés, positif : la caméra regarde vers le haut",
            "panoramique": "degrés, positif : la caméra regarde vers la droite",
            "image": "numéro d'image depuis 0, dans le plan",
            "interpolation": "lineaire : vitesse constante jusqu'à la clé suivante ; adoucie : accélère et ralentit sur adouci_sortie et adouci_entree images (adouci_ancien : un demi-intervalle)",
        },
        "plans": out_plans,
    });
    CameraExport { json: serde_json::to_string_pretty(&doc_json).expect("écriture du JSON") + "\n", keys_csv, frames_csv: s.per_frame.then_some(frames_csv) }
}

/// Les chemins des fichiers pour le JSON `json` : le CSV des clés et celui des images sont à côté, avec le même nom de base.
pub fn csv_paths(json: &Path) -> (PathBuf, PathBuf) {
    let stem = json.file_stem().map_or_else(|| "camera".to_string(), |s| s.to_string_lossy().into_owned());
    let dir = json.parent().unwrap_or(Path::new("."));
    (dir.join(format!("{stem}_cles.csv")), dir.join(format!("{stem}_images.csv")))
}

/// Écrit l'export dans `json` (et les CSV à côté si `s.csv`) ; rend les fichiers écrits.
pub fn write(export: &CameraExport, json: &Path, s: &CameraSettings) -> Result<Vec<PathBuf>, String> {
    let mut written = vec![json.to_path_buf()];
    crate::export::write_atomic(json, export.json.as_bytes())?;
    if s.csv {
        let (keys, frames) = csv_paths(json);
        crate::export::write_atomic(&keys, export.keys_csv.as_bytes())?;
        written.push(keys);
        if let Some(text) = &export.frames_csv {
            crate::export::write_atomic(&frames, text.as_bytes())?;
            written.push(frames);
        }
    }
    Ok(written)
}
