//! « Calques séparés » (exports V1, 2026-10-30) : pour l'étendue choisie, **un dossier par plan** (nommé comme le plan : `SQ01_PL012A`) et,
//! dedans, **une suite d'images PNG transparentes par calque visible** (`01_Fond_0001.png`, `02_Personnage_0001.png`…), pour reprendre
//! les calques dans un logiciel de compositing.
//!
//! - **Caméra appliquée** : chaque image est ce que la caméra filme (la taille de l'image finale), exactement comme dans le film ; posées
//!   l'une sur l'autre dans l'ordre des numéros, en mode normal, les images d'un plan redonnent l'image du film (essai pixel pour pixel).
//! - **Sans caméra** (« feuille entière ») : chaque image est la feuille entière du calque, à la taille de la feuille.
//! - Un calque qui n'a rien à une image écrit une image transparente : la suite reste continue de la première à la dernière image du plan.
//! - Pas écrits : les dossiers, les calques cachés (eux ou leur dossier) et les calques d'effet (ils agissent sur ce qui est dessous : ils
//!   n'ont pas d'image à eux ; leur effet n'est donc pas dans les images des calques qu'ils couvrent). Les calques d'annotation ne le sont que
//!   si « Avec les annotations » est coché.
//!
//! Même tâche de fond que tous les exports : les images se calculent et s'écrivent sur d'autres fils, sur une copie du projet ; annuler
//! efface les fichiers déjà écrits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::export::ExportJob;
use crate::exportrender::{Calcul, Image};
use crate::render;

/// Les choix de la fenêtre « Calques séparés ».
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LayersSettings {
    /// La caméra est appliquée (la taille de l'image finale) ; sinon : la feuille entière de chaque calque.
    pub with_camera: bool,
}

impl Default for LayersSettings {
    fn default() -> Self {
        LayersSettings { with_camera: true }
    }
}

/// Un plan à exporter : son rang, le nom de son dossier et ses calques (rang du calque dans le plan, début du nom des fichiers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanLayers {
    pub panel: usize,
    pub folder: String,
    pub layers: Vec<(usize, String)>,
}

/// Les plans `plans` et leurs calques : seuls ceux qui ont des images (ou une couleur unie) sont écrits, numérotés de bas en haut
/// (`01_Fond`, `02_Personnage`…).
pub fn plans_info(doc: &Document, plans: &[usize], annotations: bool) -> Vec<PlanLayers> {
    let mut used: Vec<String> = Vec::new();
    plans
        .iter()
        .filter(|&&p| p < doc.panels.len())
        .map(|&p| {
            let panel = &doc.panels[p];
            let layers = render::separable_layers(panel, annotations)
                .into_iter()
                .filter(|&li| panel.layers[li].frames.iter().any(Option::is_some) || panel.layers[li].unie.is_some())
                .enumerate()
                .map(|(rank, li)| (li, format!("{:02}_{}", rank + 1, crate::exportstems::file_name_of(&panel.layers[li].name, "Calque"))))
                .collect();
            // (Deux plans de même nom — deux cases d'une même scène — n'écrivent pas dans le même dossier.)
            let base = crate::editorial::file_safe(&crate::editorial::clip_name(doc, p));
            let mut folder = base.clone();
            let mut n = 1;
            while used.contains(&folder) {
                n += 1;
                folder = format!("{base}_{n}");
            }
            used.push(folder.clone());
            PlanLayers { panel: p, folder, layers }
        })
        .collect()
}

/// Les images de calques que l'export écrira : une par calque et par image des plans `frames`.
pub fn file_count(info: &[PlanLayers], frames: &[(usize, usize)]) -> usize {
    frames.iter().map(|(p, _)| info.iter().find(|i| i.panel == *p).map_or(0, |i| i.layers.len())).sum()
}

/// Les calques que l'export laisse de côté dans les plans `plans` parce qu'ils sont des calques d'effet.
pub fn effect_layers(doc: &Document, plans: &[usize]) -> usize {
    plans.iter().filter(|&&p| p < doc.panels.len()).map(|&p| doc.panels[p].layers.iter().filter(|l| l.effet.is_some() && !l.annotation).count()).sum()
}

