//! La tâche de l'export de la fiche technique (`fiche.rs` décrit, `ficheformats.rs` met en forme, ceci
//! écrit).
//!
//! Comme l'export pour la relecture et celui du storyboard : l'interface ne calcule que les images des
//! vignettes, quelques-unes par image d'écran (40 ms au plus, `ExportJob::collect`) ; leur réduction en
//! PNG, les fichiers de vignettes et la fiche elle-même s'écrivent sur un fil à part. Une fiche sans
//! colonne « Vignette » n'a aucune image à calculer : elle s'écrit tout de suite.
//!
//! # Écriture
//!
//! - **Atomique** : chaque fichier (la fiche, chaque vignette) est écrit sous un nom en `.part` puis
//!   renommé. Un export arrêté, un disque plein ou un fichier verrouillé ne laissent ni demi-fichier ni
//!   fiche précédente abîmée.
//! - **Jamais en silence sur un fichier ouvert** : Excel garde ouvert (et verrouillé) le classeur qu'il
//!   montre. Avant de commencer, SHOT essaie d'ouvrir la fiche existante en écriture ; s'il ne peut pas, il le
//!   dit tout de suite (« ouvert dans un autre programme ») et n'écrit rien. Si le fichier se verrouille
//!   pendant l'export, le renommage final échoue : même message, la fiche d'avant est intacte, les fichiers
//!   `.part` sont effacés.
//! - **Annuler** (ou fermer SHOT) efface tout ce que cet export a commencé d'écrire : les `.part` et les
//!   vignettes déjà posées.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::Color32;
use image::ImageEncoder;

use crate::document::Document;
use crate::export::{ExportJob, Progress};
use crate::fiche::{self, Fiche};
use crate::ficheformats::{self, Format, Separateur};

/// Le compte rendu d'un export réussi.
#[derive(Clone, Debug)]
pub struct Report {
    pub fichier: PathBuf,
    pub lignes: usize,
    pub vignettes: usize,
    /// Le dossier des vignettes, s'il y en a.
    pub dossier_vignettes: Option<PathBuf>,
    pub octets: u64,
    pub avertissements: Vec<String>,
}

