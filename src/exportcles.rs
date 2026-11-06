//! « Images à la clé » (exports V1, partie 105) : une image par **dessin clé**, numérotée, pour les plans de l'étendue choisie.
//!
//! - **Une clé = un dessin à l'image où il commence.** Pas chaque image de son exposition : un dessin tenu sur 6 images ne donne qu'une image,
//!   celle où il apparaît ; un même dessin qui revient après un vide recommence une clé. Les clés viennent des calques que l'export des
//!   calques séparés écrirait (`render::separable_layers` : ni dossier, ni calque d'effet, ni calque caché, lui ou son dossier, ni calque
//!   d'annotation sauf « Avec les annotations ») qui ont des dessins ; un calque vidéo n'a pas de dessin à lui, donc pas de clé (ses images
//!   sont dans l'image composée, il n'en fait pas naître). Les pré / post-comportements (boucle, maintien) ne créent pas de clé non plus.
//! - **Numéro de clé** : le rang, dans le plan, de l'image où au moins un calque commence un dessin (`K01`, `K02`…). Le nombre de chiffres est
//!   le même pour tout l'export (au moins deux) : les fichiers se trient par plan puis par numéro de clé. Le numéro est celui du plan, pas celui
//!   du calque : l'image composée `K03` et le calque seul `K03` sont de la même image.
//! - **Sans l'option par calque** : l'image composée (ce que la caméra filme) à chaque image où une clé commence, sur n'importe quel calque :
//!   `SQ01_PL012A_K01.png`. PNG ou JPEG, fond transparent si on le veut, taille au choix : comme les autres images par plan.
//! - **Avec l'option par calque** : pour chaque calque qui commence un dessin à cette image, le calque seul, fond transparent, en PNG, à la
//!   taille choisie, caméra appliquée : `SQ01_PL012A_K03_Personnage.png` (le nom du calque en suffixe ; si le modèle de noms a `{calque}`, c'est
//!   lui qui place le nom du calque). Un seul dossier : le tri donne plan, clé, puis calque.
//!
//! Même tâche de fond que tous les exports : les images se calculent et s'écrivent sur d'autres fils, sur une copie du projet ; annuler
//! efface les fichiers déjà écrits ; rien ne modifie le projet.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::Color32;

use crate::document::Document;
use crate::export::ExportJob;
use crate::exportlayers::Porte;
use crate::exportrender::{Calcul, Image};
use crate::exportstills::{Still, StillFormat, StillsPer, StillsSettings};
use crate::render;

/// Une clé : l'image du plan où commence un dessin, et les calques (rang dans le plan) dont un dessin commence là.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cle {
    pub plan: usize,
    /// Rang de la clé dans le plan, à partir de 1.
    pub numero: usize,
    pub frame: usize,
    pub layers: Vec<usize>,
}

/// Les calques d'un plan qui donnent des clés : ceux de l'export des calques séparés, avec des dessins, sans la vidéo.
fn calques_a_cles(doc: &Document, p: usize, annotations: bool) -> Vec<usize> {
    let panel = &doc.panels[p];
    render::separable_layers(panel, annotations).into_iter().filter(|&li| panel.layers[li].video.is_none() && panel.layers[li].frames.iter().any(Option::is_some)).collect()
}

/// Les clés du plan `p`, dans l'ordre des images.
pub fn cles_du_plan(doc: &Document, p: usize, annotations: bool) -> Vec<Cle> {
    if p >= doc.panels.len() {
        return Vec::new();
    }
    let panel = &doc.panels[p];
    let n = panel.frame_count();
    let mut par_image: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for li in calques_a_cles(doc, p, annotations) {
        let frames = &panel.layers[li].frames;
        for f in 0..frames.len().min(n) {
            if frames[f].is_some() && (f == 0 || frames[f - 1] != frames[f]) {
                par_image.entry(f).or_default().push(li);
            }
        }
    }
    par_image.into_iter().enumerate().map(|(k, (frame, layers))| Cle { plan: p, numero: k + 1, frame, layers }).collect()
}

