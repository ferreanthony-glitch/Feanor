//! « Images par plan » (exports V1, 2026-10-30) : une image par plan, ou par case (le début du plan et chacune de ses marques d'image),
//! en PNG ou en JPEG, nommées comme l'export pour le montage (`SQ01_PL012A`, `editorial::clip_name`), sur fond transparent si on le veut
//! (PNG), à la taille qu'on choisit.
//!
//! Même tâche de fond que tous les exports (`ExportJob::collect_with`) : l'interface ne calcule aucune image, elles se calculent et
//! s'écrivent sur d'autres fils, sur une copie du projet. Annuler efface les fichiers déjà écrits.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::export::ExportJob;
use crate::exportvideo::VideoSize;

/// Une image par plan ou par case.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum StillsPer {
    /// La première image de chaque plan.
    #[default]
    Plan,
    /// Le début du plan et chacune de ses marques d'image.
    Case,
    /// Chaque dessin clé, à l'image où il commence (`exportcles.rs`).
    Cle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum StillFormat {
    #[default]
    Png,
    Jpeg,
}

impl StillFormat {
    pub fn extension(self) -> &'static str {
        match self {
            StillFormat::Png => "png",
            StillFormat::Jpeg => "jpg",
        }
    }
}

/// Les choix de la fenêtre « Images par plan ».
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct StillsSettings {
    pub per: StillsPer,
    pub format: StillFormat,
    /// Fond transparent (PNG seulement : un JPEG n'a pas de transparence, il est posé sur du blanc).
    pub transparent: bool,
    pub size: VideoSize,
    /// Qualité du JPEG, de 50 à 100.
    pub quality: u8,
    /// À la clé seulement : chaque calque seul (PNG, fond transparent) au lieu de l'image composée (`exportcles.rs`).
    pub par_calque: bool,
}

impl Default for StillsSettings {
    fn default() -> Self {
        StillsSettings { per: StillsPer::Plan, format: StillFormat::Png, transparent: false, size: VideoSize::Project, quality: 92, par_calque: false }
    }
}

/// Une image à exporter : l'image du plan à rendre et le nom du fichier (sans extension).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Still {
    pub frame: (usize, usize),
    pub name: String,
}

/// Les images des plans `plans` : le nom de chacune est celui de l'export pour le montage (« SQ01_PL012A », avec « _V2 » pour un plan à
/// plusieurs versions), plus « _C02 », « _C03 »… pour les cases d'après la première ; deux noms jamais identiques.
pub fn items(doc: &Document, plans: &[usize], per: StillsPer) -> Vec<Still> {
    items_avec(doc, plans, per, false)
}

/// Comme `items` ; `annotations` : les calques d'annotation comptent pour les clés (`StillsPer::Cle`, `exportcles.rs`).
pub fn items_avec(doc: &Document, plans: &[usize], per: StillsPer, annotations: bool) -> Vec<Still> {
    if per == StillsPer::Cle {
        return crate::exportcles::items(doc, plans, annotations);
    }
    let mut out: Vec<Still> = Vec::new();
    for &p in plans {
        if p >= doc.panels.len() {
            continue;
        }
        let spans = crate::board::plan_boards(doc, p);
        let wanted = if per == StillsPer::Plan { 1 } else { spans.len() };
        for (k, (_, start, _)) in spans.into_iter().take(wanted).enumerate() {
            // (Le modèle de noms du projet, `nommage.rs` ; d'origine « SQ01_PL012A », puis « _C02 »… pour les cases d'après la première.)
            let base = crate::editorial::file_safe(&doc.nom_de_plan_avec(p, (per == StillsPer::Case).then_some(k + 1), None));
            let mut name = base.clone();
            let mut n = 1;
            while out.iter().any(|s| s.name == name) {
                n += 1;
                name = format!("{base}_{n}");
            }
            out.push(Still { frame: (p, start), name });
        }
    }
    out
}

/// Comme `items_avec`, avec la nomenclature choisie (`nomenclature.rs`) : la forme d'origine sans réglage rend exactement `items_avec`. Erreur :
/// le modèle ne se lit pas ou deux fichiers auraient le même nom.
pub fn items_nom(doc: &Document, plans: &[usize], per: StillsPer, annotations: bool, nom: &crate::nomenclature::Nomenclature) -> Result<Vec<Still>, String> {
    if per == StillsPer::Cle {
        return crate::exportcles::items_nom(doc, plans, annotations, nom);
    }
    if nom.est_defaut() {
        return Ok(items_avec(doc, plans, per, annotations));
    }
    let mut entrees = Vec::new();
    let mut cadres = Vec::new();
    for &p in plans.iter().filter(|&&p| p < doc.panels.len()) {
        let spans = crate::board::plan_boards(doc, p);
        let wanted = if per == StillsPer::Plan { 1 } else { spans.len() };
        for (k, (_, start, _)) in spans.into_iter().take(wanted).enumerate() {
            entrees.push(crate::nomenclature::Entree { plan: p, cle: None, case: (per == StillsPer::Case).then_some(k + 1), calque: None });
            cadres.push((p, start));
        }
    }
    let noms = nom.noms(doc, &entrees, 2)?;
    Ok(cadres.into_iter().zip(noms).map(|(frame, name)| Still { frame, name }).collect())
}