/// `<nom>.xlsx` -> `<nom>.xlsx.part`.
pub fn part_de(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

/// Le dossier des vignettes d'une fiche écrite en `fichier` : `vignettes/` à côté.
pub fn dossier_vignettes(fichier: &Path) -> PathBuf {
    fichier.parent().unwrap_or(Path::new(".")).join(fiche::DOSSIER_VIGNETTES)
}

/// Une erreur d'écriture, dite en français ; `ouvert` : le fichier est tenu par un autre programme.
pub fn explique(path: &Path, e: &io::Error) -> String {
    let nom = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    // 5 : accès refusé, 32 : violation de partage, 33 : verrouillé (Windows).
    if matches!(e.raw_os_error(), Some(5 | 32 | 33)) || e.kind() == io::ErrorKind::PermissionDenied {
        format!("« {nom} » est ouvert dans un autre programme (Excel ?) ou protégé en écriture : ferme-le puis recommence. Rien n'a été modifié.")
    } else {
        format!("{} : {e}", path.display())
    }
}

/// Peut-on écrire `path` ? Un fichier existant doit pouvoir s'ouvrir en écriture (sans le modifier) ; sinon le
/// dossier doit exister.
pub fn verifie_ecriture(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        return Err(format!("« {} » est un dossier.", path.display()));
    }
    if path.exists() {
        return fs::OpenOptions::new().write(true).open(path).map(|_| ()).map_err(|e| explique(path, &e));
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    if !dir.is_dir() {
        return Err(format!("Le dossier « {} » n'existe pas.", dir.display()));
    }
    Ok(())
}

/// Écrit `octets` sous `path` : un fichier `.part` puis le renommage.
pub fn ecrit_entier(path: &Path, octets: &[u8]) -> Result<(), String> {
    let part = part_de(path);
    fs::write(&part, octets).map_err(|e| {
        let _ = fs::remove_file(&part);
        explique(&part, &e)
    })?;
    if let Err(e) = fs::rename(&part, path) {
        let _ = fs::remove_file(&part);
        return Err(explique(path, &e));
    }
    Ok(())
}

/// Une image du film réduite à `largeur_max` pixels de large au plus, en PNG, sur fond blanc.
pub fn png_sur_blanc(img: &[Color32], size: [usize; 2], largeur_max: u32) -> Result<Vec<u8>, String> {
    let [w, h] = size;
    if w == 0 || h == 0 || img.len() < w * h {
        return Err("image invalide".into());
    }
    let rgb: Vec<u8> = img
        .iter()
        .take(w * h)
        .flat_map(|c| {
            // Couleurs prémultipliées posées sur du blanc.
            let blanc = 255 - c.a();
            [c.r().saturating_add(blanc), c.g().saturating_add(blanc), c.b().saturating_add(blanc)]
        })
        .collect();
    let mut image = image::RgbImage::from_raw(w as u32, h as u32, rgb).ok_or("image invalide")?;
    if image.width() > largeur_max {
        let nh = ((image.height() as f32 * largeur_max as f32 / image.width() as f32).round() as u32).max(1);
        image = image::imageops::resize(&image, largeur_max, nh, image::imageops::FilterType::Lanczos3);
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(image.as_raw(), image.width(), image.height(), image::ExtendedColorType::Rgb8).map_err(|e| e.to_string())?;
    Ok(png)
}

/// L'export en cours.
pub struct FicheJob {
    job: ExportJob,
    fichier: PathBuf,
    lignes: usize,
    vignettes: usize,
    avec_dossier: bool,
    /// Demande d'arrêt, lue par le fil d'écriture entre deux fichiers.
    arret: Arc<AtomicBool>,
    /// Posé quand la fiche est renommée à sa place : le travail est fini, même si l'on annule à cet instant.
    fini: Arc<AtomicBool>,
    /// Les avertissements du fil d'écriture (texte coupé…).
    avertissements: Arc<Mutex<Vec<String>>>,
    /// Les avertissements de la construction de la fiche.
    avertissements_fiche: Vec<String>,
    report: Option<Result<Report, String>>,
}

impl FicheJob {
    /// Prépare l'export de `fiche` vers `fichier`. Les images démarrent au premier `step`. Refuse tout de
    /// suite un fichier qu'on ne peut pas écrire.
    pub fn start(doc: &Document, fiche: Fiche, format: Format, sep: Separateur, fichier: &Path) -> Result<FicheJob, String> {
        verifie_ecriture(fichier)?;
        // Un `.part` resté d'un plantage n'a plus de valeur.
        let part = part_de(fichier);
        if part.exists() {
            fs::remove_file(&part).map_err(|e| explique(&part, &e))?;
        }
        let frames = fiche.images_vignettes();
        let fichiers = fiche.fichiers_vignettes();
        let (lignes, vignettes) = (fiche.lignes.len(), frames.len());
        let avec_dossier = vignettes > 0;
        let frame = doc.frame;
        let arret = Arc::new(AtomicBool::new(false));
        let fini = Arc::new(AtomicBool::new(false));
        let avertissements = Arc::new(Mutex::new(Vec::new()));
        let avertissements_fiche = fiche.avertissements.clone();
        let (a, fi, av) = (arret.clone(), fini.clone(), avertissements.clone());
        let cible = fichier.to_path_buf();
        let job = ExportJob::collect(
            format!("Fiche technique ({lignes} lignes)"),
            "Vignette",
            &part_de(fichier),
            frames,
            move |_, px| png_sur_blanc(px, frame, fiche::VIGNETTE_LARGEUR),
            move |pngs: Vec<Vec<u8>>| ecrire(&fiche, &pngs, &fichiers, format, sep, &cible, &a, &fi, &av),
        );
        Ok(FicheJob { job, fichier: fichier.to_path_buf(), lignes, vignettes, avec_dossier, arret, fini, avertissements, avertissements_fiche, report: None })
    }

    /// Calcule des images pendant `budget` au plus (à appeler à chaque image d'écran).
    pub fn step(&mut self, doc: &Document, budget: Duration) {
        self.job.step(doc, budget);
        if self.report.is_none() {
            if let Some(outcome) = &self.job.outcome {
                self.report = Some(match outcome {
                    Ok(_) => Ok(self.rapport()),
                    Err(e) => {
                        self.nettoie();
                        Err(e.clone())
                    }
                });
            }
        }
    }

    fn rapport(&self) -> Report {
        let mut avertissements = self.avertissements_fiche.clone();
        avertissements.extend(self.avertissements.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned());
        Report {
            fichier: self.fichier.clone(),
            lignes: self.lignes,
            vignettes: self.vignettes,
            dossier_vignettes: self.avec_dossier.then(|| dossier_vignettes(&self.fichier)),
            octets: fs::metadata(&self.fichier).map_or(0, |m| m.len()),
            avertissements,
        }
    }

    pub fn progress(&self) -> Progress {
        self.job.progress()
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.job.remaining()
    }

    /// SHOT calcule encore les images : le projet ne doit pas changer.
    pub fn rendering(&self) -> bool {
        self.report.is_none() && matches!(self.job.progress(), Progress::Running(..))
    }

    pub fn running(&self) -> bool {
        self.report.is_none()
    }

    /// Le compte rendu, quand l'export est fini (réussi, raté ou arrêté).
    pub fn report(&self) -> Option<&Result<Report, String>> {
        self.report.as_ref()
    }

    /// Arrête l'export : les fichiers `.part` et les vignettes déjà posées sont effacés (une fiche déjà
    /// exportée sous le même nom reste). Si la fiche venait d'être posée, l'export est réussi.
    pub fn cancel(&mut self) {
        if self.report.is_some() {
            return;
        }
        self.arret.store(true, Ordering::SeqCst);
        self.job.cancel();
        if self.fini.load(Ordering::SeqCst) {
            self.report = Some(Ok(self.rapport()));
        } else {
            self.nettoie();
            self.report = Some(Err("Export annulé.".into()));
        }
    }

    /// Efface le `.part` de la fiche (le fil d'écriture a effacé ses vignettes quand il s'est arrêté).
    fn nettoie(&self) {
        let _ = fs::remove_file(part_de(&self.fichier));
    }
}

impl Drop for FicheJob {
    fn drop(&mut self) {
        // Fermeture de SHOT pendant l'export : pas de demi-fichier.
        if self.report.is_none() {
            self.cancel();
        }
    }
}

/// Une fois toutes les vignettes arrivées (fil d'écriture) : les fichiers PNG, puis la fiche. Au moindre
/// échec ou arrêt, ce que ce fil a écrit est effacé.
#[allow(clippy::too_many_arguments)]
fn ecrire(f: &Fiche, pngs: &[Vec<u8>], fichiers: &[String], format: Format, sep: Separateur, cible: &Path, arret: &AtomicBool, fini: &AtomicBool, avertissements: &Mutex<Vec<String>>) -> Result<(), String> {
    let mut posees: Vec<PathBuf> = Vec::new();
    let resultat = (|| -> Result<(), String> {
        let verifie = || if arret.load(Ordering::SeqCst) { Err("annulé".to_string()) } else { Ok(()) };
        if !pngs.is_empty() {
            let dossier = dossier_vignettes(cible);
            fs::create_dir_all(&dossier).map_err(|e| explique(&dossier, &e))?;
            for (png, nom) in pngs.iter().zip(fichiers) {
                verifie()?;
                let chemin = dossier.join(nom);
                // (Une vignette d'une fiche précédente est remplacée, jamais effacée si l'export échoue.)
                let existait = chemin.exists();
                ecrit_entier(&chemin, png)?;
                if !existait {
                    posees.push(chemin);
                }
            }
        }
        verifie()?;
        let octets = match format {
            Format::Csv => ficheformats::csv(f, sep),
            Format::Xlsx => {
                let c = ficheformats::xlsx(f, pngs)?;
                avertissements.lock().unwrap_or_else(|e| e.into_inner()).extend(c.avertissements);
                c.octets
            }
        };
        verifie()?;
        // La fiche : `.part` puis renommage (échoue, sans rien casser, si Excel la tient ouverte).
        let part = part_de(cible);
        fs::write(&part, &octets).map_err(|e| explique(&part, &e))?;
        verifie().inspect_err(|_| {
            let _ = fs::remove_file(&part);
        })?;
        if let Err(e) = fs::rename(&part, cible) {
            let _ = fs::remove_file(&part);
            return Err(explique(cible, &e));
        }
        fini.store(true, Ordering::SeqCst);
        Ok(())
    })();
    if resultat.is_err() {
        for p in &posees {
            let _ = fs::remove_file(p);
        }
        // (Le dossier, s'il est vide et que cet export l'a fait.)
        let _ = fs::remove_dir(dossier_vignettes(cible));
    }
    resultat
}