/// Le nombre de clés des plans `plans` et le nombre de plans qui n'en ont aucune (rien de dessiné, ou rien de visible).
pub fn compter(doc: &Document, plans: &[usize], annotations: bool) -> (usize, usize) {
    let mut cles = 0;
    let mut sans = 0;
    for &p in plans {
        if p >= doc.panels.len() {
            continue;
        }
        let k = cles_du_plan(doc, p, annotations).len();
        cles += k;
        sans += usize::from(k == 0);
    }
    (cles, sans)
}

/// Le nombre de fichiers que l'option par calque écrirait (un par calque et par clé où il commence un dessin).
pub fn compter_calques(doc: &Document, plans: &[usize], annotations: bool) -> usize {
    plans.iter().map(|&p| cles_du_plan(doc, p, annotations).iter().map(|c| c.layers.len()).sum::<usize>()).sum()
}

/// Le nombre de chiffres du numéro de clé : le même pour tout l'export, au moins deux.
pub fn largeur(cles_max: usize) -> usize {
    cles_max.to_string().len().max(2)
}

/// Les noms (sans extension) des plans `plans`, deux jamais identiques même en ignorant les majuscules, à quoi s'ajoute le numéro de clé.
fn noms_de_plans(doc: &Document, plans: &[usize]) -> Vec<(usize, String)> {
    let mut vus: Vec<String> = Vec::new();
    plans
        .iter()
        .filter(|&&p| p < doc.panels.len())
        .map(|&p| {
            let base = crate::editorial::file_safe(&doc.nom_de_plan_avec(p, None, None));
            let mut nom = base.clone();
            let mut n = 1;
            while vus.contains(&nom.to_lowercase()) {
                n += 1;
                nom = format!("{base}_{n}");
            }
            vus.push(nom.to_lowercase());
            (p, nom)
        })
        .collect()
}

/// Les images composées : une par clé, nommées `<plan>_K01`… Comme `exportstills::items`, rend les images à calculer et leur nom.
pub fn items(doc: &Document, plans: &[usize], annotations: bool) -> Vec<Still> {
    let tous: Vec<Vec<Cle>> = plans.iter().map(|&p| cles_du_plan(doc, p, annotations)).collect();
    let w = largeur(tous.iter().map(Vec::len).max().unwrap_or(0));
    let mut out: Vec<Still> = Vec::new();
    let noms = noms_de_plans(doc, plans);
    for (p, base) in noms {
        let Some(k) = plans.iter().position(|&q| q == p) else { continue };
        for c in &tous[k] {
            out.push(Still { frame: (p, c.frame), name: format!("{base}_K{:0w$}", c.numero) });
        }
    }
    out
}

/// Une image de calque seul : l'image du plan, le calque (rang dans le plan) et le nom du fichier (sans extension).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleCalque {
    pub frame: (usize, usize),
    pub layer: usize,
    pub name: String,
}

/// Les images de calques seuls : pour chaque clé, un fichier par calque qui y commence un dessin.
pub fn items_calques(doc: &Document, plans: &[usize], annotations: bool) -> Vec<CleCalque> {
    let tous: Vec<Vec<Cle>> = plans.iter().map(|&p| cles_du_plan(doc, p, annotations)).collect();
    let w = largeur(tous.iter().map(Vec::len).max().unwrap_or(0));
    let modele = doc.modele_de_noms();
    let par_modele = crate::nommage::contient(&modele, crate::nommage::Jeton::Calque);
    let noms = noms_de_plans(doc, plans);
    let mut vus: Vec<String> = Vec::new();
    let mut out: Vec<CleCalque> = Vec::new();
    for (p, base) in noms {
        let Some(k) = plans.iter().position(|&q| q == p) else { continue };
        for c in &tous[k] {
            for &li in &c.layers {
                let calque = &doc.panels[p].layers[li].name;
                let propre = if par_modele { crate::editorial::file_safe(&doc.nom_avec_modele(&modele, p, None, Some(calque))) } else { base.clone() };
                let mut nom = if par_modele { format!("{propre}_K{:0w$}", c.numero) } else { format!("{propre}_K{:0w$}_{}", c.numero, crate::exportstems::file_name_of(calque, "Calque")) };
                // (Deux calques de même nom, ou deux plans de même nom : jamais le même fichier.)
                let base_nom = nom.clone();
                let mut n = 1;
                while vus.contains(&nom.to_lowercase()) {
                    n += 1;
                    nom = format!("{base_nom}_{n}");
                }
                vus.push(nom.to_lowercase());
                out.push(CleCalque { frame: (p, c.frame), layer: li, name: nom });
            }
        }
    }
    out
}

