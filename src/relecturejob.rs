//! La tâche de l'export « Pour relecture » (`relecture.rs` décrit, ceci écrit).
//!
//! Comme l'export du film et celui pour le montage : l'interface ne calcule aucune image. Le projet est copié au
//! départ (`exportrender.rs`), les images se calculent sur d'autres fils, et leur réduction en JPEG, la page HTML et
//! le PDF se font à côté (`ExportJob::collect`), avec une barre d'avancement et un bouton « Arrêter ». Le projet
//! n'est retenu que le temps de la copie (`RelectureJob::rendering`, quelques images d'écran).
//!
//! Les fichiers sont d'abord écrits sous un nom en `.part` puis renommés : un export arrêté ou raté ne laisse
//! ni demi-fichier, ni ne détruit une page déjà exportée sous le même nom.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::document::Document;
use crate::export::{self, ExportJob, Progress};
use crate::pdf;
use crate::relecture::{self, Meta, Options, Package};

/// Le compte rendu d'un export réussi.
#[derive(Clone, Debug)]
pub struct Report {
    pub html: PathBuf,
    pub pdf: Option<PathBuf>,
    pub plans: usize,
    /// Images de storyboard dans la page.
    pub stills: usize,
    /// Octets de la page.
    pub bytes: u64,
}

/// Une image calculée, réduite.
struct Encoded {
    jpeg: Vec<u8>,
    pdf: Option<pdf::BoardImage>,
}

pub struct RelectureJob {
    job: ExportJob,
    html: PathBuf,
    pdf: Option<PathBuf>,
    plans: usize,
    stills: usize,
    report: Option<Result<Report, String>>,
}

/// `<nom>.html` -> `<nom>.html.part`.
fn part_of(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

/// Écrit `bytes` sous `path` : un fichier `.part` puis le renommage.
fn write_whole(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let part = part_of(path);
    std::fs::write(&part, bytes).map_err(|e| format!("{} : {e}", part.display()))?;
    if let Err(e) = std::fs::rename(&part, path) {
        // (Un fichier de même nom ouvert ailleurs : on le dit, et le travail n'est pas perdu dans `.part`.)
        let _ = std::fs::remove_file(&part);
        return Err(format!("{} : {e}", path.display()));
    }
    Ok(())
}

impl RelectureJob {
    /// Prépare l'export des plans `panels` vers `html` (et son PDF, `<nom>.pdf` à côté, si `opts.pdf`). Les
    /// images démarrent au premier `step`.
    pub fn start(doc: &Document, panels: &[usize], opts: &Options, meta: Meta, html: &Path) -> Result<RelectureJob, String> {
        let (package, frames) = relecture::build(doc, panels, opts, meta);
        if package.plans.is_empty() {
            return Err("Aucun plan à exporter.".into());
        }
        let pdf_path = opts.pdf.then(|| html.with_extension("pdf"));
        let (plans, stills) = (package.plans.len(), package.stills);
        let frame = doc.frame;
        let want_pdf = opts.pdf;
        let title = package.meta.title.clone();
        let (shots, boards) = export::storyboard_of_with(doc, panels, opts.comments);
        debug_assert_eq!(shots.len(), stills);
        let aspect = frame[0] as f32 / frame[1].max(1) as f32;
        let (html_path, pdf_out) = (html.to_path_buf(), pdf_path.clone());
        let n_stills = stills;
        let job = ExportJob::collect(
            format!("Export pour relecture ({})", relecture::plural(plans, "plan", "plans")),
            "Image",
            &part_of(html),
            frames,
            move |i, px| {
                let still = i < n_stills;
                let (width, quality) = if still { (relecture::STILL_WIDTH, relecture::STILL_QUALITY) } else { (relecture::PLAY_WIDTH, relecture::PLAY_QUALITY) };
                let (_, _, jpeg) = export::jpeg_on_white(px, frame, width, quality)?;
                let pdf = if still && want_pdf { Some(export::board_image(px, frame)?) } else { None };
                Ok(Encoded { jpeg, pdf })
            },
            move |all: Vec<Encoded>| finish(package, all, &html_path, pdf_out.as_deref(), &title, &boards, aspect),
        );
        Ok(RelectureJob { job, html: html.to_path_buf(), pdf: pdf_path, plans, stills, report: None })
    }

    /// Calcule des images pendant `budget` au plus (à appeler à chaque image d'écran).
    pub fn step(&mut self, doc: &Document, budget: Duration) {
        self.job.step(doc, budget);
        if self.report.is_none() {
            if let Some(outcome) = &self.job.outcome {
                self.report = Some(match outcome {
                    Ok(_) => {
                        let bytes = std::fs::metadata(&self.html).map_or(0, |m| m.len());
                        Ok(Report { html: self.html.clone(), pdf: self.pdf.clone(), plans: self.plans, stills: self.stills, bytes })
                    }
                    Err(e) => {
                        self.remove_parts();
                        Err(e.clone())
                    }
                });
            }
        }
    }

    pub fn progress(&self) -> Progress {
        self.job.progress()
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.job.remaining()
    }

    /// Les dessins `ids` sont restés en rendu rapide : l'export les veut exacts (refaits dans le cliché, sur un autre fil).
    pub fn with_exact(mut self, ids: Vec<crate::document::CelId>) -> RelectureJob {
        self.job.set_exact(ids);
        self
    }

    /// L'export copie encore le projet (les premières images d'écran) : il ne doit pas changer, ni d'onglet. Ensuite
    /// les images se calculent sur une copie, sur d'autres fils.
    pub fn rendering(&self) -> bool {
        self.report.is_none() && self.job.needs_project()
    }

    /// Le compte rendu, quand l'export est fini (réussi, raté ou arrêté).
    pub fn report(&self) -> Option<&Result<Report, String>> {
        self.report.as_ref()
    }

    pub fn running(&self) -> bool {
        self.report.is_none()
    }

    /// Arrête l'export : les fichiers `.part` sont effacés, une page déjà exportée sous le même nom reste.
    pub fn cancel(&mut self) {
        self.job.cancel();
        self.remove_parts();
        if self.report.is_none() {
            self.report = Some(Err("Export annulé.".into()));
        }
    }

    fn remove_parts(&self) {
        let _ = std::fs::remove_file(part_of(&self.html));
        if let Some(p) = &self.pdf {
            let _ = std::fs::remove_file(part_of(p));
        }
    }
}

impl Drop for RelectureJob {
    fn drop(&mut self) {
        // Fermeture de SHOT pendant l'export : pas de demi-fichier.
        if self.report.is_none() {
            self.cancel();
        }
    }
}

/// Une fois toutes les images arrivées (fil d'écriture) : la page, puis le PDF.
fn finish(mut package: Package, all: Vec<Encoded>, html: &Path, pdf_path: Option<&Path>, title: &str, boards: &[pdf::Board], aspect: f32) -> Result<(), String> {
    let (jpegs, pdfs): (Vec<Vec<u8>>, Vec<Option<pdf::BoardImage>>) = all.into_iter().map(|e| (e.jpeg, e.pdf)).unzip();
    for plan in &mut package.plans {
        plan.play = relecture::merge_identical(&plan.play, &jpegs);
    }
    write_whole(html, relecture::page(&package, &jpegs).as_bytes())?;
    if let Some(path) = pdf_path {
        let images: Vec<pdf::BoardImage> = pdfs.into_iter().take(package.stills).flatten().collect();
        if images.len() != boards.len() {
            return Err("images du PDF manquantes".into());
        }
        write_whole(path, &pdf::storyboard_with(title, boards, &images, aspect, true))?;
    }
    Ok(())
}
