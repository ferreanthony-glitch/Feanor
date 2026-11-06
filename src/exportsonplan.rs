//! « Son par plan » (exports V1, partie 105) : un fichier son par plan de l'étendue choisie, **le mixage de ce qui s'entend pendant ce plan**,
//! en WAV ou en AIFF, 16 ou 24 bits, toujours à 48 kHz stéréo (comme « Son par piste », `exportstems.rs` : mêmes écritures).
//!
//! - **Bornes exactes** : le fichier d'un plan commence à l'échantillon où le plan commence dans le montage et dure exactement le plan
//!   (`export::sample_span`, la même règle que le son du film : la somme des fichiers des plans qui se suivent est exactement le son du film,
//!   à l'échantillon près, même quand une image ne tombe pas sur un nombre entier d'échantillons).
//! - **Ce qui s'entend** : le mixage du montage (volume des pistes et général, fondus, clés de volume, égaliseur, écho ; pistes muettes
//!   retirées, solo respecté : `audio::mix_clips_blocking`). Un son qui chevauche une coupe est coupé au bord du plan : les deux fichiers
//!   en gardent chacun sa part.
//! - **Transitions** : elles ne changent rien. Ici une transition ne déplace aucun plan et ne touche pas au son (`transitions.rs` : la durée
//!   du film ne change pas, le son reste calé) ; le plan sortant ne déborde donc pas sur le suivant, aucune poignée n'est ajoutée, aucun
//!   fondu enchaîné sonore n'est fait.
//! - **Un plan où rien ne s'entend n'est pas écrit** (comme une piste sans extrait dans « Son par piste ») ; la fenêtre le dit.
//! - **Noms** : ceux des images par plan (modèle de noms du projet, `nommage.rs`), `SQ01_PL012A.wav`, deux jamais identiques.
//!
//! Même tâche de fond que les autres exports sans image (`ExportJob::task`) : sur une copie du projet, annulable ; annuler ou rater efface
//! les fichiers déjà écrits.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::audio::{self, MixClip};
use crate::document::Document;
use crate::export;
use crate::exportstems::{self, AudioFormat};

/// Un fichier à écrire : le plan, son nom de fichier et les échantillons qu'il couvre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SonDePlan {
    pub plan: usize,
    pub file: String,
    pub start: u64,
    pub len: u64,
}

/// Les plans de `plans` où quelque chose s'entend, avec leur nom de fichier et leurs bornes. `clips` : le mixage du montage.
pub fn plan_avec(doc: &Document, plans: &[usize], format: AudioFormat, clips: &[MixClip]) -> Vec<SonDePlan> {
    let ext = format.container.extension();
    let mut vus: Vec<String> = Vec::new();
    let mut out: Vec<SonDePlan> = Vec::new();
    for &p in plans {
        if p >= doc.panels.len() {
            continue;
        }
        let (start, len) = export::sample_span(doc.fps, doc.panel_start(p), doc.panels[p].frame_count());
        if len == 0 || !export::has_sound(clips, start, len) {
            continue;
        }
        let base = crate::editorial::file_safe(&doc.nom_de_plan_avec(p, None, None));
        let mut nom = base.clone();
        let mut n = 1;
        while vus.contains(&nom.to_lowercase()) {
            n += 1;
            nom = format!("{base}_{n}");
        }
        vus.push(nom.to_lowercase());
        out.push(SonDePlan { plan: p, file: format!("{nom}.{ext}"), start, len });
    }
    out
}

/// Comme `plan_avec`, avec les noms de la nomenclature choisie (`nomenclature.rs`) ; la forme d'origine sans réglage rend exactement `plan_avec`.
pub fn plan_nom(doc: &Document, plans: &[usize], format: AudioFormat, clips: &[MixClip], nom: &crate::nomenclature::Nomenclature) -> Result<Vec<SonDePlan>, String> {
    let mut v = plan_avec(doc, plans, format, clips);
    if nom.est_defaut() {
        return Ok(v);
    }
    let entrees: Vec<crate::nomenclature::Entree> = v.iter().map(|s| crate::nomenclature::Entree { plan: s.plan, cle: None, case: None, calque: None }).collect();
    let noms = nom.noms(doc, &entrees, 2)?;
    for (s, n) in v.iter_mut().zip(noms) {
        s.file = format!("{n}.{}", format.container.extension());
    }
    Ok(v)
}

/// Comme `plan_avec`, avec le mixage du projet (sans attendre les effets : sert à savoir quels plans ont du son).
pub fn plan(doc: &Document, plans: &[usize], format: AudioFormat) -> Vec<SonDePlan> {
    plan_avec(doc, plans, format, &audio::mix_clips(doc))
}

/// Les plans de `plans` où quelque chose s'entend (pour la fenêtre : sans les noms).
pub fn compter(doc: &Document, plans: &[usize]) -> usize {
    let clips = audio::mix_clips(doc);
    plans.iter().filter(|&&p| p < doc.panels.len()).filter(|&&p| {
        let (start, len) = export::sample_span(doc.fps, doc.panel_start(p), doc.panels[p].frame_count());
        len > 0 && export::has_sound(&clips, start, len)
    }).count()
}

/// Écrit les fichiers `files` dans `dir` ; les fichiers déjà écrits sont effacés si l'on annule ou si l'un ne s'écrit pas.
pub fn write_all(doc: &Document, dir: &Path, files: &[SonDePlan], format: AudioFormat, cancel: &AtomicBool) -> Result<Vec<PathBuf>, String> {
    let clips = audio::mix_clips_blocking(doc);
    let mut written: Vec<PathBuf> = Vec::new();
    let efface = |written: &[PathBuf]| written.iter().for_each(|p| drop(std::fs::remove_file(p)));
    for f in files {
        if cancel.load(Ordering::Relaxed) {
            efface(&written);
            return Err("annulé".into());
        }
        let path = dir.join(&f.file);
        match exportstems::write_audio_until(&path, clips.clone(), f.start, f.len, format, Some(cancel)) {
            Ok(true) => written.push(path),
            Ok(false) => {
                efface(&written);
                return Err("annulé".into());
            }
            Err(e) => {
                efface(&written);
                return Err(format!("{} : {e}", path.display()));
            }
        }
    }
    Ok(written)
}