/// Comme `items`, avec la nomenclature choisie (`nomenclature.rs`) ; la forme d'origine sans réglage rend exactement `items`.
pub fn items_nom(doc: &Document, plans: &[usize], annotations: bool, nom: &crate::nomenclature::Nomenclature) -> Result<Vec<Still>, String> {
    if nom.est_defaut() {
        return Ok(items(doc, plans, annotations));
    }
    let (entrees, cadres, largeur) = entrees(doc, plans, annotations, false);
    let noms = nom.noms(doc, &entrees, largeur)?;
    Ok(cadres.into_iter().zip(noms).map(|(frame, name)| Still { frame, name }).collect())
}

/// Comme `items_calques`, avec la nomenclature choisie.
pub fn items_calques_nom(doc: &Document, plans: &[usize], annotations: bool, nom: &crate::nomenclature::Nomenclature) -> Result<Vec<CleCalque>, String> {
    if nom.est_defaut() {
        return Ok(items_calques(doc, plans, annotations));
    }
    let (entrees, cadres, largeur) = entrees(doc, plans, annotations, true);
    let noms = nom.noms(doc, &entrees, largeur)?;
    let calques: Vec<usize> = plans.iter().flat_map(|&p| cles_du_plan(doc, p, annotations).into_iter().flat_map(|c| c.layers)).collect();
    Ok(cadres.into_iter().zip(noms).zip(calques).map(|((frame, name), layer)| CleCalque { frame, layer, name }).collect())
}

/// Les noms à fabriquer pour les clés des plans `plans` (un par clé, ou un par calque et par clé), l'image de chacun et le nombre de chiffres
/// du numéro de clé.
pub fn entrees(doc: &Document, plans: &[usize], annotations: bool, par_calque: bool) -> (Vec<crate::nomenclature::Entree>, Vec<(usize, usize)>, usize) {
    use crate::nomenclature::Entree;
    let tous: Vec<Vec<Cle>> = plans.iter().map(|&p| cles_du_plan(doc, p, annotations)).collect();
    let largeur = largeur(tous.iter().map(Vec::len).max().unwrap_or(0));
    let mut e: Vec<Entree> = Vec::new();
    let mut cadres: Vec<(usize, usize)> = Vec::new();
    for (&p, cles) in plans.iter().zip(&tous) {
        for c in cles {
            if par_calque {
                for &li in &c.layers {
                    e.push(Entree { plan: p, cle: Some(c.numero), case: None, calque: Some(doc.panels[p].layers[li].name.clone()) });
                    cadres.push((p, c.frame));
                }
            } else {
                e.push(Entree { plan: p, cle: Some(c.numero), case: None, calque: None });
                cadres.push((p, c.frame));
            }
        }
    }
    (e, cadres, largeur)
}

/// La tâche d'export des images composées des clés : celle des images par plan, avec les noms des clés.
pub fn job(dir: &Path, items: &[Still], frame: [usize; 2], s: &StillsSettings, annotations: bool) -> ExportJob {
    let mut job = crate::exportstills::job(dir, items, frame, &StillsSettings { per: StillsPer::Cle, ..*s }, annotations);
    let n = items.len();
    job = job.avec_message(format!("{n} image{} à la clé exportée{} ({} × {} points) : {}", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }, crate::exportstills::still_size(s.size, frame)[0], crate::exportstills::still_size(s.size, frame)[1], dir.display()));
    job
}