/// La taille d'une image : celle du cadre tel quel pour « Taille du projet » (pas de rognage à une taille paire, ce n'est pas une vidéo).
pub fn still_size(size: VideoSize, frame: [usize; 2]) -> [usize; 2] {
    if size == VideoSize::Project {
        [frame[0].max(1), frame[1].max(1)]
    } else {
        size.dimensions(frame)
    }
}

/// L'image `px` (prémultipliée, de la taille `frame`) en PNG ou en JPEG, à la taille `out`.
pub fn encode_still(px: &[Color32], frame: [usize; 2], out: [usize; 2], s: &StillsSettings) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let [w, h] = frame;
    let mut buf = Vec::new();
    match s.format {
        StillFormat::Png => {
            let raw: Vec<u8> = px.iter().flat_map(|c| c.to_array()).collect();
            let mut img = image::RgbaImage::from_raw(w as u32, h as u32, raw).ok_or("image invalide")?;
            if out != frame {
                // (Réduite ou agrandie sur les couleurs prémultipliées : pas de frange autour du dessin.)
                img = image::imageops::resize(&img, out[0] as u32, out[1] as u32, image::imageops::FilterType::Lanczos3);
            }
            let straight: Vec<u8> = img.pixels().flat_map(|p| crate::raster::unpremultiply(Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))).collect();
            image::codecs::png::PngEncoder::new(&mut buf).write_image(&straight, img.width(), img.height(), image::ExtendedColorType::Rgba8).map_err(|e| e.to_string())?;
        }
        StillFormat::Jpeg => {
            let rgb: Vec<u8> = px
                .iter()
                .flat_map(|c| {
                    let white = 255 - c.a();
                    [c.r().saturating_add(white), c.g().saturating_add(white), c.b().saturating_add(white)]
                })
                .collect();
            let mut img = image::RgbImage::from_raw(w as u32, h as u32, rgb).ok_or("image invalide")?;
            if out != frame {
                img = image::imageops::resize(&img, out[0] as u32, out[1] as u32, image::imageops::FilterType::Lanczos3);
            }
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, s.quality.clamp(50, 100)).encode(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8).map_err(|e| e.to_string())?;
        }
    }
    Ok(buf)
}

/// La tâche d'export des images `items` dans le dossier `dir`.
pub fn job(dir: &Path, items: &[Still], frame: [usize; 2], s: &StillsSettings, annotations: bool) -> ExportJob {
    job_avec_fin(dir, items, frame, s, annotations, || Ok(()))
}

/// Comme job, et in s'exécute une fois toutes les images écrites (la liste des plans y écrit son fichier : annulé, ni les images ni le fichier ne restent ;
/// si in échoue, les images sont effacées).
pub fn job_avec_fin(dir: &Path, items: &[Still], frame: [usize; 2], s: &StillsSettings, annotations: bool, fin: impl FnOnce() -> Result<(), String> + Send + 'static) -> ExportJob {
    let n = items.len();
    let frames: Vec<(usize, usize)> = items.iter().map(|i| i.frame).collect();
    let names: Arc<Vec<String>> = Arc::new(items.iter().map(|i| i.name.clone()).collect());
    let size = still_size(s.size, frame);
    let settings = *s;
    let written: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
    let (w_encode, w_clean) = (written.clone(), written.clone());
    let d = dir.to_path_buf();
    let ext = s.format.extension();
    let message = format!("{n} image{} par plan exportée{} ({} × {} points) : {}", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }, size[0], size[1], dir.display());
    let job = ExportJob::collect_with(
        format!("Export de {n} image{} par plan", if n > 1 { "s" } else { "" }),
        "Image",
        dir,
        frames,
        move |k, px| -> Result<(), String> {
            let path = d.join(format!("{}.{ext}", names[k]));
            let bytes = encode_still(px, frame, size, &settings)?;
            w_encode.lock().unwrap_or_else(|e| e.into_inner()).push(path.clone());
            crate::export::write_atomic(&path, &bytes)
        },
        move |_: Vec<()>| fin(),
        move || {
            for p in w_clean.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
                let _ = std::fs::remove_file(p);
            }
        },
    );
    let job = if s.transparent && s.format == StillFormat::Png { job.sans_papier() } else { job };
    job.avec_annotations(annotations).avec_message(message)
}