/// Écrit les images de tous les calques du plan `pl`, image `f` du plan, dans son dossier de `dir`.
#[allow(clippy::too_many_arguments)]
pub fn write_frame(doc: &Document, pl: &PlanLayers, f: usize, dir: &Path, with_camera: bool, annotations: bool, written: &Mutex<Vec<PathBuf>>, cancel: &AtomicBool) -> Result<(), String> {
    let folder = dir.join(&pl.folder);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{} : {e}", folder.display()))?;
    let plans = render::panel_layer_plans(doc, pl.panel, f, annotations);
    let (w, h) = if with_camera { (doc.frame[0], doc.frame[1]) } else { (doc.width, doc.height) };
    for (li, stem) in &pl.layers {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let pixels: Vec<Color32> = match plans.iter().find(|(l, _)| l == li) {
            Some((_, plan)) if with_camera => render::camera_image(doc, plan, &doc.panel_camera(pl.panel, f)),
            Some((_, plan)) => render::composite_full(doc, plan),
            None => vec![Color32::TRANSPARENT; w * h],
        };
        let path = folder.join(format!("{stem}_{:04}.png", f + 1));
        let part = crate::export::part_of(&path);
        written.lock().unwrap_or_else(|e| e.into_inner()).push(path.clone());
        crate::io::write_png(&part, &pixels, w, h).and_then(|()| std::fs::rename(&part, &path).map_err(|e| e.to_string())).map_err(|e| {
            let _ = std::fs::remove_file(&part);
            format!("{} : {e}", path.display())
        })?;
    }
    Ok(())
}

/// La tâche d'export : les images `frames` (plan, image du plan) des plans de `info`, dans `dir`.
pub fn job(dir: &Path, info: Vec<PlanLayers>, frames: Vec<(usize, usize)>, s: &LayersSettings, annotations: bool) -> ExportJob {
    let files = file_count(&info, &frames);
    let plans = info.len();
    let message = format!("{files} image{} de calques exportée{} pour {plans} plan{} : {}", if files > 1 { "s" } else { "" }, if files > 1 { "s" } else { "" }, if plans > 1 { "s" } else { "" }, dir.display());
    let written: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
    let (w_calc, w_clean) = (written.clone(), written);
    let folders: Vec<PathBuf> = info.iter().map(|i| dir.join(&i.folder)).collect();
    let by_panel: Arc<HashMap<usize, PlanLayers>> = Arc::new(info.into_iter().map(|i| (i.panel, i)).collect());
    let list = Arc::new(frames.clone());
    let (d, with_camera) = (dir.to_path_buf(), s.with_camera);
    let job = ExportJob::collect_with(
        format!("Export des calques séparés ({} image{})", frames.len(), if frames.len() > 1 { "s" } else { "" }),
        "Image",
        dir,
        frames,
        |_, _| Ok(()),
        |_: Vec<()>| Ok(()),
        move || {
            for p in w_clean.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
                let _ = std::fs::remove_file(&p);
                let _ = std::fs::remove_file(crate::export::part_of(&p));
            }
            // (Les dossiers de plans restés vides s'en vont aussi.)
            for f in &folders {
                let _ = std::fs::remove_dir(f);
            }
        },
    );
    job.avec_message(message).with_calc(Box::new(move |doc: Arc<Document>, cancel: Arc<AtomicBool>, fail: Arc<Mutex<Option<String>>>| -> Arc<Calcul> {
        Arc::new(move |i: usize| -> Option<Image> {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let (p, f) = list[i];
            if let Some(pl) = by_panel.get(&p) {
                if let Err(e) = write_frame(&doc, pl, f, &d, with_camera, annotations, &w_calc, &cancel) {
                    let mut slot = fail.lock().unwrap_or_else(|e| e.into_inner());
                    slot.get_or_insert(e);
                    return None;
                }
            }
            // (Rien à passer à l'écriture : les images sont déjà écrites ; un pixel suffit au passeur.)
            Some(Arc::new(vec![Color32::TRANSPARENT; 1]))
        })
    }))
}