/// La tâche d'export des calques seuls aux clés : `items` (voir `items_calques`) dans `dir`, en PNG transparent à la taille de `s`.
pub fn job_calques(dir: &Path, items: Vec<CleCalque>, frame: [usize; 2], s: &StillsSettings, annotations: bool) -> ExportJob {
    let files = items.len();
    let size = crate::exportstills::still_size(s.size, frame);
    let mut frames: Vec<(usize, usize)> = Vec::new();
    for i in &items {
        if frames.last() != Some(&i.frame) {
            frames.push(i.frame);
        }
    }
    let message = format!("{files} image{} de calque à la clé exportée{} ({} × {} points) : {}", if files > 1 { "s" } else { "" }, if files > 1 { "s" } else { "" }, size[0], size[1], dir.display());
    // (Un calque seul est toujours un PNG sur fond transparent.)
    let reglages = StillsSettings { per: StillsPer::Cle, format: StillFormat::Png, transparent: true, par_calque: true, ..*s };
    let written: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
    let (w_calc, w_clean) = (written.clone(), written);
    let porte: Arc<Porte> = Arc::default();
    let porte_nettoyage = porte.clone();
    let mut par_image: HashMap<(usize, usize), Vec<(usize, String)>> = HashMap::new();
    for i in items {
        par_image.entry(i.frame).or_default().push((i.layer, i.name));
    }
    let par_image = Arc::new(par_image);
    let liste = Arc::new(frames.clone());
    let d = dir.to_path_buf();
    let job = ExportJob::collect_with(
        format!("Export des calques à la clé ({files} image{})", if files > 1 { "s" } else { "" }),
        "Image",
        dir,
        frames,
        |_, _| Ok(()),
        |_: Vec<()>| Ok(()),
        move || {
            porte_nettoyage.fermer_et_attendre();
            for p in w_clean.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
                let _ = std::fs::remove_file(&p);
                let _ = std::fs::remove_file(crate::export::part_of(&p));
            }
        },
    );
    job.avec_message(message).with_calc(Box::new(move |doc: Arc<Document>, cancel: Arc<AtomicBool>, fail: Arc<Mutex<Option<String>>>| -> Arc<Calcul> {
        Arc::new(move |i: usize| -> Option<Image> {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let (p, f) = liste[i];
            if let Some(calques) = par_image.get(&(p, f)) {
                if let Err(e) = ecrire_image(&doc, p, f, calques, &d, &reglages, annotations, &w_calc, &cancel, &porte) {
                    fail.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(e);
                    return None;
                }
            }
            Some(Arc::new(vec![Color32::TRANSPARENT; 1]))
        })
    }))
}

/// Écrit les calques `calques` (rang, nom de fichier) du plan `p` à l'image `f`, chacun seul sur fond transparent, caméra appliquée.
#[allow(clippy::too_many_arguments)]
fn ecrire_image(doc: &Document, p: usize, f: usize, calques: &[(usize, String)], dir: &Path, s: &StillsSettings, annotations: bool, written: &Mutex<Vec<PathBuf>>, cancel: &AtomicBool, porte: &Porte) -> Result<(), String> {
    let frame = doc.frame;
    let size = crate::exportstills::still_size(s.size, frame);
    let plans = render::panel_layer_plans(doc, p, f, annotations);
    for (li, nom) in calques {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let pixels: Vec<Color32> = match plans.iter().find(|(l, _)| l == li) {
            Some((_, plan)) => render::camera_image(doc, plan, &doc.panel_camera(p, f)),
            None => vec![Color32::TRANSPARENT; frame[0] * frame[1]],
        };
        let bytes = crate::exportstills::encode_still(&pixels, frame, size, s)?;
        let path = dir.join(format!("{nom}.png"));
        let part = crate::export::part_of(&path);
        // Porte fermée (annulé ou raté pendant le calcul de l'image) : rien n'est écrit.
        if !porte.entrer() {
            return Ok(());
        }
        let res = (|| {
            written.lock().unwrap_or_else(|e| e.into_inner()).push(path.clone());
            std::fs::write(&part, &bytes).and_then(|()| std::fs::rename(&part, &path)).map_err(|e| {
                let _ = std::fs::remove_file(&part);
                format!("{} : {e}", path.display())
            })
        })();
        porte.sortir();
        res?;
    }
    Ok(())
}
